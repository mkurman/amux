import { create } from "zustand";
import { CommandLogEntry, WorkspaceId, SurfaceId, PaneId } from "./types";
import { DEFAULT_SETTINGS } from "./types";
import {
  readPersistedJson,
  scheduleJsonWrite,
} from "./persistence";
import { useSettingsStore } from "./settingsStore";

let _logId = 0;
const COMMAND_LOG_FILE = "command-log.json";

function syncLogId(entries: CommandLogEntry[]) {
  let maxId = 0;
  for (const entry of entries) {
    const match = /^cmd_(\d+)$/.exec(entry.id);
    if (match) {
      maxId = Math.max(maxId, Number(match[1]));
    }
  }
  _logId = Math.max(_logId, maxId);
}

function normalizeEntries(entries: unknown): CommandLogEntry[] {
  if (!Array.isArray(entries)) return [];
  return entries
    .filter((entry): entry is CommandLogEntry => {
      if (!entry || typeof entry !== "object") return false;
      const candidate = entry as Partial<CommandLogEntry>;
      return typeof candidate.id === "string"
        && typeof candidate.command === "string"
        && typeof candidate.timestamp === "number";
    })
    .map((entry) => ({
      id: entry.id,
      command: entry.command,
      timestamp: entry.timestamp,
      path: (entry as Partial<CommandLogEntry>).path ?? null,
      cwd: entry.cwd ?? null,
      workspaceId: entry.workspaceId ?? null,
      surfaceId: entry.surfaceId ?? null,
      paneId: entry.paneId ?? null,
      exitCode: entry.exitCode ?? null,
      durationMs: entry.durationMs ?? null,
    }));
}

function mergeEntries(primary: CommandLogEntry[], secondary: CommandLogEntry[]): CommandLogEntry[] {
  const byId = new Map<string, CommandLogEntry>();
  for (const entry of [...primary, ...secondary]) {
    const existing = byId.get(entry.id);
    if (!existing || entry.timestamp >= existing.timestamp) {
      byId.set(entry.id, entry);
    }
  }

  return [...byId.values()].sort((a, b) => b.timestamp - a.timestamp);
}

function readRetentionDays(): number {
  const retentionDays = useSettingsStore.getState().settings.commandLogRetentionDays;
  return Number.isFinite(retentionDays)
    ? retentionDays
    : DEFAULT_SETTINGS.commandLogRetentionDays;
}

function loadEntries(): CommandLogEntry[] {
  return [];
}

function persistEntries(entries: CommandLogEntry[]) {
  scheduleJsonWrite(COMMAND_LOG_FILE, entries, 200);
}

function pruneEntries(entries: CommandLogEntry[]): CommandLogEntry[] {
  const retentionDays = readRetentionDays();

  if (retentionDays <= 0) {
    return entries;
  }

  const cutoff = Date.now() - retentionDays * 24 * 60 * 60 * 1000;
  return entries.filter((entry) => entry.timestamp >= cutoff);
}

export interface CommandLogState {
  entries: CommandLogEntry[];

  addEntry: (opts: {
    command: string;
    path?: string | null;
    cwd?: string | null;
    workspaceId?: WorkspaceId | null;
    surfaceId?: SurfaceId | null;
    paneId?: PaneId | null;
    exitCode?: number | null;
    durationMs?: number | null;
  }) => void;
  completeLatestPendingEntry: (opts: {
    paneId?: PaneId | null;
    exitCode?: number | null;
    finishedAt?: number;
  }) => void;

  getByWorkspace: (workspaceId: WorkspaceId) => CommandLogEntry[];
  getByPane: (paneId: PaneId) => CommandLogEntry[];
  search: (query: string) => CommandLogEntry[];
  removeEntry: (id: string) => void;
  clearAll: () => void;
  getRecentCommands: (limit: number) => string[];
  getRecentEntries: (limit: number) => CommandLogEntry[];
  hydrateEntries: (entries: CommandLogEntry[]) => void;
}

const initialEntries = pruneEntries(loadEntries());

export const useCommandLogStore = create<CommandLogState>((set, get) => ({
  entries: initialEntries,

  addEntry: (opts) => {
    const entry: CommandLogEntry = {
      id: `cmd_${++_logId}`,
      command: opts.command,
      timestamp: Date.now(),
      path: opts.path ?? null,
      cwd: opts.cwd ?? null,
      workspaceId: opts.workspaceId ?? null,
      surfaceId: opts.surfaceId ?? null,
      paneId: opts.paneId ?? null,
      exitCode: opts.exitCode ?? null,
      durationMs: opts.durationMs ?? null,
    };
    set((s) => {
      const entries = pruneEntries([entry, ...s.entries]);
      persistEntries(entries);
      return { entries };
    });
  },

  completeLatestPendingEntry: (opts) => {
    if (!opts.paneId) return;

    set((state) => {
      const index = state.entries.findIndex((entry) =>
        entry.paneId === opts.paneId
        && entry.exitCode === null
        && entry.durationMs === null,
      );

      if (index === -1) {
        return state;
      }

      const finishedAt = Number.isFinite(opts.finishedAt)
        ? Number(opts.finishedAt)
        : Date.now();
      const target = state.entries[index];
      const durationMs = Math.max(0, Math.round(finishedAt - target.timestamp));

      const entries = [...state.entries];
      entries[index] = {
        ...target,
        exitCode: opts.exitCode ?? null,
        durationMs,
      };

      const pruned = pruneEntries(entries);
      persistEntries(pruned);
      return { entries: pruned };
    });
  },

  getByWorkspace: (workspaceId) =>
    get().entries.filter((e) => e.workspaceId === workspaceId),

  getByPane: (paneId) =>
    get().entries.filter((e) => e.paneId === paneId),

  search: (query) => {
    const lower = query.toLowerCase();
    return get().entries.filter(
      (e) =>
        e.command.toLowerCase().includes(lower) ||
        (e.path && e.path.toLowerCase().includes(lower)) ||
        (e.cwd && e.cwd.toLowerCase().includes(lower))
    );
  },

  removeEntry: (id) => {
    set((state) => {
      const entries = state.entries.filter((entry) => entry.id !== id);
      persistEntries(entries);
      return { entries };
    });
  },

  clearAll: () => {
    persistEntries([]);
    set({ entries: [] });
  },

  getRecentCommands: (limit) => {
    const seen = new Set<string>();
    const result: string[] = [];
    for (const entry of get().entries) {
      if (!seen.has(entry.command)) {
        seen.add(entry.command);
        result.push(entry.command);
        if (result.length >= limit) break;
      }
    }
    return result;
  },

  getRecentEntries: (limit) => get().entries.slice(0, limit),

  hydrateEntries: (entries) => {
    const pruned = pruneEntries(normalizeEntries(entries));
    syncLogId(pruned);
    persistEntries(pruned);
    set({ entries: pruned });
  },
}));

export async function hydrateCommandLogStore(): Promise<void> {
  const diskEntries = normalizeEntries(await readPersistedJson<CommandLogEntry[]>(COMMAND_LOG_FILE));
  const merged = pruneEntries(mergeEntries(diskEntries, []));
  useCommandLogStore.getState().hydrateEntries(merged);

  if (merged.length === 0) {
    if (initialEntries.length > 0) {
      scheduleJsonWrite(COMMAND_LOG_FILE, initialEntries, 0);
    }
    return;
  }
}
