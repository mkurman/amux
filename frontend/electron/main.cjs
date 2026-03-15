const { app, BrowserWindow, Menu, clipboard, ipcMain, screen, shell, session } = require('electron');
const { spawn, spawnSync, execSync, execFileSync } = require('child_process');
const { Client, GatewayIntentBits, Partials } = require('discord.js');
const path = require('path');
const net = require('net');
const fs = require('fs');
const os = require('os');

const DAEMON_NAME = 'tamux-daemon';
const CLI_NAME = 'tamux';
const DAEMON_TCP_HOST = '127.0.0.1';
const DAEMON_TCP_PORT = 17563;
const CLONE_SESSION_PREFIX = 'clone:';
const MAX_TERMINAL_HISTORY_BYTES = 1024 * 1024;
const MAX_REATTACH_HISTORY_BYTES = 64 * 1024;
const VISION_SCREENSHOT_TTL_MS = 10 * 60 * 1000;
let mainWindow = null;
const terminalBridges = new Map();
const paneSessionHints = new Map();

// ---------------------------------------------------------------------------
// WhatsApp bridge sidecar management
// ---------------------------------------------------------------------------
let whatsappProcess = null;
let whatsappRpcId = 0;
const whatsappPendingCalls = new Map();
let discordClient = null;
let discordClientToken = null;
let discordListenerAttached = false;
let slackBotToken = null;
let slackPollTimer = null;
const slackLastMessageTsByChannel = new Map();
let telegramBotToken = null;
let telegramPollTimer = null;
let telegramUpdateOffset = 0;

function normalizeDiscordSnowflake(value) {
    if (typeof value !== 'string') return null;
    const trimmed = value.trim();
    if (!trimmed) return null;
    const match = trimmed.match(/\d{17,20}/);
    return match ? match[0] : trimmed;
}

function cleanupDiscordClient() {
    if (discordClient) {
        try {
            discordClient.destroy();
        } catch {
            // Ignore cleanup errors.
        }
    }
    discordClient = null;
    discordClientToken = null;
    discordListenerAttached = false;
}

async function slackApiRequest(method, token, body) {
    const response = await fetch(`https://slack.com/api/${method}`, {
        method: 'POST',
        headers: {
            Authorization: `Bearer ${token}`,
            'Content-Type': 'application/json; charset=utf-8',
        },
        body: JSON.stringify(body || {}),
    });

    const data = await response.json().catch(() => ({}));
    if (!response.ok || data?.ok === false) {
        const detail = data?.error || response.statusText || 'Slack API error';
        throw new Error(String(detail));
    }
    return data;
}

function stopSlackBridge() {
    if (slackPollTimer) {
        clearInterval(slackPollTimer);
        slackPollTimer = null;
    }
    slackBotToken = null;
    slackLastMessageTsByChannel.clear();
}

async function pollSlackInbox() {
    if (!slackBotToken) return;
    if (!mainWindow || mainWindow.isDestroyed()) return;

    try {
        const listData = await slackApiRequest('conversations.list', slackBotToken, {
            limit: 200,
            types: 'public_channel,private_channel,im,mpim',
            exclude_archived: true,
        });

        const channels = Array.isArray(listData?.channels) ? listData.channels : [];

        for (const channel of channels) {
            const channelId = typeof channel?.id === 'string' ? channel.id : '';
            if (!channelId) continue;

            const lastTs = slackLastMessageTsByChannel.get(channelId);
            const historyData = await slackApiRequest('conversations.history', slackBotToken, {
                channel: channelId,
                limit: 20,
                oldest: lastTs || '0',
                inclusive: false,
            });

            const messages = Array.isArray(historyData?.messages) ? historyData.messages : [];
            const chronological = messages.slice().reverse();

            for (const message of chronological) {
                if (!message || message.subtype) continue;
                const text = typeof message.text === 'string' ? message.text : '';
                const userId = typeof message.user === 'string' ? message.user : '';
                const ts = typeof message.ts === 'string' ? message.ts : '';

                if (!text.trim() || !ts) continue;

                const prevTs = slackLastMessageTsByChannel.get(channelId);
                if (prevTs && Number(ts) <= Number(prevTs)) continue;

                slackLastMessageTsByChannel.set(channelId, ts);

                mainWindow.webContents.send('slack-message', {
                    channelId,
                    channelName: typeof channel?.name === 'string' ? channel.name : null,
                    userId,
                    username: userId || 'slack-user',
                    content: text,
                    messageTs: ts,
                    createdAt: Date.now(),
                });
            }
        }
    } catch (error) {
        logToFile('warn', 'slack poll failed', {
            message: error?.message ?? String(error),
        });
    }
}

async function ensureSlackConnected(_event, payload = {}) {
    const token = typeof payload.token === 'string' ? payload.token.trim() : '';
    if (!token) {
        return { ok: false, error: 'Slack bot token missing' };
    }

    try {
        const auth = await slackApiRequest('auth.test', token, {});
        slackBotToken = token;

        if (!slackPollTimer) {
            void pollSlackInbox();
            slackPollTimer = setInterval(() => {
                void pollSlackInbox();
            }, 5000);
        }

        return {
            ok: true,
            userId: auth?.user_id ?? null,
            username: auth?.user ?? null,
            team: auth?.team ?? null,
        };
    } catch (error) {
        return { ok: false, error: error?.message ?? String(error) };
    }
}

async function sendSlackMessage(_event, payload = {}) {
    const token = typeof payload.token === 'string' ? payload.token.trim() : '';
    const channelId = typeof payload.channelId === 'string' ? payload.channelId.trim() : '';
    const message = typeof payload.message === 'string' ? payload.message : '';

    if (!token) {
        return { ok: false, error: 'Slack bot token missing' };
    }
    if (!channelId) {
        return { ok: false, error: 'Slack channelId is required' };
    }
    if (!message.trim()) {
        return { ok: false, error: 'Slack message is empty' };
    }

    try {
        const response = await slackApiRequest('chat.postMessage', token, {
            channel: channelId,
            text: message,
        });

        return {
            ok: true,
            channelId,
            messageTs: response?.ts ?? null,
        };
    } catch (error) {
        return { ok: false, error: error?.message ?? String(error) };
    }
}

async function telegramApiRequest(token, method, params = {}) {
    const response = await fetch(`https://api.telegram.org/bot${token}/${method}`, {
        method: 'POST',
        headers: {
            'Content-Type': 'application/json; charset=utf-8',
        },
        body: JSON.stringify(params),
    });

    const data = await response.json().catch(() => ({}));
    if (!response.ok || data?.ok === false) {
        const detail = data?.description || response.statusText || 'Telegram API error';
        throw new Error(String(detail));
    }
    return data;
}

function stopTelegramBridge() {
    if (telegramPollTimer) {
        clearInterval(telegramPollTimer);
        telegramPollTimer = null;
    }
    telegramBotToken = null;
    telegramUpdateOffset = 0;
}

async function pollTelegramInbox() {
    if (!telegramBotToken) return;
    if (!mainWindow || mainWindow.isDestroyed()) return;

    try {
        const updates = await telegramApiRequest(telegramBotToken, 'getUpdates', {
            offset: telegramUpdateOffset,
            timeout: 0,
            allowed_updates: ['message'],
        });

        const items = Array.isArray(updates?.result) ? updates.result : [];
        for (const update of items) {
            const updateId = Number(update?.update_id);
            if (Number.isFinite(updateId)) {
                telegramUpdateOffset = Math.max(telegramUpdateOffset, updateId + 1);
            }

            const msg = update?.message;
            if (!msg) continue;

            const chatId = msg?.chat?.id != null ? String(msg.chat.id) : '';
            const text = typeof msg?.text === 'string' ? msg.text : '';
            if (!chatId || !text.trim()) continue;

            const userId = msg?.from?.id != null ? String(msg.from.id) : '';
            const username = msg?.from?.username || msg?.from?.first_name || 'telegram-user';

            mainWindow.webContents.send('telegram-message', {
                chatId,
                userId,
                username,
                content: text,
                createdAt: Date.now(),
            });
        }
    } catch (error) {
        logToFile('warn', 'telegram poll failed', {
            message: error?.message ?? String(error),
        });
    }
}

async function ensureTelegramConnected(_event, payload = {}) {
    const token = typeof payload.token === 'string' ? payload.token.trim() : '';
    if (!token) {
        return { ok: false, error: 'Telegram bot token missing' };
    }

    try {
        const me = await telegramApiRequest(token, 'getMe', {});
        telegramBotToken = token;

        if (!telegramPollTimer) {
            void pollTelegramInbox();
            telegramPollTimer = setInterval(() => {
                void pollTelegramInbox();
            }, 4000);
        }

        return {
            ok: true,
            userId: me?.result?.id != null ? String(me.result.id) : null,
            username: me?.result?.username ?? null,
        };
    } catch (error) {
        return { ok: false, error: error?.message ?? String(error) };
    }
}

async function sendTelegramMessage(_event, payload = {}) {
    const token = typeof payload.token === 'string' ? payload.token.trim() : '';
    const chatId = payload.chatId != null ? String(payload.chatId).trim() : '';
    const message = typeof payload.message === 'string' ? payload.message : '';

    if (!token) {
        return { ok: false, error: 'Telegram bot token missing' };
    }
    if (!chatId) {
        return { ok: false, error: 'Telegram chatId is required' };
    }
    if (!message.trim()) {
        return { ok: false, error: 'Telegram message is empty' };
    }

    try {
        const result = await telegramApiRequest(token, 'sendMessage', {
            chat_id: chatId,
            text: message,
        });

        return {
            ok: true,
            chatId,
            messageId: result?.result?.message_id ?? null,
        };
    } catch (error) {
        return { ok: false, error: error?.message ?? String(error) };
    }
}

function attachDiscordListeners(client) {
    if (discordListenerAttached) return;

    client.on('messageCreate', (message) => {
        try {
            if (!message || message.author?.bot) return;
            if (!mainWindow || mainWindow.isDestroyed()) return;

            mainWindow.webContents.send('discord-message', {
                messageId: message.id,
                channelId: message.channelId,
                guildId: message.guildId ?? null,
                userId: message.author?.id ?? null,
                username: message.author?.username ?? null,
                content: typeof message.content === 'string' ? message.content : '',
                isDM: Boolean(message.channel?.isDMBased?.()),
                createdAt: Date.now(),
            });
        } catch (error) {
            logToFile('warn', 'failed to handle discord inbound message', {
                message: error?.message ?? String(error),
            });
        }
    });

    discordListenerAttached = true;
}

async function getDiscordClient(token) {
    if (!token || typeof token !== 'string' || !token.trim()) {
        throw new Error('Discord bot token is required');
    }

    const normalizedToken = token.trim();
    if (discordClient && discordClientToken === normalizedToken && discordClient.isReady()) {
        return discordClient;
    }

    cleanupDiscordClient();

    const client = new Client({
        intents: [
            GatewayIntentBits.Guilds,
            GatewayIntentBits.GuildMessages,
            GatewayIntentBits.MessageContent,
            GatewayIntentBits.DirectMessages,
        ],
        partials: [Partials.Channel],
    });

    await new Promise((resolve, reject) => {
        let settled = false;
        const timeout = setTimeout(() => {
            if (settled) return;
            settled = true;
            reject(new Error('Discord login timeout'));
        }, 15000);

        client.once('ready', () => {
            if (settled) return;
            settled = true;
            clearTimeout(timeout);
            resolve();
        });

        client.once('error', (error) => {
            if (settled) return;
            settled = true;
            clearTimeout(timeout);
            reject(error);
        });

        client.login(normalizedToken).catch((error) => {
            if (settled) return;
            settled = true;
            clearTimeout(timeout);
            reject(error);
        });
    });

    discordClient = client;
    discordClientToken = normalizedToken;
    attachDiscordListeners(client);
    return client;
}

