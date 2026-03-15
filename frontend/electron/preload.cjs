const { contextBridge, ipcRenderer } = require('electron');

const loadedExternalPluginEntries = new Set();

async function injectInstalledPluginScript(entry) {
    const normalizedEntryPath = String(entry.sourceUrl || entry.entryPath || '');
    if (!normalizedEntryPath) {
        return {
            packageName: entry.packageName,
            pluginName: entry.pluginName,
            status: 'error',
            error: 'Plugin entry path is missing.',
        };
    }

    if (loadedExternalPluginEntries.has(normalizedEntryPath)) {
        return {
            packageName: entry.packageName,
            pluginName: entry.pluginName,
            status: 'already-loaded',
        };
    }

    if (typeof entry.source !== 'string' || !entry.source.trim()) {
        return {
            packageName: entry.packageName,
            pluginName: entry.pluginName,
            status: 'error',
            error: 'Plugin source payload is missing.',
        };
    }

    const script = document.createElement('script');
    script.type = 'text/javascript';
    script.dataset.amuxExternalPlugin = entry.packageName;
    script.textContent = `${entry.source}\n//# sourceURL=${normalizedEntryPath.replace(/\\/g, '/')}`;

    try {
        (document.head || document.documentElement).appendChild(script);
        loadedExternalPluginEntries.add(normalizedEntryPath);
        return {
            packageName: entry.packageName,
            pluginName: entry.pluginName,
            status: 'loaded',
        };
    } finally {
        script.remove();
    }
}

async function loadInstalledPlugins() {
    const installed = await ipcRenderer.invoke('plugin-load-installed');
    const results = [];

    for (const entry of installed) {
        try {
            if (entry?.status === 'error') {
                results.push({
                    packageName: entry.packageName,
                    pluginName: entry.pluginName,
                    status: 'error',
                    error: entry.error || 'Plugin load failed in the main process.',
                });
                continue;
            }

            if (entry.format !== 'script') {
                results.push({
                    packageName: entry.packageName,
                    pluginName: entry.pluginName,
                    status: 'skipped',
                    error: `Unsupported plugin format '${entry.format}'`,
                });
                continue;
            }

            if (!entry.source) {
                results.push({
                    packageName: entry.packageName,
                    pluginName: entry.pluginName,
                    status: 'error',
                    error: 'Plugin source payload is missing.',
                });
                continue;
            }

            results.push(await injectInstalledPluginScript(entry));
        } catch (error) {
            results.push({
                packageName: entry.packageName,
                pluginName: entry.pluginName,
                status: 'error',
                error: error?.message || String(error),
            });
        }
    }

    return results;
}

