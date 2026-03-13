export type CodingAgentKind = "coding-cli" | "agent-runtime" | "gateway-runtime";

export type CodingAgentCapability =
    | "interactive"
    | "prompted-task"
    | "skills"
    | "memory"
    | "subagents"
    | "mcp"
    | "gateway"
    | "multichannel"
    | "browser"
    | "automation";

export interface CodingAgentLaunchMode {
    id: string;
    label: string;
    description: string;
    args?: string[];
    promptArgs?: string[];
    requiresPrompt?: boolean;
    promptPlaceholder?: string;
    recommended?: boolean;
}

export interface CodingAgentCheckResult {
    label: string;
    path: string;
    exists: boolean;
}

export type CodingAgentReadiness = "ready" | "needs-setup" | "missing";

export interface CodingAgentDefinition {
    id: string;
    label: string;
    description: string;
    kind: CodingAgentKind;
    executables: string[];
    versionArgs?: string[];
    launchArgs?: string[];
    installCommand?: string;
    setupCommand?: string;
    homepage?: string;
    configPaths?: string[];
    requirements?: string[];
    installHints?: string[];
    capabilities?: CodingAgentCapability[];
    launchModes?: CodingAgentLaunchMode[];
}

export interface DiscoveredCodingAgent extends CodingAgentDefinition {
    available: boolean;
    executable: string | null;
    path: string | null;
    version: string | null;
    readiness?: CodingAgentReadiness;
    checks?: CodingAgentCheckResult[];
    runtimeNotes?: string[];
    gatewayLabel?: string | null;
    gatewayReachable?: boolean | null;
    error?: string | null;
}

export type CodingAgentsDiscoveryStatus = "idle" | "loading" | "ready" | "error";