async function ensureDiscordConnected(_event, payload = {}) {
    const token = typeof payload.token === 'string' ? payload.token.trim() : '';
    if (!token) {
        return { ok: false, error: 'Discord bot token missing' };
    }

    try {
        const client = await getDiscordClient(token);
        attachDiscordListeners(client);
        return {
            ok: true,
            userId: client.user?.id ?? null,
            username: client.user?.username ?? null,
        };
    } catch (error) {
        const message = error?.message ?? String(error);
        return { ok: false, error: message };
    }
}

async function sendDiscordMessage(_event, payload = {}) {
    const token = typeof payload.token === 'string' ? payload.token : '';
    const channelId = normalizeDiscordSnowflake(payload.channelId);
    const userId = normalizeDiscordSnowflake(payload.userId);
    const message = typeof payload.message === 'string' ? payload.message : '';

    if (!token.trim()) {
        return { ok: false, error: 'Discord bot token missing' };
    }
    if (!message.trim()) {
        return { ok: false, error: 'Discord message is empty' };
    }
    if (!channelId && !userId) {
        return { ok: false, error: 'No channelId or userId provided' };
    }

    try {
        const client = await getDiscordClient(token);

        // Prefer channel delivery when a channel is available. DM is only a fallback.
        if (channelId) {
            const channel = await client.channels.fetch(channelId, { force: true });
            if (!channel || !channel.isTextBased() || typeof channel.send !== 'function') {
                return { ok: false, error: `Channel ${channelId} is not text-send capable` };
            }

            const sent = await channel.send({ content: message });
            return {
                ok: true,
                destination: 'channel',
                channelId,
                messageId: sent.id,
            };
        }

        if (userId) {
            const user = await client.users.fetch(userId, { force: true });
            const dm = await user.createDM();
            const sent = await dm.send(message);
            return {
                ok: true,
                destination: 'dm',
                channelId: dm.id,
                userId: user.id,
                messageId: sent.id,
            };
        }

        return { ok: false, error: 'No resolvable Discord destination provided' };
    } catch (error) {
        const rawMessage = error && error.message ? error.message : String(error);
        const statusCode = error && typeof error.status === 'number' ? error.status : null;
        const code = error && typeof error.code !== 'undefined' ? String(error.code) : null;
        let hint = '';

        if (statusCode === 404 || code === '10003') {
            hint = ' (Discord returned Not Found: verify bot access and that channel/user IDs are valid snowflakes)';
        } else if (statusCode === 403 || code === '50013') {
            hint = ' (Discord returned Forbidden: bot lacks Send Messages permission for target channel)';
        }

        return { ok: false, error: `${rawMessage}${hint}` };
    }
}

function startWhatsAppBridge() {
    if (whatsappProcess) return;

    const bridgePath = path.join(__dirname, 'whatsapp-bridge.cjs');
    if (!fs.existsSync(bridgePath)) {
        logToFile('warn', 'whatsapp-bridge.cjs not found');
        throw new Error('WhatsApp bridge script not found');
    }

    logToFile('info', 'starting WhatsApp bridge sidecar');
    whatsappProcess = spawn(process.execPath, [bridgePath], {
        stdio: ['pipe', 'pipe', 'pipe'],
        env: { ...process.env },
    });

    let buffer = '';
    whatsappProcess.stdout.on('data', (chunk) => {
        buffer += chunk.toString();
        const lines = buffer.split('\n');
        buffer = lines.pop() || '';

        for (const line of lines) {
            if (!line.trim()) continue;
            try {
                const msg = JSON.parse(line);
                handleWhatsAppMessage(msg);
            } catch (err) {
                logToFile('warn', `WhatsApp bridge invalid JSON: ${err.message}`);
            }
        }
    });

    whatsappProcess.stderr.on('data', (chunk) => {
        const text = chunk.toString().trim();
        logToFile('warn', `WhatsApp bridge stderr: ${text}`);
        // Forward critical errors to the UI so the user sees why it fails
        if (mainWindow && text) {
            mainWindow.webContents.send('whatsapp-error', `Bridge error: ${text}`);
        }
    });

    whatsappProcess.on('close', (code) => {
        logToFile('info', `WhatsApp bridge exited with code ${code}`);
        whatsappProcess = null;
        // Reject pending calls
        for (const [, { reject }] of whatsappPendingCalls) {
            reject(new Error('WhatsApp bridge process exited'));
        }
        whatsappPendingCalls.clear();
        // Notify UI that bridge died
        if (mainWindow && code !== 0) {
            mainWindow.webContents.send('whatsapp-error', `WhatsApp bridge exited unexpectedly (code ${code}). Check that dependencies are installed.`);
            mainWindow.webContents.send('whatsapp-disconnected');
        }
    });

    whatsappProcess.on('error', (err) => {
        logToFile('error', `WhatsApp bridge spawn error: ${err.message}`);
        whatsappProcess = null;
    });
}

function handleWhatsAppMessage(msg) {
    // RPC response
    if (msg.id !== undefined && whatsappPendingCalls.has(msg.id)) {
        const { resolve, reject } = whatsappPendingCalls.get(msg.id);
        whatsappPendingCalls.delete(msg.id);
        if (msg.error) reject(new Error(msg.error));
        else resolve(msg.result);
        return;
    }

    // Event from bridge
    if (msg.event && mainWindow) {
        switch (msg.event) {
            case 'qr':
                mainWindow.webContents.send('whatsapp-qr', msg.data);
                break;
            case 'connected':
                mainWindow.webContents.send('whatsapp-connected', msg.data);
                break;
            case 'disconnected':
                mainWindow.webContents.send('whatsapp-disconnected');
                break;
            case 'error':
                mainWindow.webContents.send('whatsapp-error', msg.data);
                break;
            case 'message':
                mainWindow.webContents.send('whatsapp-message', msg.data);
                break;
            case 'reconnecting':
                logToFile('info', 'WhatsApp bridge reconnecting...');
                break;
            case 'ready':
                logToFile('info', 'WhatsApp bridge sidecar ready');
                break;
        }
    }
}

function whatsappRpc(method, params) {
    return new Promise((resolve, reject) => {
        if (!whatsappProcess) {
            reject(new Error('WhatsApp bridge not running'));
            return;
        }
        const id = ++whatsappRpcId;
        whatsappPendingCalls.set(id, { resolve, reject });
        const msg = JSON.stringify({ id, method, params }) + '\n';
        whatsappProcess.stdin.write(msg);

        // Timeout after 60 seconds (Baileys connection can be slow)
        setTimeout(() => {
            if (whatsappPendingCalls.has(id)) {
                whatsappPendingCalls.delete(id);
                reject(new Error('WhatsApp RPC timeout'));
            }
        }, 60000);
    });
}

function stopWhatsAppBridge() {
    if (whatsappProcess) {
        whatsappProcess.kill('SIGTERM');
        whatsappProcess = null;
    }
}

function getLegacyAmuxDataDir() {
    if (process.platform === 'win32' && process.env.LOCALAPPDATA) {
        return path.join(process.env.LOCALAPPDATA, 'amux');
    }
    return path.join(os.homedir(), '.amux');
}

function getTamuxDataDir() {
    if (process.platform === 'win32' && process.env.LOCALAPPDATA) {
        return path.join(process.env.LOCALAPPDATA, 'tamux');
    }
    return path.join(os.homedir(), '.tamux');
}

function ensureTamuxDataDir() {
    const dataDir = getTamuxDataDir();
    const legacyDir = getLegacyAmuxDataDir();
    if (!fs.existsSync(dataDir) && fs.existsSync(legacyDir)) {
        try {
            fs.mkdirSync(path.dirname(dataDir), { recursive: true });
            fs.renameSync(legacyDir, dataDir);
        } catch {
            // Ignore migration failure and continue with the new directory.
        }
    }
    fs.mkdirSync(dataDir, { recursive: true });
    return dataDir;
}

function getVisionTempDir() {
    const dir = path.join(ensureTamuxDataDir(), 'tmp', 'vision');
    fs.mkdirSync(dir, { recursive: true });
    return dir;
}

function cleanupVisionScreenshots() {
    try {
        const dir = getVisionTempDir();
        const now = Date.now();
        const entries = fs.readdirSync(dir);
        for (const entry of entries) {
            const fullPath = path.join(dir, entry);
            try {
                const stats = fs.statSync(fullPath);
                if (!stats.isFile()) continue;
                if (now - stats.mtimeMs > VISION_SCREENSHOT_TTL_MS) {
                    fs.unlinkSync(fullPath);
                }
            } catch {
                // Ignore per-file cleanup errors.
            }
        }
    } catch {
        // Ignore cleanup errors.
    }
}

function saveVisionScreenshot(_event, payload = {}) {
    try {
        const dataUrl = typeof payload.dataUrl === 'string' ? payload.dataUrl.trim() : '';
        if (!dataUrl.startsWith('data:image/png;base64,')) {
            return { ok: false, error: 'Invalid PNG data URL' };
        }

        cleanupVisionScreenshots();

        const base64 = dataUrl.slice('data:image/png;base64,'.length);
        const buffer = Buffer.from(base64, 'base64');
        const now = Date.now();
        const filename = `ss_${now}_${Math.random().toString(36).slice(2, 8)}.png`;
        const fullPath = path.join(getVisionTempDir(), filename);
        fs.writeFileSync(fullPath, buffer);

        setTimeout(() => {
            try {
                if (fs.existsSync(fullPath)) {
                    fs.unlinkSync(fullPath);
                }
            } catch {
                // Ignore deferred cleanup errors.
            }
        }, VISION_SCREENSHOT_TTL_MS);

        return {
            ok: true,
            path: fullPath,
            expiresAt: now + VISION_SCREENSHOT_TTL_MS,
        };
    } catch (error) {
        return { ok: false, error: error?.message ?? String(error) };
    }
}

function configureChromiumRuntimePaths() {
    try {
        const dataDir = ensureTamuxDataDir();
        const userDataDir = path.join(dataDir, 'electron-profile');
        const cacheDir = path.join(dataDir, 'chromium-cache');

        fs.mkdirSync(userDataDir, { recursive: true });
        fs.mkdirSync(cacheDir, { recursive: true });

        app.setPath('userData', userDataDir);
        app.setPath('sessionData', cacheDir);
        app.commandLine.appendSwitch('disk-cache-dir', cacheDir);
    } catch (error) {
        logToFile('warn', 'failed to configure chromium runtime paths', {
            message: error?.message ?? String(error),
        });
    }

    // In constrained environments (WSL, locked-down profiles), GPU cache
    // creation may fail and break UI flows like QR rendering.
    app.disableHardwareAcceleration();
    app.commandLine.appendSwitch('disable-gpu');
    app.commandLine.appendSwitch('disable-gpu-compositing');
    app.commandLine.appendSwitch('disable-gpu-shader-disk-cache');
    app.commandLine.appendSwitch('disable-gpu-program-cache');
}

function resolveDataPath(relativePath) {
    if (typeof relativePath !== 'string' || !relativePath.trim()) {
        throw new Error('A relative path is required.');
    }

    const baseDir = path.resolve(ensureTamuxDataDir());
    const normalized = path.normalize(relativePath).replace(/^(\.\.(\\|\/|$))+/, '');
    const targetPath = path.resolve(baseDir, normalized);

    if (targetPath !== baseDir && !targetPath.startsWith(`${baseDir}${path.sep}`)) {
        throw new Error('Path escapes the tamux data directory.');
    }

    return targetPath;
}

function readJsonFile(relativePath) {
    const filePath = resolveDataPath(relativePath);
    if (!fs.existsSync(filePath)) {
        return null;
    }

    return JSON.parse(fs.readFileSync(filePath, 'utf8'));
}

function writeJsonFile(relativePath, data) {
    const filePath = resolveDataPath(relativePath);
    fs.mkdirSync(path.dirname(filePath), { recursive: true });
    fs.writeFileSync(filePath, JSON.stringify(data, null, 2), 'utf8');
    return true;
}

