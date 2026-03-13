import { create } from "zustand";
import {
  Workspace,
  Surface,
  WorkspaceId,
  SurfaceId,
  PaneId,
} from "./types";
import {
  BspTree,
  SplitDirection,
  createLeaf,
  splitPane,
  removePane,
  allLeafIds,
  setSessionId,
  buildPresetLayout,
  PresetLayout,
  Direction,
  findAdjacentPane,
  findLeaf,
  normalizeBspTree,
  syncPaneIdCounter,
  equalizeLayoutRatios,
  updateRatio,
} from "./bspTree";
import { PersistedSession } from "./types";
import { useSettingsStore } from "./settingsStore";
import { useTranscriptStore } from "./transcriptStore";
import { getTerminalSnapshot } from "./terminalRegistry";

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

let _wsId = 0;
function newWorkspaceId(): WorkspaceId {
  return `ws_${++_wsId}`;
}

let _sfId = 0;
function newSurfaceId(): SurfaceId {
  return `sf_${++_sfId}`;
}

function syncWorkspaceAndSurfaceCounters(workspaces: Workspace[]) {
  let maxWorkspaceId = _wsId;
  let maxSurfaceId = _sfId;

  for (const workspace of workspaces) {
    const workspaceMatch = /^ws_(\d+)$/.exec(workspace.id);
    if (workspaceMatch) {
      maxWorkspaceId = Math.max(maxWorkspaceId, Number(workspaceMatch[1]));
    }

    for (const surface of workspace.surfaces) {
      const surfaceMatch = /^sf_(\d+)$/.exec(surface.id);
      if (surfaceMatch) {
        maxSurfaceId = Math.max(maxSurfaceId, Number(surfaceMatch[1]));
      }
    }
  }

  _wsId = maxWorkspaceId;
  _sfId = maxSurfaceId;
}

const ACCENT_COLORS = [
  "#89b4fa",
  "#a6e3a1",
  "#f9e2af",
  "#f38ba8",
  "#f5c2e7",
  "#94e2d5",
  "#fab387",
  "#cba6f7",
];

function pickAccent(idx: number): string {
  return ACCENT_COLORS[idx % ACCENT_COLORS.length];
}

function createDefaultSurface(workspaceId: WorkspaceId): Surface {
  const leaf = createLeaf();
  return {
    id: newSurfaceId(),
    workspaceId,
    name: "Terminal",
    icon: "term",
    layout: leaf,
    paneNames: { [leaf.id]: "Pane 1" },
    activePaneId: leaf.id,
    createdAt: Date.now(),
  };
}

function buildPaneNames(paneIds: PaneId[], existing?: Record<PaneId, string>): Record<PaneId, string> {
  const names: Record<PaneId, string> = {};
  let nextIndex = 1;

  for (const paneId of paneIds) {
    const candidate = existing?.[paneId]?.trim();
    if (candidate) {
      names[paneId] = candidate;
      continue;
    }

    names[paneId] = `Pane ${nextIndex}`;
    nextIndex += 1;
  }

  return names;
}

function createDefaultWorkspace(name?: string): Workspace {
  const id = newWorkspaceId();
  const surface = createDefaultSurface(id);
  return {
    id,
    name: name ?? `Workspace ${_wsId}`,
    icon: "terminal",
    accentColor: pickAccent(_wsId - 1),
    cwd: "",
    gitBranch: null,
    gitDirty: false,
    listeningPorts: [],
    unreadCount: 0,
    surfaces: [surface],
    activeSurfaceId: surface.id,
    createdAt: Date.now(),
  };
}

function stopPaneSessions(paneIds: string[]) {
  const amux = (window as any).amux;
  if (!amux?.stopTerminalSession) return;

  for (const paneId of paneIds) {
    void amux.stopTerminalSession(paneId, true);
  }
}

function captureTranscriptForPane(opts: {
  paneId: string;
  workspaceId?: string | null;
  surfaceId?: string | null;
  cwd?: string | null;
  reason: "pane-close" | "surface-close" | "workspace-close";
}) {
  const settings = useSettingsStore.getState().settings;
  if (!settings.captureTranscriptsOnClose) {
    return;
  }

  const content = getTerminalSnapshot(opts.paneId).trim();
  if (!content) {
    return;
  }

  useTranscriptStore.getState().addTranscript({
    content,
    reason: opts.reason,
    workspaceId: opts.workspaceId ?? null,
    surfaceId: opts.surfaceId ?? null,
    paneId: opts.paneId,
    cwd: opts.cwd ?? null,
  });
}

type PaneSnapshot = {
  id: string;
  sessionId?: string;
};

function collectPaneSnapshots(tree: BspTree, activePaneId: PaneId | null): PaneSnapshot[] {
  const paneIds = allLeafIds(tree);
  const orderedPaneIds = activePaneId && paneIds.includes(activePaneId)
    ? [activePaneId, ...paneIds.filter((paneId) => paneId !== activePaneId)]
    : paneIds;

  const panes: PaneSnapshot[] = [];

  for (const paneId of orderedPaneIds) {
    const leaf = findLeaf(tree, paneId);
    if (!leaf) {
      continue;
    }

    panes.push({
      id: leaf.id,
      sessionId: leaf.sessionId,
    });
  }

  return panes;
}

