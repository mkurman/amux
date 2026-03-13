import type {
    CodingAgentCapability,
    CodingAgentDefinition,
    CodingAgentLaunchMode,
    DiscoveredCodingAgent,
} from "./types";

const DEFAULT_INTERACTIVE_MODE: CodingAgentLaunchMode = {
    id: "interactive",
    label: "Interactive CLI",
    description: "Start the agent's interactive terminal experience in the selected pane.",
    recommended: true,
};

export const CODING_AGENT_CAPABILITY_LABELS: Record<CodingAgentCapability, string> = {
    interactive: "Interactive",
    "prompted-task": "Prompted Task",
    skills: "Skills",
    memory: "Memory",
    subagents: "Subagents",
    mcp: "MCP",
    gateway: "Gateway",
    multichannel: "Multi-Channel",
    browser: "Browser",
    automation: "Automation",
};

export const KNOWN_CODING_AGENT_DEFINITIONS: CodingAgentDefinition[] = [
    {
        id: "claude",
        label: "Claude Code",
        description: "Anthropic's terminal coding agent.",
        kind: "coding-cli",
        executables: ["claude"],
        versionArgs: ["--version"],
        capabilities: ["interactive", "automation"],
    },
    {
        id: "codex",
        label: "Codex CLI",
        description: "OpenAI Codex terminal workflow.",
        kind: "coding-cli",
        executables: ["codex"],
        versionArgs: ["--version"],
        installCommand: "npm install -g @openai/codex",
        capabilities: ["interactive", "automation"],
    },
    {
        id: "gemini",
        label: "Gemini CLI",
        description: "Google Gemini terminal agent.",
        kind: "coding-cli",
        executables: ["gemini"],
        versionArgs: ["--version"],
        installCommand: "npm install -g @google/gemini-cli",
        capabilities: ["interactive", "automation"],
    },
    {
        id: "hermes",
        label: "Hermes Agent",
        description: "Nous Research's self-improving agent runtime with skills, memory, subagents, and MCP support.",
        kind: "agent-runtime",
        executables: ["hermes"],
        versionArgs: ["--version"],
        installCommand: "python3 -m pip install \"hermes-agent[all]\"",
        setupCommand: "hermes setup",
        homepage: "https://github.com/NousResearch/hermes-agent",
        configPaths: ["~/.hermes/config.yaml", "~/.hermes/.env"],
        requirements: [
            "Python 3.11+ runtime",
            "Install with pip install hermes-agent or hermes-agent[all]",
            "WSL2 is recommended on Windows",
        ],
        installHints: [
            "Use hermes setup to initialize providers and toolsets.",
            "Point Hermes MCP settings at amux-mcp to unlock deeper amux-aware workflows.",
        ],
        capabilities: ["interactive", "prompted-task", "skills", "memory", "subagents", "mcp", "gateway", "browser", "automation"],
        launchModes: [
            {
                id: "interactive",
                label: "Interactive CLI",
                description: "Open Hermes' terminal UI in the selected pane.",
                recommended: true,
            },
            {
                id: "one-shot",
                label: "One-shot Task",
                description: "Run a single task through Hermes with an inline prompt.",
                requiresPrompt: true,
                promptPlaceholder: "Analyze this repository, propose a fix, or execute a coding task.",
                promptArgs: ["-q", "{prompt}"],
            },
            {
                id: "gateway",
                label: "Gateway Runner",
                description: "Start Hermes gateway mode for chat-platform and long-running automation workflows.",
                args: ["gateway"],
            },
        ],
    },
    {
        id: "pi",
        label: "pi.dev",
        description: "Pi's minimal but highly extensible terminal coding harness.",
        kind: "agent-runtime",
        executables: ["pi"],
        versionArgs: ["--version"],
        installCommand: "npm install -g @mariozechner/pi-coding-agent",
        homepage: "https://github.com/badlogic/pi-mono/tree/main/packages/coding-agent",
        configPaths: ["~/.pi/agent/settings.json", "~/.pi/agent/sessions", "~/.pi/agent/AGENTS.md"],
        requirements: [
            "Node.js runtime for the published npm package",
            "Install globally with npm install -g @mariozechner/pi-coding-agent",
            "Authenticate with a supported provider or subscription before interactive use",
        ],
        installHints: [
            "Use pi /login or provider API keys before launching provider-backed tasks.",
            "Pi packages, skills, prompts, and extensions live under ~/.pi/agent or project-local .pi directories.",
        ],
        capabilities: ["interactive", "prompted-task", "skills", "memory", "automation"],
        launchModes: [
            {
                id: "interactive",
                label: "Interactive CLI",
                description: "Open pi's interactive terminal experience in the selected pane.",
                recommended: true,
            },
            {
                id: "print",
                label: "One-shot Print",
                description: "Run a single prompt and print the result before exiting.",
                requiresPrompt: true,
                promptPlaceholder: "Review this repository, propose a patch, or summarize a file.",
                promptArgs: ["-p", "{prompt}"],
            },
            {
                id: "rpc",
                label: "RPC Mode",
                description: "Start pi in JSONL RPC mode for process integration experiments.",
                args: ["--mode", "rpc"],
            },
        ],
    },
    {
        id: "opencode",
        label: "OpenCode",
        description: "OpenCode terminal coding assistant.",
        kind: "coding-cli",
        executables: ["opencode"],
        versionArgs: ["--version"],
        capabilities: ["interactive", "automation"],
    },
    {
        id: "openclaw",
        label: "OpenClaw",
        description: "OpenClaw's self-hosted agent and gateway runtime for direct tasks and multi-channel orchestration.",
        kind: "gateway-runtime",
        executables: ["openclaw"],
        versionArgs: ["--version"],
        installCommand: "npm install -g openclaw",
        setupCommand: "openclaw onboard",
        homepage: "https://github.com/openclaw/openclaw",
        configPaths: ["~/.openclaw/openclaw.json", "~/.openclaw/workspace", "~/.openclaw/state"],
        requirements: [
            "Node.js 22.16+ or 24.x runtime",
            "Configured provider and channel credentials for full gateway mode",
        ],
        installHints: [
            "Use openclaw onboard or the project installer before relying on gateway mode.",
            "Direct agent mode is the fastest path if you only want one-off agent execution inside amux.",
        ],
        capabilities: ["prompted-task", "gateway", "multichannel", "browser", "memory", "automation"],
        launchModes: [
            {
                id: "direct-agent",
                label: "Direct Agent",
                description: "Launch a one-off OpenClaw agent task in the selected pane.",
                requiresPrompt: true,
                promptPlaceholder: "Summarize a spec, draft a reply, or route a task through OpenClaw.",
                promptArgs: ["agent", "--message", "{prompt}"],
                recommended: true,
            },
            {
                id: "gateway",
                label: "Gateway Runtime",
                description: "Start the long-running OpenClaw gateway for multi-channel agent workflows.",
                args: ["gateway"],
            },
        ],
    },
    {
        id: "kimi",
        label: "Kimi CLI",
        description: "Moonshot Kimi coding assistant.",
        kind: "coding-cli",
        executables: ["kimi"],
        versionArgs: ["--version"],
        capabilities: ["interactive", "automation"],
    },
    {
        id: "aider",
        label: "Aider",
        description: "Aider pair-programming CLI.",
        kind: "coding-cli",
        executables: ["aider"],
        versionArgs: ["--version"],
        capabilities: ["interactive", "automation"],
    },
    {
        id: "goose",
        label: "Goose",
        description: "Goose local coding agent.",
        kind: "coding-cli",
        executables: ["goose"],
        versionArgs: ["--version"],
        capabilities: ["interactive", "automation"],
    },
    {
        id: "qwen-coder",
        label: "Qwen Coder",
        description: "Qwen coding CLI if installed locally.",
        kind: "coding-cli",
        executables: ["qwen", "qwen-coder"],
        versionArgs: ["--version"],
        capabilities: ["interactive", "automation"],
    },
];