function readTextFile(relativePath) {
    const filePath = resolveDataPath(relativePath);
    if (!fs.existsSync(filePath)) {
        return null;
    }

    return fs.readFileSync(filePath, 'utf8');
}

function writeTextFile(relativePath, content) {
    const filePath = resolveDataPath(relativePath);
    fs.mkdirSync(path.dirname(filePath), { recursive: true });
    fs.writeFileSync(filePath, typeof content === 'string' ? content : '', 'utf8');
    return true;
}

function deleteDataPath(relativePath) {
    const filePath = resolveDataPath(relativePath);
    if (!fs.existsSync(filePath)) {
        return false;
    }

    fs.rmSync(filePath, { recursive: true, force: true });
    return true;
}

function listDataDir(relativeDir = '') {
    const dirPath = resolveDataPath(relativeDir || '.');
    if (!fs.existsSync(dirPath) || !fs.statSync(dirPath).isDirectory()) {
        return [];
    }

    return fs.readdirSync(dirPath, { withFileTypes: true }).map((entry) => {
        const absolutePath = path.join(dirPath, entry.name);
        return {
            name: entry.name,
            path: path.relative(ensureTamuxDataDir(), absolutePath).replace(/\\/g, '/'),
            isDirectory: entry.isDirectory(),
        };
    });
}

function getPluginsRootDir() {
    const pluginsDir = path.join(ensureTamuxDataDir(), 'plugins');
    fs.mkdirSync(pluginsDir, { recursive: true });
    return pluginsDir;
}

function normalizeInstalledPluginRecord(entry) {
    if (!entry || typeof entry !== 'object') {
        return null;
    }

    const entryPath = typeof entry.entry_path === 'string' ? entry.entry_path.trim() : '';
    if (!entryPath) {
        return null;
    }

    return {
        packageName: String(entry.package_name || ''),
        packageVersion: String(entry.package_version || ''),
        pluginName: String(entry.plugin_name || entry.package_name || ''),
        entryPath,
        format: String(entry.format || 'script'),
        installedAt: Number(entry.installed_at || 0),
    };
}

function listInstalledPlugins() {
    const registry = readJsonFile('plugins/registry.json');
    const plugins = Array.isArray(registry?.plugins) ? registry.plugins : [];
    return plugins
        .map(normalizeInstalledPluginRecord)
        .filter(Boolean);
}

function resolveInstalledPluginEntryPath(entryPath) {
    const pluginsRoot = path.resolve(getPluginsRootDir());
    const resolvedPath = path.resolve(entryPath);

    if (resolvedPath !== pluginsRoot && !resolvedPath.startsWith(`${pluginsRoot}${path.sep}`)) {
        throw new Error('Installed plugin entry path escapes the tamux plugins directory.');
    }

    return resolvedPath;
}

function loadInstalledPluginScripts() {
    return listInstalledPlugins().map((entry) => {
        try {
            if (entry.format !== 'script') {
                return {
                    packageName: entry.packageName,
                    pluginName: entry.pluginName,
                    entryPath: entry.entryPath,
                    format: entry.format,
                    status: 'skipped',
                    error: `Unsupported plugin format '${entry.format}'`,
                };
            }

            const resolvedEntryPath = resolveInstalledPluginEntryPath(entry.entryPath);
            if (!fs.existsSync(resolvedEntryPath) || !fs.statSync(resolvedEntryPath).isFile()) {
                return {
                    packageName: entry.packageName,
                    pluginName: entry.pluginName,
                    entryPath: entry.entryPath,
                    format: entry.format,
                    status: 'error',
                    error: 'Plugin entry file does not exist.',
                };
            }

            return {
                packageName: entry.packageName,
                pluginName: entry.pluginName,
                entryPath: entry.entryPath,
                format: entry.format,
                sourceUrl: resolvedEntryPath.replace(/\\/g, '/'),
                source: fs.readFileSync(resolvedEntryPath, 'utf8'),
            };
        } catch (error) {
            return {
                packageName: entry.packageName,
                pluginName: entry.pluginName,
                entryPath: entry.entryPath,
                format: entry.format,
                status: 'error',
                error: error?.message ?? String(error),
            };
        }
    });
}

function resolveFsPath(targetPath) {
    if (typeof targetPath !== 'string' || !targetPath.trim()) {
        throw new Error('A path is required.');
    }

    const expanded = targetPath.startsWith('~/')
        ? path.join(os.homedir(), targetPath.slice(2))
        : targetPath;
    return path.resolve(expanded);
}

function listFsDir(targetDir) {
    const resolvedDir = resolveFsPath(targetDir || '.');
    if (!fs.existsSync(resolvedDir) || !fs.statSync(resolvedDir).isDirectory()) {
        return [];
    }

    return fs.readdirSync(resolvedDir, { withFileTypes: true }).map((entry) => {
        const absolutePath = path.join(resolvedDir, entry.name);
        let stats = null;
        try {
            stats = fs.statSync(absolutePath);
        } catch {
            stats = null;
        }
        return {
            name: entry.name,
            path: absolutePath,
            isDirectory: entry.isDirectory(),
            sizeBytes: stats?.size ?? null,
            modifiedAt: stats?.mtimeMs ?? null,
        };
    });
}

function copyFsPath(sourcePath, destinationPath) {
    const source = resolveFsPath(sourcePath);
    const destination = resolveFsPath(destinationPath);
    const sourceStats = fs.statSync(source);

    if (sourceStats.isDirectory()) {
        fs.cpSync(source, destination, { recursive: true, force: true });
    } else {
        fs.mkdirSync(path.dirname(destination), { recursive: true });
        fs.copyFileSync(source, destination);
    }
    return true;
}

function moveFsPath(sourcePath, destinationPath) {
    const source = resolveFsPath(sourcePath);
    const destination = resolveFsPath(destinationPath);
    fs.mkdirSync(path.dirname(destination), { recursive: true });
    fs.renameSync(source, destination);
    return true;
}

function deleteFsPath(targetPath) {
    const resolved = resolveFsPath(targetPath);
    if (!fs.existsSync(resolved)) return false;
    fs.rmSync(resolved, { recursive: true, force: true });
    return true;
}

function createFsDirectory(targetDirPath) {
    const resolved = resolveFsPath(targetDirPath);
    fs.mkdirSync(resolved, { recursive: true });
    return true;
}

function getFsPathInfo(targetPath) {
    const resolved = resolveFsPath(targetPath);
    if (!fs.existsSync(resolved)) {
        return null;
    }

    const stats = fs.statSync(resolved);
    return {
        path: resolved,
        isDirectory: stats.isDirectory(),
        sizeBytes: stats.size,
        modifiedAt: stats.mtimeMs,
        createdAt: stats.birthtimeMs,
    };
}

function openDataPath(relativePath) {
    const filePath = resolveDataPath(relativePath);
    if (!fs.existsSync(filePath)) {
        return 'Path does not exist';
    }

    return shell.openPath(filePath);
}

function revealDataPath(relativePath) {
    const filePath = resolveDataPath(relativePath);
    if (!fs.existsSync(filePath)) {
        return false;
    }

    shell.showItemInFolder(filePath);
    return true;
}

function logToFile(level, message, details) {
    try {
        const logDir = getTamuxDataDir();
        fs.mkdirSync(logDir, { recursive: true });
        const line = [
            new Date().toISOString(),
            level.toUpperCase(),
            message,
            details ? JSON.stringify(details) : '',
        ].filter(Boolean).join(' ') + '\n';
        fs.appendFileSync(path.join(logDir, 'tamux-electron.log'), line, 'utf8');
    } catch {
        // Ignore logging failures.
    }
}

function getDaemonPath() {
    const resolved = getCompanionBinaryPath(DAEMON_NAME);
    logToFile('info', 'resolved daemon path', { resolved });
    return resolved;
}

function getCliPath() {
    const resolved = getCompanionBinaryPath(CLI_NAME);
    logToFile('info', 'resolved cli path', { resolved });
    return resolved;
}

function getCompanionBinaryPath(binaryName) {
    const isDev = !app.isPackaged;
    const exeName = binaryName + (process.platform === 'win32' ? '.exe' : '');

    if (isDev) {
        const repoRoot = path.join(__dirname, '..', '..');
        const candidates = [
            path.join(repoRoot, 'target', 'debug', exeName),
            path.join(repoRoot, 'target', 'release', exeName),
            path.join(repoRoot, 'dist', exeName),
            path.join(repoRoot, 'target', 'x86_64-pc-windows-gnu', 'release', exeName),
        ];

        const existing = candidates.find((candidate) => fs.existsSync(candidate));
        return existing || candidates[0];
    }

    const exeDir = path.dirname(app.getPath('exe'));
    const resourceDir = process.resourcesPath;
    const packagedCandidates = [
        path.join(resourceDir, 'bin', exeName),
        path.join(resourceDir, exeName),
        path.join(resourceDir, 'dist', exeName),
        path.join(resourceDir, 'app.asar.unpacked', 'dist', exeName),
        path.join(exeDir, exeName),
    ];
    const existing = packagedCandidates.find((candidate) => fs.existsSync(candidate));
    return existing || packagedCandidates[0];
}

function sendAppCommand(command) {
    mainWindow?.webContents.send('app-command', command);
}

function buildAppMenu() {
    const template = [
        {
            label: 'File',
            submenu: [
                { label: 'New Workspace', accelerator: 'Ctrl+Shift+N', click: () => sendAppCommand('new-workspace') },
                { label: 'New Surface', accelerator: 'Ctrl+T', click: () => sendAppCommand('new-surface') },
                { type: 'separator' },
                { label: 'Settings', accelerator: 'Ctrl+,', click: () => sendAppCommand('toggle-settings') },
                { type: 'separator' },
                { role: 'quit', label: 'Exit' },
            ],
        },
        {
            label: 'Edit',
            submenu: [
                { role: 'undo' },
                { role: 'redo' },
                { type: 'separator' },
                { role: 'cut' },
                {
                    label: 'Copy',
                    accelerator: 'Ctrl+C',
                    click: () => {
                        mainWindow?.webContents.copy();
                        sendAppCommand('copy');
                    },
                },
                {
                    label: 'Paste',
                    accelerator: 'Ctrl+V',
                    click: () => {
                        mainWindow?.webContents.paste();
                        sendAppCommand('paste');
                    },
                },
                {
                    label: 'Select All',
                    accelerator: 'Ctrl+A',
                    click: () => {
                        mainWindow?.webContents.selectAll();
                        sendAppCommand('select-all');
                    },
                },
            ],
        },
        {
            label: 'View',
            submenu: [
                { label: 'Command Palette', accelerator: 'Ctrl+Shift+P', click: () => sendAppCommand('toggle-command-palette') },
                { label: 'Search', accelerator: 'Ctrl+Shift+F', click: () => sendAppCommand('toggle-search') },
                { label: 'File Manager', accelerator: 'Ctrl+Shift+E', click: () => sendAppCommand('toggle-file-manager') },
                { label: 'Toggle Sidebar', accelerator: 'Ctrl+B', click: () => sendAppCommand('toggle-sidebar') },
                { type: 'separator' },
                { role: 'reload' },
                { role: 'forceReload' },
                { role: 'toggleDevTools' },
                { type: 'separator' },
                { role: 'resetZoom' },
                { role: 'zoomIn' },
                { role: 'zoomOut' },
                { role: 'togglefullscreen' },
            ],
        },
        {
            label: 'Features',
            submenu: [
                { label: 'Mission Console', click: () => sendAppCommand('toggle-mission') },
                { label: 'Command History', click: () => sendAppCommand('toggle-command-history') },
                { label: 'Command Log', click: () => sendAppCommand('toggle-command-log') },
                { label: 'Session Vault', click: () => sendAppCommand('toggle-session-vault') },
                { label: 'System Monitor', click: () => sendAppCommand('toggle-system-monitor') },
                { label: 'Execution Canvas', click: () => sendAppCommand('toggle-canvas') },
                { label: 'Time Travel Snapshots', click: () => sendAppCommand('toggle-time-travel') },
            ],
        },
        {
            label: 'Window',
            submenu: [
                { label: 'Split Right', accelerator: 'Ctrl+D', click: () => sendAppCommand('split-right') },
                { label: 'Split Down', accelerator: 'Ctrl+Shift+D', click: () => sendAppCommand('split-down') },
                { label: 'Zoom Pane', accelerator: 'Ctrl+Shift+Z', click: () => sendAppCommand('toggle-zoom') },
                { type: 'separator' },
                { role: 'minimize' },
                { role: 'close' },
            ],
        },
        {
            label: 'Help',
            submenu: [
                { label: 'About', click: () => sendAppCommand('about') },
            ],
        },
    ];

    return Menu.buildFromTemplate(template);
}

