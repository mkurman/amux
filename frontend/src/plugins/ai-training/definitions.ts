import type { AITrainingDefinition, AITrainingLaunchMode, DiscoveredAITraining } from "./types";

const DEFAULT_LAUNCH_MODE: AITrainingLaunchMode = {
    id: "default",
    label: "Default Launch",
    description: "Run the default command for this training profile.",
    recommended: true,
};

export const KNOWN_AI_TRAINING_DEFINITIONS: AITrainingDefinition[] = [
    {
        id: "prime-verifiers",
        label: "Prime Intellect Verifiers",
        description: "Prime Intellect's environment, evaluation, and RL workflow toolkit.",
        kind: "training-runtime",
        executables: ["prime"],
        versionArgs: ["--version"],
        installCommandTemplate: "curl -LsSf https://astral.sh/uv/install.sh | sh && uv tool install prime",
        setupCommandTemplate: "prime lab setup",
        setupRequiresWorkspace: true,
        homepage: "https://github.com/PrimeIntellect-ai/verifiers",
        requirements: [
            "uv and prime CLI installed locally",
            "Prime Intellect authentication for platform-backed workflows",
            "A workspace initialized with prime lab setup for full lab features",
        ],
        installHints: [
            "Install uv first, then use uv tool install prime.",
            "Run prime lab setup inside the target workspace to create configs/, environments/, and starter files.",
        ],
        launchModes: [
            {
                id: "lab-setup",
                label: "Lab Setup",
                description: "Initialize the selected workspace as a Prime lab workspace.",
                executable: "prime",
                args: ["lab", "setup"],
                requiresWorkspace: true,
                recommended: true,
            },
            {
                id: "env-init",
                label: "Environment Init",
                description: "Create a new verifier environment template inside the selected workspace.",
                executable: "prime",
                args: ["env", "init"],
                promptArgs: ["{prompt}"],
                requiresPrompt: true,
                promptPlaceholder: "my-env",
                requiresWorkspace: true,
            },
            {
                id: "eval-run",
                label: "Evaluation Run",
                description: "Run a Prime evaluation for an environment or hub identifier.",
                executable: "prime",
                args: ["eval", "run"],
                promptArgs: ["{prompt}"],
                requiresPrompt: true,
                promptPlaceholder: "my-env -m gpt-5-nano",
                requiresWorkspace: true,
            },
            {
                id: "eval-tui",
                label: "Eval TUI",
                description: "Open Prime's local evaluation terminal UI.",
                executable: "prime",
                args: ["eval", "tui"],
                requiresWorkspace: true,
            },
        ],
    },
    {
        id: "autoresearch",
        label: "AutoResearch",
        description: "Karpathy's autonomous single-GPU research loop driven by program.md.",
        kind: "repository-workflow",
        executables: ["uv"],
        versionArgs: ["--version"],
        installCommandTemplate: "curl -LsSf https://astral.sh/uv/install.sh | sh",
        setupCommandTemplate: "uv sync && uv run prepare.py",
        setupRequiresWorkspace: true,
        homepage: "https://github.com/karpathy/autoresearch",
        requirements: [
            "Python 3.10+ and uv installed locally",
            "A compatible GPU for meaningful training runs",
            "A workspace checked out from the AutoResearch repository",
        ],
        installHints: [
            "Run uv sync once to install dependencies.",
            "Run uv run prepare.py before the first training loop to download data and build the tokenizer.",
        ],
        launchModes: [
            {
                id: "prepare",
                label: "Prepare Data",
                description: "Download data and build the tokenizer inside the selected AutoResearch workspace.",
                commandTemplate: "uv run prepare.py",
                requiresWorkspace: true,
            },
            {
                id: "train",
                label: "Train Once",
                description: "Run a single AutoResearch training experiment from the selected workspace.",
                commandTemplate: "uv run train.py",
                requiresWorkspace: true,
                recommended: true,
            },
            {
                id: "program-open",
                label: "Open Program Context",
                description: "Print a reminder to start the autonomous loop from program.md in the selected workspace.",
                commandTemplate: "printf '%s\n' 'Open program.md, verify setup, and then kick off a new experiment with your coding agent.'",
                requiresWorkspace: true,
            },
        ],
    },
    {
        id: "autorl",
        label: "AutoRL",
        description: "Autonomous RL environment search scaffold built around a tiny mutable surface and a fixed evaluator.",
        kind: "repository-workflow",
        executables: ["python3"],
        versionArgs: ["--version"],
        setupCommandTemplate: "python3 -m venv .venv && .venv/bin/pip install -e vendor/simverse",
        setupRequiresWorkspace: true,
        homepage: "https://github.com/harshbhatt7585/autoRL/tree/autorl/mar11",
        requirements: [
            "python3 installed locally",
            "A checked-out AutoRL workspace with vendor/simverse present",
            "A local .venv with vendor/simverse installed for evaluator runs",
        ],
        installHints: [
            "Create a virtualenv with python3 -m venv .venv.",
            "Install the vendored package with .venv/bin/pip install -e vendor/simverse before running train.py.",
        ],
        launchModes: [
            {
                id: "venv-setup",
                label: "Virtualenv Setup",
                description: "Create the local virtual environment and install vendor/simverse.",
                commandTemplate: "python3 -m venv .venv && .venv/bin/pip install -e vendor/simverse",
                requiresWorkspace: true,
            },
            {
                id: "evaluator",
                label: "Evaluator Run",
                description: "Run the fixed evaluator through the workspace virtual environment.",
                commandTemplate: ".venv/bin/python train.py",
                requiresWorkspace: true,
                recommended: true,
            },
            {
                id: "labeled-run",
                label: "Labeled Run",
                description: "Run the evaluator with a description label for the recorded result row.",
                commandTemplate: ".venv/bin/python train.py --description {prompt}",
                requiresPrompt: true,
                promptPlaceholder: "candidate baseline or experiment note",
                requiresWorkspace: true,
            },
        ],
    },
];

