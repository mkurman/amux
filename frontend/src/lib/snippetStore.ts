import { create } from "zustand";
import { readPersistedJson, scheduleJsonWrite } from "./persistence";

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------
export interface Snippet {
  id: string;
  name: string;
  content: string;
  owner: "user" | "assistant";
  category: string;
  tags: string[];
  description: string;
  createdAt: number;
  updatedAt: number;
  useCount: number;
  isFavorite: boolean;
}

export interface SnippetState {
  snippets: Snippet[];
  addSnippet: (opts: Partial<Snippet> & { name: string; content: string }) => void;
  updateSnippet: (id: string, updates: Partial<Snippet>) => void;
  deleteSnippet: (id: string) => void;
  incrementUseCount: (id: string) => void;
  toggleFavorite: (id: string) => void;
  search: (query: string) => Snippet[];
  getCategories: () => string[];
}

// ---------------------------------------------------------------------------
// Seed defaults (matching amux-windows)
// ---------------------------------------------------------------------------
const SEED_SNIPPETS: Array<Omit<Snippet, "owner">> = [
  {
    id: "seed_1", name: "Git Status", content: "git status -sb",
    category: "Git", tags: ["git"], description: "Short branch status",
    createdAt: 0, updatedAt: 0, useCount: 0, isFavorite: false,
  },
  {
    id: "seed_2", name: "Git Log Pretty", content: "git log --oneline --graph --decorate -20",
    category: "Git", tags: ["git", "log"], description: "Pretty git log",
    createdAt: 0, updatedAt: 0, useCount: 0, isFavorite: false,
  },
  {
    id: "seed_3", name: "Git Diff Staged", content: "git diff --staged",
    category: "Git", tags: ["git", "diff"], description: "Show staged changes",
    createdAt: 0, updatedAt: 0, useCount: 0, isFavorite: false,
  },
  {
    id: "seed_4", name: "Docker PS", content: "docker ps --format 'table {{.Names}}\\t{{.Status}}\\t{{.Ports}}'",
    category: "Docker", tags: ["docker"], description: "List running containers",
    createdAt: 0, updatedAt: 0, useCount: 0, isFavorite: false,
  },
  {
    id: "seed_5", name: "Find Large Files", content: "find . -type f -size +10M -exec ls -lh {} + | sort -k5 -h -r | head -20",
    category: "System", tags: ["files", "disk"], description: "Find files > 10MB",
    createdAt: 0, updatedAt: 0, useCount: 0, isFavorite: false,
  },
  {
    id: "seed_6", name: "Kill Port", content: "lsof -ti:{{port}} | xargs kill -9",
    category: "Network", tags: ["port", "kill"], description: "Kill process on a port",
    createdAt: 0, updatedAt: 0, useCount: 0, isFavorite: false,
  },
  {
    id: "seed_7", name: "SSH Connect", content: "ssh {{user}}@{{host}}",
    category: "Network", tags: ["ssh"], description: "SSH to a remote host",
    createdAt: 0, updatedAt: 0, useCount: 0, isFavorite: false,
  },
  {
    id: "seed_8", name: "Disk Usage", content: "du -sh * | sort -rh | head -20",
    category: "System", tags: ["disk"], description: "Top 20 directory sizes",
    createdAt: 0, updatedAt: 0, useCount: 0, isFavorite: false,
  },
  {
    id: "seed_9", name: "Process by Name", content: "ps aux | grep {{name}}",
    category: "System", tags: ["process"], description: "Search processes by name",
    createdAt: 0, updatedAt: 0, useCount: 0, isFavorite: false,
  },
  {
    id: "seed_10", name: "Watch Directory", content: "watch -n 1 'ls -la {{path}}'",
    category: "System", tags: ["watch", "files"], description: "Watch directory changes",
    createdAt: 0, updatedAt: 0, useCount: 0, isFavorite: false,
  },
];

const SNIPPETS_FILE = "snippets.json";