function setWindowOpacity(opacity) {
    const normalized = Number.isFinite(opacity) ? Math.min(1, Math.max(0.35, Number(opacity))) : 1;
    if (mainWindow && typeof mainWindow.setOpacity === 'function') {
        mainWindow.setOpacity(normalized);
    }
    return normalized;
}

function emitTerminalEvent(paneId, event) {
    if (event.type === 'error' || event.type === 'session-exited' || event.type === 'ready') {
        logToFile('info', 'terminal event', { paneId, event });
    }
    mainWindow?.webContents.send('terminal-event', { paneId, ...event });
}

function rememberTerminalOutput(bridge, base64Chunk) {
    const size = Buffer.byteLength(base64Chunk, 'base64');
    bridge.outputHistory.push(base64Chunk);
    bridge.outputHistoryBytes += size;

    while (bridge.outputHistoryBytes > MAX_TERMINAL_HISTORY_BYTES && bridge.outputHistory.length > 1) {
        const removed = bridge.outputHistory.shift();
        if (!removed) break;
        bridge.outputHistoryBytes -= Buffer.byteLength(removed, 'base64');
    }
}

function getReplayHistory(bridge, maxBytes = MAX_REATTACH_HISTORY_BYTES) {
    if (!bridge || !Array.isArray(bridge.outputHistory) || bridge.outputHistory.length === 0) {
        return [];
    }

    const replay = [];
    let totalBytes = 0;

    for (let index = bridge.outputHistory.length - 1; index >= 0; index -= 1) {
        const chunk = bridge.outputHistory[index];
        const chunkBytes = Buffer.byteLength(chunk, 'base64');

        if (replay.length > 0 && totalBytes + chunkBytes > maxBytes) {
            break;
        }

        replay.unshift(chunk);
        totalBytes += chunkBytes;
    }

    return replay;
}

function parseCloneSessionToken(value) {
    if (typeof value !== 'string') return null;
    let trimmed = value.trim();
    if (!trimmed.startsWith(CLONE_SESSION_PREFIX)) return null;
    for (let depth = 0; depth < 4; depth += 1) {
        if (!trimmed.startsWith(CLONE_SESSION_PREFIX)) {
            break;
        }
        trimmed = trimmed.slice(CLONE_SESSION_PREFIX.length).trim();
        if (!trimmed) {
            return null;
        }
    }
    return trimmed || null;
}

function sendBridgeCommand(bridge, command) {
    if (!bridge || bridge.process.killed || !bridge.process.stdin.writable) return;
    bridge.process.stdin.write(`${JSON.stringify(command)}\n`);
}

function getBridgeForPane(paneId) {
    const bridge = terminalBridges.get(paneId);
    if (!bridge) {
        throw new Error(`terminal bridge not found for pane ${paneId}`);
    }
    return bridge;
}

function closeBridgeStdin(bridge) {
    if (!bridge || bridge.process.killed || bridge.process.stdin.destroyed) return;
    if (bridge.process.stdin.writableEnded) return;
    bridge.process.stdin.end();
}

function forceKillBridgeProcess(bridge) {
    if (!bridge || bridge.process.killed) return;
    try {
        bridge.process.kill();
    } catch (error) {
        logToFile('error', 'failed to kill bridge process', { paneId: bridge.paneId, message: error.message });
    }
}

function stopTerminalBridge(paneId, killSession = false, force = false) {
    const bridge = terminalBridges.get(paneId);
    if (!bridge) return false;

    bridge.closing = true;
    if (!bridge.process.killed) {
        if (force) {
            forceKillBridgeProcess(bridge);
        } else {
            sendBridgeCommand(bridge, { type: killSession ? 'kill-session' : 'shutdown' });
            closeBridgeStdin(bridge);
            setTimeout(() => {
                forceKillBridgeProcess(bridge);
            }, 1000).unref?.();
        }
    }

    if (bridge.process.killed) {
        terminalBridges.delete(paneId);
    }
    if (killSession) {
        paneSessionHints.delete(paneId);
    }
    return true;
}

function stopAllTerminalBridges(killSessions = false, force = false) {
    for (const paneId of [...terminalBridges.keys()]) {
        stopTerminalBridge(paneId, killSessions, force);
    }
}

async function startTerminalBridge(_event, options = {}) {
    const paneId = typeof options.paneId === 'string' ? options.paneId : '';
    if (!paneId) {
        throw new Error('paneId is required');
    }

    logToFile('info', 'starting terminal bridge', { paneId, options });

    const existing = terminalBridges.get(paneId);
    if (existing) {
        return {
            sessionId: existing.sessionId,
            initialOutput: getReplayHistory(existing),
            state: existing.ready ? 'reachable' : 'checking',
        };
    }

    const daemonReady = await spawnDaemon();
    if (!daemonReady) {
        throw new Error('daemon is not reachable');
    }

    const cliPath = getCliPath();
    if (!fs.existsSync(cliPath)) {
        logToFile('error', 'cli binary missing', { cliPath });
        throw new Error(`tamux CLI not found at ${cliPath}`);
    }

    const cols = Number.isFinite(options.cols) ? Math.max(2, Math.trunc(options.cols)) : 80;
    const rows = Number.isFinite(options.rows) ? Math.max(2, Math.trunc(options.rows)) : 24;
    const args = ['bridge', '--cols', String(cols), '--rows', String(rows)];
    let requestedSessionId = typeof options.sessionId === 'string' ? options.sessionId.trim() : '';
    const cloneFromSessionId = parseCloneSessionToken(requestedSessionId);
    if (cloneFromSessionId) {
        const cloned = await cloneTerminalSession(null, {
            sourcePaneId: typeof options.sourcePaneId === 'string' ? options.sourcePaneId : null,
            sourceSessionId: cloneFromSessionId,
            workspaceId: typeof options.workspaceId === 'string' ? options.workspaceId : null,
            cols,
            rows,
        });
        requestedSessionId = typeof cloned?.sessionId === 'string' ? cloned.sessionId.trim() : '';
    }

    if (requestedSessionId) {
        args.push('--session', requestedSessionId);
    }
    if (typeof options.shell === 'string' && options.shell) {
        args.push('--shell', options.shell);
    }
    if (typeof options.cwd === 'string' && options.cwd) {
        args.push('--cwd', options.cwd);
    }
    if (typeof options.workspaceId === 'string' && options.workspaceId) {
        args.push('--workspace', options.workspaceId);
    }

    const bridgeProcess = spawn(cliPath, args, {
        cwd: path.dirname(cliPath),
        windowsHide: true,
        stdio: ['pipe', 'pipe', 'pipe'],
    });

    const bridge = {
        process: bridgeProcess,
        paneId,
        sessionId: requestedSessionId || null,
        ready: false,
        closing: false,
        outputHistory: [],
        outputHistoryBytes: 0,
        stdoutBuffer: '',
        stderrBuffer: '',
    };
    if (typeof bridge.sessionId === 'string' && bridge.sessionId) {
        paneSessionHints.set(paneId, bridge.sessionId);
    }

    terminalBridges.set(paneId, bridge);

    bridgeProcess.stdout.on('data', (chunk) => {
        bridge.stdoutBuffer += chunk.toString('utf8');
        const lines = bridge.stdoutBuffer.split(/\r?\n/);
        bridge.stdoutBuffer = lines.pop() ?? '';

        for (const line of lines) {
            if (!line.trim()) continue;

            let event;
            try {
                event = JSON.parse(line);
            } catch (error) {
                emitTerminalEvent(paneId, {
                    type: 'error',
                    message: `invalid bridge output: ${error.message}`,
                });
                continue;
            }

            if (event.type === 'ready') {
                bridge.ready = true;
                bridge.sessionId = event.session_id;
                paneSessionHints.set(paneId, event.session_id);
                emitTerminalEvent(paneId, {
                    type: 'ready',
                    sessionId: event.session_id,
                });
                continue;
            }

            if (event.type === 'output') {
                rememberTerminalOutput(bridge, event.data);
                emitTerminalEvent(paneId, {
                    type: 'output',
                    sessionId: event.session_id,
                    data: event.data,
                });
                continue;
            }

            if (event.type === 'session-exited') {
                emitTerminalEvent(paneId, {
                    type: 'session-exited',
                    sessionId: event.session_id,
                    exitCode: event.exit_code,
                });
                paneSessionHints.delete(paneId);
                terminalBridges.delete(paneId);
                continue;
            }

            if (event.type === 'command-finished') {
                emitTerminalEvent(paneId, {
                    type: 'command-finished',
                    sessionId: event.session_id,
                    exitCode: event.exit_code,
                });
                continue;
            }

            if (event.type === 'command-started') {
                emitTerminalEvent(paneId, {
                    type: 'command-started',
                    sessionId: event.session_id,
                    commandB64: event.command_b64,
                });
                continue;
            }

            if (event.type === 'managed-queued') {
                emitTerminalEvent(paneId, {
                    type: 'managed-queued',
                    sessionId: event.session_id,
                    executionId: event.execution_id,
                    position: event.position,
                    snapshot: event.snapshot ?? null,
                });
                continue;
            }

            if (event.type === 'approval-required') {
                emitTerminalEvent(paneId, {
                    type: 'approval-required',
                    sessionId: event.session_id,
                    approval: event.approval,
                });
                continue;
            }

            if (event.type === 'approval-resolved') {
                emitTerminalEvent(paneId, {
                    type: 'approval-resolved',
                    sessionId: event.session_id,
                    approvalId: event.approval_id,
                    decision: event.decision,
                });
                continue;
            }

            if (event.type === 'managed-started') {
                emitTerminalEvent(paneId, {
                    type: 'managed-started',
                    sessionId: event.session_id,
                    executionId: event.execution_id,
                    command: event.command,
                    source: event.source,
                });
                continue;
            }

            if (event.type === 'managed-finished') {
                emitTerminalEvent(paneId, {
                    type: 'managed-finished',
                    sessionId: event.session_id,
                    executionId: event.execution_id,
                    command: event.command,
                    exitCode: event.exit_code,
                    durationMs: event.duration_ms,
                    snapshot: event.snapshot ?? null,
                });
                continue;
            }

            if (event.type === 'managed-rejected') {
                emitTerminalEvent(paneId, {
                    type: 'managed-rejected',
                    sessionId: event.session_id,
                    executionId: event.execution_id,
                    message: event.message,
                });
                continue;
            }

            if (event.type === 'history-search-result') {
                emitTerminalEvent(paneId, {
                    type: 'history-search-result',
                    query: event.query,
                    summary: event.summary,
                    hits: event.hits,
                });
                continue;
            }

            if (event.type === 'skill-generated') {
                emitTerminalEvent(paneId, {
                    type: 'skill-generated',
                    title: event.title,
                    path: event.path,
                });
                continue;
            }

            if (event.type === 'symbol-search-result') {
                emitTerminalEvent(paneId, {
                    type: 'symbol-search-result',
                    symbol: event.symbol,
                    matches: event.matches,
                });
                continue;
            }

            if (event.type === 'snapshot-list') {
                emitTerminalEvent(paneId, {
                    type: 'snapshot-list',
                    snapshots: event.snapshots,
                });
                continue;
            }

            if (event.type === 'snapshot-restored') {
                emitTerminalEvent(paneId, {
                    type: 'snapshot-restored',
                    snapshotId: event.snapshot_id,
                    ok: event.ok,
                    message: event.message,
                });
                continue;
            }

            if (event.type === 'error') {
                emitTerminalEvent(paneId, {
                    type: 'error',
                    message: event.message,
                });
            }
        }
    });

    bridgeProcess.stderr.on('data', (chunk) => {
        bridge.stderrBuffer += chunk.toString('utf8');
        const message = bridge.stderrBuffer.trim();
        if (message) {
            logToFile('error', 'bridge stderr', { paneId, message });
            emitTerminalEvent(paneId, { type: 'error', message });
            bridge.stderrBuffer = '';
        }
    });

    bridgeProcess.on('error', (error) => {
        logToFile('error', 'bridge process error', { paneId, message: error.message });
        emitTerminalEvent(paneId, {
            type: 'error',
            message: error.message,
        });
        terminalBridges.delete(paneId);
    });

    bridgeProcess.on('exit', (code, signal) => {
        logToFile('info', 'bridge process exit', { paneId, code, signal, closing: bridge.closing });
        if (!bridge.closing && code !== 0) {
            emitTerminalEvent(paneId, {
                type: 'error',
                message: `terminal bridge exited with ${signal ?? code}`,
            });
        }
        if (terminalBridges.get(paneId) === bridge) {
            terminalBridges.delete(paneId);
        }
    });

    return {
        sessionId: bridge.sessionId,
        initialOutput: [],
        state: 'checking',
    };
}

