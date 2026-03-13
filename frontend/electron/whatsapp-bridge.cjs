/**
 * WhatsApp Web bridge sidecar for tamux-gateway.
 *
 * Uses @whiskeysockets/baileys to connect to WhatsApp via the multi-device
 * protocol. Communicates with the Electron main process via JSON-RPC over
 * stdin/stdout.
 *
 * Protocol:
 *   Main -> Bridge:  { "id": 1, "method": "connect" }
 *   Bridge -> Main:  { "id": 1, "result": "ok" }
 *   Bridge -> Main:  { "event": "qr", "data": "data:image/png;base64,..." }
 *   Bridge -> Main:  { "event": "connected", "data": { "phone": "+1234..." } }
 *   Bridge -> Main:  { "event": "message", "data": { "from": "...", ... } }
 *   Bridge -> Main:  { "event": "disconnected" }
 *   Bridge -> Main:  { "event": "error", "data": "..." }
 */

const { default: makeWASocket, DisconnectReason, useMultiFileAuthState, makeCacheableSignalKeyStore } = require('@whiskeysockets/baileys');
const { Boom } = require('@hapi/boom');
const path = require('path');
const fs = require('fs');
const os = require('os');
const QRCode = require('qrcode');
const pino = require('pino');

// Auth state directory
const AUTH_DIR = path.join(
    process.platform === 'win32' && process.env.LOCALAPPDATA
        ? path.join(process.env.LOCALAPPDATA, 'tamux')
        : path.join(os.homedir(), '.tamux'),
    'whatsapp-auth'
);
fs.mkdirSync(AUTH_DIR, { recursive: true });

const logger = pino({ level: 'silent' });

let sock = null;
let isConnected = false;

// ---------------------------------------------------------------------------
// JSON-RPC communication
// ---------------------------------------------------------------------------

function sendEvent(event, data) {
    const msg = JSON.stringify({ event, data });
    process.stdout.write(msg + '\n');
}

function sendResult(id, result) {
    const msg = JSON.stringify({ id, result });
    process.stdout.write(msg + '\n');
}

function sendError(id, error) {
    const msg = JSON.stringify({ id, error });
    process.stdout.write(msg + '\n');
}

// ---------------------------------------------------------------------------
// WhatsApp connection
// ---------------------------------------------------------------------------

async function connectWhatsApp() {
    const { state, saveCreds } = await useMultiFileAuthState(AUTH_DIR);

    sock = makeWASocket({
        auth: {
            creds: state.creds,
            keys: makeCacheableSignalKeyStore(state.keys, logger),
        },
        printQRInTerminal: false,
        logger,
        browser: ['tamux', 'Desktop', '1.0.0'],
        generateHighQualityLinkPreview: false,
    });

    sock.ev.on('creds.update', saveCreds);

    sock.ev.on('connection.update', async (update) => {
        const { connection, lastDisconnect, qr } = update;

        if (qr) {
            try {
                const dataUrl = await QRCode.toDataURL(qr, {
                    width: 256,
                    margin: 2,
                    color: { dark: '#000000', light: '#ffffff' },
                });
                sendEvent('qr', dataUrl);
            } catch (err) {
                sendEvent('error', `QR generation failed: ${err.message}`);
            }
        }

        if (connection === 'close') {
            isConnected = false;
            const statusCode = (lastDisconnect?.error)?.output?.statusCode;
            const shouldReconnect = statusCode !== DisconnectReason.loggedOut;

            if (shouldReconnect) {
                sendEvent('reconnecting', null);
                setTimeout(() => connectWhatsApp(), 3000);
            } else {
                sendEvent('disconnected', null);
                // Clear auth state on logout
                fs.rmSync(AUTH_DIR, { recursive: true, force: true });
                fs.mkdirSync(AUTH_DIR, { recursive: true });
            }
        } else if (connection === 'open') {
            isConnected = true;
            const phoneNumber = sock.user?.id?.split(':')[0] || 'Unknown';
            sendEvent('connected', { phone: `+${phoneNumber}` });
        }
    });

    sock.ev.on('messages.upsert', (m) => {
        if (m.type !== 'notify') return;
        for (const msg of m.messages) {
            if (msg.key.fromMe) continue; // skip own messages
            const text =
                msg.message?.conversation ||
                msg.message?.extendedTextMessage?.text ||
                msg.message?.imageMessage?.caption ||
                '';
            if (!text) continue;

            const from = msg.key.remoteJid || 'unknown';
            const pushName = msg.pushName || '';

            sendEvent('message', {
                from,
                pushName,
                text,
                timestamp: msg.messageTimestamp,
                messageId: msg.key.id,
            });
        }
    });
}

async function disconnectWhatsApp() {
    if (sock) {
        await sock.logout().catch(() => {});
        sock = null;
        isConnected = false;
    }
}

function getStatus() {
    if (!sock) return { status: 'disconnected', phone: null };
    if (isConnected) {
        const phoneNumber = sock.user?.id?.split(':')[0] || null;
        return { status: 'connected', phone: phoneNumber ? `+${phoneNumber}` : null };
    }
    return { status: 'connecting', phone: null };
}

async function sendWhatsAppMessage(jid, text) {
    if (!sock || !isConnected) {
        throw new Error('WhatsApp not connected');
    }
    await sock.sendMessage(jid, { text });
}

// ---------------------------------------------------------------------------
// stdin command handler
// ---------------------------------------------------------------------------

let inputBuffer = '';
process.stdin.setEncoding('utf8');
process.stdin.on('data', (chunk) => {
    inputBuffer += chunk;
    const lines = inputBuffer.split('\n');
    inputBuffer = lines.pop() || '';

    for (const line of lines) {
        if (!line.trim()) continue;
        try {
            const msg = JSON.parse(line);
            handleCommand(msg);
        } catch (err) {
            sendEvent('error', `Invalid JSON: ${err.message}`);
        }
    }
});

async function handleCommand(msg) {
    const { id, method, params } = msg;

    try {
        switch (method) {
            case 'connect':
                // Reply immediately so the RPC never times out.
                // QR / connected / error events arrive asynchronously.
                sendResult(id, 'ok');
                connectWhatsApp().catch((err) => {
                    sendEvent('error', `Connection failed: ${err.message || String(err)}`);
                });
                break;
            case 'disconnect':
                await disconnectWhatsApp();
                sendResult(id, 'ok');
                break;
            case 'status':
                sendResult(id, getStatus());
                break;
            case 'send':
                await sendWhatsAppMessage(params.jid, params.text);
                sendResult(id, 'ok');
                break;
            case 'ping':
                sendResult(id, 'pong');
                break;
            default:
                sendError(id, `Unknown method: ${method}`);
        }
    } catch (err) {
        sendError(id, err.message || String(err));
    }
}

// Graceful shutdown
process.on('SIGTERM', async () => {
    if (sock) await sock.end(undefined).catch(() => {});
    process.exit(0);
});

process.on('SIGINT', async () => {
    if (sock) await sock.end(undefined).catch(() => {});
    process.exit(0);
});

sendEvent('ready', null);