const bridgeApi = {
    getSocketPath: () => ipcRenderer.invoke('getSocketPath'),
    checkDaemon: () => ipcRenderer.invoke('checkDaemon'),
    spawnDaemon: () => ipcRenderer.invoke('spawnDaemon'),
    getSystemFonts: () => ipcRenderer.invoke('getSystemFonts'),
    getAvailableShells: () => ipcRenderer.invoke('getAvailableShells'),
    getSystemMonitorSnapshot: (options) => ipcRenderer.invoke('system-monitor-snapshot', options),
    getDaemonPath: () => ipcRenderer.invoke('getDaemonPath'),
    getPlatform: () => ipcRenderer.invoke('getPlatform'),
    discoverCodingAgents: () => ipcRenderer.invoke('coding-agents-discover'),
    discoverAITraining: (workspacePath) => ipcRenderer.invoke('ai-training-discover', workspacePath),
    listInstalledPlugins: () => ipcRenderer.invoke('plugin-list-installed'),
    loadInstalledPlugins: () => loadInstalledPlugins(),
    checkLspHealth: () => ipcRenderer.invoke('diagnostics-check-lsp'),
    checkMcpHealth: (servers) => ipcRenderer.invoke('diagnostics-check-mcp', servers),
    getDataDir: () => ipcRenderer.invoke('persistence-get-data-dir'),
    readJsonFile: (relativePath) => ipcRenderer.invoke('persistence-read-json', relativePath),
    writeJsonFile: (relativePath, data) => ipcRenderer.invoke('persistence-write-json', relativePath, data),
    readTextFile: (relativePath) => ipcRenderer.invoke('persistence-read-text', relativePath),
    writeTextFile: (relativePath, content) => ipcRenderer.invoke('persistence-write-text', relativePath, content),
    deleteDataPath: (relativePath) => ipcRenderer.invoke('persistence-delete-path', relativePath),
    listDataDir: (relativeDir) => ipcRenderer.invoke('persistence-list-dir', relativeDir),
    openDataPath: (relativePath) => ipcRenderer.invoke('persistence-open-path', relativePath),
    revealDataPath: (relativePath) => ipcRenderer.invoke('persistence-reveal-path', relativePath),
    listFsDir: (targetDir) => ipcRenderer.invoke('fs-list-dir', targetDir),
    copyFsPath: (sourcePath, destinationPath) => ipcRenderer.invoke('fs-copy-path', sourcePath, destinationPath),
    moveFsPath: (sourcePath, destinationPath) => ipcRenderer.invoke('fs-move-path', sourcePath, destinationPath),
    deleteFsPath: (targetPath) => ipcRenderer.invoke('fs-delete-path', targetPath),
    createFsDirectory: (targetDirPath) => ipcRenderer.invoke('fs-mkdir', targetDirPath),
    openFsPath: (targetPath) => ipcRenderer.invoke('fs-open-path', targetPath),
    revealFsPath: (targetPath) => ipcRenderer.invoke('fs-reveal-path', targetPath),
    readFsText: (targetPath) => ipcRenderer.invoke('fs-read-text', targetPath),
    writeFsText: (targetPath, content) => ipcRenderer.invoke('fs-write-text', targetPath, content),
    getFsPathInfo: (targetPath) => ipcRenderer.invoke('fs-path-info', targetPath),
    readClipboardText: () => ipcRenderer.invoke('clipboard-read-text'),
    writeClipboardText: (text) => ipcRenderer.invoke('clipboard-write-text', text),
    startTerminalSession: (options) => ipcRenderer.invoke('terminal-start', options),
    sendTerminalInput: (paneId, data) => ipcRenderer.invoke('terminal-input', paneId, data),
    executeManagedCommand: (paneId, payload) => ipcRenderer.invoke('terminal-execute-managed', paneId, payload),
    resolveManagedApproval: (paneId, approvalId, decision) => ipcRenderer.invoke('terminal-approval-decision', paneId, approvalId, decision),
    searchManagedHistory: (paneId, query, limit) => ipcRenderer.invoke('terminal-search-history', paneId, query, limit),
    generateManagedSkill: (paneId, query, title) => ipcRenderer.invoke('terminal-generate-skill', paneId, query, title),
    findManagedSymbol: (paneId, workspaceRoot, symbol, limit) => ipcRenderer.invoke('terminal-find-symbol', paneId, workspaceRoot, symbol, limit),
    listSnapshots: (paneId, workspaceId) => ipcRenderer.invoke('terminal-list-snapshots', paneId, workspaceId),
    restoreSnapshot: (paneId, snapshotId) => ipcRenderer.invoke('terminal-restore-snapshot', paneId, snapshotId),
    cloneTerminalSession: (payload) => ipcRenderer.invoke('terminal-clone-session', payload),
    resizeTerminalSession: (paneId, cols, rows) => ipcRenderer.invoke('terminal-resize', paneId, cols, rows),
    stopTerminalSession: (paneId, killSession) => ipcRenderer.invoke('terminal-stop', paneId, killSession),
    onTerminalEvent: (cb) => {
        const listener = (_event, payload) => cb(payload);
        ipcRenderer.on('terminal-event', listener);
        return () => ipcRenderer.removeListener('terminal-event', listener);
    },
    onAppCommand: (cb) => {
        const listener = (_event, command) => cb(command);
        ipcRenderer.on('app-command', listener);
        return () => ipcRenderer.removeListener('app-command', listener);
    },
    windowMinimize: () => ipcRenderer.invoke('window-minimize'),
    windowMaximize: () => ipcRenderer.invoke('window-maximize'),
    windowClose: () => ipcRenderer.invoke('window-close'),
    windowIsMaximized: () => ipcRenderer.invoke('window-isMaximized'),
    setWindowOpacity: (opacity) => ipcRenderer.invoke('window-set-opacity', opacity),
    saveVisionScreenshot: (payload) => ipcRenderer.invoke('vision-save-screenshot', payload),
    onWindowState: (cb) => {
        ipcRenderer.on('window-state', (_e, state) => cb(state));
        return () => ipcRenderer.removeAllListeners('window-state');
    },
    ensureDiscordConnected: (payload) => ipcRenderer.invoke('discord-ensure-connected', payload),
    sendDiscordMessage: (payload) => ipcRenderer.invoke('discord-send-message', payload),
    onDiscordMessage: (cb) => {
        const listener = (_event, msg) => cb(msg);
        ipcRenderer.on('discord-message', listener);
        return () => ipcRenderer.removeListener('discord-message', listener);
    },

    // Slack bridge
    ensureSlackConnected: (payload) => ipcRenderer.invoke('slack-ensure-connected', payload),
    sendSlackMessage: (payload) => ipcRenderer.invoke('slack-send-message', payload),
    onSlackMessage: (cb) => {
        const listener = (_event, msg) => cb(msg);
        ipcRenderer.on('slack-message', listener);
        return () => ipcRenderer.removeListener('slack-message', listener);
    },

    // Telegram bridge
    ensureTelegramConnected: (payload) => ipcRenderer.invoke('telegram-ensure-connected', payload),
    sendTelegramMessage: (payload) => ipcRenderer.invoke('telegram-send-message', payload),
    onTelegramMessage: (cb) => {
        const listener = (_event, msg) => cb(msg);
        ipcRenderer.on('telegram-message', listener);
        return () => ipcRenderer.removeListener('telegram-message', listener);
    },

    // WhatsApp bridge
    whatsappConnect: () => ipcRenderer.invoke('whatsapp-connect'),
    whatsappDisconnect: () => ipcRenderer.invoke('whatsapp-disconnect'),
    whatsappStatus: () => ipcRenderer.invoke('whatsapp-status'),
    whatsappSend: (jid, text) => ipcRenderer.invoke('whatsapp-send', jid, text),
    onWhatsAppQR: (cb) => {
        const listener = (_event, dataUrl) => cb(dataUrl);
        ipcRenderer.on('whatsapp-qr', listener);
        return () => ipcRenderer.removeListener('whatsapp-qr', listener);
    },
    onWhatsAppConnected: (cb) => {
        const listener = (_event, info) => cb(info);
        ipcRenderer.on('whatsapp-connected', listener);
        return () => ipcRenderer.removeListener('whatsapp-connected', listener);
    },
    onWhatsAppDisconnected: (cb) => {
        const listener = () => cb();
        ipcRenderer.on('whatsapp-disconnected', listener);
        return () => ipcRenderer.removeListener('whatsapp-disconnected', listener);
    },
    onWhatsAppError: (cb) => {
        const listener = (_event, msg) => cb(msg);
        ipcRenderer.on('whatsapp-error', listener);
        return () => ipcRenderer.removeListener('whatsapp-error', listener);
    },
    onWhatsAppMessage: (cb) => {
        const listener = (_event, msg) => cb(msg);
        ipcRenderer.on('whatsapp-message', listener);
        return () => ipcRenderer.removeListener('whatsapp-message', listener);
    },
};

contextBridge.exposeInMainWorld('tamux', bridgeApi);
contextBridge.exposeInMainWorld('amux', bridgeApi);
