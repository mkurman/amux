import { create } from "zustand";
import {
  Workspace,
  Surface,
  WorkspaceId,
  SurfaceId,
  PaneId,
  SurfaceLayoutMode,
  CanvasPanel,
  CanvasState,
  CanvasViewSnapshot,
  CanvasPanelStatus,
  PersistedCanvasPanel,
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
import { normalizeIconId } from "./iconRegistry";

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

const CANVAS_MIN_ZOOM = 0.04;
const CANVAS_MAX_ZOOM = 2.2;
const CANVAS_GRID_SIZE = 32;
const DEFAULT_CANVAS_PANEL_WIDTH = 760;
const DEFAULT_CANVAS_PANEL_HEIGHT = 440;
const CANVAS_AUTO_GAP_X = 48;
const CANVAS_AUTO_GAP_Y = 40;

function snapCanvasCoord(value: number): number {
  return Math.round(value / CANVAS_GRID_SIZE) * CANVAS_GRID_SIZE;
}

function createDefaultCanvasState(): CanvasState {
  return {
    panX: 0,
    panY: 0,
    zoomLevel: 1,
    previousView: null,
    focusRequestNonce: 0,
  };
}

function sanitizeCanvasState(value: Partial<CanvasState> | undefined): CanvasState {
  const zoom = typeof value?.zoomLevel === "number" ? value.zoomLevel : 1;
  const previousView = value?.previousView && typeof value.previousView === "object"
    ? {
      panX: Number.isFinite(value.previousView.panX) ? value.previousView.panX : 0,
      panY: Number.isFinite(value.previousView.panY) ? value.previousView.panY : 0,
      zoomLevel: Number.isFinite(value.previousView.zoomLevel)
        ? Math.max(CANVAS_MIN_ZOOM, Math.min(CANVAS_MAX_ZOOM, value.previousView.zoomLevel))
        : 1,
    }
    : null;

  return {
    panX: Number.isFinite(value?.panX) ? Number(value?.panX) : 0,
    panY: Number.isFinite(value?.panY) ? Number(value?.panY) : 0,
    zoomLevel: Math.max(CANVAS_MIN_ZOOM, Math.min(CANVAS_MAX_ZOOM, Number.isFinite(zoom) ? zoom : 1)),
    previousView,
    focusRequestNonce: Number.isFinite(value?.focusRequestNonce)
      ? Math.max(0, Math.floor(Number(value?.focusRequestNonce)))
      : 0,
  };
}

function defaultCanvasPanelPosition(index: number): { x: number; y: number } {
  const col = index % 3;
  const row = Math.floor(index / 3);
  return {
    x: snapCanvasCoord(80 + col * (DEFAULT_CANVAS_PANEL_WIDTH + 48)),
    y: snapCanvasCoord(60 + row * (DEFAULT_CANVAS_PANEL_HEIGHT + 48)),
  };
}

function buildCanvasPanel(opts: {
  paneId: string;
  paneName?: string;
  index: number;
  persisted?: Partial<PersistedCanvasPanel>;
  status?: CanvasPanelStatus;
}): CanvasPanel {
  const fallbackPos = defaultCanvasPanelPosition(opts.index);
  return {
    id: typeof opts.persisted?.id === "string" && opts.persisted.id
      ? opts.persisted.id
      : `cp_${opts.paneId}`,
    paneId: opts.paneId,
    title: opts.paneName ?? `Pane ${opts.index + 1}`,
    icon: normalizeIconId(opts.persisted?.icon),
    x: Number.isFinite(opts.persisted?.x) ? Number(opts.persisted?.x) : fallbackPos.x,
    y: Number.isFinite(opts.persisted?.y) ? Number(opts.persisted?.y) : fallbackPos.y,
    width: Number.isFinite(opts.persisted?.width)
      ? Math.max(320, Number(opts.persisted?.width))
      : DEFAULT_CANVAS_PANEL_WIDTH,
    height: Number.isFinite(opts.persisted?.height)
      ? Math.max(220, Number(opts.persisted?.height))
      : DEFAULT_CANVAS_PANEL_HEIGHT,
    status: opts.status ?? opts.persisted?.status ?? "running",
    sessionId: typeof opts.persisted?.sessionId === "string" ? opts.persisted.sessionId : null,
    lastActivityAt: Number.isFinite(opts.persisted?.lastActivityAt)
      ? Number(opts.persisted?.lastActivityAt)
      : Date.now(),
  };
}

function isOverlappingPanel(
  panels: CanvasPanel[],
  candidate: { x: number; y: number; width: number; height: number }
): boolean {
  return panels.some((panel) => (
    candidate.x < panel.x + panel.width + 20
    && candidate.x + candidate.width + 20 > panel.x
    && candidate.y < panel.y + panel.height + 20
    && candidate.y + candidate.height + 20 > panel.y
  ));
}

function findCanvasPlacement(surface: Surface, anchorPaneId?: string | null): { x: number; y: number } {
  const anchor = surface.canvasPanels.find((panel) => panel.paneId === anchorPaneId)
    ?? surface.canvasPanels.find((panel) => panel.paneId === surface.activePaneId)
    ?? surface.canvasPanels[surface.canvasPanels.length - 1];
  const stepX = DEFAULT_CANVAS_PANEL_WIDTH + CANVAS_AUTO_GAP_X;
  const stepY = DEFAULT_CANVAS_PANEL_HEIGHT + CANVAS_AUTO_GAP_Y;
  const baseX = anchor ? anchor.x : 80;
  const baseY = anchor ? anchor.y : 60;
  const candidateSize = { width: DEFAULT_CANVAS_PANEL_WIDTH, height: DEFAULT_CANVAS_PANEL_HEIGHT };

  // Prefer appending to the right of the anchor on the same row first.
  for (let rowOffset = 0; rowOffset < 18; rowOffset += 1) {
    for (let colOffset = 1; colOffset < 18; colOffset += 1) {
      const candidate = {
        x: snapCanvasCoord(baseX + colOffset * stepX),
        y: snapCanvasCoord(baseY + rowOffset * stepY),
        ...candidateSize,
      };
      if (!isOverlappingPanel(surface.canvasPanels, candidate)) {
        return { x: candidate.x, y: candidate.y };
      }
    }
  }

  for (let rowOffset = 1; rowOffset < 18; rowOffset += 1) {
    for (let colOffset = 0; colOffset < 18; colOffset += 1) {
      const candidate = {
        x: snapCanvasCoord(baseX + colOffset * stepX),
        y: snapCanvasCoord(baseY - rowOffset * stepY),
        ...candidateSize,
      };
      if (!isOverlappingPanel(surface.canvasPanels, candidate)) {
        return { x: candidate.x, y: candidate.y };
      }
    }
  }

  return { x: snapCanvasCoord(baseX), y: snapCanvasCoord(baseY) };
}

function normalizeCanvasPanels(surface: Surface): Surface {
  if (surface.layoutMode !== "canvas") {
    return {
      ...surface,
      paneIcons: buildPaneIcons(allLeafIds(surface.layout), surface.paneIcons),
      canvasState: sanitizeCanvasState(surface.canvasState),
      canvasPanels: [],
    };
  }

  const paneIds = allLeafIds(surface.layout);
  const panelByPaneId = new Map(surface.canvasPanels.map((panel) => [panel.paneId, panel]));
  const canvasPanels = paneIds.map((paneId, index) => {
    const existing = panelByPaneId.get(paneId);
    const base = buildCanvasPanel({
      paneId,
      paneName: surface.paneNames[paneId],
      index,
      persisted: existing ?? undefined,
      status: existing?.status,
    });

    return {
      ...base,
      title: surface.paneNames[paneId] ?? base.title,
      icon: surface.paneIcons?.[paneId] ?? existing?.icon ?? base.icon,
      status: existing?.status ?? "running",
      sessionId: existing?.sessionId ?? findLeaf(surface.layout, paneId)?.sessionId ?? null,
    };
  });

  const activePaneId = surface.activePaneId && paneIds.includes(surface.activePaneId)
    ? surface.activePaneId
    : paneIds[0] ?? null;

  return {
    ...surface,
    activePaneId,
    paneIcons: buildPaneIcons(paneIds, surface.paneIcons),
    canvasState: sanitizeCanvasState(surface.canvasState),
    canvasPanels,
  };
}

function createDefaultSurface(workspaceId: WorkspaceId, layoutMode: SurfaceLayoutMode = "bsp"): Surface {
  const leaf = createLeaf();
  const paneNames = { [leaf.id]: "Pane 1" };
  const paneIcons = { [leaf.id]: "terminal" };
  const panel = buildCanvasPanel({
    paneId: leaf.id,
    paneName: paneNames[leaf.id],
    persisted: { icon: paneIcons[leaf.id] },
    index: 0,
    status: "running",
  });

  return {
    id: newSurfaceId(),
    workspaceId,
    name: layoutMode === "canvas" ? "Infinite Canvas" : "Terminal",
    icon: layoutMode === "canvas" ? "canvas" : "terminal",
    layoutMode,
    layout: leaf,
    paneNames,
    paneIcons,
    activePaneId: leaf.id,
    canvasState: createDefaultCanvasState(),
    canvasPanels: layoutMode === "canvas" ? [panel] : [],
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

function buildPaneIcons(paneIds: PaneId[], existing?: Record<PaneId, string>): Record<PaneId, string> {
  const icons: Record<PaneId, string> = {};
  for (const paneId of paneIds) {
    icons[paneId] = normalizeIconId(existing?.[paneId]);
  }
  return icons;
}

function createDefaultWorkspace(name?: string, layoutMode: SurfaceLayoutMode = "bsp"): Workspace {
  const id = newWorkspaceId();
  const surface = createDefaultSurface(id, layoutMode);
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

function stopPaneSessions(paneIds: string[], killSessions: boolean = true) {
  const amux = (window as any).tamux ?? (window as any).amux;
  if (!amux?.stopTerminalSession) return;

  for (const paneId of paneIds) {
    void amux.stopTerminalSession(paneId, killSessions);
  }
}

function resolvePaneSessionId(workspaces: Workspace[], paneId: PaneId): string | null {
  for (const workspace of workspaces) {
    for (const surface of workspace.surfaces) {
      const leaf = findLeaf(surface.layout, paneId);
      if (!leaf) continue;
      const panelSessionId = surface.canvasPanels.find((panel) => panel.paneId === paneId)?.sessionId ?? null;
      return panelSessionId ?? leaf.sessionId ?? null;
    }
  }
  return null;
}

function hasAnotherPaneForSession(
  workspaces: Workspace[],
  sessionId: string,
  excludingPaneId: PaneId,
): boolean {
  for (const workspace of workspaces) {
    for (const surface of workspace.surfaces) {
      const paneIds = allLeafIds(surface.layout);
      for (const paneId of paneIds) {
        if (paneId === excludingPaneId) {
          continue;
        }
        const leaf = findLeaf(surface.layout, paneId);
        if (!leaf) {
          continue;
        }
        const panelSessionId = surface.canvasPanels.find((panel) => panel.paneId === paneId)?.sessionId ?? null;
        const candidateSessionId = panelSessionId ?? leaf.sessionId ?? null;
        if (candidateSessionId === sessionId) {
          return true;
        }
      }
    }
  }
  return false;
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
  createWorkspace: (name?: string, opts?: { layoutMode?: SurfaceLayoutMode }) => void;
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
  createSurface: (workspaceId?: WorkspaceId, opts?: { layoutMode?: SurfaceLayoutMode }) => void;
  renameSurface: (surfaceId: SurfaceId, name: string) => void;
  setSurfaceIcon: (surfaceId: SurfaceId, icon: string) => void;
  closeSurface: (surfaceId: SurfaceId) => void;
  nextSurface: () => void;
  prevSurface: () => void;
  setActiveSurface: (surfaceId: SurfaceId) => void;

  // -- Pane actions --
  splitActive: (
    direction: SplitDirection,
    newPaneName?: string,
    opts?: { sessionId?: string | null; paneIcon?: string },
  ) => void;
  closePane: (paneId: PaneId, opts?: { stopSession?: boolean; captureTranscript?: boolean }) => void;
  setActivePaneId: (paneId: PaneId) => void;
  clearActivePaneFocus: (surfaceId?: SurfaceId) => void;
  setPaneSessionId: (paneId: PaneId, sessionId: string) => void;
  setPaneName: (paneId: PaneId, name: string) => void;
  setPaneIcon: (paneId: PaneId, icon: string) => void;
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
  createCanvasPanel: (
    surfaceId?: SurfaceId,
    opts?: {
      paneName?: string;
      paneIcon?: string;
      sessionId?: string | null;
      x?: number;
      y?: number;
      width?: number;
      height?: number;
    },
  ) => PaneId | null;
  moveCanvasPanel: (paneId: PaneId, x: number, y: number) => void;
  resizeCanvasPanel: (paneId: PaneId, width: number, height: number) => void;
  arrangeCanvasPanels: (surfaceId?: SurfaceId) => void;
  setCanvasView: (surfaceId: SurfaceId, view: Partial<CanvasViewSnapshot>) => void;
  setCanvasPreviousView: (surfaceId: SurfaceId, snapshot: CanvasViewSnapshot | null) => void;
  focusCanvasPanel: (paneId: PaneId, opts?: { storePreviousView?: boolean }) => void;
  clearCanvasPanelStatus: (paneId: PaneId) => void;
  setCanvasPanelStatus: (paneId: PaneId, status: CanvasPanelStatus) => void;
  setCanvasPanelIcon: (paneId: PaneId, icon: string) => void;
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

    createWorkspace: (name?: string, opts?: { layoutMode?: SurfaceLayoutMode }) => {
      const safeName = typeof name === "string" ? name.trim() : "";
      const layoutMode = opts?.layoutMode ?? "bsp";
      const ws = createDefaultWorkspace(safeName || undefined, layoutMode);
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
      const nextName = typeof name === "string" ? name.trim() : "";
      if (!nextName) return;
      set((s) => ({
        workspaces: s.workspaces.map((ws) =>
          ws.id === id ? { ...ws, name: nextName } : ws
        ),
      }));
    },

    setWorkspaceIcon: (id, icon) => {
      const nextIcon = normalizeIconId(icon);
      set((s) => ({
        workspaces: s.workspaces.map((ws) =>
          ws.id === id ? { ...ws, icon: nextIcon } : ws,
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

    createSurface: (workspaceId?: WorkspaceId, opts?: { layoutMode?: SurfaceLayoutMode }) => {
      const wsId = workspaceId ?? get().activeWorkspaceId;
      if (!wsId) return;
      const layoutMode = opts?.layoutMode ?? "bsp";
      const sf = createDefaultSurface(wsId, layoutMode);
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
      const nextName = typeof name === "string" ? name.trim() : "";
      if (!nextName) return;
      updateSurface(surfaceId, (sf) => ({ ...sf, name: nextName }));
    },

    setSurfaceIcon: (surfaceId, icon) => {
      const nextIcon = normalizeIconId(icon);
      updateSurface(surfaceId, (sf) => ({ ...sf, icon: nextIcon }));
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
        zoomedPaneId: null,
        ...activateWorkspaceBrowserState(s.workspaceBrowserState, pair.ws.id),
        workspaces: s.workspaces.map((w) =>
          w.id === pair.ws.id ? { ...w, activeSurfaceId: surfaceId } : w
        ),
      }));
    },

    // ===== Pane actions =====

    splitActive: (direction: SplitDirection, newPaneName?: string, opts?: { sessionId?: string | null; paneIcon?: string }) => {
      const sf = getActiveSurface();
      if (!sf) return;
      if (sf.layoutMode === "canvas") {
        get().createCanvasPanel(sf.id, {
          paneName: newPaneName,
          paneIcon: opts?.paneIcon,
          sessionId: opts?.sessionId ?? null,
        });
        return;
      }
      const target = sf.activePaneId ?? allLeafIds(sf.layout)[0];
      if (!target) return;
      const result = splitPane(sf.layout, target, direction);
      const layout = typeof opts?.sessionId === "string" && opts.sessionId
        ? setSessionId(result.tree, result.newPaneId, opts.sessionId)
        : result.tree;
      const trimmedName = (newPaneName ?? "").trim();
      updateSurface(sf.id, (s) => ({
        ...s,
        layout,
        paneNames: buildPaneNames(allLeafIds(layout), {
          ...s.paneNames,
          [result.newPaneId]: trimmedName || s.paneNames[result.newPaneId] || `Pane ${allLeafIds(layout).length}`,
        }),
        paneIcons: buildPaneIcons(allLeafIds(layout), {
          ...s.paneIcons,
          [result.newPaneId]: normalizeIconId(opts?.paneIcon ?? s.paneIcons[result.newPaneId] ?? "terminal"),
        }),
        activePaneId: result.newPaneId,
      }));
      set({ zoomedPaneId: null });
    },

    closePane: (paneId, opts) => {
      let shouldStopSession = opts?.stopSession !== false;
      const shouldCaptureTranscript = opts?.captureTranscript !== false;
      if (shouldStopSession) {
        const { workspaces } = get();
        const sessionId = resolvePaneSessionId(workspaces, paneId);
        if (sessionId && hasAnotherPaneForSession(workspaces, sessionId, paneId)) {
          shouldStopSession = false;
        }
      }
      const sf = getActiveSurface();
      if (!sf) return;
      if (sf.layoutMode === "canvas") {
        const pair = findWsSurfaceAndPane(paneId);
        if (pair && shouldCaptureTranscript) {
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
        updateSurface(sf.id, (s) => normalizeCanvasPanels({
          ...s,
          layout: leaf,
          paneNames: { [leaf.id]: "Pane 1" },
          paneIcons: { [leaf.id]: "terminal" },
          activePaneId: leaf.id,
          canvasPanels: [
            buildCanvasPanel({
              paneId: leaf.id,
              paneName: "Pane 1",
              index: 0,
              persisted: { icon: "terminal" },
              status: "idle",
            }),
          ],
        }));
          stopPaneSessions([paneId], shouldStopSession);
          return;
        }

        const remaining = allLeafIds(newTree);
      updateSurface(sf.id, (s) => normalizeCanvasPanels({
        ...s,
        layout: newTree,
        paneNames: buildPaneNames(remaining, s.paneNames),
        paneIcons: buildPaneIcons(remaining, s.paneIcons),
        activePaneId: s.activePaneId === paneId ? remaining[0] ?? null : s.activePaneId,
        canvasPanels: s.canvasPanels.filter((panel) => panel.paneId !== paneId),
      }));
        stopPaneSessions([paneId], shouldStopSession);
        return;
      }
      const pair = findWsSurfaceAndPane(paneId);
      if (pair && shouldCaptureTranscript) {
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
          paneIcons: { [leaf.id]: "terminal" },
          activePaneId: leaf.id,
        }));
        stopPaneSessions([paneId], shouldStopSession);
        return;
      }
      const remaining = allLeafIds(newTree);
      updateSurface(sf.id, (s) => ({
        ...s,
        layout: newTree,
        paneNames: buildPaneNames(remaining, s.paneNames),
        paneIcons: buildPaneIcons(remaining, s.paneIcons),
        activePaneId:
          s.activePaneId === paneId
            ? remaining[0] ?? null
            : s.activePaneId,
      }));
      const { zoomedPaneId } = get();
      if (zoomedPaneId === paneId) set({ zoomedPaneId: null });
      stopPaneSessions([paneId], shouldStopSession);
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

    clearActivePaneFocus: (surfaceId) => {
      const targetSurface = surfaceId
        ? findWsAndSurface(surfaceId)?.sf
        : getActiveSurface();
      if (!targetSurface) return;

      updateSurface(targetSurface.id, (surface) => ({
        ...surface,
        activePaneId: null,
      }));
    },

    setPaneSessionId: (paneId, sessionId) => {
      const pair = findWsSurfaceAndPane(paneId);
      if (!pair) return;
      if (pair.sf.layoutMode === "canvas") {
        updateSurface(pair.sf.id, (s) => normalizeCanvasPanels({
          ...s,
          layout: setSessionId(s.layout, paneId, sessionId),
          canvasPanels: s.canvasPanels.map((panel) =>
            panel.paneId === paneId
              ? { ...panel, sessionId, status: "running", lastActivityAt: Date.now() }
              : panel
          ),
        }));
        return;
      }
      updateSurface(pair.sf.id, (s) => ({
        ...s,
        layout: setSessionId(s.layout, paneId, sessionId),
      }));
    },

    setPaneName: (paneId, name) => {
      const pair = findWsSurfaceAndPane(paneId);
      if (!pair) return;
      const nextName = typeof name === "string" ? name.trim() : "";
      if (!nextName) return;
      if (pair.sf.layoutMode === "canvas") {
        updateSurface(pair.sf.id, (s) => normalizeCanvasPanels({
          ...s,
          paneNames: {
            ...s.paneNames,
            [paneId]: nextName,
          },
          canvasPanels: s.canvasPanels.map((panel) =>
            panel.paneId === paneId
              ? { ...panel, title: nextName }
              : panel
          ),
        }));
        return;
      }
      updateSurface(pair.sf.id, (s) => ({
        ...s,
        paneNames: {
          ...s.paneNames,
          [paneId]: nextName,
        },
      }));
    },

    setPaneIcon: (paneId, icon) => {
      const pair = findWsSurfaceAndPane(paneId);
      if (!pair) return;
      const nextIcon = normalizeIconId(icon);

      if (pair.sf.layoutMode === "canvas") {
        updateSurface(pair.sf.id, (s) => normalizeCanvasPanels({
          ...s,
          paneIcons: {
            ...s.paneIcons,
            [paneId]: nextIcon,
          },
          canvasPanels: s.canvasPanels.map((panel) =>
            panel.paneId === paneId ? { ...panel, icon: nextIcon } : panel
          ),
        }));
        return;
      }

      updateSurface(pair.sf.id, (s) => ({
        ...s,
        paneIcons: {
          ...s.paneIcons,
          [paneId]: nextIcon,
        },
      }));
    },

    focusDirection: (direction: Direction) => {
      const sf = getActiveSurface();
      if (!sf || !sf.activePaneId) return;
      if (sf.layoutMode === "canvas") {
        const activePanel = sf.canvasPanels.find((panel) => panel.paneId === sf.activePaneId);
        if (!activePanel) return;
        const activeCenterX = activePanel.x + activePanel.width / 2;
        const activeCenterY = activePanel.y + activePanel.height / 2;

        const candidate = sf.canvasPanels
          .filter((panel) => panel.paneId !== sf.activePaneId)
          .map((panel) => {
            const centerX = panel.x + panel.width / 2;
            const centerY = panel.y + panel.height / 2;
            const dx = centerX - activeCenterX;
            const dy = centerY - activeCenterY;
            const isEligible = direction === "left"
              ? dx < 0
              : direction === "right"
                ? dx > 0
                : direction === "up"
                  ? dy < 0
                  : dy > 0;
            if (!isEligible) return null;
            const primary = direction === "left" || direction === "right" ? Math.abs(dx) : Math.abs(dy);
            const secondary = direction === "left" || direction === "right" ? Math.abs(dy) : Math.abs(dx);
            return {
              paneId: panel.paneId,
              score: primary * 10 + secondary,
            };
          })
          .filter((entry): entry is { paneId: string; score: number } => Boolean(entry))
          .sort((a, b) => a.score - b.score)[0];

        if (candidate) {
          updateSurface(sf.id, (s) => ({ ...s, activePaneId: candidate.paneId }));
        }
        return;
      }
      const next = findAdjacentPane(sf.layout, sf.activePaneId, direction);
      if (next) {
        updateSurface(sf.id, (s) => ({ ...s, activePaneId: next }));
      }
    },

    toggleZoom: () => {
      const sf = getActiveSurface();
      if (!sf) return;
      if (sf.layoutMode === "canvas") {
        return;
      }
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
      if (sf.layoutMode === "canvas") return;

      const existingPanes = collectPaneSnapshots(sf.layout, sf.activePaneId);
      const layout = applyPaneSnapshots(buildPresetLayout(preset), existingPanes);
      const panes = allLeafIds(layout);

      updateSurface(sf.id, (s) => ({
        ...s,
        layout,
        paneNames: buildPaneNames(panes, s.paneNames),
        paneIcons: buildPaneIcons(panes, s.paneIcons),
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
      if (sf.layoutMode === "canvas") return;
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
      if (sf.layoutMode === "canvas") return;
      updateSurface(sf.id, (s) => ({
        ...s,
        layout: updateRatio(s.layout, paneId, newRatio),
      }));
    },

    createCanvasPanel: (surfaceId, opts) => {
      const surface = surfaceId
        ? findWsAndSurface(surfaceId)?.sf
        : getActiveSurface();
      if (!surface || surface.layoutMode !== "canvas") return null;
      const targetPaneId = surface.activePaneId ?? allLeafIds(surface.layout)[0];
      if (!targetPaneId) return null;
      const split = splitPane(surface.layout, targetPaneId, "horizontal");
      const nextLayout = typeof opts?.sessionId === "string" && opts.sessionId
        ? setSessionId(split.tree, split.newPaneId, opts.sessionId)
        : split.tree;
      const placement = findCanvasPlacement(surface, targetPaneId);
      const requestedName = typeof opts?.paneName === "string" ? opts.paneName.trim() : "";
      const nextIcon = normalizeIconId(opts?.paneIcon ?? "terminal");
      const persistedSessionId = typeof opts?.sessionId === "string" && opts.sessionId
        ? opts.sessionId
        : undefined;

      updateSurface(surface.id, (s) => {
        const paneIds = allLeafIds(nextLayout);
        const paneNames = buildPaneNames(paneIds, {
          ...s.paneNames,
          [split.newPaneId]: requestedName || `Pane ${paneIds.length}`,
        });
        const paneIcons = buildPaneIcons(paneIds, {
          ...s.paneIcons,
          [split.newPaneId]: nextIcon,
        });
        const nextPanels = [
          ...s.canvasPanels,
          buildCanvasPanel({
            paneId: split.newPaneId,
            paneName: paneNames[split.newPaneId],
            index: s.canvasPanels.length,
            persisted: {
              x: Number.isFinite(opts?.x) ? Number(opts?.x) : placement.x,
              y: Number.isFinite(opts?.y) ? Number(opts?.y) : placement.y,
              width: Number.isFinite(opts?.width) ? Number(opts?.width) : DEFAULT_CANVAS_PANEL_WIDTH,
              height: Number.isFinite(opts?.height) ? Number(opts?.height) : DEFAULT_CANVAS_PANEL_HEIGHT,
              icon: paneIcons[split.newPaneId],
              ...(persistedSessionId ? { sessionId: persistedSessionId } : {}),
            },
            status: persistedSessionId ? "running" : "idle",
          }),
        ];

        return normalizeCanvasPanels({
          ...s,
          layout: nextLayout,
          paneNames,
          paneIcons,
          activePaneId: split.newPaneId,
          canvasPanels: nextPanels,
        });
      });

      return split.newPaneId;
    },

    moveCanvasPanel: (paneId, x, y) => {
      const pair = findWsSurfaceAndPane(paneId);
      if (!pair || pair.sf.layoutMode !== "canvas") return;
      updateSurface(pair.sf.id, (s) => normalizeCanvasPanels({
        ...s,
        canvasPanels: s.canvasPanels.map((panel) =>
          panel.paneId === paneId
            ? { ...panel, x, y, lastActivityAt: Date.now() }
            : panel
        ),
      }));
    },

    resizeCanvasPanel: (paneId, width, height) => {
      const pair = findWsSurfaceAndPane(paneId);
      if (!pair || pair.sf.layoutMode !== "canvas") return;
      updateSurface(pair.sf.id, (s) => normalizeCanvasPanels({
        ...s,
        canvasPanels: s.canvasPanels.map((panel) =>
          panel.paneId === paneId
            ? {
              ...panel,
              width: Math.max(320, width),
              height: Math.max(220, height),
              lastActivityAt: Date.now(),
            }
            : panel
        ),
      }));
    },

    arrangeCanvasPanels: (surfaceId) => {
      const surface = surfaceId
        ? findWsAndSurface(surfaceId)?.sf
        : getActiveSurface();
      if (!surface || surface.layoutMode !== "canvas" || surface.canvasPanels.length === 0) {
        return;
      }

      const panels = surface.canvasPanels;
      const maxWidth = Math.max(...panels.map((panel) => Math.max(320, panel.width)));
      const maxHeight = Math.max(...panels.map((panel) => Math.max(220, panel.height)));
      const columnCount = Math.max(1, Math.min(5, Math.round(Math.sqrt(panels.length * 1.4))));
      const stepX = maxWidth + CANVAS_AUTO_GAP_X;
      const stepY = maxHeight + CANVAS_AUTO_GAP_Y;
      const ordered = [
        ...panels.filter((panel) => panel.paneId === surface.activePaneId),
        ...panels.filter((panel) => panel.paneId !== surface.activePaneId),
      ];
      const positions = new Map<string, { x: number; y: number }>();

      ordered.forEach((panel, index) => {
        const row = Math.floor(index / columnCount);
        const col = index % columnCount;
        positions.set(panel.paneId, {
          x: snapCanvasCoord(80 + col * stepX),
          y: snapCanvasCoord(60 + row * stepY),
        });
      });

      updateSurface(surface.id, (s) => normalizeCanvasPanels({
        ...s,
        canvasPanels: s.canvasPanels.map((panel) => {
          const position = positions.get(panel.paneId);
          if (!position) {
            return panel;
          }
          return {
            ...panel,
            x: position.x,
            y: position.y,
            lastActivityAt: Date.now(),
          };
        }),
      }));
    },

    setCanvasView: (surfaceId, view) => {
      updateSurface(surfaceId, (surface) => {
        if (surface.layoutMode !== "canvas") return surface;
        return normalizeCanvasPanels({
          ...surface,
          canvasState: sanitizeCanvasState({
            ...surface.canvasState,
            ...view,
          }),
        });
      });
    },

    setCanvasPreviousView: (surfaceId, snapshot) => {
      updateSurface(surfaceId, (surface) => {
        if (surface.layoutMode !== "canvas") return surface;
        const normalizedSnapshot = snapshot
          ? {
            panX: Number.isFinite(snapshot.panX) ? snapshot.panX : 0,
            panY: Number.isFinite(snapshot.panY) ? snapshot.panY : 0,
            zoomLevel: Math.max(CANVAS_MIN_ZOOM, Math.min(CANVAS_MAX_ZOOM, snapshot.zoomLevel)),
          }
          : null;
        return {
          ...surface,
          canvasState: {
            ...surface.canvasState,
            previousView: normalizedSnapshot,
          },
        };
      });
    },

    focusCanvasPanel: (paneId, opts) => {
      const pair = findWsSurfaceAndPane(paneId);
      if (!pair || pair.sf.layoutMode !== "canvas") return;
      const nextNonce = (pair.sf.canvasState.focusRequestNonce ?? 0) + 1;
      const previous = opts?.storePreviousView !== false
        ? {
          panX: pair.sf.canvasState.panX,
          panY: pair.sf.canvasState.panY,
          zoomLevel: pair.sf.canvasState.zoomLevel,
        }
        : pair.sf.canvasState.previousView;

      updateSurface(pair.sf.id, (surface) => normalizeCanvasPanels({
        ...surface,
        activePaneId: paneId,
        canvasState: sanitizeCanvasState({
          ...surface.canvasState,
          previousView: previous,
          focusRequestNonce: nextNonce,
        }),
      }));
    },

    clearCanvasPanelStatus: (paneId) => {
      const pair = findWsSurfaceAndPane(paneId);
      if (!pair || pair.sf.layoutMode !== "canvas") return;
      updateSurface(pair.sf.id, (surface) => normalizeCanvasPanels({
        ...surface,
        canvasPanels: surface.canvasPanels.map((panel) =>
          panel.paneId === paneId
            ? { ...panel, status: "running", lastActivityAt: Date.now() }
            : panel
        ),
      }));
    },

    setCanvasPanelStatus: (paneId, status) => {
      const pair = findWsSurfaceAndPane(paneId);
      if (!pair || pair.sf.layoutMode !== "canvas") return;
      updateSurface(pair.sf.id, (surface) => normalizeCanvasPanels({
        ...surface,
        canvasPanels: surface.canvasPanels.map((panel) =>
          panel.paneId === paneId
            ? { ...panel, status, lastActivityAt: Date.now() }
            : panel
        ),
      }));
    },

    setCanvasPanelIcon: (paneId, icon) => {
      const pair = findWsSurfaceAndPane(paneId);
      if (!pair || pair.sf.layoutMode !== "canvas") return;
      const nextIcon = normalizeIconId(icon);
      updateSurface(pair.sf.id, (surface) => normalizeCanvasPanels({
        ...surface,
        paneIcons: {
          ...surface.paneIcons,
          [paneId]: nextIcon,
        },
        canvasPanels: surface.canvasPanels.map((panel) =>
          panel.paneId === paneId ? { ...panel, icon: nextIcon } : panel
        ),
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
            const layoutMode: SurfaceLayoutMode = surface.layoutMode === "canvas" ? "canvas" : "bsp";
            const fallbackPaneIds = [
              ...(typeof surface.activePaneId === "string" ? [surface.activePaneId] : []),
              ...(Array.isArray(surface.panes)
                ? surface.panes
                  .map((pane) => pane?.id)
                  .filter((paneId): paneId is string => typeof paneId === "string" && paneId.length > 0)
                : []),
              ...(Array.isArray(surface.canvasPanels)
                ? surface.canvasPanels
                  .map((panel) => panel?.paneId)
                  .filter((paneId): paneId is string => typeof paneId === "string" && paneId.length > 0)
                : []),
            ];
            const layout = normalizeBspTree(surface.layout, fallbackPaneIds);
            const paneIds = allLeafIds(layout);
            const activePaneId = typeof surface.activePaneId === "string" && paneIds.includes(surface.activePaneId)
              ? surface.activePaneId
              : paneIds[0] ?? null;
            const paneNames = buildPaneNames(
              paneIds,
              typeof surface.paneNames === "object" && surface.paneNames
                ? surface.paneNames as Record<PaneId, string>
                : undefined,
            );
            const paneIcons = buildPaneIcons(
              paneIds,
              typeof surface.paneIcons === "object" && surface.paneIcons
                ? surface.paneIcons as Record<PaneId, string>
                : undefined,
            );
            const canvasState = sanitizeCanvasState(surface.canvasState);
            const persistedPanelByPane = new Map(
              Array.isArray(surface.canvasPanels)
                ? surface.canvasPanels
                  .filter((panel): panel is PersistedCanvasPanel => Boolean(panel?.paneId))
                  .map((panel) => [panel.paneId, panel])
                : []
            );
            const canvasPanels = layoutMode === "canvas"
              ? paneIds.map((paneId, index) => buildCanvasPanel({
                paneId,
                paneName: paneNames[paneId],
                index,
                persisted: persistedPanelByPane.get(paneId),
                status: persistedPanelByPane.get(paneId)?.status,
              }))
              : [];

            const hydratedSurface: Surface = {
              id: typeof surface.id === "string" && surface.id ? surface.id : newSurfaceId(),
              workspaceId,
              name: typeof surface.name === "string" && surface.name
                ? surface.name
                : `Surface ${surfaceIndex + 1}`,
              icon: normalizeIconId(surface.icon),
              layoutMode,
              layout,
              paneNames,
              paneIcons,
              activePaneId,
              canvasState,
              canvasPanels,
              createdAt: Date.now(),
            };

            return normalizeCanvasPanels(hydratedSurface);
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
          icon: normalizeIconId(workspace.icon),
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
