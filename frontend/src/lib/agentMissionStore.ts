import { create } from "zustand";
import { useAgentStore } from "./agentStore";
import { useSettingsStore } from "./settingsStore";
import type { AmuxSettings } from "./types";
import { readPersistedJson, readPersistedText, scheduleJsonWrite, scheduleTextWrite } from "./persistence";
import { useSnippetStore } from "./snippetStore";

export type RiskLevel = "medium" | "high" | "critical";

export interface OperationalEvent {
    id: string;
    timestamp: number;
    paneId: string;
    workspaceId: string | null;
    surfaceId: string | null;
    sessionId: string | null;
    kind:
    | "session-ready"
    | "command-started"
    | "command-finished"
    | "session-exited"
    | "error"
    | "approval-requested"
    | "approval-approved"
    | "approval-denied"
    | "tool-call";
    command: string | null;
    message: string | null;
    exitCode: number | null;
    durationMs: number | null;
    riskLevel: RiskLevel | null;
    blastRadius: string | null;
}

export interface CognitiveEvent {
    id: string;
    timestamp: number;
    paneId: string;
    workspaceId: string | null;
    surfaceId: string | null;
    sessionId: string | null;
    source: "inner-monologue" | "scratchpad";
    content: string;
}

export interface ContextSnapshot {
    id: string;
    timestamp: number;
    paneId: string;
    workspaceId: string | null;
    surfaceId: string | null;
    sessionId: string | null;
    activeProvider: string;
    model: string;
    threadCount: number;
    snippetCount: number;
    tokenBudget: number;
    systemMemoryChars: number;
    userMemoryChars: number;
}

export interface ApprovalRequest {
    id: string;
    createdAt: number;
    paneId: string;
    workspaceId: string | null;
    surfaceId: string | null;
    sessionId: string | null;
    command: string;
    reasons: string[];
    riskLevel: RiskLevel;
    blastRadius: string;
    status: "pending" | "approved-once" | "approved-session" | "denied";
    handledAt: number | null;
}

export interface SnapshotRecord {
    snapshotId: string;
    workspaceId: string | null;
    sessionId: string | null;
    command: string | null;
    kind: string;
    label: string;
    path: string;
    createdAt: number;
    status: string;
    details: string;
}

export interface HistoryRecallHit {
    id: string;
    kind: string;
    title: string;
    excerpt: string;
    path: string | null;
    timestamp: number;
    score: number;
}

export interface SymbolRecallHit {
    path: string;
    line: number;
    kind: string;
    snippet: string;
}

type MissionMemory = {
    frozenSnapshot: string;
    userProfile: string;
};

type PersistedMissionState = {
    operationalEvents?: OperationalEvent[];
    cognitiveEvents?: CognitiveEvent[];
    contextSnapshots?: ContextSnapshot[];
    approvals?: ApprovalRequest[];
    sessionAllowlist?: Record<string, string[]>;
};

const MISSION_DIR = "agent-mission";
const OPERATIONAL_FILE = `${MISSION_DIR}/operational.json`;
const COGNITIVE_FILE = `${MISSION_DIR}/cognitive.json`;
const CONTEXT_FILE = `${MISSION_DIR}/context.json`;
const APPROVAL_FILE = `${MISSION_DIR}/approvals.json`;
const ALLOWLIST_FILE = `${MISSION_DIR}/session-allowlist.json`;
const MEMORY_FILE = `${MISSION_DIR}/MEMORY.md`;
const USER_FILE = `${MISSION_DIR}/USER.md`;
const MAX_OPERATIONAL_EVENTS = 400;
const MAX_COGNITIVE_EVENTS = 200;
const MAX_CONTEXT_SNAPSHOTS = 120;
const MAX_APPROVALS = 120;
const MEMORY_MAX_CHARS = 2200;
const USER_MAX_CHARS = 1375;

let operationalId = 0;
let cognitiveId = 0;
let contextId = 0;
let approvalId = 0;

function limitItems<T>(items: T[], maxItems: number): T[] {
    return items.slice(0, maxItems);
}

function nextId(prefix: string, counter: "operational" | "cognitive" | "context" | "approval") {
    if (counter === "operational") return `${prefix}_${++operationalId}`;
    if (counter === "cognitive") return `${prefix}_${++cognitiveId}`;
    if (counter === "context") return `${prefix}_${++contextId}`;
    return `${prefix}_${++approvalId}`;
}

