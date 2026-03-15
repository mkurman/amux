import { useCallback } from "react";
import type { MutableRefObject } from "react";
import type { Terminal } from "@xterm/xterm";
import type { SerializeAddon } from "@xterm/addon-serialize";
import { stripAnsi } from "./utils";

export function useTerminalTranscript({
    termRef,
    serializeAddonRef,
    addTranscript,
    upsertLiveTranscript,
    paneId,
    paneWorkspaceId,
    paneSurfaceId,
    paneWorkspaceCwd,
}: {
    termRef: MutableRefObject<Terminal | null>;
    serializeAddonRef: MutableRefObject<SerializeAddon | null>;
    addTranscript: (entry: {
        content: string;
        reason: "pane-close" | "terminal-clear" | "manual";
        workspaceId: string | undefined;
        surfaceId: string | undefined;
        paneId: string;
        cwd: string | null;
    }) => void;
    upsertLiveTranscript: (entry: {
        content: string;
        workspaceId: string | undefined;
        surfaceId: string | undefined;
        paneId: string;
        cwd: string | null;
    }) => void;
    paneId: string;
    paneWorkspaceId: string | undefined;
    paneSurfaceId: string | undefined;
    paneWorkspaceCwd: string | undefined;
}) {
    const captureTranscript = useCallback((reason: "pane-close" | "terminal-clear" | "manual") => {
        const term = termRef.current;
        const serializeAddon = serializeAddonRef.current;
        if (!term || !serializeAddon) return;

        const content = stripAnsi(serializeAddon.serialize()).trim();
        if (!content) return;

        addTranscript({
            content,
            reason,
            workspaceId: paneWorkspaceId,
            surfaceId: paneSurfaceId,
            paneId,
            cwd: paneWorkspaceCwd ?? null,
        });
    }, [addTranscript, paneId, paneSurfaceId, paneWorkspaceCwd, paneWorkspaceId, serializeAddonRef, termRef]);

    const captureRollingTranscript = useCallback(() => {
        const term = termRef.current;
        const serializeAddon = serializeAddonRef.current;
        if (!term || !serializeAddon) return;

        // Limit serialization to the last 500 rows to avoid blocking the main
        // thread on large scrollback buffers. The full serialize + stripAnsi
        // path was a major source of input lag.
        const content = stripAnsi(serializeAddon.serialize({ scrollback: 500 })).trim();
        if (!content) return;

        upsertLiveTranscript({
            content,
            workspaceId: paneWorkspaceId,
            surfaceId: paneSurfaceId,
            paneId,
            cwd: paneWorkspaceCwd ?? null,
        });
    }, [paneId, paneSurfaceId, paneWorkspaceCwd, paneWorkspaceId, serializeAddonRef, termRef, upsertLiveTranscript]);

    return {
        captureTranscript,
        captureRollingTranscript,
    };
}