function syncSnippetCounter(snippets: Snippet[]): void {
  let maxId = 100;
  for (const snippet of snippets) {
    const match = /^snip_(\d+)$/.exec(snippet.id);
    if (match) {
      maxId = Math.max(maxId, Number(match[1]));
    }
  }
  _id = maxId;
}

function loadSnippets(): Snippet[] {
  const seeded = SEED_SNIPPETS.map((snippet) => ({ ...snippet, owner: "user" as const }));
  syncSnippetCounter(seeded);
  return seeded;
}

function saveSnippets(snippets: Snippet[]) {
  scheduleJsonWrite(SNIPPETS_FILE, snippets, 200);
}

let _id = 100;

// ---------------------------------------------------------------------------
// Store
// ---------------------------------------------------------------------------
export const useSnippetStore = create<SnippetState>((set, get) => ({
  snippets: loadSnippets(),

  addSnippet: (opts) => {
    const now = Date.now();
    const snippet: Snippet = {
      id: `snip_${++_id}`,
      name: opts.name,
      content: opts.content,
      owner: opts.owner === "assistant" ? "assistant" : "user",
      category: opts.category ?? "General",
      tags: opts.tags ?? [],
      description: opts.description ?? "",
      createdAt: now,
      updatedAt: now,
      useCount: 0,
      isFavorite: false,
    };
    set((s) => {
      const updated = [...s.snippets, snippet];
      saveSnippets(updated);
      return { snippets: updated };
    });
  },

  updateSnippet: (id, updates) => {
    set((s) => {
      const updated = s.snippets.map((sn) =>
        sn.id === id ? { ...sn, ...updates, updatedAt: Date.now() } : sn
      );
      saveSnippets(updated);
      return { snippets: updated };
    });
  },

  deleteSnippet: (id) => {
    set((s) => {
      const updated = s.snippets.filter((sn) => sn.id !== id);
      saveSnippets(updated);
      return { snippets: updated };
    });
  },

  incrementUseCount: (id) => {
    set((s) => {
      const updated = s.snippets.map((sn) =>
        sn.id === id ? { ...sn, useCount: sn.useCount + 1, updatedAt: Date.now() } : sn
      );
      saveSnippets(updated);
      return { snippets: updated };
    });
  },

  toggleFavorite: (id) => {
    set((s) => {
      const updated = s.snippets.map((sn) =>
        sn.id === id ? { ...sn, isFavorite: !sn.isFavorite } : sn
      );
      saveSnippets(updated);
      return { snippets: updated };
    });
  },

  search: (query) => {
    const lower = query.toLowerCase();
    return get().snippets.filter(
      (s) =>
        s.name.toLowerCase().includes(lower) ||
        s.content.toLowerCase().includes(lower) ||
        s.category.toLowerCase().includes(lower) ||
        s.description.toLowerCase().includes(lower) ||
        s.tags.some((t) => t.toLowerCase().includes(lower))
    );
  },

  getCategories: () => {
    const cats = new Set(get().snippets.map((s) => s.category));
    return [...cats].sort();
  },
}));

export async function hydrateSnippetStore(): Promise<void> {
  const persisted = await readPersistedJson<Snippet[]>(SNIPPETS_FILE);
  if (Array.isArray(persisted) && persisted.length > 0) {
    const normalized: Snippet[] = persisted.map((snippet) => ({
      ...snippet,
      owner: snippet?.owner === "assistant" ? "assistant" : "user",
    }));
    syncSnippetCounter(normalized);
    useSnippetStore.setState({ snippets: normalized });
    return;
  }

  scheduleJsonWrite(SNIPPETS_FILE, useSnippetStore.getState().snippets, 0);
}

/** Resolve template placeholders: {{key}} → value */
export function resolveSnippetTemplate(
  content: string,
  params: Record<string, string>
): string {
  return content.replace(/\{\{(\w+)\}\}/g, (_, key) => params[key] ?? `{{${key}}}`);
}

/** Extract placeholder names from snippet content. */
export function getSnippetPlaceholders(content: string): string[] {
  const matches = content.matchAll(/\{\{(\w+)\}\}/g);
  return [...new Set([...matches].map((m) => m[1]))];
}