function trimBoundedText(text: string, maxChars: number): string {
    return text.slice(0, maxChars).trimEnd();
}

function defaultFrozenSnapshot(): string {
    return trimBoundedText(
        [
            "Environment facts:",
            "- tamux uses a daemon-first PTY backend with persistent sessions.",
            "- The frontend exposes agent traces, approvals, and execution graphs.",
            "- Snippets act as portable procedural skills for repeated workflows.",
            "- Risky shell commands require explicit approval before Enter is sent.",
        ].join("\n"),
        MEMORY_MAX_CHARS,
    );
}

function defaultUserProfile(): string {
    return trimBoundedText(
        [
            "Operator profile:",
            "- Prefer concise, high-signal operational summaries.",
            "- Show traces, blast radius, and next action before risky execution.",
        ].join("\n"),
        USER_MAX_CHARS,
    );
}

function stripAnsi(text: string): string {
    return text
        .replace(/\u001b\[[0-?]*[ -/]*[@-~]/g, "")
        .replace(/\u001b\][^\u0007]*(?:\u0007|\u001b\\)/g, "");
}

function extractCognitiveSegments(text: string): Array<{ source: "inner-monologue" | "scratchpad"; content: string }> {
    // Fast path: skip expensive regex work when tags can't possibly be present
    if (!text.includes("<INNER_MONOLOGUE>") && !text.includes("<SCRATCHPAD>")) {
        return [];
    }

    const cleaned = stripAnsi(text);
    const matches: Array<{ source: "inner-monologue" | "scratchpad"; content: string }> = [];
    const patterns: Array<{ source: "inner-monologue" | "scratchpad"; regex: RegExp }> = [
        { source: "inner-monologue", regex: /<INNER_MONOLOGUE>([\s\S]*?)<\/INNER_MONOLOGUE>/gi },
        { source: "scratchpad", regex: /<SCRATCHPAD>([\s\S]*?)<\/SCRATCHPAD>/gi },
    ];

    for (const pattern of patterns) {
        for (const match of cleaned.matchAll(pattern.regex)) {
            const content = match[1]?.trim();
            if (content) {
                matches.push({ source: pattern.source, content });
            }
        }
    }

    return matches;
}

function syncCounters(state: PersistedMissionState): void {
    for (const event of state.operationalEvents ?? []) {
        const match = /^op_(\d+)$/.exec(event.id);
        if (match) operationalId = Math.max(operationalId, Number(match[1]));
    }
    for (const event of state.cognitiveEvents ?? []) {
        const match = /^cog_(\d+)$/.exec(event.id);
        if (match) cognitiveId = Math.max(cognitiveId, Number(match[1]));
    }
    for (const snapshot of state.contextSnapshots ?? []) {
        const match = /^ctx_(\d+)$/.exec(snapshot.id);
        if (match) contextId = Math.max(contextId, Number(match[1]));
    }
    for (const approval of state.approvals ?? []) {
        const match = /^apr_(\d+)$/.exec(approval.id);
        if (match) approvalId = Math.max(approvalId, Number(match[1]));
    }
}

function persistMissionState(state: PersistedMissionState): void {
    const payload: PersistedMissionState = {
        operationalEvents: limitItems(state.operationalEvents ?? [], MAX_OPERATIONAL_EVENTS),
        cognitiveEvents: limitItems(state.cognitiveEvents ?? [], MAX_COGNITIVE_EVENTS),
        contextSnapshots: limitItems(state.contextSnapshots ?? [], MAX_CONTEXT_SNAPSHOTS),
        approvals: limitItems(state.approvals ?? [], MAX_APPROVALS),
        sessionAllowlist: state.sessionAllowlist ?? {},
    };
    scheduleJsonWrite(OPERATIONAL_FILE, payload.operationalEvents, 200);
    scheduleJsonWrite(COGNITIVE_FILE, payload.cognitiveEvents, 200);
    scheduleJsonWrite(CONTEXT_FILE, payload.contextSnapshots, 200);
    scheduleJsonWrite(APPROVAL_FILE, payload.approvals, 200);
    scheduleJsonWrite(ALLOWLIST_FILE, payload.sessionAllowlist, 200);
}

