import type { OperationalEvent } from "./agentMissionStore";
import { encodeTextToBase64, stripAnsi } from "../components/terminal-pane/utils";

function cleanCommand(value: string): string {
  return stripAnsi(value).replace(/[\u0000-\u001f\u007f]/g, " ").trim();
}

export function resolveDuplicateBootstrapCommand(
  paneId: string,
  events: OperationalEvent[],
): string | null {
  const lastCommand = events
    .filter((event) => event.paneId === paneId && event.kind === "command-started" && typeof event.command === "string")
    .sort((a, b) => b.timestamp - a.timestamp)[0]?.command;

  if (!lastCommand) return null;
  return cleanCommand(lastCommand) || null;
}

export function resolveDuplicateActiveBootstrapCommand(
  paneId: string,
  events: OperationalEvent[],
): string | null {
  let activeCommand: string | null = null;
  const timeline = events
    .filter((event) => event.paneId === paneId)
    .sort((a, b) => a.timestamp - b.timestamp);

  for (const event of timeline) {
    if (event.kind === "command-started" && typeof event.command === "string" && event.command.trim()) {
      activeCommand = event.command.trim();
      continue;
    }
    if (event.kind === "command-finished" || event.kind === "session-exited") {
      activeCommand = null;
    }
  }

  if (!activeCommand) {
    return null;
  }

  return cleanCommand(activeCommand) || null;
}

export function resolveDuplicateSourceSessionId(
  paneId: string,
  sessionId: string | null | undefined,
  events: OperationalEvent[],
): string | null {
  if (typeof sessionId === "string" && sessionId) {
    return unwrapCloneSessionId(sessionId);
  }

  const latest = events
    .filter((event) => event.paneId === paneId && typeof event.sessionId === "string" && event.sessionId)
    .sort((a, b) => b.timestamp - a.timestamp)[0];

  return typeof latest?.sessionId === "string" && latest.sessionId
    ? unwrapCloneSessionId(latest.sessionId)
    : null;
}

const CLONE_SESSION_PREFIX = "clone:";

export function createCloneSessionToken(sourceSessionId: string): string {
  const normalized = sourceSessionId.trim();
  return `${CLONE_SESSION_PREFIX}${normalized}`;
}

export function isCloneSessionToken(sessionId: string | null | undefined): boolean {
  return typeof sessionId === "string" && sessionId.startsWith(CLONE_SESSION_PREFIX);
}

export function parseCloneSessionToken(sessionId: string | null | undefined): string | null {
  if (!isCloneSessionToken(sessionId)) return null;
  const value = String(sessionId).slice(CLONE_SESSION_PREFIX.length).trim();
  return value || null;
}

export function unwrapCloneSessionId(sessionId: string | null | undefined): string | null {
  if (typeof sessionId !== "string") return null;
  let value = sessionId.trim();
  if (!value) return null;

  for (let depth = 0; depth < 4; depth += 1) {
    const parsed = parseCloneSessionToken(value);
    if (!parsed) {
      return value;
    }
    value = parsed;
  }

  return value || null;
}

export function queuePaneBootstrapCommand(paneId: string, command: string): void {
  const payload = encodeTextToBase64(`${command}\r`);
  let attempts = 10;

  const sendAttempt = async () => {
    const bridge = (window as any).tamux ?? (window as any).amux;
    if (!bridge?.sendTerminalInput) {
      return;
    }

    let accepted = false;
    try {
      accepted = Boolean(await bridge.sendTerminalInput(paneId, payload));
    } catch {
      accepted = false;
    }

    if (accepted || attempts <= 0) {
      return;
    }

    attempts -= 1;
    window.setTimeout(() => {
      void sendAttempt();
    }, 140);
  };

  window.setTimeout(() => {
    void sendAttempt();
  }, 120);
}

export async function cloneSessionForDuplication(
  sourcePaneId: string,
  sourceSessionId?: string | null,
  opts?: {
    workspaceId?: string | null;
    cwd?: string | null;
    cols?: number;
    rows?: number;
  },
): Promise<{ sessionId: string; activeCommand: string | null } | null> {
  const bridge = (window as any).tamux ?? (window as any).amux;
  if (!bridge?.cloneTerminalSession) {
    return null;
  }

  try {
    const normalizedSourceSessionId = unwrapCloneSessionId(sourceSessionId);
    if (!normalizedSourceSessionId) {
      return null;
    }
    const rawCols = opts?.cols;
    const rawRows = opts?.rows;
    const cols = typeof rawCols === "number" && Number.isFinite(rawCols)
      ? Math.max(2, Math.trunc(rawCols))
      : undefined;
    const rows = typeof rawRows === "number" && Number.isFinite(rawRows)
      ? Math.max(2, Math.trunc(rawRows))
      : undefined;
    const result = await bridge.cloneTerminalSession({
      sourcePaneId,
      sourceSessionId: normalizedSourceSessionId,
      ...(opts?.workspaceId ? { workspaceId: opts.workspaceId } : {}),
      ...(opts?.cwd ? { cwd: opts.cwd } : {}),
      ...(typeof cols === "number" ? { cols } : {}),
      ...(typeof rows === "number" ? { rows } : {}),
    });
    const sessionId = typeof result?.sessionId === "string" ? result.sessionId.trim() : "";
    if (!sessionId) return null;
    const activeCommand = typeof result?.activeCommand === "string" ? result.activeCommand : null;
    return { sessionId, activeCommand };
  } catch (error) {
    console.warn("cloneSessionForDuplication failed", {
      sourcePaneId,
      sourceSessionId: unwrapCloneSessionId(sourceSessionId),
      workspaceId: opts?.workspaceId ?? null,
      cols: opts?.cols ?? null,
      rows: opts?.rows ?? null,
      error: error instanceof Error ? error.message : String(error),
    });
    return null;
  }
}