function sendTerminalInput(_event, paneId, data) {
    const bridge = terminalBridges.get(paneId);
    if (!bridge || typeof data !== 'string' || !data) return false;
    sendBridgeCommand(bridge, { type: 'input', data });
    return true;
}

function resizeTerminalSession(_event, paneId, cols, rows) {
    const bridge = terminalBridges.get(paneId);
    if (!bridge) return false;
    sendBridgeCommand(bridge, {
        type: 'resize',
        cols: Math.max(2, Math.trunc(cols)),
        rows: Math.max(2, Math.trunc(rows)),
    });
    return true;
}

async function cloneTerminalSession(_event, payload = {}) {
    const sourcePaneId = typeof payload.sourcePaneId === 'string' ? payload.sourcePaneId.trim() : '';
    const requestedSourceSessionIdRaw = typeof payload.sourceSessionId === 'string' ? payload.sourceSessionId.trim() : '';
    const requestedSourceSessionId = parseCloneSessionToken(requestedSourceSessionIdRaw) || requestedSourceSessionIdRaw;
    let sourceSessionId = requestedSourceSessionId;

    if (sourcePaneId) {
        const bridge = terminalBridges.get(sourcePaneId);
        if (bridge && typeof bridge.sessionId === 'string' && bridge.sessionId.trim()) {
            sourceSessionId = bridge.sessionId.trim();
        }
        if (!sourceSessionId) {
            const hinted = paneSessionHints.get(sourcePaneId);
            if (typeof hinted === 'string' && hinted.trim()) {
                sourceSessionId = hinted.trim();
            }
        }
    }

    logToFile('info', 'clone terminal request', {
        sourcePaneId: sourcePaneId || null,
        requestedSourceSessionId: requestedSourceSessionId || null,
        resolvedSourceSessionId: sourceSessionId || null,
        hasLiveBridge: sourcePaneId ? terminalBridges.has(sourcePaneId) : false,
        hasHint: sourcePaneId ? paneSessionHints.has(sourcePaneId) : false,
    });

    if (!sourceSessionId) {
        throw new Error('sourceSessionId is required (and no live source pane session was found)');
    }

    const daemonReady = await spawnDaemon();
    if (!daemonReady) {
        throw new Error('daemon is not reachable');
    }

    const cliPath = getCliPath();
    if (!fs.existsSync(cliPath)) {
        throw new Error(`tamux CLI not found at ${cliPath}`);
    }

    const args = ['clone', '--source', sourceSessionId];
    if (typeof payload.workspaceId === 'string' && payload.workspaceId.trim()) {
        args.push('--workspace', payload.workspaceId.trim());
    }
    if (Number.isFinite(payload.cols)) {
        args.push('--cols', String(Math.max(2, Math.trunc(payload.cols))));
    }
    if (Number.isFinite(payload.rows)) {
        args.push('--rows', String(Math.max(2, Math.trunc(payload.rows))));
    }
    if (typeof payload.cwd === 'string' && payload.cwd.trim()) {
        args.push('--cwd', payload.cwd.trim());
    }

    logToFile('info', 'cloning terminal session', {
        sourcePaneId: sourcePaneId || null,
        requestedSourceSessionId: requestedSourceSessionId || null,
        sourceSessionId,
        workspaceId: payload.workspaceId ?? null,
        cols: payload.cols ?? null,
        rows: payload.rows ?? null,
    });

    try {
        const result = await new Promise((resolve, reject) => {
            const child = spawn(cliPath, args, {
                cwd: path.dirname(cliPath),
                windowsHide: true,
                stdio: ['ignore', 'pipe', 'pipe'],
            });

            let stdout = '';
            let stderr = '';

            child.stdout.on('data', (chunk) => {
                stdout += chunk.toString('utf8');
            });
            child.stderr.on('data', (chunk) => {
                stderr += chunk.toString('utf8');
            });
            child.on('error', (error) => {
                reject(error);
            });
            child.on('exit', (code) => {
                if (code !== 0) {
                    reject(new Error((stderr || stdout || `tamux clone exited with code ${code}`).trim()));
                    return;
                }

                const lines = stdout
                    .split(/\r?\n/)
                    .map((line) => line.trim())
                    .filter(Boolean);
                const sessionId = lines[0] ?? '';
                if (!sessionId) {
                    reject(new Error('tamux clone did not return a session id'));
                    return;
                }

                // Parse optional active_command from daemon
                const cmdLine = lines.find((l) => l.startsWith('active_command:'));
                const activeCommand = cmdLine ? cmdLine.slice('active_command:'.length) : null;

                resolve({ sessionId, activeCommand });
            });
        });

        logToFile('info', 'cloned terminal session', {
            sourceSessionId,
            clonedSessionId: result.sessionId,
            activeCommand: result.activeCommand ?? null,
        });
        return result;
    } catch (error) {
        logToFile('error', 'failed to clone terminal session', {
            sourceSessionId,
            message: error?.message ?? String(error),
        });
        throw error;
    }
}

function executeManagedCommand(_event, paneId, payload = {}) {
    const bridge = getBridgeForPane(paneId);
    sendBridgeCommand(bridge, {
        type: 'execute-managed',
        command: typeof payload.command === 'string' ? payload.command : '',
        rationale: typeof payload.rationale === 'string' ? payload.rationale : 'Operator requested managed execution',
        allow_network: Boolean(payload.allowNetwork),
        sandbox_enabled: Boolean(payload.sandboxEnabled),
        security_level: typeof payload.securityLevel === 'string' ? payload.securityLevel : 'moderate',
        cwd: typeof payload.cwd === 'string' ? payload.cwd : null,
        language_hint: typeof payload.languageHint === 'string' ? payload.languageHint : null,
        source: typeof payload.source === 'string' ? payload.source : 'agent',
    });
    return true;
}

function commandExists(binary) {
    if (typeof binary !== 'string' || !binary.trim()) return false;
    try {
        const checker = process.platform === 'win32' ? 'where' : 'which';
        const result = spawnSync(checker, [binary], { stdio: 'ignore' });
        return result.status === 0;
    } catch {
        return false;
    }
}

const KNOWN_CODING_AGENTS = [
    {
        id: 'claude',
        label: 'Claude Code',
        description: "Anthropic's terminal coding agent.",
        executables: ['claude'],
        versionArgs: ['--version'],
        launchArgs: [],
    },
    {
        id: 'codex',
        label: 'Codex CLI',
        description: 'OpenAI Codex terminal workflow.',
        executables: ['codex'],
        versionArgs: ['--version'],
        launchArgs: [],
    },
    {
        id: 'gemini',
        label: 'Gemini CLI',
        description: 'Google Gemini terminal agent.',
        executables: ['gemini'],
        versionArgs: ['--version'],
        launchArgs: [],
    },
    {
        id: 'hermes',
        label: 'Hermes Agent',
        description: 'Nous Research Hermes agent runtime.',
        executables: ['hermes'],
        versionArgs: ['--version'],
        configPaths: ['~/.hermes/config.yaml', '~/.hermes/.env'],
        launchArgs: [],
    },
    {
        id: 'pi',
        label: 'pi.dev',
        description: 'Pi terminal coding harness.',
        executables: ['pi'],
        versionArgs: ['--version'],
        configPaths: ['~/.pi/agent/settings.json', '~/.pi/agent/sessions', '~/.pi/agent/AGENTS.md'],
        launchArgs: [],
    },
    {
        id: 'opencode',
        label: 'OpenCode',
        description: 'OpenCode terminal coding assistant.',
        executables: ['opencode'],
        versionArgs: ['--version'],
        launchArgs: [],
    },
    {
        id: 'openclaw',
        label: 'OpenClaw',
        description: 'OpenClaw agent and gateway runtime.',
        executables: ['openclaw'],
        versionArgs: ['--version'],
        configPaths: ['~/.openclaw/openclaw.json', '~/.openclaw/workspace', '~/.openclaw/state'],
        launchArgs: [],
    },
    {
        id: 'kimi',
        label: 'Kimi CLI',
        description: 'Moonshot Kimi coding assistant.',
        executables: ['kimi'],
        versionArgs: ['--version'],
        launchArgs: [],
    },
    {
        id: 'aider',
        label: 'Aider',
        description: 'Aider pair-programming CLI.',
        executables: ['aider'],
        versionArgs: ['--version'],
        launchArgs: [],
    },
    {
        id: 'goose',
        label: 'Goose',
        description: 'Goose local coding agent.',
        executables: ['goose'],
        versionArgs: ['--version'],
        launchArgs: [],
    },
    {
        id: 'qwen-coder',
        label: 'Qwen Coder',
        description: 'Qwen coding CLI if installed locally.',
        executables: ['qwen', 'qwen-coder'],
        versionArgs: ['--version'],
        launchArgs: [],
    },
];

const KNOWN_AI_TRAINING = [
    {
        id: 'prime-verifiers',
        label: 'Prime Intellect Verifiers',
        kind: 'training-runtime',
        description: 'Prime Intellect environments, evaluation, and RL workflow runtime.',
        executables: ['prime'],
        versionArgs: ['--version'],
        systemChecks: [
            { label: 'prime CLI', path: 'prime', type: 'command' },
            { label: 'uv', path: 'uv', type: 'command' },
        ],
        workspaceChecks: [
            { label: 'configs/', path: 'configs' },
            { label: 'environments/', path: 'environments' },
            { label: 'AGENTS.md', path: 'AGENTS.md' },
        ],
    },
    {
        id: 'autoresearch',
        label: 'AutoResearch',
        kind: 'repository-workflow',
        description: 'Karpathy\'s repo-local autonomous research loop for a single-GPU training harness.',
        executables: ['uv'],
        versionArgs: ['--version'],
        systemChecks: [
            { label: 'uv', path: 'uv', type: 'command' },
            { label: 'python3', path: 'python3', type: 'command' },
            { label: 'git', path: 'git', type: 'command' },
        ],
        workspaceChecks: [
            { label: 'program.md', path: 'program.md' },
            { label: 'train.py', path: 'train.py' },
            { label: 'prepare.py', path: 'prepare.py' },
            { label: 'pyproject.toml', path: 'pyproject.toml' },
        ],
    },
    {
        id: 'autorl',
        label: 'AutoRL',
        kind: 'repository-workflow',
        description: 'Repo-local autonomous RL environment search scaffold backed by Simverse.',
        executables: ['python3'],
        versionArgs: ['--version'],
        systemChecks: [
            { label: 'python3', path: 'python3', type: 'command' },
            { label: 'git', path: 'git', type: 'command' },
        ],
        workspaceChecks: [
            { label: 'program.md', path: 'program.md' },
            { label: 'train.py', path: 'train.py' },
            { label: 'framework.py', path: 'framework.py' },
            { label: 'vendor/simverse', path: 'vendor/simverse' },
            { label: '.venv', path: '.venv' },
        ],
    },
];