function buildContextSnapshot(opts: {
    paneId: string;
    workspaceId?: string | null;
    surfaceId?: string | null;
    sessionId?: string | null;
    frozenSnapshot: string;
    userProfile: string;
}): ContextSnapshot {
    const { agentSettings, threads } = useAgentStore.getState();
    const snippets = useSnippetStore.getState().snippets;
    const activeProviderConfig = agentSettings[agentSettings.activeProvider] as { model: string };

    return {
        id: nextId("ctx", "context"),
        timestamp: Date.now(),
        paneId: opts.paneId,
        workspaceId: opts.workspaceId ?? null,
        surfaceId: opts.surfaceId ?? null,
        sessionId: opts.sessionId ?? null,
        activeProvider: agentSettings.activeProvider,
        model: activeProviderConfig?.model ?? "",
        threadCount: threads.length,
        snippetCount: snippets.length,
        tokenBudget: agentSettings.contextBudgetTokens,
        systemMemoryChars: opts.frozenSnapshot.length,
        userMemoryChars: opts.userProfile.length,
    };
}

export type RiskAssessment = {
    requiresApproval: boolean;
    riskLevel: RiskLevel;
    reasons: string[];
    blastRadius: string;
};

function shouldRequireApproval(
    securityLevel: AmuxSettings["securityLevel"],
    riskLevel: RiskLevel,
    reasons: string[],
): boolean {
    if (securityLevel === "yolo") return false;
    if (securityLevel === "highest") return true;
    if (securityLevel === "lowest") return riskLevel === "critical";
    return reasons.length > 0;
}

export function assessCommandRisk(
    command: string,
    securityLevel?: AmuxSettings["securityLevel"],
): RiskAssessment {
    const normalized = command.trim().toLowerCase();
    const effectiveSecurityLevel = securityLevel ?? useSettingsStore.getState().settings.securityLevel ?? "moderate";
    if (!normalized) {
        return {
            requiresApproval: false,
            riskLevel: "medium",
            reasons: [],
            blastRadius: "none",
        };
    }

    const reasons: string[] = [];
    let riskLevel: RiskLevel = "medium";
    let blastRadius = "local pane";

    const checks: Array<{ test: RegExp; level: RiskLevel; reason: string; radius: string }> = [
        { test: /(^|\s)rm\s+-rf\s+(\/|~|\.\.?)(\s|$)/, level: "critical", reason: "destructive recursive delete", radius: "filesystem-wide" },
        { test: /(^|\s)(mkfs|fdisk|parted|dd)\b/, level: "critical", reason: "disk or block-device mutation", radius: "disk-level" },
        { test: /(^|\s)(shutdown|reboot|halt|poweroff)\b/, level: "critical", reason: "host power-state change", radius: "host-wide" },
        { test: /(^|\s)git\s+push\b.*(--force|-f)(\s|$)/, level: "high", reason: "force push rewrites remote history", radius: "remote repository" },
        { test: /(^|\s)git\s+reset\s+--hard\b/, level: "high", reason: "hard reset discards local changes", radius: "workspace" },
        { test: /(^|\s)(chmod|chown)\b.*-r/, level: "high", reason: "recursive permission or ownership change", radius: "workspace or subtree" },
        { test: /curl\b[^|\n]*\|\s*(sh|bash|zsh)\b/, level: "high", reason: "executes remote script directly", radius: "remote code execution" },
        { test: /(^|\s)(docker\s+system\s+prune|kubectl\s+delete|terraform\s+destroy)\b/, level: "high", reason: "infrastructure-destructive operation", radius: "container or cluster resources" },
        { test: /(^|\s)(systemctl|service)\s+(stop|restart|disable)\b/, level: "high", reason: "service lifecycle mutation", radius: "host services" },
        { test: /(^|\s)npm\s+publish\b|(^|\s)cargo\s+publish\b/, level: "high", reason: "publishes external artifact", radius: "package registry" },
        { test: /(^|\s)(remove-item|ri)\b[^\n]*\b(-recurse|-r)\b/, level: "high", reason: "recursive file deletion on Windows", radius: "workspace or subtree" },
        { test: /(^|\s)(rd|rmdir)\s+[^\n]*\s+\/s\b/, level: "high", reason: "recursive directory delete via cmd.exe", radius: "workspace or subtree" },
        { test: /(^|\s)(del|erase)\s+[^\n]*\s+\/s\b/, level: "high", reason: "recursive file delete via cmd.exe", radius: "workspace or subtree" },
        { test: /(invoke-webrequest|iwr)\b[^|\n]*\|\s*(iex|invoke-expression)\b/, level: "high", reason: "downloads and executes remote PowerShell content", radius: "remote code execution" },
        { test: /(^|\s)(stop-service|restart-service|set-service)\b/, level: "high", reason: "mutates Windows service lifecycle", radius: "host services" },
        { test: /(^|\s)(format|diskpart)\b/, level: "critical", reason: "disk or volume mutation on Windows", radius: "disk-level" },
    ];

    for (const check of checks) {
        if (!check.test.test(normalized)) continue;
        reasons.push(check.reason);
        blastRadius = check.radius;
        if (check.level === "critical" || riskLevel === "medium") {
            riskLevel = check.level;
        }
    }

    if (effectiveSecurityLevel === "highest" && reasons.length === 0) {
        reasons.push("strict policy requires approval for every managed command");
    }

    return {
        requiresApproval: shouldRequireApproval(effectiveSecurityLevel, riskLevel, reasons),
        riskLevel,
        reasons,
        blastRadius,
    };
}