function quoteShellArg(value: string): string {
    if (!value) {
        return "''";
    }

    if (/^[A-Za-z0-9_./:=+-]+$/.test(value)) {
        return value;
    }

    return `'${value.replace(/'/g, `'"'"'`)}'`;
}

export function getCodingAgentLaunchModes(agent: Pick<CodingAgentDefinition, "launchModes">): CodingAgentLaunchMode[] {
    return agent.launchModes?.length ? agent.launchModes : [DEFAULT_INTERACTIVE_MODE];
}

export function getCodingAgentLaunchMode(
    agent: Pick<CodingAgentDefinition, "launchModes">,
    modeId?: string | null,
): CodingAgentLaunchMode {
    const modes = getCodingAgentLaunchModes(agent);
    if (modeId) {
        const matched = modes.find((mode) => mode.id === modeId);
        if (matched) {
            return matched;
        }
    }

    return modes.find((mode) => mode.recommended) ?? modes[0];
}

export function buildCodingAgentLaunchCommand(
    agent: Pick<DiscoveredCodingAgent, "executable" | "launchArgs" | "launchModes">,
    modeId?: string | null,
    prompt?: string | null,
): string {
    const executable = agent.executable?.trim();
    if (!executable) {
        return "";
    }

    const mode = getCodingAgentLaunchMode(agent, modeId);
    const promptText = (prompt ?? "").trim() || mode.promptPlaceholder || "your task here";
    const baseArgs = (agent.launchArgs ?? []).map((value) => value.trim()).filter(Boolean);
    const modeArgs = (mode.args ?? []).map((value) => value.trim()).filter(Boolean);
    const promptArgs = (mode.promptArgs ?? [])
        .map((value) => value === "{prompt}" ? promptText : value.trim())
        .filter(Boolean);

    return [executable, ...baseArgs, ...modeArgs, ...promptArgs].map(quoteShellArg).join(" ");
}

export function buildCodingAgentInstallCommand(
    agent: Pick<DiscoveredCodingAgent, "installCommand" | "setupCommand" | "readiness">,
): string {
    if (agent.readiness === "needs-setup") {
        return agent.setupCommand?.trim() ?? agent.installCommand?.trim() ?? "";
    }

    return agent.installCommand?.trim() ?? agent.setupCommand?.trim() ?? "";
}

export function createUnavailableCodingAgents(error: string): DiscoveredCodingAgent[] {
    return KNOWN_CODING_AGENT_DEFINITIONS.map((agent) => ({
        ...agent,
        available: false,
        executable: agent.executables[0] ?? null,
        path: null,
        version: null,
        error,
    }));
}