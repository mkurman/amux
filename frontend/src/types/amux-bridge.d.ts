declare global {
    type AmuxCodingAgentCheck = {
        label: string;
        path: string;
        exists: boolean;
    };

    type AmuxCodingAgentDiscoveryResult = {
        id: string;
        available: boolean;
        executable: string | null;
        path: string | null;
        version: string | null;
        error?: string | null;
        readiness?: "ready" | "needs-setup" | "missing";
        checks?: AmuxCodingAgentCheck[];
        runtimeNotes?: string[];
        gatewayLabel?: string | null;
        gatewayReachable?: boolean | null;
    };

    type AmuxAITrainingCheck = {
        label: string;
        path: string;
        exists: boolean;
        scope: "system" | "workspace";
    };

    type AmuxAITrainingDiscoveryResult = {
        id: string;
        available: boolean;
        executable: string | null;
        path: string | null;
        version: string | null;
        readiness: "ready" | "needs-setup" | "missing";
        checks: AmuxAITrainingCheck[];
        runtimeNotes?: string[];
        workspacePath?: string | null;
        error?: string | null;
    };

    type AmuxInstalledPluginRecord = {
        packageName: string;
        packageVersion: string;
        pluginName: string;
        entryPath: string;
        format: string;
        installedAt: number;
    };

    type AmuxInstalledPluginLoadResult = {
        packageName: string;
        pluginName: string;
        status: "loaded" | "already-loaded" | "skipped" | "error";
        error?: string;
    };

    type AmuxBridge = {
        discoverCodingAgents?: () => Promise<AmuxCodingAgentDiscoveryResult[]>;
        discoverAITraining?: (workspacePath?: string | null) => Promise<AmuxAITrainingDiscoveryResult[]>;
        getDataDir?: () => Promise<string>;
        readJsonFile?: (relativePath: string) => Promise<unknown>;
        writeJsonFile?: (relativePath: string, data: unknown) => Promise<boolean>;
        readTextFile?: (relativePath: string) => Promise<string | null>;
        writeTextFile?: (relativePath: string, content: string) => Promise<boolean>;
        deleteDataPath?: (relativePath: string) => Promise<boolean>;
        listDataDir?: (relativeDir?: string) => Promise<Array<{ name: string; path: string; isDirectory: boolean }>>;
        openDataPath?: (relativePath: string) => Promise<string>;
        revealDataPath?: (relativePath: string) => Promise<boolean>;
        sendTerminalInput?: (paneId: string | null, data: string) => Promise<boolean>;
        cloneTerminalSession?: (payload: {
            sourcePaneId?: string;
            sourceSessionId?: string | null;
            workspaceId?: string | null;
            cols?: number;
            rows?: number;
        }) => Promise<{ sessionId: string }>;
        listInstalledPlugins?: () => Promise<AmuxInstalledPluginRecord[]>;
        loadInstalledPlugins?: () => Promise<AmuxInstalledPluginLoadResult[]>;
    };

    interface Window {
        tamux?: AmuxBridge;
        amux?: AmuxBridge;
    }
}

export { };