const initialState: PersistedMissionState = {
    operationalEvents: [],
    cognitiveEvents: [],
    contextSnapshots: [],
    approvals: [],
    sessionAllowlist: {},
};
syncCounters(initialState);

export interface AgentMissionState {
    operationalEvents: OperationalEvent[];
    cognitiveEvents: CognitiveEvent[];
    contextSnapshots: ContextSnapshot[];
    approvals: ApprovalRequest[];
    sessionAllowlist: Record<string, string[]>;
    memory: MissionMemory;
    sharedCursorMode: "idle" | "human" | "agent" | "approval";
    historySummary: string;
    historyHits: HistoryRecallHit[];
    symbolHits: SymbolRecallHit[];
    snapshots: SnapshotRecord[];

    updateMemory: (kind: keyof MissionMemory, text: string) => void;
    setSharedCursorMode: (mode: AgentMissionState["sharedCursorMode"]) => void;
    setHistoryResults: (summary: string, hits: HistoryRecallHit[]) => void;
    setSymbolHits: (hits: SymbolRecallHit[]) => void;
    setSnapshots: (hits: SnapshotRecord[]) => void;
    upsertDaemonApproval: (opts: { id: string; paneId: string; workspaceId?: string | null; surfaceId?: string | null; sessionId?: string | null; command: string; reasons: string[]; riskLevel: RiskLevel; blastRadius: string }) => void;
    recordSessionReady: (opts: { paneId: string; workspaceId?: string | null; surfaceId?: string | null; sessionId?: string | null }) => void;
    recordCommandStarted: (opts: { paneId: string; workspaceId?: string | null; surfaceId?: string | null; sessionId?: string | null; command: string }) => void;
    recordCommandFinished: (opts: { paneId: string; workspaceId?: string | null; surfaceId?: string | null; sessionId?: string | null; command?: string | null; exitCode?: number | null; durationMs?: number | null }) => void;
    recordSessionExited: (opts: { paneId: string; workspaceId?: string | null; surfaceId?: string | null; sessionId?: string | null; exitCode?: number | null }) => void;
    recordError: (opts: { paneId: string; workspaceId?: string | null; surfaceId?: string | null; sessionId?: string | null; message: string }) => void;
    recordCognitiveOutput: (opts: { paneId: string; workspaceId?: string | null; surfaceId?: string | null; sessionId?: string | null; text: string }) => void;
    requestApproval: (opts: { paneId: string; workspaceId?: string | null; surfaceId?: string | null; sessionId?: string | null; command: string; reasons: string[]; riskLevel: RiskLevel; blastRadius: string }) => string;
    resolveApproval: (id: string, status: "approved-once" | "approved-session" | "denied") => void;
    markApprovalHandled: (id: string) => void;
    isCommandAllowed: (sessionKey: string, command: string) => boolean;
    recordToolCall: (opts: { toolName: string; arguments: string }) => void;
    hydrate: (payload: PersistedMissionState & { memory?: Partial<MissionMemory> }) => void;
}