function applyPaneSnapshots(
  tree: BspTree,
  panes: PaneSnapshot[]
): BspTree {
  let index = 0;

  const visit = (node: BspTree): BspTree => {
    if (node.type === "leaf") {
      const preservedPane = panes[index];
      index += 1;

      if (!preservedPane) {
        return node;
      }

      return {
        type: "leaf",
        id: preservedPane.id,
        sessionId: preservedPane.sessionId,
      };
    }

    return {
      ...node,
      first: visit(node.first),
      second: visit(node.second),
    };
  };

  return visit(tree);
}

function normalizeBrowserUrl(value: string): string {
  const trimmed = value.trim();
  if (!trimmed) return "about:blank";
  if (/^(about:|https?:\/\/|file:\/\/)/i.test(trimmed)) return trimmed;
  return `https://${trimmed}`;
}

const DEFAULT_WEB_BROWSER_URL = "https://example.com";

type WorkspaceBrowserState = {
  open: boolean;
  fullscreen: boolean;
  url: string;
  history: string[];
  historyIndex: number;
  reloadToken: number;
};

type WorkspaceBrowserProjection = {
  webBrowserOpen: boolean;
  webBrowserFullscreen: boolean;
  webBrowserUrl: string;
  webBrowserHistory: string[];
  webBrowserHistoryIndex: number;
  webBrowserReloadToken: number;
};

function createDefaultWorkspaceBrowserState(seedUrl: string = DEFAULT_WEB_BROWSER_URL): WorkspaceBrowserState {
  const normalizedUrl = normalizeBrowserUrl(seedUrl);
  return {
    open: false,
    fullscreen: false,
    url: normalizedUrl,
    history: [normalizedUrl],
    historyIndex: 0,
    reloadToken: 0,
  };
}

function normalizeWorkspaceBrowserState(input?: Partial<WorkspaceBrowserState>): WorkspaceBrowserState {
  const normalizedUrl = normalizeBrowserUrl(typeof input?.url === "string" ? input.url : DEFAULT_WEB_BROWSER_URL);
  const history = Array.isArray(input?.history)
    ? input.history
      .filter((value): value is string => typeof value === "string" && value.trim().length > 0)
      .map((value) => normalizeBrowserUrl(value))
    : [];
  const normalizedHistory = history.length > 0 ? history : [normalizedUrl];
  const normalizedHistoryIndex = typeof input?.historyIndex === "number" && Number.isFinite(input.historyIndex)
    ? Math.max(0, Math.min(normalizedHistory.length - 1, Math.floor(input.historyIndex)))
    : normalizedHistory.length - 1;

  return {
    open: Boolean(input?.open),
    fullscreen: Boolean(input?.fullscreen),
    url: normalizedHistory[normalizedHistoryIndex] ?? normalizedUrl,
    history: normalizedHistory,
    historyIndex: normalizedHistoryIndex,
    reloadToken: typeof input?.reloadToken === "number" && Number.isFinite(input.reloadToken)
      ? Math.max(0, Math.floor(input.reloadToken))
      : 0,
  };
}

function projectWorkspaceBrowserState(browser: WorkspaceBrowserState): WorkspaceBrowserProjection {
  return {
    webBrowserOpen: browser.open,
    webBrowserFullscreen: browser.fullscreen,
    webBrowserUrl: browser.url,
    webBrowserHistory: browser.history,
    webBrowserHistoryIndex: browser.historyIndex,
    webBrowserReloadToken: browser.reloadToken,
  };
}

// ---------------------------------------------------------------------------
// Store interface
// ---------------------------------------------------------------------------

export interface WorkspaceState {
  workspaces: Workspace[];
  activeWorkspaceId: WorkspaceId | null;
  sidebarVisible: boolean;
  sidebarWidth: number;
  zoomedPaneId: PaneId | null;
  commandPaletteOpen: boolean;
  searchOpen: boolean;
  settingsOpen: boolean;
  notificationPanelOpen: boolean;
  sessionVaultOpen: boolean;
  commandLogOpen: boolean;
  commandHistoryOpen: boolean;
  snippetPickerOpen: boolean;
  agentPanelOpen: boolean;
  systemMonitorOpen: boolean;
  fileManagerOpen: boolean;
  canvasOpen: boolean;
  timeTravelOpen: boolean;
  workspaceBrowserState: Record<WorkspaceId, WorkspaceBrowserState>;
  webBrowserOpen: boolean;
  webBrowserFullscreen: boolean;
  webBrowserUrl: string;
  webBrowserHistory: string[];
  webBrowserHistoryIndex: number;
  webBrowserReloadToken: number;

  // -- Workspace actions --
  createWorkspace: (name?: string) => void;
  renameWorkspace: (id: WorkspaceId, name: string) => void;
  setWorkspaceIcon: (id: WorkspaceId, icon: string) => void;
  closeWorkspace: (id: WorkspaceId) => void;
  setActiveWorkspace: (id: WorkspaceId) => void;
  switchWorkspaceByIndex: (index: number) => void;
  nextWorkspace: () => void;
  prevWorkspace: () => void;
  updateWorkspaceGit: (id: WorkspaceId, branch: string | null, dirty: boolean) => void;
  updateWorkspaceCwd: (id: WorkspaceId, cwd: string) => void;
  updateWorkspacePorts: (id: WorkspaceId, ports: number[]) => void;
  clearWorkspaceUnread: (id: WorkspaceId) => void;