function resolveExecutablePath(binary) {
    if (typeof binary !== 'string' || !binary.trim()) return null;
    try {
        const checker = process.platform === 'win32' ? 'where' : 'which';
        const result = spawnSync(checker, [binary], {
            encoding: 'utf8',
            timeout: 5000,
            windowsHide: true,
        });
        if (result.status !== 0) {
            return null;
        }

        const firstLine = `${result.stdout || ''}`.split(/\r?\n/).map((entry) => entry.trim()).find(Boolean);
        return firstLine || null;
    } catch {
        return null;
    }
}

function probeExecutableVersion(commandPath, versionArgs = ['--version']) {
    if (!commandPath) {
        return null;
    }

    try {
        const result = spawnSync(commandPath, versionArgs, {
            encoding: 'utf8',
            timeout: 5000,
            windowsHide: true,
        });

        const output = `${result.stdout || result.stderr || ''}`.split(/\r?\n/).map((entry) => entry.trim()).find(Boolean);
        return output || null;
    } catch {
        return null;
    }
}

function expandHomePath(targetPath) {
    if (typeof targetPath !== 'string' || !targetPath.trim()) {
        return targetPath;
    }

    if (targetPath === '~') {
        return os.homedir();
    }

    if (targetPath.startsWith('~/')) {
        return path.join(os.homedir(), targetPath.slice(2));
    }

    return targetPath;
}

function collectConfigChecks(paths = []) {
    return paths.map((entry) => {
        const expandedPath = expandHomePath(entry);
        const resolvedPath = path.resolve(expandedPath);
        return {
            label: path.basename(entry) || entry,
            path: entry,
            exists: fs.existsSync(resolvedPath),
        };
    });
}

function resolveWorkspacePath(workspacePath) {
    if (typeof workspacePath !== 'string' || !workspacePath.trim()) {
        return null;
    }

    return path.resolve(expandHomePath(workspacePath));
}

function collectAITrainingChecks(definition, workspacePath) {
    const checks = [];

    for (const check of definition.systemChecks || []) {
        const exists = check.type === 'command'
            ? commandExists(check.path)
            : fs.existsSync(path.resolve(expandHomePath(check.path)));
        checks.push({
            label: check.label,
            path: check.path,
            exists,
            scope: 'system',
        });
    }

    for (const check of definition.workspaceChecks || []) {
        const targetPath = workspacePath ? path.join(workspacePath, check.path) : null;
        checks.push({
            label: check.label,
            path: check.path,
            exists: targetPath ? fs.existsSync(targetPath) : false,
            scope: 'workspace',
        });
    }

    return checks;
}

function hasWorkspaceChecks(checks, paths) {
    return paths.every((targetPath) => checks.some((check) => check.scope === 'workspace' && check.path === targetPath && check.exists));
}

function summarizeRuntimeReadiness(agent, available, checks, gatewayReachable) {
    if (!available) {
        return {
            readiness: 'missing',
            runtimeNotes: [`${agent.label} is not installed on PATH.`],
        };
    }

    const existingChecks = checks.filter((check) => check.exists);
    const missingChecks = checks.filter((check) => !check.exists);
    const runtimeNotes = [];

    if (agent.id === 'hermes') {
        if (existingChecks.length > 0) {
            runtimeNotes.push('Hermes configuration was detected. Consider wiring tamux-mcp into Hermes MCP settings for deeper integration.');
            return { readiness: 'ready', runtimeNotes };
        }

        runtimeNotes.push('Hermes is installed, but no ~/.hermes config was detected yet. Run hermes setup before expecting provider-backed workflows.');
        return { readiness: 'needs-setup', runtimeNotes };
    }

    if (agent.id === 'openclaw') {
        if (existingChecks.length === 0) {
            runtimeNotes.push('OpenClaw is installed, but no ~/.openclaw runtime files were detected yet. Run onboarding before expecting gateway-backed workflows.');
            return { readiness: 'needs-setup', runtimeNotes };
        }

        if (gatewayReachable === true) {
            runtimeNotes.push('OpenClaw gateway responded on 127.0.0.1:18789.');
            return { readiness: 'ready', runtimeNotes };
        }

        runtimeNotes.push('OpenClaw configuration is present, but the local gateway did not respond on 127.0.0.1:18789. Direct agent mode should still be usable.');
        return { readiness: 'needs-setup', runtimeNotes };
    }

    if (agent.id === 'pi') {
        if (existingChecks.length > 0) {
            runtimeNotes.push('Pi configuration and session storage were detected under ~/.pi/agent.');
            return { readiness: 'ready', runtimeNotes };
        }

        runtimeNotes.push('pi is installed, but no ~/.pi/agent profile was detected yet. Run pi once and complete provider login or API-key setup.');
        return { readiness: 'needs-setup', runtimeNotes };
    }

    if (missingChecks.length > 0 && existingChecks.length === 0 && checks.length > 0) {
        runtimeNotes.push(`${agent.label} is installed, but none of the known config paths were detected.`);
        return { readiness: 'needs-setup', runtimeNotes };
    }

    return { readiness: 'ready', runtimeNotes };
}

function checkLocalTcpPort(host, port, timeoutMs = 300) {
    return new Promise((resolve) => {
        const socket = net.createConnection({ host, port });
        let settled = false;

        const finish = (value) => {
            if (settled) {
                return;
            }
            settled = true;
            try {
                socket.destroy();
            } catch {
                // Ignore socket cleanup errors.
            }
            resolve(value);
        };

        socket.setTimeout(timeoutMs);
        socket.once('connect', () => finish(true));
        socket.once('timeout', () => finish(false));
        socket.once('error', () => finish(false));
        socket.once('close', () => finish(false));
    });
}

async function discoverCodingAgents() {
    const discovered = await Promise.all(KNOWN_CODING_AGENTS.map(async (agent) => {
        const executable = agent.executables.find((candidate) => resolveExecutablePath(candidate)) || null;
        const commandPath = executable ? resolveExecutablePath(executable) : null;
        const checks = collectConfigChecks(agent.configPaths || []);
        const gatewayReachable = agent.id === 'openclaw'
            ? await checkLocalTcpPort('127.0.0.1', 18789)
            : null;
        const readinessSummary = summarizeRuntimeReadiness(agent, Boolean(commandPath), checks, gatewayReachable);

        return {
            id: agent.id,
            available: Boolean(commandPath),
            executable,
            path: commandPath,
            version: commandPath ? probeExecutableVersion(commandPath, agent.versionArgs) : null,
            readiness: readinessSummary.readiness,
            checks,
            runtimeNotes: readinessSummary.runtimeNotes,
            gatewayLabel: agent.id === 'openclaw' ? '127.0.0.1:18789' : null,
            gatewayReachable,
            error: commandPath ? null : `${agent.label} was not found on PATH.`,
        };
    }));

    return discovered;
}

function summarizeAITrainingReadiness(definition, available, checks, workspacePath) {
    if (!available) {
        return {
            readiness: 'missing',
            runtimeNotes: [`${definition.label} is missing a required system dependency.`],
        };
    }

    const runtimeNotes = [];
    const systemChecks = checks.filter((check) => check.scope === 'system');
    const workspaceChecks = checks.filter((check) => check.scope === 'workspace');
    const missingSystem = systemChecks.filter((check) => !check.exists);
    const presentWorkspace = workspaceChecks.filter((check) => check.exists);
    const missingWorkspace = workspaceChecks.filter((check) => !check.exists);

    if (missingSystem.length > 0) {
        runtimeNotes.push(`Missing system prerequisites: ${missingSystem.map((check) => check.label).join(', ')}.`);
        return { readiness: 'missing', runtimeNotes };
    }

    if (!workspacePath) {
        runtimeNotes.push('Select a workspace with a configured cwd to evaluate repository-specific training readiness.');
        return { readiness: 'needs-setup', runtimeNotes };
    }

    if (definition.id === 'prime-verifiers') {
        if (missingWorkspace.length === 0) {
            runtimeNotes.push('Prime lab workspace files were detected and should be ready for evaluation or environment work.');
            return { readiness: 'ready', runtimeNotes };
        }

        runtimeNotes.push('Prime CLI is available, but this workspace does not look fully initialized. Run prime lab setup in the target workspace.');
        return { readiness: 'needs-setup', runtimeNotes };
    }

    if (definition.id === 'autoresearch') {
        if (missingWorkspace.length === 0) {
            runtimeNotes.push('AutoResearch repo files were detected. A compatible GPU is still required for meaningful training runs.');
            return { readiness: 'ready', runtimeNotes };
        }

        runtimeNotes.push('This workspace is missing one or more AutoResearch files. Clone the repo and keep program.md, train.py, prepare.py, and pyproject.toml together.');
        return { readiness: 'needs-setup', runtimeNotes };
    }

    if (definition.id === 'autorl') {
        const baseReady = presentWorkspace.some((check) => check.path === 'program.md')
            && presentWorkspace.some((check) => check.path === 'train.py')
            && presentWorkspace.some((check) => check.path === 'framework.py')
            && presentWorkspace.some((check) => check.path === 'vendor/simverse');
        const venvReady = presentWorkspace.some((check) => check.path === '.venv');

        if (baseReady && venvReady) {
            runtimeNotes.push('AutoRL workspace and virtual environment were detected. The evaluator should be runnable from this workspace.');
            return { readiness: 'ready', runtimeNotes };
        }

        if (baseReady) {
            runtimeNotes.push('AutoRL repo files were detected, but .venv is missing. Create the virtualenv and install vendor/simverse before evaluator runs.');
            return { readiness: 'needs-setup', runtimeNotes };
        }

        runtimeNotes.push('This workspace does not look like the AutoRL scaffold yet. Clone the repo branch and keep vendor/simverse plus the training files together.');
        return { readiness: 'needs-setup', runtimeNotes };
    }

    return { readiness: 'ready', runtimeNotes };
}

async function discoverAITraining(workspacePath) {
    const resolvedWorkspacePath = resolveWorkspacePath(workspacePath);

    return Promise.all(KNOWN_AI_TRAINING.map(async (definition) => {
        const systemExecutable = definition.executables.find((candidate) => resolveExecutablePath(candidate)) || null;
        const systemPath = systemExecutable ? resolveExecutablePath(systemExecutable) : null;
        const checks = collectAITrainingChecks(definition, resolvedWorkspacePath);
        const readinessSummary = summarizeAITrainingReadiness(definition, Boolean(systemPath), checks, resolvedWorkspacePath);
        let available = Boolean(systemPath);
        let executable = systemExecutable;
        let path = systemPath;
        let error = systemPath ? null : `${definition.label} prerequisites were not found on PATH.`;

        if (definition.kind === 'repository-workflow') {
            const baseWorkspaceReady = definition.id === 'autoresearch'
                ? hasWorkspaceChecks(checks, ['program.md', 'train.py', 'prepare.py', 'pyproject.toml'])
                : hasWorkspaceChecks(checks, ['program.md', 'train.py', 'framework.py', 'vendor/simverse']);

            available = Boolean(systemPath) && baseWorkspaceReady;
            executable = definition.id === 'autoresearch'
                ? 'uv run train.py'
                : '.venv/bin/python train.py';
            path = resolvedWorkspacePath;

            if (!resolvedWorkspacePath) {
                error = 'Select a workspace with a configured cwd.';
            } else if (!systemPath) {
                error = `${definition.label} is missing one or more required system tools.`;
            } else if (!baseWorkspaceReady) {
                error = `${definition.label} repository files were not detected in the selected workspace.`;
            } else {
                error = null;
            }
        }

        return {
            id: definition.id,
            available,
            executable,
            path,
            version: systemPath ? probeExecutableVersion(systemPath, definition.versionArgs) : null,
            readiness: readinessSummary.readiness,
            checks,
            runtimeNotes: readinessSummary.runtimeNotes,
            workspacePath: resolvedWorkspacePath,
            error,
        };
    }));
}