export const useAgentMissionStore = create<AgentMissionState>((set, get) => ({
    operationalEvents: initialState.operationalEvents ?? [],
    cognitiveEvents: initialState.cognitiveEvents ?? [],
    contextSnapshots: initialState.contextSnapshots ?? [],
    approvals: initialState.approvals ?? [],
    sessionAllowlist: initialState.sessionAllowlist ?? {},
    memory: {
        frozenSnapshot: defaultFrozenSnapshot(),
        userProfile: defaultUserProfile(),
    },
    sharedCursorMode: "idle",
    historySummary: "",
    historyHits: [],
    symbolHits: [],
    snapshots: [],

    setSharedCursorMode: (mode) => set({ sharedCursorMode: mode }),

    setHistoryResults: (summary, hits) => set({ historySummary: summary, historyHits: hits }),

    setSymbolHits: (hits) => set({ symbolHits: hits }),

    setSnapshots: (hits) => set({
        snapshots: hits.slice().sort((a, b) => b.createdAt - a.createdAt),
    }),

    updateMemory: (kind, text) => {
        set((state) => {
            const bounded = kind === "frozenSnapshot"
                ? trimBoundedText(text, MEMORY_MAX_CHARS)
                : trimBoundedText(text, USER_MAX_CHARS);
            const memory = { ...state.memory, [kind]: bounded };

            scheduleTextWrite(kind === "frozenSnapshot" ? MEMORY_FILE : USER_FILE, bounded, 200);
            persistMissionState({
                operationalEvents: state.operationalEvents,
                cognitiveEvents: state.cognitiveEvents,
                contextSnapshots: state.contextSnapshots,
                approvals: state.approvals,
                sessionAllowlist: state.sessionAllowlist,
            });

            return { memory };
        });
    },

    recordSessionReady: (opts) => {
        set((state) => {
            const event: OperationalEvent = {
                id: nextId("op", "operational"),
                timestamp: Date.now(),
                paneId: opts.paneId,
                workspaceId: opts.workspaceId ?? null,
                surfaceId: opts.surfaceId ?? null,
                sessionId: opts.sessionId ?? null,
                kind: "session-ready",
                command: null,
                message: "session attached",
                exitCode: null,
                durationMs: null,
                riskLevel: null,
                blastRadius: null,
            };

            const operationalEvents = limitItems([event, ...state.operationalEvents], MAX_OPERATIONAL_EVENTS);
            persistMissionState({ ...state, operationalEvents });
            return { operationalEvents };
        });
    },

    recordCommandStarted: (opts) => {
        set((state) => {
            const event: OperationalEvent = {
                id: nextId("op", "operational"),
                timestamp: Date.now(),
                paneId: opts.paneId,
                workspaceId: opts.workspaceId ?? null,
                surfaceId: opts.surfaceId ?? null,
                sessionId: opts.sessionId ?? null,
                kind: "command-started",
                command: opts.command,
                message: null,
                exitCode: null,
                durationMs: null,
                riskLevel: null,
                blastRadius: null,
            };
            const snapshot = buildContextSnapshot({
                paneId: opts.paneId,
                workspaceId: opts.workspaceId,
                surfaceId: opts.surfaceId,
                sessionId: opts.sessionId,
                frozenSnapshot: state.memory.frozenSnapshot,
                userProfile: state.memory.userProfile,
            });

            const operationalEvents = limitItems([event, ...state.operationalEvents], MAX_OPERATIONAL_EVENTS);
            const contextSnapshots = limitItems([snapshot, ...state.contextSnapshots], MAX_CONTEXT_SNAPSHOTS);
            persistMissionState({ ...state, operationalEvents, contextSnapshots });
            return { operationalEvents, contextSnapshots };
        });
    },

    recordCommandFinished: (opts) => {
        set((state) => {
            const event: OperationalEvent = {
                id: nextId("op", "operational"),
                timestamp: Date.now(),
                paneId: opts.paneId,
                workspaceId: opts.workspaceId ?? null,
                surfaceId: opts.surfaceId ?? null,
                sessionId: opts.sessionId ?? null,
                kind: "command-finished",
                command: opts.command ?? null,
                message: null,
                exitCode: opts.exitCode ?? null,
                durationMs: opts.durationMs ?? null,
                riskLevel: null,
                blastRadius: null,
            };

            const operationalEvents = limitItems([event, ...state.operationalEvents], MAX_OPERATIONAL_EVENTS);
            persistMissionState({ ...state, operationalEvents });
            return { operationalEvents };
        });
    },

    recordSessionExited: (opts) => {
        set((state) => {
            const event: OperationalEvent = {
                id: nextId("op", "operational"),
                timestamp: Date.now(),
                paneId: opts.paneId,
                workspaceId: opts.workspaceId ?? null,
                surfaceId: opts.surfaceId ?? null,
                sessionId: opts.sessionId ?? null,
                kind: "session-exited",
                command: null,
                message: null,
                exitCode: opts.exitCode ?? null,
                durationMs: null,
                riskLevel: null,
                blastRadius: null,
            };

            const operationalEvents = limitItems([event, ...state.operationalEvents], MAX_OPERATIONAL_EVENTS);
            persistMissionState({ ...state, operationalEvents });
            return { operationalEvents };
        });
    },

    recordError: (opts) => {
        set((state) => {
            const event: OperationalEvent = {
                id: nextId("op", "operational"),
                timestamp: Date.now(),
                paneId: opts.paneId,
                workspaceId: opts.workspaceId ?? null,
                surfaceId: opts.surfaceId ?? null,
                sessionId: opts.sessionId ?? null,
                kind: "error",
                command: null,
                message: opts.message,
                exitCode: null,
                durationMs: null,
                riskLevel: null,
                blastRadius: null,
            };

            const operationalEvents = limitItems([event, ...state.operationalEvents], MAX_OPERATIONAL_EVENTS);
            persistMissionState({ ...state, operationalEvents });
            return { operationalEvents };
        });
    },

    recordCognitiveOutput: (opts) => {
        const segments = extractCognitiveSegments(opts.text);
        if (segments.length === 0) return;

        set((state) => {
            const now = Date.now();
            const events = segments.map((segment, index) => ({
                id: `cog_${cognitiveId + index + 1}`,
                timestamp: now,
                paneId: opts.paneId,
                workspaceId: opts.workspaceId ?? null,
                surfaceId: opts.surfaceId ?? null,
                sessionId: opts.sessionId ?? null,
                source: segment.source,
                content: segment.content,
            } satisfies CognitiveEvent));
            cognitiveId += events.length;

            const cognitiveEvents = limitItems([...events.reverse(), ...state.cognitiveEvents], MAX_COGNITIVE_EVENTS);
            persistMissionState({ ...state, cognitiveEvents });
            return { cognitiveEvents };
        });
    },

    requestApproval: (opts) => {
        const id = nextId("apr", "approval");
        get().upsertDaemonApproval({ ...opts, id });

        return id;
    },

    upsertDaemonApproval: (opts) => {
        set((state) => {
            const approval: ApprovalRequest = {
                id: opts.id,
                createdAt: Date.now(),
                paneId: opts.paneId,
                workspaceId: opts.workspaceId ?? null,
                surfaceId: opts.surfaceId ?? null,
                sessionId: opts.sessionId ?? null,
                command: opts.command,
                reasons: opts.reasons,
                riskLevel: opts.riskLevel,
                blastRadius: opts.blastRadius,
                status: "pending",
                handledAt: null,
            };
            const event: OperationalEvent = {
                id: nextId("op", "operational"),
                timestamp: approval.createdAt,
                paneId: opts.paneId,
                workspaceId: opts.workspaceId ?? null,
                surfaceId: opts.surfaceId ?? null,
                sessionId: opts.sessionId ?? null,
                kind: "approval-requested",
                command: opts.command,
                message: opts.reasons.join(", "),
                exitCode: null,
                durationMs: null,
                riskLevel: opts.riskLevel,
                blastRadius: opts.blastRadius,
            };

            const approvals = limitItems([
                approval,
                ...state.approvals.filter((entry) => entry.id !== opts.id),
            ], MAX_APPROVALS);
            const operationalEvents = limitItems([event, ...state.operationalEvents], MAX_OPERATIONAL_EVENTS);
            persistMissionState({ ...state, approvals, operationalEvents });
            return { approvals, operationalEvents };
        });
    },

    resolveApproval: (id, status) => {
        set((state) => {
            const approval = state.approvals.find((entry) => entry.id === id);
            if (!approval) return state;

            const approvals = state.approvals.map((entry) => {
                if (entry.id !== id) return entry;
                return { ...entry, status };
            });

            const sessionAllowlist = { ...state.sessionAllowlist };
            if (status === "approved-session") {
                const key = approval.sessionId ?? approval.paneId;
                const allowed = new Set(sessionAllowlist[key] ?? []);
                allowed.add(approval.command);
                sessionAllowlist[key] = [...allowed];
            }

            const event: OperationalEvent = {
                id: nextId("op", "operational"),
                timestamp: Date.now(),
                paneId: approval.paneId,
                workspaceId: approval.workspaceId,
                surfaceId: approval.surfaceId,
                sessionId: approval.sessionId,
                kind: status === "denied" ? "approval-denied" : "approval-approved",
                command: approval.command,
                message: approval.reasons.join(", "),
                exitCode: null,
                durationMs: null,
                riskLevel: approval.riskLevel,
                blastRadius: approval.blastRadius,
            };

            const operationalEvents = limitItems([event, ...state.operationalEvents], MAX_OPERATIONAL_EVENTS);
            persistMissionState({ ...state, approvals, operationalEvents, sessionAllowlist });
            return { approvals, operationalEvents, sessionAllowlist };
        });
    },

    markApprovalHandled: (id) => {
        set((state) => {
            const approvals = state.approvals.map((entry) => {
                if (entry.id !== id || entry.handledAt) return entry;
                return { ...entry, handledAt: Date.now() };
            });
            persistMissionState({ ...state, approvals });
            return { approvals };
        });
    },

    isCommandAllowed: (sessionKey, command) => {
        return (get().sessionAllowlist[sessionKey] ?? []).includes(command);
    },

    recordToolCall: (opts) => {
        set((state) => {
            const event: OperationalEvent = {
                id: nextId("op", "operational"),
                timestamp: Date.now(),
                paneId: "agent",
                workspaceId: null,
                surfaceId: null,
                sessionId: null,
                kind: "tool-call",
                command: opts.toolName,
                message: opts.arguments.slice(0, 500),
                exitCode: null,
                durationMs: null,
                riskLevel: null,
                blastRadius: null,
            };
            const operationalEvents = limitItems([event, ...state.operationalEvents], MAX_OPERATIONAL_EVENTS);
            persistMissionState({ ...state, operationalEvents });
            return { operationalEvents };
        });
    },

    hydrate: (payload) => {
        syncCounters(payload);
        set(() => ({
            operationalEvents: Array.isArray(payload.operationalEvents) ? payload.operationalEvents : [],
            cognitiveEvents: Array.isArray(payload.cognitiveEvents) ? payload.cognitiveEvents : [],
            contextSnapshots: Array.isArray(payload.contextSnapshots) ? payload.contextSnapshots : [],
            approvals: Array.isArray(payload.approvals) ? payload.approvals : [],
            sessionAllowlist: payload.sessionAllowlist ?? {},
            memory: {
                frozenSnapshot: trimBoundedText(payload.memory?.frozenSnapshot ?? defaultFrozenSnapshot(), MEMORY_MAX_CHARS),
                userProfile: trimBoundedText(payload.memory?.userProfile ?? defaultUserProfile(), USER_MAX_CHARS),
            },
        }));
    },
}));