  // -- Surface actions --
  createSurface: (workspaceId?: WorkspaceId) => void;
  renameSurface: (surfaceId: SurfaceId, name: string) => void;
  setSurfaceIcon: (surfaceId: SurfaceId, icon: string) => void;
  closeSurface: (surfaceId: SurfaceId) => void;
  nextSurface: () => void;
  prevSurface: () => void;
  setActiveSurface: (surfaceId: SurfaceId) => void;

  // -- Pane actions --
  splitActive: (direction: SplitDirection, newPaneName?: string) => void;
  closePane: (paneId: PaneId) => void;
  setActivePaneId: (paneId: PaneId) => void;
  setPaneSessionId: (paneId: PaneId, sessionId: string) => void;
  setPaneName: (paneId: PaneId, name: string) => void;
  paneName: (paneId: PaneId) => string | null;
  focusDirection: (direction: Direction) => void;
  toggleZoom: () => void;
  applyPresetLayout: (preset: PresetLayout) => void;
  equalizeLayout: () => void;

  // -- UI panel toggles --
  toggleSidebar: () => void;
  setSidebarWidth: (width: number) => void;
  toggleCommandPalette: () => void;
  toggleSearch: () => void;
  toggleSettings: () => void;
  toggleNotificationPanel: () => void;
  toggleSessionVault: () => void;
  toggleCommandLog: () => void;
  toggleCommandHistory: () => void;
  toggleSnippetPicker: () => void;
  toggleAgentPanel: () => void;
  toggleSystemMonitor: () => void;
  toggleFileManager: () => void;
  toggleCanvas: () => void;
  toggleTimeTravel: () => void;
  toggleWebBrowser: () => void;
  setWebBrowserOpen: (open: boolean) => void;
  navigateWebBrowser: (url: string) => void;
  webBrowserBack: () => void;
  webBrowserForward: () => void;
  webBrowserReload: () => void;
  toggleWebBrowserFullscreen: () => void;
  setWebBrowserFullscreen: (fullscreen: boolean) => void;
  updateNodeRatio: (paneId: PaneId, newRatio: number) => void;
  hydrateSession: (session: PersistedSession) => void;

  // -- Helpers (derived) --
  activeWorkspace: () => Workspace | undefined;
  activeSurface: () => Surface | undefined;
  activePaneId: () => PaneId | null;
}

// ---------------------------------------------------------------------------
// Zustand implementation
// ---------------------------------------------------------------------------

