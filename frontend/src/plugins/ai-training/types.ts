export type AITrainingKind = "training-runtime" | "repository-workflow";

export type AITrainingCheckScope = "system" | "workspace";

export interface AITrainingLaunchMode {
    id: string;
    label: string;
    description: string;
    executable?: string;
    args?: string[];
    promptArgs?: string[];
    commandTemplate?: string;
    requiresPrompt?: boolean;
    promptPlaceholder?: string;
    requiresWorkspace?: boolean;
    recommended?: boolean;
}

export interface AITrainingCheckResult {
    label: string;
    path: string;
    exists: boolean;
    scope: AITrainingCheckScope;
}

export type AITrainingReadiness = "ready" | "needs-setup" | "missing";

export interface AITrainingDefinition {
    id: string;
    label: string;
    description: string;
    kind: AITrainingKind;
    executables: string[];
    versionArgs?: string[];
    installCommandTemplate?: string;
    installRequiresWorkspace?: boolean;
    setupCommandTemplate?: string;
    setupRequiresWorkspace?: boolean;
    homepage?: string;
    requirements?: string[];
    installHints?: string[];
    launchModes?: AITrainingLaunchMode[];
}

export interface DiscoveredAITraining extends AITrainingDefinition {
    available: boolean;
    executable: string | null;
    path: string | null;
    version: string | null;
    readiness: AITrainingReadiness;
    checks: AITrainingCheckResult[];
    runtimeNotes?: string[];
    workspacePath?: string | null;
    error?: string | null;
}

export type AITrainingDiscoveryStatus = "idle" | "loading" | "ready" | "error";