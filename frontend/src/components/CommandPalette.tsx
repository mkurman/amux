import { useEffect, useRef, useState } from "react";
import { useWorkspaceStore } from "../lib/workspaceStore";
import { useSettingsStore } from "../lib/settingsStore";
import { useKeybindStore } from "../lib/keybindStore";
import { BUILTIN_THEMES } from "../lib/themes";
import { CommandPaletteHeader } from "./command-palette/CommandPaletteHeader";
import { CommandPaletteResults } from "./command-palette/CommandPaletteResults";
import type { Command, CommandPaletteProps } from "./command-palette/shared";

export function CommandPalette({ style, className }: CommandPaletteProps = {}) {
  const open = useWorkspaceStore((s) => s.commandPaletteOpen);
  const toggle = useWorkspaceStore((s) => s.toggleCommandPalette);
  const splitActive = useWorkspaceStore((s) => s.splitActive);
  const createSurface = useWorkspaceStore((s) => s.createSurface);
  const closeSurface = useWorkspaceStore((s) => s.closeSurface);
  const createWorkspace = useWorkspaceStore((s) => s.createWorkspace);
  const activePaneId = useWorkspaceStore((s) => s.activePaneId);
  const closePane = useWorkspaceStore((s) => s.closePane);
  const toggleZoom = useWorkspaceStore((s) => s.toggleZoom);
  const toggleSidebar = useWorkspaceStore((s) => s.toggleSidebar);
  const toggleNotificationPanel = useWorkspaceStore((s) => s.toggleNotificationPanel);
  const toggleSettings = useWorkspaceStore((s) => s.toggleSettings);
  const toggleSessionVault = useWorkspaceStore((s) => s.toggleSessionVault);
  const toggleCommandLog = useWorkspaceStore((s) => s.toggleCommandLog);
  const toggleSearch = useWorkspaceStore((s) => s.toggleSearch);
  const toggleSnippetPicker = useWorkspaceStore((s) => s.toggleSnippetPicker);
  const toggleAgentPanel = useWorkspaceStore((s) => s.toggleAgentPanel);
  const toggleCommandHistory = useWorkspaceStore((s) => s.toggleCommandHistory);
  const toggleSystemMonitor = useWorkspaceStore((s) => s.toggleSystemMonitor);
  const toggleCanvas = useWorkspaceStore((s) => s.toggleCanvas);
  const toggleTimeTravel = useWorkspaceStore((s) => s.toggleTimeTravel);
  const applyPresetLayout = useWorkspaceStore((s) => s.applyPresetLayout);
  const activeSurface = useWorkspaceStore((s) => s.activeSurface());
  const updateSetting = useSettingsStore((s) => s.updateSetting);
  const settings = useSettingsStore((s) => s.settings);
  const bindings = useKeybindStore((s) => s.bindings);

  const inputRef = useRef<HTMLInputElement>(null);
  const [query, setQuery] = useState("");
  const [selectedIndex, setSelectedIndex] = useState(0);

  const shortcutFor = (action: string) => bindings.find((binding) => binding.action === action)?.combo;

  const commands: Command[] = [
    { id: "split-h", label: "Split Horizontal", category: "Layout", shortcut: shortcutFor("splitHorizontal"), action: () => splitActive("horizontal") },
    { id: "split-v", label: "Split Vertical", category: "Layout", shortcut: shortcutFor("splitVertical"), action: () => splitActive("vertical") },
    { id: "close-pane", label: "Close Active Pane", category: "Layout", shortcut: shortcutFor("closePane"), action: () => { const id = activePaneId(); if (id) closePane(id); } },
    { id: "zoom-pane", label: "Toggle Zoom Pane", category: "Layout", shortcut: shortcutFor("toggleZoom"), action: toggleZoom },
    { id: "layout-single", label: "Layout: Single Pane", category: "Layout", action: () => applyPresetLayout("single") },
    { id: "layout-2col", label: "Layout: 2 Columns", category: "Layout", action: () => applyPresetLayout("2-columns") },
    { id: "layout-3col", label: "Layout: 3 Columns", category: "Layout", action: () => applyPresetLayout("3-columns") },
    { id: "layout-grid", label: "Layout: Grid 2×2", category: "Layout", action: () => applyPresetLayout("grid-2x2") },
    { id: "layout-main-stack", label: "Layout: Main + Stack", category: "Layout", action: () => applyPresetLayout("main-stack") },
    { id: "new-surface", label: "New Surface", category: "Surface", shortcut: shortcutFor("newSurface"), action: () => createSurface() },
    { id: "close-surface", label: "Close Surface", category: "Surface", shortcut: shortcutFor("closeSurface"), action: () => { if (activeSurface) closeSurface(activeSurface.id); } },
    { id: "new-workspace", label: "New Workspace", category: "Workspace", shortcut: shortcutFor("newWorkspace"), action: () => createWorkspace() },
    { id: "toggle-sidebar", label: "Toggle Sidebar", category: "View", shortcut: shortcutFor("toggleSidebar"), action: toggleSidebar },
    { id: "notifications", label: "Notifications", category: "View", shortcut: shortcutFor("toggleNotifications"), action: toggleNotificationPanel },
    { id: "settings", label: "Settings", category: "View", shortcut: shortcutFor("toggleSettings"), action: toggleSettings },
    { id: "session-vault", label: "Session Vault", category: "View", shortcut: shortcutFor("toggleSessionVault"), action: toggleSessionVault },
    { id: "command-log", label: "Command Log", category: "View", shortcut: shortcutFor("toggleCommandLog"), action: toggleCommandLog },
    { id: "find-in-buffer", label: "Find in Buffer", category: "View", shortcut: shortcutFor("toggleSearch"), action: toggleSearch },
    { id: "snippets", label: "Snippets", category: "Agent", shortcut: shortcutFor("toggleSnippets"), action: toggleSnippetPicker },
    { id: "agent-panel", label: "Mission Console", category: "Agent", shortcut: shortcutFor("toggleAgentPanel"), action: toggleAgentPanel },
    { id: "command-history", label: "Command History", category: "Agent", shortcut: shortcutFor("toggleCommandHistory"), action: toggleCommandHistory },
    { id: "system-monitor", label: "System Monitor", category: "View", shortcut: shortcutFor("toggleSystemMonitor"), action: toggleSystemMonitor },
    { id: "execution-canvas", label: "Execution Canvas", category: "View", shortcut: shortcutFor("toggleCanvas"), action: toggleCanvas },
    { id: "time-travel", label: "Time Travel Snapshots", category: "View", shortcut: shortcutFor("toggleTimeTravel"), action: toggleTimeTravel },
    { id: "verify-integrity", label: "Verify WORM Integrity", category: "Infrastructure", action: () => { ((window as any).tamux ?? (window as any).amux)?.verifyIntegrity?.(); } },
    { id: "reload-cdui-views", label: "Reload CDUI Views", category: "Infrastructure", action: () => { window.dispatchEvent(new Event("tamux-cdui-views-reload")); window.dispatchEvent(new Event("amux-cdui-views-reload")); } },
    { id: "generate-skill", label: "Generate Skill from History", category: "Infrastructure", action: toggleAgentPanel },
    { id: "toggle-sandbox", label: "Toggle Sandbox", category: "Infrastructure", action: () => updateSetting("sandboxEnabled", !settings.sandboxEnabled) },
    ...BUILTIN_THEMES.map((theme) => ({
      id: `theme-${theme.name}`,
      label: `Theme: ${theme.name}`,
      category: "Theme",
      action: () => updateSetting("themeName", theme.name),
    })),
  ];

  const filtered = commands.filter(
    (c) =>
      c.label.toLowerCase().includes(query.toLowerCase()) ||
      c.id.toLowerCase().includes(query.toLowerCase()) ||
      (c.category && c.category.toLowerCase().includes(query.toLowerCase()))
  );

  const grouped = filtered.reduce((acc, cmd) => {
    const cat = cmd.category || "Other";
    if (!acc[cat]) acc[cat] = [];
    acc[cat].push(cmd);
    return acc;
  }, {} as Record<string, Command[]>);

  const categories = Object.keys(grouped).sort();
  const flatFiltered = categories.flatMap((cat) => grouped[cat]);

  useEffect(() => {
    if (open) {
      setQuery("");
      setSelectedIndex(0);
      setTimeout(() => inputRef.current?.focus(), 0);
    }
  }, [open]);

  useEffect(() => {
    setSelectedIndex((current) => {
      if (flatFiltered.length === 0) return 0;
      return Math.min(current, flatFiltered.length - 1);
    });
  }, [flatFiltered.length]);

  if (!open) return null;

  const executeAndClose = (command: Command) => {
    command.action();
    toggle();
  };

  return (
    <div
      onClick={toggle}
      style={{
        position: "fixed",
        inset: 0,
        background: "var(--bg-overlay)",
        display: "flex",
        alignItems: "flex-start",
        justifyContent: "center",
        paddingTop: 80,
        zIndex: 5000,
        backdropFilter: "none",
        ...(style ?? {}),
      }}
      className={className}
    >
      <div
        onClick={(e) => e.stopPropagation()}
        style={{
          background: "var(--bg-primary)",
          border: "1px solid var(--border)",
          borderRadius: "var(--radius-xl)",
          width: 640,
          maxWidth: "92vw",
          maxHeight: "70vh",
          overflow: "hidden",
          display: "flex",
          flexDirection: "column",
        }}
      >
        <CommandPaletteHeader commandCount={filtered.length} />

        <input
          ref={inputRef}
          value={query}
          onChange={(e) => setQuery(e.target.value)}
          onKeyDown={(e) => {
            if (e.key === "Escape") toggle();
            if (e.key === "ArrowDown") {
              e.preventDefault();
              setSelectedIndex((current) => Math.min(current + 1, flatFiltered.length - 1));
            }
            if (e.key === "ArrowUp") {
              e.preventDefault();
              setSelectedIndex((current) => Math.max(current - 1, 0));
            }
            if (e.key === "Enter" && flatFiltered.length > 0) {
              const command = flatFiltered[selectedIndex];
              if (command) {
                executeAndClose(command);
              }
            }
          }}
          placeholder="Find commands, layouts, tools, or actions..."
          style={{
            width: "100%",
            padding: "var(--space-4)",
            background: "transparent",
            border: "none",
            borderBottom: "1px solid var(--border)",
            color: "var(--text-primary)",
            fontSize: "var(--text-md)",
            fontFamily: "inherit",
            outline: "none",
          }}
        />

        <CommandPaletteResults
          filtered={filtered}
          grouped={grouped}
          categories={categories}
          flatFiltered={flatFiltered}
          selectedIndex={selectedIndex}
          setSelectedIndex={setSelectedIndex}
          onExecute={executeAndClose}
        />
      </div>
    </div>
  );
}