export const useWorkspaceStore = create<WorkspaceState>((set, get) => {
  function findWsAndSurface(surfaceId: SurfaceId) {
    const { workspaces } = get();
    for (const ws of workspaces) {
      const sf = ws.surfaces.find((s) => s.id === surfaceId);
      if (sf) return { ws, sf };
    }
    return null;
  }

  function findWsSurfaceAndPane(paneId: PaneId) {
    const { workspaces } = get();
    for (const ws of workspaces) {
      for (const sf of ws.surfaces) {
        if (allLeafIds(sf.layout).includes(paneId)) {
          return { ws, sf };
        }
      }
    }
    return null;
  }

  function updateSurface(
    surfaceId: SurfaceId,
    updater: (sf: Surface) => Surface
  ) {
    const { workspaces } = get();
    set({
      workspaces: workspaces.map((ws) => ({
        ...ws,
        surfaces: ws.surfaces.map((sf) =>
          sf.id === surfaceId ? updater(sf) : sf
        ),
      })),
    });
  }

  function getActiveSurface(): Surface | undefined {
    const { workspaces, activeWorkspaceId } = get();
    const ws = workspaces.find((w) => w.id === activeWorkspaceId);
    if (!ws) return undefined;
    return ws.surfaces.find((s) => s.id === ws.activeSurfaceId);
  }

  function browserStateForWorkspace(
    workspaceBrowserState: Record<WorkspaceId, WorkspaceBrowserState>,
    workspaceId: WorkspaceId,
  ): WorkspaceBrowserState {
    return workspaceBrowserState[workspaceId] ?? createDefaultWorkspaceBrowserState();
  }

  function activateWorkspaceBrowserState(
    workspaceBrowserState: Record<WorkspaceId, WorkspaceBrowserState>,
    workspaceId: WorkspaceId | null,
  ): {
    workspaceBrowserState: Record<WorkspaceId, WorkspaceBrowserState>;
  } & WorkspaceBrowserProjection {
    if (!workspaceId) {
      const fallback = createDefaultWorkspaceBrowserState();
      return {
        workspaceBrowserState,
        ...projectWorkspaceBrowserState(fallback),
      };
    }

    const browser = browserStateForWorkspace(workspaceBrowserState, workspaceId);
    const normalizedMap = workspaceBrowserState[workspaceId]
      ? workspaceBrowserState
      : { ...workspaceBrowserState, [workspaceId]: browser };

    return {
      workspaceBrowserState: normalizedMap,
      ...projectWorkspaceBrowserState(browser),
    };
  }

  return {
    workspaces: [],
    activeWorkspaceId: null,
    sidebarVisible: true,
    sidebarWidth: 400,
    zoomedPaneId: null,
    commandPaletteOpen: false,
    searchOpen: false,
    settingsOpen: false,
    notificationPanelOpen: false,
    sessionVaultOpen: false,
    commandLogOpen: false,
    commandHistoryOpen: false,
    snippetPickerOpen: false,
    agentPanelOpen: false,
    systemMonitorOpen: false,
    fileManagerOpen: false,
    canvasOpen: false,
    timeTravelOpen: false,
    workspaceBrowserState: {},
    webBrowserOpen: false,
    webBrowserFullscreen: false,
    webBrowserUrl: DEFAULT_WEB_BROWSER_URL,
    webBrowserHistory: [DEFAULT_WEB_BROWSER_URL],
    webBrowserHistoryIndex: 0,
    webBrowserReloadToken: 0,

    // ===== Workspace actions =====

    createWorkspace: (name?: string) => {
      const ws = createDefaultWorkspace(name);
      set((s) => {
        const workspaceBrowserState = {
          ...s.workspaceBrowserState,
          [ws.id]: createDefaultWorkspaceBrowserState(),
        };
        return {
          workspaces: [...s.workspaces, ws],
          activeWorkspaceId: ws.id,
          ...activateWorkspaceBrowserState(workspaceBrowserState, ws.id),
        };
      });
    },

    renameWorkspace: (id, name) => {
      set((s) => ({
        workspaces: s.workspaces.map((ws) =>
          ws.id === id ? { ...ws, name } : ws
        ),
      }));
    },

    setWorkspaceIcon: (id, icon) => {
      set((s) => ({
        workspaces: s.workspaces.map((ws) =>
          ws.id === id ? { ...ws, icon: icon.trim() || ws.icon } : ws,
        ),
      }));
    },

    closeWorkspace: (id) => {
      const { workspaces, activeWorkspaceId } = get();
      const removedWorkspace = workspaces.find((workspace) => workspace.id === id);
      if (removedWorkspace) {
        const paneIds = removedWorkspace.surfaces.flatMap((surface) => {
          const ids = allLeafIds(surface.layout);
          for (const paneId of ids) {
            captureTranscriptForPane({
              paneId,
              workspaceId: removedWorkspace.id,
              surfaceId: surface.id,
              cwd: removedWorkspace.cwd,
              reason: "workspace-close",
            });
          }
          return ids;
        });
        stopPaneSessions(paneIds);
      }
      const remaining = workspaces.filter((w) => w.id !== id);
      if (remaining.length === 0) {
        // Always keep at least one workspace.
        const ws = createDefaultWorkspace();
        const workspaceBrowserState = {
          [ws.id]: createDefaultWorkspaceBrowserState(),
        };
        set({
          workspaces: [ws],
          activeWorkspaceId: ws.id,
          ...activateWorkspaceBrowserState(workspaceBrowserState, ws.id),
        });
        return;
      }
      set((s) => {
        const nextWorkspaceBrowserState = { ...s.workspaceBrowserState };
        delete nextWorkspaceBrowserState[id];
        const nextActiveWorkspaceId =
          activeWorkspaceId === id
            ? remaining[0].id
            : activeWorkspaceId;
        return {
          workspaces: remaining,
          activeWorkspaceId: nextActiveWorkspaceId,
          ...activateWorkspaceBrowserState(nextWorkspaceBrowserState, nextActiveWorkspaceId),
        };
      });
    },

    setActiveWorkspace: (id) => set((s) => ({
      activeWorkspaceId: id,
      zoomedPaneId: null,
      ...activateWorkspaceBrowserState(s.workspaceBrowserState, id),
    })),

    switchWorkspaceByIndex: (index) => {
      const { workspaces } = get();
      // index 9 = last, 1..8 = indices 0..7
      const target =
        index === 9
          ? workspaces[workspaces.length - 1]
          : workspaces[index - 1];
      if (target) {
        set((s) => ({
          activeWorkspaceId: target.id,
          zoomedPaneId: null,
          ...activateWorkspaceBrowserState(s.workspaceBrowserState, target.id),
        }));
      }
    },

    nextWorkspace: () => {
      const { workspaces, activeWorkspaceId } = get();
      const idx = workspaces.findIndex((w) => w.id === activeWorkspaceId);
      const next = workspaces[(idx + 1) % workspaces.length];
      if (next) {
        set((s) => ({
          activeWorkspaceId: next.id,
          zoomedPaneId: null,
          ...activateWorkspaceBrowserState(s.workspaceBrowserState, next.id),
        }));
      }
    },

    prevWorkspace: () => {
      const { workspaces, activeWorkspaceId } = get();
      const idx = workspaces.findIndex((w) => w.id === activeWorkspaceId);
      const prev =
        workspaces[(idx - 1 + workspaces.length) % workspaces.length];
      if (prev) {
        set((s) => ({
          activeWorkspaceId: prev.id,
          zoomedPaneId: null,
          ...activateWorkspaceBrowserState(s.workspaceBrowserState, prev.id),
        }));
      }
    },

    updateWorkspaceGit: (id, branch, dirty) => {
      set((s) => ({
        workspaces: s.workspaces.map((ws) =>
          ws.id === id ? { ...ws, gitBranch: branch, gitDirty: dirty } : ws
        ),
      }));
    },

    updateWorkspaceCwd: (id, cwd) => {
      set((s) => ({
        workspaces: s.workspaces.map((ws) =>
          ws.id === id ? { ...ws, cwd } : ws
        ),
      }));
    },

    updateWorkspacePorts: (id, ports) => {
      set((s) => ({
        workspaces: s.workspaces.map((ws) =>
          ws.id === id ? { ...ws, listeningPorts: ports } : ws
        ),
      }));
    },

    clearWorkspaceUnread: (id) => {
      set((s) => ({
        workspaces: s.workspaces.map((ws) =>
          ws.id === id ? { ...ws, unreadCount: 0 } : ws
        ),
      }));
    },

    // ===== Surface actions =====

    createSurface: (workspaceId?: WorkspaceId) => {
      const wsId = workspaceId ?? get().activeWorkspaceId;
      if (!wsId) return;
      const sf = createDefaultSurface(wsId);
      set((s) => ({
        workspaces: s.workspaces.map((ws) =>
          ws.id === wsId
            ? {
              ...ws,
              surfaces: [...ws.surfaces, sf],
              activeSurfaceId: sf.id,
            }
            : ws
        ),
      }));
    },

    renameSurface: (surfaceId, name) => {
      updateSurface(surfaceId, (sf) => ({ ...sf, name }));
    },

    setSurfaceIcon: (surfaceId, icon) => {
      updateSurface(surfaceId, (sf) => ({ ...sf, icon: icon.trim() || sf.icon }));
    },

    closeSurface: (surfaceId) => {
      const { workspaces } = get();
      const surfacePair = findWsAndSurface(surfaceId);
      if (surfacePair) {
        const paneIds = allLeafIds(surfacePair.sf.layout);
        for (const paneId of paneIds) {
          captureTranscriptForPane({
            paneId,
            workspaceId: surfacePair.ws.id,
            surfaceId: surfacePair.sf.id,
            cwd: surfacePair.ws.cwd,
            reason: "surface-close",
          });
        }
        stopPaneSessions(paneIds);
      }
      set({
        workspaces: workspaces.map((ws) => {
          const remaining = ws.surfaces.filter((s) => s.id !== surfaceId);
          if (remaining.length === 0) {
            const sf = createDefaultSurface(ws.id);
            return {
              ...ws,
              surfaces: [sf],
              activeSurfaceId: sf.id,
            };
          }
          return {
            ...ws,
            surfaces: remaining,
            activeSurfaceId:
              ws.activeSurfaceId === surfaceId
                ? remaining[0].id
                : ws.activeSurfaceId,
          };
        }),
      });
    },

    nextSurface: () => {
      const ws = get().activeWorkspace();
      if (!ws) return;
      const idx = ws.surfaces.findIndex(
        (s) => s.id === ws.activeSurfaceId
      );
      const next = ws.surfaces[(idx + 1) % ws.surfaces.length];
      if (next) {
        set((s) => ({
          workspaces: s.workspaces.map((w) =>
            w.id === ws.id ? { ...w, activeSurfaceId: next.id } : w
          ),
        }));
      }
    },

    prevSurface: () => {
      const ws = get().activeWorkspace();
      if (!ws) return;
      const idx = ws.surfaces.findIndex(
        (s) => s.id === ws.activeSurfaceId
      );
      const prev =
        ws.surfaces[
        (idx - 1 + ws.surfaces.length) % ws.surfaces.length
        ];
      if (prev) {
        set((s) => ({
          workspaces: s.workspaces.map((w) =>
            w.id === ws.id ? { ...w, activeSurfaceId: prev.id } : w
          ),
        }));
      }
    },

    setActiveSurface: (surfaceId) => {
      const pair = findWsAndSurface(surfaceId);
      if (!pair) return;
      set((s) => ({
        activeWorkspaceId: pair.ws.id,
        ...activateWorkspaceBrowserState(s.workspaceBrowserState, pair.ws.id),
        workspaces: s.workspaces.map((w) =>
          w.id === pair.ws.id ? { ...w, activeSurfaceId: surfaceId } : w
        ),
      }));
    },

    // ===== Pane actions =====

    splitActive: (direction: SplitDirection, newPaneName?: string) => {
      const sf = getActiveSurface();
      if (!sf) return;
      const target = sf.activePaneId ?? allLeafIds(sf.layout)[0];
      if (!target) return;
      const result = splitPane(sf.layout, target, direction);
      const trimmedName = (newPaneName ?? "").trim();
      updateSurface(sf.id, (s) => ({
        ...s,
        layout: result.tree,
        paneNames: buildPaneNames(allLeafIds(result.tree), {
          ...s.paneNames,
          [result.newPaneId]: trimmedName || s.paneNames[result.newPaneId] || `Pane ${allLeafIds(result.tree).length}`,
        }),
        activePaneId: result.newPaneId,
      }));
      set({ zoomedPaneId: null });
    },

    closePane: (paneId) => {
      const sf = getActiveSurface();
      if (!sf) return;
      const pair = findWsSurfaceAndPane(paneId);
      if (pair) {
        captureTranscriptForPane({
          paneId,
          workspaceId: pair.ws.id,
          surfaceId: pair.sf.id,
          cwd: pair.ws.cwd,
          reason: "pane-close",
        });
      }
      const newTree = removePane(sf.layout, paneId);
      if (newTree === null) {
        const leaf = createLeaf();
        updateSurface(sf.id, (s) => ({
          ...s,
          layout: leaf,
          paneNames: { [leaf.id]: "Pane 1" },
          activePaneId: leaf.id,
        }));
        stopPaneSessions([paneId]);
        return;
      }
      const remaining = allLeafIds(newTree);
      updateSurface(sf.id, (s) => ({
        ...s,
        layout: newTree,
        paneNames: buildPaneNames(remaining, s.paneNames),
        activePaneId:
          s.activePaneId === paneId
            ? remaining[0] ?? null
            : s.activePaneId,
      }));
      const { zoomedPaneId } = get();
      if (zoomedPaneId === paneId) set({ zoomedPaneId: null });
      stopPaneSessions([paneId]);
    },

    setActivePaneId: (paneId) => {
      const pair = findWsSurfaceAndPane(paneId);
      if (!pair) return;
      set((s) => ({
        activeWorkspaceId: pair.ws.id,
        ...activateWorkspaceBrowserState(s.workspaceBrowserState, pair.ws.id),
        workspaces: s.workspaces.map((workspace) => {
          if (workspace.id !== pair.ws.id) {
            return workspace;
          }

          return {
            ...workspace,
            activeSurfaceId: pair.sf.id,
            surfaces: workspace.surfaces.map((surface) =>
              surface.id === pair.sf.id
                ? { ...surface, activePaneId: paneId }
                : surface
            ),
          };
        }),
      }));
    },

    setPaneSessionId: (paneId, sessionId) => {
      const pair = findWsSurfaceAndPane(paneId);
      if (!pair) return;
      updateSurface(pair.sf.id, (s) => ({
        ...s,
        layout: setSessionId(s.layout, paneId, sessionId),
      }));
    },

    setPaneName: (paneId, name) => {
      const pair = findWsSurfaceAndPane(paneId);
      if (!pair) return;
      const nextName = name.trim();
      if (!nextName) return;
      updateSurface(pair.sf.id, (s) => ({
        ...s,
        paneNames: {
          ...s.paneNames,
          [paneId]: nextName,
        },
      }));
    },

    focusDirection: (direction: Direction) => {
      const sf = getActiveSurface();
      if (!sf || !sf.activePaneId) return;
      const next = findAdjacentPane(sf.layout, sf.activePaneId, direction);
      if (next) {
        updateSurface(sf.id, (s) => ({ ...s, activePaneId: next }));
      }
    },

    toggleZoom: () => {
      const sf = getActiveSurface();
      if (!sf) return;
      const { zoomedPaneId } = get();
      if (zoomedPaneId) {
        set({ zoomedPaneId: null });
      } else if (sf.activePaneId) {
        set({ zoomedPaneId: sf.activePaneId });
      }
    },

    applyPresetLayout: (preset: PresetLayout) => {
      const sf = getActiveSurface();
      if (!sf) return;

      const existingPanes = collectPaneSnapshots(sf.layout, sf.activePaneId);
      const layout = applyPaneSnapshots(buildPresetLayout(preset), existingPanes);
      const panes = allLeafIds(layout);

      updateSurface(sf.id, (s) => ({
        ...s,
        layout,
        paneNames: buildPaneNames(panes, s.paneNames),
        activePaneId: s.activePaneId && panes.includes(s.activePaneId)
          ? s.activePaneId
          : panes[0] ?? null,
      }));

      const orphanedPaneIds = existingPanes
        .map((pane) => pane.id)
        .filter((paneId) => !panes.includes(paneId));

      stopPaneSessions(orphanedPaneIds);
      set({ zoomedPaneId: null });
    },

    equalizeLayout: () => {
      const sf = getActiveSurface();
      if (!sf) return;
      updateSurface(sf.id, (s) => ({
        ...s,
        layout: equalizeLayoutRatios(s.layout),
      }));
    },

    // ===== UI panel toggles =====

    toggleSidebar: () => set((s) => ({ sidebarVisible: !s.sidebarVisible })),
    setSidebarWidth: (width) => set((state) => ({
      sidebarWidth: Math.max(180, Math.min(540, Math.round(width))),
      sidebarVisible: true,
      workspaces: state.workspaces,
    })),
    toggleCommandPalette: () =>
      set((s) => ({ commandPaletteOpen: !s.commandPaletteOpen })),
    toggleSearch: () => set((s) => ({ searchOpen: !s.searchOpen })),
    toggleSettings: () => set((s) => ({ settingsOpen: !s.settingsOpen })),
    toggleNotificationPanel: () =>
      set((s) => ({ notificationPanelOpen: !s.notificationPanelOpen })),
    toggleSessionVault: () =>
      set((s) => ({ sessionVaultOpen: !s.sessionVaultOpen })),
    toggleCommandLog: () =>
      set((s) => ({ commandLogOpen: !s.commandLogOpen })),
    toggleCommandHistory: () =>
      set((s) => ({ commandHistoryOpen: !s.commandHistoryOpen })),
    toggleSnippetPicker: () =>
      set((s) => ({ snippetPickerOpen: !s.snippetPickerOpen })),
    toggleAgentPanel: () =>
      set((s) => ({ agentPanelOpen: !s.agentPanelOpen })),
    toggleSystemMonitor: () =>
      set((s) => ({ systemMonitorOpen: !s.systemMonitorOpen })),
    toggleFileManager: () =>
      set((s) => ({ fileManagerOpen: !s.fileManagerOpen })),
    toggleCanvas: () =>
      set((s) => ({ canvasOpen: !s.canvasOpen })),
    toggleTimeTravel: () =>
      set((s) => ({ timeTravelOpen: !s.timeTravelOpen })),

    toggleWebBrowser: () =>
      set((s) => {
        const workspaceId = s.activeWorkspaceId;
        if (!workspaceId) return {};
        const current = browserStateForWorkspace(s.workspaceBrowserState, workspaceId);
        const nextBrowserState: WorkspaceBrowserState = {
          ...current,
          open: !current.open,
        };
        const nextWorkspaceBrowserState = {
          ...s.workspaceBrowserState,
          [workspaceId]: nextBrowserState,
        };
        return activateWorkspaceBrowserState(nextWorkspaceBrowserState, workspaceId);
      }),

    setWebBrowserOpen: (open) => set((s) => {
      const workspaceId = s.activeWorkspaceId;
      if (!workspaceId) return {};
      const current = browserStateForWorkspace(s.workspaceBrowserState, workspaceId);
      const nextBrowserState: WorkspaceBrowserState = {
        ...current,
        open,
      };
      const nextWorkspaceBrowserState = {
        ...s.workspaceBrowserState,
        [workspaceId]: nextBrowserState,
      };
      return activateWorkspaceBrowserState(nextWorkspaceBrowserState, workspaceId);
    }),

    navigateWebBrowser: (url) => {
      const normalized = normalizeBrowserUrl(url);
      set((s) => {
        const workspaceId = s.activeWorkspaceId;
        if (!workspaceId) return {};
        const current = browserStateForWorkspace(s.workspaceBrowserState, workspaceId);
        if (current.url === normalized) {
          const nextWorkspaceBrowserState = {
            ...s.workspaceBrowserState,
            [workspaceId]: { ...current, open: true },
          };
          return activateWorkspaceBrowserState(nextWorkspaceBrowserState, workspaceId);
        }
        const prefix = current.history.slice(0, current.historyIndex + 1);
        const history = [...prefix, normalized];
        const nextWorkspaceBrowserState = {
          ...s.workspaceBrowserState,
          [workspaceId]: {
            ...current,
            open: true,
            url: normalized,
            history,
            historyIndex: history.length - 1,
          },
        };
        return activateWorkspaceBrowserState(nextWorkspaceBrowserState, workspaceId);
      });
    },

    webBrowserBack: () => {
      set((s) => {
        const workspaceId = s.activeWorkspaceId;
        if (!workspaceId) return {};
        const current = browserStateForWorkspace(s.workspaceBrowserState, workspaceId);
        const nextIndex = Math.max(0, current.historyIndex - 1);
        const nextUrl = current.history[nextIndex] ?? current.url;
        const nextWorkspaceBrowserState = {
          ...s.workspaceBrowserState,
          [workspaceId]: {
            ...current,
            historyIndex: nextIndex,
            url: nextUrl,
          },
        };
        return activateWorkspaceBrowserState(nextWorkspaceBrowserState, workspaceId);
      });
    },

    webBrowserForward: () => {
      set((s) => {
        const workspaceId = s.activeWorkspaceId;
        if (!workspaceId) return {};
        const current = browserStateForWorkspace(s.workspaceBrowserState, workspaceId);
        const nextIndex = Math.min(current.history.length - 1, current.historyIndex + 1);
        const nextUrl = current.history[nextIndex] ?? current.url;
        const nextWorkspaceBrowserState = {
          ...s.workspaceBrowserState,
          [workspaceId]: {
            ...current,
            historyIndex: nextIndex,
            url: nextUrl,
          },
        };
        return activateWorkspaceBrowserState(nextWorkspaceBrowserState, workspaceId);
      });
    },

    webBrowserReload: () => {
      set((s) => {
        const workspaceId = s.activeWorkspaceId;
        if (!workspaceId) return {};
        const current = browserStateForWorkspace(s.workspaceBrowserState, workspaceId);
        const nextWorkspaceBrowserState = {
          ...s.workspaceBrowserState,
          [workspaceId]: {
            ...current,
            reloadToken: current.reloadToken + 1,
          },
        };
        return activateWorkspaceBrowserState(nextWorkspaceBrowserState, workspaceId);
      });
    },

    toggleWebBrowserFullscreen: () => {
      set((s) => {
        const workspaceId = s.activeWorkspaceId;
        if (!workspaceId) return {};
        const current = browserStateForWorkspace(s.workspaceBrowserState, workspaceId);
        const nextWorkspaceBrowserState = {
          ...s.workspaceBrowserState,
          [workspaceId]: {
            ...current,
            fullscreen: !current.fullscreen,
          },
        };
        return activateWorkspaceBrowserState(nextWorkspaceBrowserState, workspaceId);
      });
    },

    setWebBrowserFullscreen: (fullscreen) => {
      set((s) => {
        const workspaceId = s.activeWorkspaceId;
        if (!workspaceId) return {};
        const current = browserStateForWorkspace(s.workspaceBrowserState, workspaceId);
        const nextWorkspaceBrowserState = {
          ...s.workspaceBrowserState,
          [workspaceId]: {
            ...current,
            fullscreen,
          },
        };
        return activateWorkspaceBrowserState(nextWorkspaceBrowserState, workspaceId);
      });
    },

    updateNodeRatio: (paneId, newRatio) => {
      const sf = getActiveSurface();
      if (!sf) return;
      updateSurface(sf.id, (s) => ({
        ...s,
        layout: updateRatio(s.layout, paneId, newRatio),
      }));
    },

    hydrateSession: (session) => {
      const sessionWorkspaces = Array.isArray(session.workspaces)
        ? session.workspaces
        : [];
      const hydratedWorkspaceBrowserState: Record<WorkspaceId, WorkspaceBrowserState> = {};

      const hydratedWorkspaces = sessionWorkspaces.map((workspace, workspaceIndex) => {
        const workspaceId = typeof workspace.id === "string" && workspace.id
          ? workspace.id
          : newWorkspaceId();
        hydratedWorkspaceBrowserState[workspaceId] = normalizeWorkspaceBrowserState({
          open: Boolean(workspace.browser?.open),
          fullscreen: Boolean(workspace.browser?.fullscreen),
          url: typeof workspace.browser?.url === "string" ? workspace.browser.url : DEFAULT_WEB_BROWSER_URL,
          history: Array.isArray(workspace.browser?.history) ? workspace.browser.history : undefined,
          historyIndex: typeof workspace.browser?.historyIndex === "number" ? workspace.browser.historyIndex : undefined,
        });
        const sessionSurfaces = Array.isArray(workspace.surfaces) ? workspace.surfaces : [];
        const surfaces = sessionSurfaces.length > 0
          ? sessionSurfaces.map((surface, surfaceIndex) => {
            const fallbackPaneIds = [
              ...(typeof surface.activePaneId === "string" ? [surface.activePaneId] : []),
              ...(Array.isArray(surface.panes)
                ? surface.panes
                  .map((pane) => pane?.id)
                  .filter((paneId): paneId is string => typeof paneId === "string" && paneId.length > 0)
                : []),
            ];
            const layout = normalizeBspTree(surface.layout, fallbackPaneIds);
            const paneIds = allLeafIds(layout);
            const activePaneId = typeof surface.activePaneId === "string" && paneIds.includes(surface.activePaneId)
              ? surface.activePaneId
              : paneIds[0] ?? null;

            return {
              id: typeof surface.id === "string" && surface.id ? surface.id : newSurfaceId(),
              workspaceId,
              name: typeof surface.name === "string" && surface.name
                ? surface.name
                : `Surface ${surfaceIndex + 1}`,
              icon: typeof surface.icon === "string" && surface.icon ? surface.icon : "term",
              layout,
              paneNames: buildPaneNames(
                paneIds,
                typeof surface.paneNames === "object" && surface.paneNames
                  ? surface.paneNames as Record<PaneId, string>
                  : undefined,
              ),
              activePaneId,
              createdAt: Date.now(),
            };
          })
          : [createDefaultSurface(workspaceId)];

        const activeSurfaceId = typeof workspace.activeSurfaceId === "string"
          && surfaces.some((surface) => surface.id === workspace.activeSurfaceId)
          ? workspace.activeSurfaceId
          : surfaces[0]?.id ?? null;

        return {
          id: workspaceId,
          name: typeof workspace.name === "string" && workspace.name
            ? workspace.name
            : `Workspace ${workspaceIndex + 1}`,
          icon: typeof workspace.icon === "string" && workspace.icon ? workspace.icon : "terminal",
          accentColor: typeof workspace.accentColor === "string" && workspace.accentColor
            ? workspace.accentColor
            : pickAccent(workspaceIndex),
          cwd: typeof workspace.cwd === "string" ? workspace.cwd : "",
          gitBranch: null,
          gitDirty: false,
          listeningPorts: [],
          unreadCount: 0,
          surfaces,
          activeSurfaceId,
          createdAt: Date.now(),
        };
      });

      const normalizedWorkspaces = hydratedWorkspaces.length > 0
        ? hydratedWorkspaces
        : [createDefaultWorkspace()];

      for (const workspace of normalizedWorkspaces) {
        if (!hydratedWorkspaceBrowserState[workspace.id]) {
          hydratedWorkspaceBrowserState[workspace.id] = createDefaultWorkspaceBrowserState();
        }
      }

      for (const workspace of normalizedWorkspaces) {
        for (const surface of workspace.surfaces) {
          syncPaneIdCounter(surface.layout);
        }
      }

      syncWorkspaceAndSurfaceCounters(normalizedWorkspaces);

      const activeWorkspaceId = typeof session.activeWorkspaceId === "string"
        && normalizedWorkspaces.some((workspace) => workspace.id === session.activeWorkspaceId)
        ? session.activeWorkspaceId
        : normalizedWorkspaces[0]?.id ?? null;

      set((state) => ({
        ...state,
        workspaces: normalizedWorkspaces,
        activeWorkspaceId,
        ...activateWorkspaceBrowserState(hydratedWorkspaceBrowserState, activeWorkspaceId),
        sidebarVisible: typeof session.sidebarVisible === "boolean"
          ? session.sidebarVisible
          : state.sidebarVisible,
        sidebarWidth: typeof session.sidebarWidth === "number" && Number.isFinite(session.sidebarWidth)
          ? session.sidebarWidth
          : state.sidebarWidth,
        zoomedPaneId: null,
      }));
    },

    // ===== Derived helpers =====

    activeWorkspace: () => {
      const { workspaces, activeWorkspaceId } = get();
      return workspaces.find((w) => w.id === activeWorkspaceId);
    },

    activeSurface: () => getActiveSurface(),

    activePaneId: () => getActiveSurface()?.activePaneId ?? null,

    paneName: (paneId) => {
      const pair = findWsSurfaceAndPane(paneId);
      if (!pair) return null;
      return pair.sf.paneNames[paneId] ?? null;
    },
  };
});
