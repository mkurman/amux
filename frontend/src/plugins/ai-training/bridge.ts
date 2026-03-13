import { sendCommandToPane } from "../coding-agents/bridge";
import { KNOWN_AI_TRAINING_DEFINITIONS, createUnavailableAITraining } from "./definitions";
import type { DiscoveredAITraining } from "./types";

type AITrainingBridge = {
    discoverAITraining?: (workspacePath?: string | null) => Promise<AmuxAITrainingDiscoveryResult[]>;
};

function getBridge(): AITrainingBridge | null {
    if (typeof window === "undefined") {
        return null;
    }

    return window.amux ?? null;
}

export async function discoverAITrainingProfiles(workspacePath?: string | null): Promise<DiscoveredAITraining[]> {
    const bridge = getBridge();
    if (!bridge?.discoverAITraining) {
        return createUnavailableAITraining("AI Training discovery is only available through the Electron bridge.");
    }

    try {
        const discovered = await bridge.discoverAITraining(workspacePath ?? null);
        const discoveredById = new Map(discovered.map((profile) => [profile.id, profile]));

        return KNOWN_AI_TRAINING_DEFINITIONS.map((definition) => {
            const match = discoveredById.get(definition.id);
            return {
                ...definition,
                available: match?.available ?? false,
                executable: match?.executable ?? definition.executables[0] ?? null,
                path: match?.path ?? null,
                version: match?.version ?? null,
                readiness: match?.readiness ?? (match?.available ? "ready" : "missing"),
                checks: match?.checks ?? [],
                runtimeNotes: match?.runtimeNotes ?? [],
                workspacePath: match?.workspacePath ?? workspacePath ?? null,
                error: match?.error ?? (match?.available ? null : "Required runtime not found."),
            } satisfies DiscoveredAITraining;
        });
    } catch (error) {
        const message = error instanceof Error ? error.message : "Failed to discover AI Training profiles.";
        return createUnavailableAITraining(message);
    }
}

export { sendCommandToPane };