function checkLspHealth() {
    return {
        rustAnalyzer: commandExists('rust-analyzer'),
        typescriptLanguageServer: commandExists('typescript-language-server'),
        pyrightLangserver: commandExists('pyright-langserver'),
    };
}

function checkMcpHealth(_event, servers = {}) {
    const checks = [];
    const entries = typeof servers === 'object' && servers !== null
        ? Object.entries(servers)
        : [];

    checks.push({
        name: 'tamux',
        command: 'tamux-mcp',
        exists: commandExists('tamux-mcp'),
    });

    for (const [name, value] of entries) {
        const command = typeof value?.command === 'string' ? value.command.trim() : '';
        if (!command) {
            checks.push({ name, command: '', exists: false, error: 'missing command' });
            continue;
        }
        checks.push({
            name,
            command,
            exists: commandExists(command),
        });
    }

    return checks;
}

function resolveManagedApproval(_event, paneId, approvalId, decision) {
    const bridge = getBridgeForPane(paneId);
    sendBridgeCommand(bridge, {
        type: 'approval-decision',
        approval_id: approvalId,
        decision,
    });
    return true;
}

function searchManagedHistory(_event, paneId, query, limit) {
    const bridge = getBridgeForPane(paneId);
    sendBridgeCommand(bridge, { type: 'search-history', query, limit });
    return true;
}

function generateManagedSkill(_event, paneId, query, title) {
    const bridge = getBridgeForPane(paneId);
    sendBridgeCommand(bridge, { type: 'generate-skill', query, title });
    return true;
}

function findManagedSymbol(_event, paneId, workspaceRoot, symbol, limit) {
    const bridge = getBridgeForPane(paneId);
    sendBridgeCommand(bridge, { type: 'find-symbol', workspace_root: workspaceRoot, symbol, limit });
    return true;
}

function listSnapshots(_event, paneId, workspaceId) {
    const bridge = getBridgeForPane(paneId);
    sendBridgeCommand(bridge, { type: 'list-snapshots', workspace_id: workspaceId ?? null });
    return true;
}

function restoreSnapshot(_event, paneId, snapshotId) {
    const bridge = getBridgeForPane(paneId);
    sendBridgeCommand(bridge, { type: 'restore-snapshot', snapshot_id: snapshotId });
    return true;
}

function getDaemonEndpoint() {
    if (process.platform === 'win32') {
        return { host: DAEMON_TCP_HOST, port: DAEMON_TCP_PORT };
    }
    const runtimeDir = process.env.XDG_RUNTIME_DIR || '/tmp';
    return { path: path.join(runtimeDir, 'tamux-daemon.sock') };
}

async function checkDaemonRunning() {
    const endpoint = getDaemonEndpoint();
    logToFile('info', 'checking daemon endpoint', endpoint);
    return new Promise((resolve) => {
        const socket = new net.Socket();
        socket.setTimeout(1000);
        socket.once('connect', () => { socket.destroy(); resolve(true); });
        socket.once('error', () => { socket.destroy(); resolve(false); });
        socket.once('timeout', () => { socket.destroy(); resolve(false); });
        socket.connect(endpoint);
    });
}

async function spawnDaemon() {
    const isRunning = await checkDaemonRunning();
    if (isRunning) { console.log('[tamux] Daemon already running'); return true; }

    const daemonPath = getDaemonPath();
    console.log('[tamux] Spawning daemon:', daemonPath);
    logToFile('info', 'spawning daemon', { daemonPath });

    if (!fs.existsSync(daemonPath)) {
        console.error('[tamux] Daemon binary not found at:', daemonPath);
        logToFile('error', 'daemon binary missing', { daemonPath });
        return false;
    }

    const daemon = spawn(daemonPath, [], {
        detached: true, stdio: 'ignore', windowsHide: true,
        cwd: path.dirname(daemonPath),
    });
    daemon.on('error', (err) => {
        console.error('[tamux] Daemon error:', err);
        logToFile('error', 'daemon process error', { message: err.message });
    });
    daemon.unref();

    for (let i = 0; i < 20; i++) {
        await new Promise(r => setTimeout(r, 250));
        if (await checkDaemonRunning()) {
            console.log('[tamux] Daemon ready');
            logToFile('info', 'daemon ready');
            return true;
        }
    }
    console.warn('[tamux] Daemon did not become ready');
    logToFile('error', 'daemon did not become ready');
    return false;
}

function getSystemFonts() {
    try {
        if (process.platform === 'win32') {
            const out = execSync(
                'powershell -NoProfile -Command "[System.Reflection.Assembly]::LoadWithPartialName(\'System.Drawing\') | Out-Null; (New-Object System.Drawing.Text.InstalledFontCollection).Families | ForEach-Object { $_.Name }"',
                { encoding: 'utf-8', timeout: 10000, windowsHide: true }
            );
            return out.split('\n').map(s => s.trim()).filter(Boolean).sort();
        }

        const out = execSync('fc-list --format="%{family[0]}\\n" | sort -u', {
            encoding: 'utf-8', timeout: 10000,
        });
        return out.split('\n').map(s => s.trim()).filter(Boolean);
    } catch {
        return ['Cascadia Code', 'Cascadia Mono', 'Consolas', 'JetBrains Mono', 'Fira Code',
            'Source Code Pro', 'Hack', 'DejaVu Sans Mono', 'Ubuntu Mono', 'Courier New', 'monospace'];
    }
}

function getAvailableShells() {
    const shells = [];
    try {
        if (process.platform === 'win32') {
            // Known Windows shell paths
            const systemRoot = process.env.SystemRoot || 'C:\\Windows';
            const windowsShells = [
                { name: 'Windows PowerShell', path: path.join(systemRoot, 'System32', 'WindowsPowerShell', 'v1.0', 'powershell.exe') },
                { name: 'Command Prompt', path: path.join(systemRoot, 'System32', 'cmd.exe') },
            ];

            // PowerShell 7 (try where.exe to find it)
            try {
                const pwshPath = execFileSync('where.exe', ['pwsh.exe'], {
                    encoding: 'utf-8', timeout: 5000, windowsHide: true,
                }).split('\n')[0].trim();
                if (pwshPath) {
                    shells.push({ name: 'PowerShell 7', path: pwshPath });
                }
            } catch {}

            for (const s of windowsShells) {
                if (fs.existsSync(s.path)) {
                    shells.push(s);
                }
            }

            // Detect WSL distributions
            try {
                const wslOut = execFileSync('wsl.exe', ['-l', '-q'], {
                    encoding: 'utf-16le', timeout: 5000, windowsHide: true,
                });
                const distros = wslOut.split('\n')
                    .map((s) => s.replace(/\0/g, '').trim())
                    .filter(Boolean);
                if (distros.length > 0) {
                    shells.push({ name: 'WSL (default)', path: 'wsl' });
                }
                for (const distro of distros) {
                    shells.push({ name: `WSL: ${distro}`, path: 'wsl', args: `-d ${distro}` });
                }
            } catch {}
        } else {
            // Unix: read /etc/shells
            try {
                const content = fs.readFileSync('/etc/shells', 'utf-8');
                const shellPaths = content.split('\n')
                    .map((line) => line.trim())
                    .filter((line) => line && !line.startsWith('#'));
                for (const shellPath of shellPaths) {
                    if (fs.existsSync(shellPath)) {
                        shells.push({ name: path.basename(shellPath), path: shellPath });
                    }
                }
            } catch {}

            // Fallback to $SHELL
            if (shells.length === 0 && process.env.SHELL) {
                shells.push({ name: path.basename(process.env.SHELL), path: process.env.SHELL });
            }
        }
    } catch {
        // Return whatever we collected so far
    }
    return shells;
}

let lastCpuSnapshot = null;

function aggregateCpuTimes() {
    const cpus = os.cpus();
    let idle = 0;
    let total = 0;

    for (const cpu of cpus) {
        idle += cpu.times.idle;
        total += cpu.times.user + cpu.times.nice + cpu.times.sys + cpu.times.idle + cpu.times.irq;
    }

    return { idle, total };
}

function getCpuUsagePercent() {
    const current = aggregateCpuTimes();

    if (!lastCpuSnapshot) {
        lastCpuSnapshot = current;
        return 0;
    }

    const totalDelta = current.total - lastCpuSnapshot.total;
    const idleDelta = current.idle - lastCpuSnapshot.idle;
    lastCpuSnapshot = current;

    if (totalDelta <= 0) {
        return 0;
    }

    return Number((((totalDelta - idleDelta) / totalDelta) * 100).toFixed(1));
}

function getSwapStats() {
    try {
        if (process.platform === 'linux') {
            const output = execSync('free -b', { encoding: 'utf8', timeout: 5000 });
            const swapLine = output.split('\n').find((line) => line.trim().startsWith('Swap:'));
            if (!swapLine) return null;

            const parts = swapLine.trim().split(/\s+/);
            return {
                totalBytes: Number(parts[1] || 0),
                usedBytes: Number(parts[2] || 0),
                freeBytes: Number(parts[3] || 0),
            };
        }
    } catch {
        return null;
    }

    return null;
}

function getGpuStats() {
    try {
        const output = execSync(
            'nvidia-smi --query-gpu=name,memory.used,memory.total,utilization.gpu --format=csv,noheader,nounits',
            { encoding: 'utf8', timeout: 5000, windowsHide: true }
        );

        return output
            .split('\n')
            .map((line) => line.trim())
            .filter(Boolean)
            .map((line, index) => {
                const [name, memoryUsedMB, memoryTotalMB, utilizationPercent] = line.split(',').map((part) => part.trim());
                return {
                    id: `gpu_${index}`,
                    name,
                    memoryUsedMB: Number(memoryUsedMB || 0),
                    memoryTotalMB: Number(memoryTotalMB || 0),
                    utilizationPercent: Number(utilizationPercent || 0),
                };
            });
    } catch {
        return [];
    }
}

function getTopProcesses(limit = 24) {
    const safeLimit = Math.max(8, Math.min(64, Number(limit) || 24));

    try {
        if (process.platform === 'win32') {
            const command = `powershell -NoProfile -Command "Get-CimInstance Win32_Process | Select-Object ProcessId,Name,WorkingSetSize,CommandLine | Sort-Object WorkingSetSize -Descending | Select-Object -First ${safeLimit} | ConvertTo-Json -Compress"`;
            const output = execSync(command, { encoding: 'utf8', timeout: 10000, windowsHide: true }).trim();
            if (!output) return [];

            const parsed = JSON.parse(output);
            const items = Array.isArray(parsed) ? parsed : [parsed];

            return items.map((item) => ({
                pid: Number(item.ProcessId || 0),
                name: String(item.Name || 'unknown'),
                cpuPercent: null,
                memoryBytes: Number(item.WorkingSetSize || 0),
                state: 'running',
                command: String(item.CommandLine || item.Name || ''),
            }));
        }

        const output = execSync(`ps -eo pid=,comm=,%cpu=,rss=,state=,args= --sort=-%cpu | head -n ${safeLimit + 1}`, {
            encoding: 'utf8',
            timeout: 10000,
        });

        return output
            .split('\n')
            .map((line) => line.trim())
            .filter(Boolean)
            .map((line) => {
                const match = line.match(/^(\d+)\s+(\S+)\s+([\d.]+)\s+(\d+)\s+(\S+)\s+(.*)$/);
                if (!match) return null;

                return {
                    pid: Number(match[1]),
                    name: match[2],
                    cpuPercent: Number(match[3]),
                    memoryBytes: Number(match[4]) * 1024,
                    state: match[5],
                    command: match[6],
                };
            })
            .filter(Boolean);
    } catch {
        return [];
    }
}