export async function hydrateAgentMissionStore(): Promise<void> {
    const [operationalEvents, cognitiveEvents, contextSnapshots, approvals, sessionAllowlist, frozenSnapshot, userProfile] = await Promise.all([
        readPersistedJson<OperationalEvent[]>(OPERATIONAL_FILE),
        readPersistedJson<CognitiveEvent[]>(COGNITIVE_FILE),
        readPersistedJson<ContextSnapshot[]>(CONTEXT_FILE),
        readPersistedJson<ApprovalRequest[]>(APPROVAL_FILE),
        readPersistedJson<Record<string, string[]>>(ALLOWLIST_FILE),
        readPersistedText(MEMORY_FILE),
        readPersistedText(USER_FILE),
    ]);

    useAgentMissionStore.getState().hydrate({
        operationalEvents: operationalEvents ?? [],
        cognitiveEvents: cognitiveEvents ?? [],
        contextSnapshots: contextSnapshots ?? [],
        approvals: approvals ?? [],
        sessionAllowlist: sessionAllowlist ?? {},
        memory: {
            frozenSnapshot: frozenSnapshot ?? defaultFrozenSnapshot(),
            userProfile: userProfile ?? defaultUserProfile(),
        },
    });

    const state = useAgentMissionStore.getState();
    scheduleTextWrite(MEMORY_FILE, state.memory.frozenSnapshot, 0);
    scheduleTextWrite(USER_FILE, state.memory.userProfile, 0);
    persistMissionState({
        operationalEvents: state.operationalEvents,
        cognitiveEvents: state.cognitiveEvents,
        contextSnapshots: state.contextSnapshots,
        approvals: state.approvals,
        sessionAllowlist: state.sessionAllowlist,
    });
}