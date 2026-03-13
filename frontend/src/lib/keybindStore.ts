import { create } from "zustand";
import type { HotkeyAction, Keybinding } from "./types";
import {
    readPersistedJson,
    scheduleJsonWrite,
} from "./persistence";

const KEYBINDS_FILE = "keybindings.json";

export const DEFAULT_KEYBINDINGS: Keybinding[] = [
    { action: "splitHorizontal", combo: "Ctrl+D", description: "Split horizontal" },
    { action: "splitVertical", combo: "Ctrl+Shift+D", description: "Split vertical" },
    { action: "closePane", combo: "Ctrl+Shift+W", description: "Close active pane" },
    { action: "toggleZoom", combo: "Ctrl+Shift+Z", description: "Toggle zoom pane" },
    { action: "focusLeft", combo: "Ctrl+Alt+ArrowLeft", description: "Focus left pane" },
    { action: "focusRight", combo: "Ctrl+Alt+ArrowRight", description: "Focus right pane" },
    { action: "focusUp", combo: "Ctrl+Alt+ArrowUp", description: "Focus upper pane" },
    { action: "focusDown", combo: "Ctrl+Alt+ArrowDown", description: "Focus lower pane" },
    { action: "newSurface", combo: "Ctrl+T", description: "New surface" },
    { action: "closeSurface", combo: "Ctrl+W", description: "Close surface" },
    { action: "nextSurface", combo: "Ctrl+Tab", description: "Next surface" },
    { action: "prevSurface", combo: "Ctrl+Shift+Tab", description: "Previous surface" },
    { action: "newWorkspace", combo: "Ctrl+Shift+N", description: "New workspace" },
    { action: "switchWorkspace1", combo: "Ctrl+1", description: "Switch workspace 1" },
    { action: "switchWorkspace2", combo: "Ctrl+2", description: "Switch workspace 2" },
    { action: "switchWorkspace3", combo: "Ctrl+3", description: "Switch workspace 3" },
    { action: "switchWorkspace4", combo: "Ctrl+4", description: "Switch workspace 4" },
    { action: "switchWorkspace5", combo: "Ctrl+5", description: "Switch workspace 5" },
    { action: "switchWorkspace6", combo: "Ctrl+6", description: "Switch workspace 6" },
    { action: "switchWorkspace7", combo: "Ctrl+7", description: "Switch workspace 7" },
    { action: "switchWorkspace8", combo: "Ctrl+8", description: "Switch workspace 8" },
    { action: "switchWorkspace9", combo: "Ctrl+9", description: "Switch workspace 9" },
    { action: "nextWorkspace", combo: "Ctrl+PageDown", description: "Next workspace" },
    { action: "prevWorkspace", combo: "Ctrl+PageUp", description: "Previous workspace" },
    { action: "toggleCommandPalette", combo: "Ctrl+Shift+P", description: "Toggle command palette" },
    { action: "toggleSidebar", combo: "Ctrl+B", description: "Toggle sidebar" },
    { action: "toggleNotifications", combo: "Ctrl+I", description: "Toggle notifications" },
    { action: "toggleSettings", combo: "Ctrl+,", description: "Toggle settings" },
    { action: "toggleSessionVault", combo: "Ctrl+Shift+V", description: "Toggle session vault" },
    { action: "toggleCommandLog", combo: "Ctrl+Shift+L", description: "Toggle command log" },
    { action: "toggleSearch", combo: "Ctrl+Shift+F", description: "Toggle search" },
    { action: "toggleCommandHistory", combo: "Ctrl+Alt+H", description: "Toggle command history" },
    { action: "toggleSnippets", combo: "Ctrl+S", description: "Toggle snippets" },
    { action: "toggleAgentPanel", combo: "Ctrl+Shift+A", description: "Toggle agent panel" },
    { action: "toggleSystemMonitor", combo: "Ctrl+Shift+M", description: "Toggle system monitor" },
    { action: "toggleFileManager", combo: "Ctrl+Shift+E", description: "Toggle file manager" },
    { action: "toggleCanvas", combo: "Ctrl+Shift+G", description: "Toggle execution canvas" },
    { action: "toggleTimeTravel", combo: "Ctrl+Shift+T", description: "Toggle time-travel snapshots" },
];

function loadKeybindings(): Keybinding[] {
    return DEFAULT_KEYBINDINGS;
}

function persistKeybindings(bindings: Keybinding[]) {
    scheduleJsonWrite(KEYBINDS_FILE, bindings, 250);
}

function normalizeKeyToken(key: string): string {
    if (key === " ") return "Space";
    if (key.length === 1) return key.toUpperCase();
    return key;
}

export function formatKeyboardEvent(event: KeyboardEvent): string {
    const parts: string[] = [];
    if (event.ctrlKey || event.metaKey) parts.push("Ctrl");
    if (event.altKey) parts.push("Alt");
    if (event.shiftKey) parts.push("Shift");

    const key = normalizeKeyToken(event.key);
    if (["Control", "Shift", "Alt", "Meta"].includes(key)) {
        return parts.join("+");
    }

    parts.push(key === "Escape" ? "Esc" : key);
    return parts.join("+");
}

export function matchesKeybinding(combo: string, event: KeyboardEvent): boolean {
    return formatKeyboardEvent(event) === combo;
}

type KeybindState = {
    bindings: Keybinding[];
    setBinding: (action: HotkeyAction, combo: string) => void;
    resetBindings: () => void;
    getBinding: (action: HotkeyAction) => Keybinding | undefined;
};

const initialBindings = loadKeybindings();

export const useKeybindStore = create<KeybindState>((set, get) => ({
    bindings: initialBindings,

    setBinding: (action, combo) => {
        const normalized = combo.trim();
        set((state) => {
            const bindings = state.bindings.map((binding) =>
                binding.action === action ? { ...binding, combo: normalized } : binding,
            );
            persistKeybindings(bindings);
            return { bindings };
        });
    },

    resetBindings: () => {
        persistKeybindings(DEFAULT_KEYBINDINGS);
        set({ bindings: DEFAULT_KEYBINDINGS });
    },

    getBinding: (action) => get().bindings.find((binding) => binding.action === action),
}));

export async function hydrateKeybindStore(): Promise<void> {
    const diskBindings = await readPersistedJson<Keybinding[]>(KEYBINDS_FILE);
    if (Array.isArray(diskBindings) && diskBindings.length > 0) {
        useKeybindStore.setState({ bindings: diskBindings });
        return;
    }

    if (initialBindings.length > 0) {
        scheduleJsonWrite(KEYBINDS_FILE, initialBindings, 0);
    }
}