function getSystemMonitorSnapshot(_event, options = {}) {
    const cpus = os.cpus();
    const totalMemoryBytes = os.totalmem();
    const freeMemoryBytes = os.freemem();
    const usedMemoryBytes = totalMemoryBytes - freeMemoryBytes;
    const swap = getSwapStats();
    const gpus = getGpuStats();
    const processLimit = options && typeof options === 'object' ? options.processLimit : undefined;

    return {
        timestamp: Date.now(),
        platform: process.platform,
        hostname: os.hostname(),
        uptimeSeconds: Math.round(os.uptime()),
        cpu: {
            usagePercent: getCpuUsagePercent(),
            coreCount: cpus.length,
            model: cpus[0]?.model || 'unknown',
            loadAverage: os.loadavg().map((value) => Number(value.toFixed(2))),
        },
        memory: {
            totalBytes: totalMemoryBytes,
            usedBytes: usedMemoryBytes,
            freeBytes: freeMemoryBytes,
            swapTotalBytes: swap?.totalBytes ?? null,
            swapUsedBytes: swap?.usedBytes ?? null,
            swapFreeBytes: swap?.freeBytes ?? null,
        },
        gpus,
        processes: getTopProcesses(processLimit),
    };
}

function createWindow() {
    const { width: screenW, height: screenH } = screen.getPrimaryDisplay().workAreaSize;
    const useNativeFrame = process.platform === 'win32';

    mainWindow = new BrowserWindow({
        width: Math.min(1400, screenW), height: Math.min(900, screenH),
        minWidth: 600, minHeight: 400,
        frame: useNativeFrame,
        titleBarStyle: useNativeFrame ? 'default' : 'hidden',
        autoHideMenuBar: false,
        titleBarOverlay: useNativeFrame ? undefined : process.platform === 'win32' ? {
            color: '#181825', symbolColor: '#cdd6f4', height: 36,
        } : undefined,
        webPreferences: {
            preload: path.join(__dirname, 'preload.cjs'),
            nodeIntegration: false, contextIsolation: true,
            webviewTag: true,
        },
        title: 'tamux',
        icon: path.join(__dirname, '..', 'assets', 'icon.ico'),
        backgroundColor: '#1e1e2e', show: false,
        opacity: 1,
    });

    const isDev = !app.isPackaged;
    if (isDev) mainWindow.loadURL('http://localhost:5173');
    else mainWindow.loadFile(path.join(__dirname, '..', 'dist', 'index.html'));

    mainWindow.webContents.setWindowOpenHandler(({ url }) => {
        if (/^https?:\/\//i.test(url)) {
            void shell.openExternal(url);
            return { action: 'deny' };
        }

        return { action: 'allow' };
    });

    mainWindow.webContents.on('will-navigate', (event, url) => {
        const currentUrl = mainWindow?.webContents.getURL();
        if (url !== currentUrl && /^https?:\/\//i.test(url)) {
            event.preventDefault();
            void shell.openExternal(url);
        }
    });

    Menu.setApplicationMenu(buildAppMenu());

    mainWindow.once('ready-to-show', () => mainWindow.show());
    if (isDev) mainWindow.webContents.openDevTools();

    mainWindow.on('maximize', () => mainWindow.webContents.send('window-state', 'maximized'));
    mainWindow.on('unmaximize', () => mainWindow.webContents.send('window-state', 'normal'));
    mainWindow.on('closed', () => {
        logToFile('info', 'main window closed');
        stopAllTerminalBridges(true, true);
        mainWindow = null;
    });
}

function registerIpcHandlers() {
    ipcMain.handle('getSocketPath', () => {
        const endpoint = getDaemonEndpoint();
        return endpoint.path ?? `${endpoint.host}:${endpoint.port}`;
    });
    ipcMain.handle('checkDaemon', () => checkDaemonRunning());
    ipcMain.handle('spawnDaemon', () => spawnDaemon());
    ipcMain.handle('getSystemFonts', () => getSystemFonts());
    ipcMain.handle('getAvailableShells', () => getAvailableShells());
    ipcMain.handle('system-monitor-snapshot', getSystemMonitorSnapshot);
    ipcMain.handle('getDaemonPath', () => getDaemonPath());
    ipcMain.handle('getPlatform', () => process.platform);
    ipcMain.handle('coding-agents-discover', () => discoverCodingAgents());
    ipcMain.handle('ai-training-discover', (_event, workspacePath) => discoverAITraining(workspacePath));
    ipcMain.handle('plugin-list-installed', () => listInstalledPlugins());
    ipcMain.handle('plugin-load-installed', () => loadInstalledPluginScripts());
    ipcMain.handle('diagnostics-check-lsp', checkLspHealth);
    ipcMain.handle('diagnostics-check-mcp', checkMcpHealth);
    ipcMain.handle('persistence-get-data-dir', () => ensureTamuxDataDir());
    ipcMain.handle('persistence-read-json', (_event, relativePath) => readJsonFile(relativePath));
    ipcMain.handle('persistence-write-json', (_event, relativePath, data) => writeJsonFile(relativePath, data));
    ipcMain.handle('persistence-read-text', (_event, relativePath) => readTextFile(relativePath));
    ipcMain.handle('persistence-write-text', (_event, relativePath, content) => writeTextFile(relativePath, content));
    ipcMain.handle('persistence-delete-path', (_event, relativePath) => deleteDataPath(relativePath));
    ipcMain.handle('persistence-list-dir', (_event, relativeDir) => listDataDir(relativeDir));
    ipcMain.handle('persistence-open-path', (_event, relativePath) => openDataPath(relativePath));
    ipcMain.handle('persistence-reveal-path', (_event, relativePath) => revealDataPath(relativePath));
    ipcMain.handle('fs-list-dir', (_event, targetDir) => listFsDir(targetDir));
    ipcMain.handle('fs-copy-path', (_event, sourcePath, destinationPath) => copyFsPath(sourcePath, destinationPath));
    ipcMain.handle('fs-move-path', (_event, sourcePath, destinationPath) => moveFsPath(sourcePath, destinationPath));
    ipcMain.handle('fs-delete-path', (_event, targetPath) => deleteFsPath(targetPath));
    ipcMain.handle('fs-mkdir', (_event, targetDirPath) => createFsDirectory(targetDirPath));
    ipcMain.handle('fs-open-path', (_event, targetPath) => shell.openPath(resolveFsPath(targetPath)));
    ipcMain.handle('fs-reveal-path', (_event, targetPath) => {
        shell.showItemInFolder(resolveFsPath(targetPath));
        return true;
    });
    ipcMain.handle('fs-read-text', (_event, targetPath) => {
        const resolved = resolveFsPath(targetPath);
        if (!fs.existsSync(resolved) || fs.statSync(resolved).isDirectory()) return null;
        return fs.readFileSync(resolved, 'utf8');
    });
    ipcMain.handle('fs-write-text', (_event, targetPath, content) => {
        const resolved = resolveFsPath(targetPath);
        fs.mkdirSync(path.dirname(resolved), { recursive: true });
        fs.writeFileSync(resolved, typeof content === 'string' ? content : '', 'utf8');
        return true;
    });
    ipcMain.handle('fs-path-info', (_event, targetPath) => getFsPathInfo(targetPath));
    ipcMain.handle('clipboard-read-text', () => clipboard.readText());
    ipcMain.handle('clipboard-write-text', (_event, text) => {
        clipboard.writeText(typeof text === 'string' ? text : '');
        return true;
    });
    ipcMain.handle('terminal-start', startTerminalBridge);
    ipcMain.handle('terminal-input', sendTerminalInput);
    ipcMain.handle('terminal-execute-managed', executeManagedCommand);
    ipcMain.handle('terminal-approval-decision', resolveManagedApproval);
    ipcMain.handle('terminal-search-history', searchManagedHistory);
    ipcMain.handle('terminal-generate-skill', generateManagedSkill);
    ipcMain.handle('terminal-find-symbol', findManagedSymbol);
    ipcMain.handle('terminal-list-snapshots', listSnapshots);
    ipcMain.handle('terminal-restore-snapshot', restoreSnapshot);
    ipcMain.handle('terminal-clone-session', cloneTerminalSession);
    ipcMain.handle('terminal-resize', resizeTerminalSession);
    ipcMain.handle('terminal-stop', (_event, paneId, killSession) => stopTerminalBridge(paneId, Boolean(killSession)));
    ipcMain.handle('window-minimize', () => mainWindow?.minimize());
    ipcMain.handle('window-maximize', () => {
        if (mainWindow?.isMaximized()) mainWindow.unmaximize();
        else mainWindow?.maximize();
    });
    ipcMain.handle('window-close', () => {
        stopAllTerminalBridges(true, true);
        app.quit();
        return true;
    });
    ipcMain.handle('window-isMaximized', () => mainWindow?.isMaximized() ?? false);
    ipcMain.handle('window-set-opacity', (_event, opacity) => setWindowOpacity(opacity));
    ipcMain.handle('vision-save-screenshot', saveVisionScreenshot);
    ipcMain.handle('discord-send-message', sendDiscordMessage);
    ipcMain.handle('discord-ensure-connected', ensureDiscordConnected);
    ipcMain.handle('slack-send-message', sendSlackMessage);
    ipcMain.handle('slack-ensure-connected', ensureSlackConnected);
    ipcMain.handle('telegram-send-message', sendTelegramMessage);
    ipcMain.handle('telegram-ensure-connected', ensureTelegramConnected);

    // WhatsApp bridge
    ipcMain.handle('whatsapp-connect', async () => {
        try {
            startWhatsAppBridge();
            await whatsappRpc('connect');
            return { ok: true };
        } catch (err) {
            return { ok: false, error: err.message };
        }
    });
    ipcMain.handle('whatsapp-disconnect', async () => {
        try {
            if (whatsappProcess) {
                await whatsappRpc('disconnect').catch(() => {});
            }
            stopWhatsAppBridge();
            return { ok: true };
        } catch (err) {
            return { ok: false, error: err.message };
        }
    });
    ipcMain.handle('whatsapp-status', async () => {
        try {
            if (!whatsappProcess) return { status: 'disconnected', phone: null };
            return await whatsappRpc('status');
        } catch {
            return { status: 'disconnected', phone: null };
        }
    });
    ipcMain.handle('whatsapp-send', async (_event, jid, text) => {
        return await whatsappRpc('send', { jid, text });
    });
}

configureChromiumRuntimePaths();

app.whenReady().then(async () => {
    logToFile('info', 'electron app ready');

    // Allow cross-origin API calls from the renderer (LLM providers, etc.)
    // Desktop apps don't need browser CORS restrictions.
    session.defaultSession.webRequest.onHeadersReceived((details, callback) => {
        callback({
            responseHeaders: {
                ...details.responseHeaders,
                'access-control-allow-origin': ['*'],
                'access-control-allow-headers': ['*'],
                'access-control-allow-methods': ['*'],
            },
        });
    });

    registerIpcHandlers();
    await spawnDaemon();
    createWindow();
    app.on('activate', () => {
        if (BrowserWindow.getAllWindows().length === 0) createWindow();
    });
});

app.on('before-quit', () => {
    logToFile('info', 'electron before-quit');
    stopAllTerminalBridges(true, true);
    stopWhatsAppBridge();
    cleanupDiscordClient();
    stopSlackBridge();
    stopTelegramBridge();
});

app.on('will-quit', () => {
    logToFile('info', 'electron will-quit');
    stopAllTerminalBridges(true, true);
    cleanupDiscordClient();
});

app.on('window-all-closed', () => {
    if (process.platform !== 'darwin') {
        stopAllTerminalBridges(true, true);
        app.quit();
        app.exit(0);
    }
});

process.on('exit', () => stopAllTerminalBridges(true, true));