function quoteShellArg(value: string): string {
    if (!value) {
        return "''";
    }

    if (/^[A-Za-z0-9_./:=+\-]+$/.test(value)) {
        return value;
    }

    return `'${value.replace(/'/g, `'"'"'`)}'`;
}

function replaceTemplateTokens(template: string, prompt: string, workspacePath: string | null): string {
    return template
        .replace(/\{prompt\}/g, quoteShellArg(prompt))
        .replace(/\{workspace\}/g, workspacePath ? quoteShellArg(workspacePath) : "");
}

export function getAITrainingLaunchModes(profile: Pick<AITrainingDefinition, "launchModes">): AITrainingLaunchMode[] {
    return profile.launchModes?.length ? profile.launchModes : [DEFAULT_LAUNCH_MODE];
}

export function getAITrainingLaunchMode(
    profile: Pick<AITrainingDefinition, "launchModes">,
    modeId?: string | null,
): AITrainingLaunchMode {
    const modes = getAITrainingLaunchModes(profile);
    if (modeId) {
        const matched = modes.find((mode) => mode.id === modeId);
        if (matched) {
            return matched;
        }
    }

    return modes.find((mode) => mode.recommended) ?? modes[0];
}

export function buildAITrainingLaunchCommand(
    profile: Pick<DiscoveredAITraining, "executable" | "launchModes">,
    modeId?: string | null,
    prompt?: string | null,
    workspacePath?: string | null,
): string {
    const mode = getAITrainingLaunchMode(profile, modeId);
    const promptText = (prompt ?? "").trim() || mode.promptPlaceholder || "task";
    const cwdPrefix = mode.requiresWorkspace && workspacePath ? `cd ${quoteShellArg(workspacePath)} && ` : "";

    if (mode.commandTemplate) {
        return `${cwdPrefix}${replaceTemplateTokens(mode.commandTemplate, promptText, workspacePath ?? null)}`.trim();
    }

    const executable = mode.executable?.trim() || profile.executable?.trim();
    if (!executable) {
        return "";
    }

    const args = (mode.args ?? []).map((value) => value.trim()).filter(Boolean);
    const promptArgs = (mode.promptArgs ?? [])
        .map((value) => value === "{prompt}" ? promptText : value.trim())
        .filter(Boolean);

    return `${cwdPrefix}${[executable, ...args, ...promptArgs].map(quoteShellArg).join(" ")}`.trim();
}

export function buildAITrainingInstallCommand(
    profile: Pick<DiscoveredAITraining, "installCommandTemplate" | "installRequiresWorkspace" | "setupCommandTemplate" | "setupRequiresWorkspace" | "readiness">,
    workspacePath?: string | null,
): string {
    const useSetup = profile.readiness === "needs-setup";
    const template = useSetup
        ? profile.setupCommandTemplate?.trim() ?? profile.installCommandTemplate?.trim()
        : profile.installCommandTemplate?.trim() ?? profile.setupCommandTemplate?.trim();
    const requiresWorkspace = useSetup
        ? profile.setupRequiresWorkspace ?? false
        : profile.installRequiresWorkspace ?? false;

    if (!template) {
        return "";
    }

    if (requiresWorkspace && !workspacePath) {
        return "";
    }

    return replaceTemplateTokens(template, "", workspacePath ?? null).trim();
}

export function createUnavailableAITraining(error: string): DiscoveredAITraining[] {
    return KNOWN_AI_TRAINING_DEFINITIONS.map((profile) => ({
        ...profile,
        available: false,
        executable: profile.executables[0] ?? null,
        path: null,
        version: null,
        readiness: "missing",
        checks: [],
        runtimeNotes: [error],
        workspacePath: null,
        error,
    }));
}