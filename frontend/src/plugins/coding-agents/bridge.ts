import { KNOWN_CODING_AGENT_DEFINITIONS, createUnavailableCodingAgents } from "./agentDefinitions";
import type { DiscoveredCodingAgent } from "./types";

type CodingAgentsBridge = {
    discoverCodingAgents?: () => Promise<AmuxCodingAgentDiscoveryResult[]>;
    sendTerminalInput?: (paneId: string | null, data: string) => Promise<boolean>;
};

function getBridge(): CodingAgentsBridge | null {
    if (typeof window === "undefined") {
        return null;
    }

    return window.amux ?? null;
}

export function encodeTerminalInput(text: string): string {
    const bytes = new TextEncoder().encode(text);
    let binary = "";
    for (const byte of bytes) {
        binary += String.fromCharCode(byte);
    }
    return btoa(binary);
}

export async function discoverCodingAgents(): Promise<DiscoveredCodingAgent[]> {
    const bridge = getBridge();
    if (!bridge?.discoverCodingAgents) {
        return createUnavailableCodingAgents("Coding agent discovery is only available through the Electron bridge.");
    }

    try {
        const discovered = await bridge.discoverCodingAgents();
        const discoveredById = new Map(discovered.map((agent) => [agent.id, agent]));

        return KNOWN_CODING_AGENT_DEFINITIONS.map((definition) => {
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
                gatewayLabel: match?.gatewayLabel ?? null,
                gatewayReachable: match?.gatewayReachable ?? null,
                error: match?.error ?? (match?.available ? null : "Not found on PATH."),
            } satisfies DiscoveredCodingAgent;
        });
    } catch (error) {
        const message = error instanceof Error ? error.message : "Failed to discover coding agents.";
        return createUnavailableCodingAgents(message);
    }
}

export async function sendCommandToPane(paneId: string, command: string): Promise<boolean> {
    const bridge = getBridge();
    if (!bridge?.sendTerminalInput) {
        throw new Error("Terminal bridge unavailable. Launch from the Electron app.");
    }

    return bridge.sendTerminalInput(paneId, encodeTerminalInput(`${command}\r`));
}