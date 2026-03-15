import { useCallback, useEffect, useMemo, useRef, useState, type CSSProperties } from "react";
import { allLeafIds, findLeaf } from "../lib/bspTree";
import { iconChoices, iconGlyph, normalizeIconId, PANE_ICON_IDS, WORKSPACE_ICON_IDS } from "../lib/iconRegistry";
import type { Surface } from "../lib/types";
import { useAgentMissionStore } from "../lib/agentMissionStore";
import { useNotificationStore } from "../lib/notificationStore";
import {
  cloneSessionForDuplication,
  queuePaneBootstrapCommand,
  resolveDuplicateActiveBootstrapCommand,
  resolveDuplicateBootstrapCommand,
  resolveDuplicateSourceSessionId,
} from "../lib/paneDuplication";
import { useWorkspaceStore } from "../lib/workspaceStore";
import { AppConfirmDialog } from "./AppConfirmDialog";
import { SidebarActions } from "./sidebar/SidebarActions";
import { SidebarHeader } from "./sidebar/SidebarHeader";
import { SidebarResizeHandle } from "./sidebar/SidebarResizeHandle";

type TreeContextMenu =
  | {
    kind: "workspace";
    workspaceId: string;
    x: number;
    y: number;
  }
  | {
    kind: "surface";
    workspaceId: string;
    surfaceId: string;
    x: number;
    y: number;
  }
  | {
    kind: "pane";
    workspaceId: string;
    surfaceId: string;
    paneId: string;
    x: number;
    y: number;
  };

type IconPickerState =
  | {
    kind: "workspace";
    workspaceId: string;
    x: number;
    y: number;
  }
  | {
    kind: "pane";
    paneIds: string[];
    x: number;
    y: number;
  };

type ConfirmDialogState = {
  title: string;
  message: string;
  confirmLabel: string;
  tone: "danger" | "warning" | "neutral";
  action: () => void;
};

export function Sidebar() {
  const sidebarWidth = useWorkspaceStore((s) => s.sidebarWidth);
  const workspaces = useWorkspaceStore((s) => s.workspaces);
  const activeWorkspaceId = useWorkspaceStore((s) => s.activeWorkspaceId);
  const setActiveWorkspace = useWorkspaceStore((s) => s.setActiveWorkspace);
  const createWorkspace = useWorkspaceStore((s) => s.createWorkspace);
  const closeWorkspace = useWorkspaceStore((s) => s.closeWorkspace);
  const renameWorkspace = useWorkspaceStore((s) => s.renameWorkspace);
  const setWorkspaceIcon = useWorkspaceStore((s) => s.setWorkspaceIcon);
  const createSurface = useWorkspaceStore((s) => s.createSurface);
  const createCanvasPanel = useWorkspaceStore((s) => s.createCanvasPanel);
  const setActiveSurface = useWorkspaceStore((s) => s.setActiveSurface);
  const closeSurface = useWorkspaceStore((s) => s.closeSurface);
  const closePane = useWorkspaceStore((s) => s.closePane);
  const setActivePaneId = useWorkspaceStore((s) => s.setActivePaneId);
  const setPaneName = useWorkspaceStore((s) => s.setPaneName);
  const setPaneIcon = useWorkspaceStore((s) => s.setPaneIcon);
  const splitActive = useWorkspaceStore((s) => s.splitActive);
  const focusCanvasPanel = useWorkspaceStore((s) => s.focusCanvasPanel);
  const setSidebarWidth = useWorkspaceStore((s) => s.setSidebarWidth);
  const toggleAgentPanel = useWorkspaceStore((s) => s.toggleAgentPanel);
  const toggleSystemMonitor = useWorkspaceStore((s) => s.toggleSystemMonitor);
  const toggleCommandPalette = useWorkspaceStore((s) => s.toggleCommandPalette);
  const toggleSearch = useWorkspaceStore((s) => s.toggleSearch);
  const toggleCommandHistory = useWorkspaceStore((s) => s.toggleCommandHistory);
  const toggleCommandLog = useWorkspaceStore((s) => s.toggleCommandLog);
  const toggleFileManager = useWorkspaceStore((s) => s.toggleFileManager);
  const toggleSessionVault = useWorkspaceStore((s) => s.toggleSessionVault);
  const toggleSettings = useWorkspaceStore((s) => s.toggleSettings);
  const getUnread = useNotificationStore((s) => s.getUnreadForWorkspace);
  const notifications = useNotificationStore((s) => s.notifications);
  const approvals = useAgentMissionStore((s) => s.approvals);
  const cognitiveEvents = useAgentMissionStore((s) => s.cognitiveEvents);
  const operationalEvents = useAgentMissionStore((s) => s.operationalEvents);

  const [query, setQuery] = useState("");
  const [collapsedWorkspaces, setCollapsedWorkspaces] = useState<Record<string, boolean>>({});
  const [collapsedSurfaces, setCollapsedSurfaces] = useState<Record<string, boolean>>({});
  const [editingWorkspaceId, setEditingWorkspaceId] = useState<string | null>(null);
  const [workspaceNameDraft, setWorkspaceNameDraft] = useState("");
  const [editingPaneId, setEditingPaneId] = useState<string | null>(null);
  const [paneNameDraft, setPaneNameDraft] = useState("");
  const [contextMenu, setContextMenu] = useState<TreeContextMenu | null>(null);
  const [iconPicker, setIconPicker] = useState<IconPickerState | null>(null);
  const [confirmDialog, setConfirmDialog] = useState<ConfirmDialogState | null>(null);
  const [selectionWorkspaceId, setSelectionWorkspaceId] = useState<string | null>(null);
  const [selectedPaneIds, setSelectedPaneIds] = useState<string[]>([]);
  const [selectedSurfaceIds, setSelectedSurfaceIds] = useState<string[]>([]);
  const [paneSelectionAnchor, setPaneSelectionAnchor] = useState<string | null>(null);
  const [surfaceSelectionAnchor, setSurfaceSelectionAnchor] = useState<string | null>(null);

  const sidebarRef = useRef<HTMLDivElement>(null);
  const contextMenuRef = useRef<HTMLDivElement | null>(null);
  const iconPickerRef = useRef<HTMLDivElement | null>(null);

  useEffect(() => {
    let resizing = false;

    const handlePointerMove = (event: PointerEvent) => {
      if (!resizing) return;
      const sidebarLeft = sidebarRef.current?.getBoundingClientRect().left ?? 0;
      setSidebarWidth(event.clientX - sidebarLeft);
    };

    const handlePointerUp = () => {
      resizing = false;
      document.body.style.cursor = "";
      document.body.style.userSelect = "";
    };

    window.addEventListener("pointermove", handlePointerMove);
    window.addEventListener("pointerup", handlePointerUp);

    const startResize = () => {
      resizing = true;
      document.body.style.cursor = "col-resize";
      document.body.style.userSelect = "none";
    };

    const handle = sidebarRef.current?.querySelector("[data-sidebar-resize-handle='true']");
    handle?.addEventListener("pointerdown", startResize);

    return () => {
      handle?.removeEventListener("pointerdown", startResize);
      window.removeEventListener("pointermove", handlePointerMove);
      window.removeEventListener("pointerup", handlePointerUp);
      document.body.style.cursor = "";
      document.body.style.userSelect = "";
    };
  }, [setSidebarWidth]);

  useEffect(() => {
    if (!contextMenu) return;

    const closeMenu = (event: MouseEvent) => {
      if (contextMenuRef.current?.contains(event.target as Node)) {
        return;
      }
      setContextMenu(null);
    };

    window.addEventListener("mousedown", closeMenu);
    return () => window.removeEventListener("mousedown", closeMenu);
  }, [contextMenu]);

  useEffect(() => {
    if (!iconPicker) return;
    const close = (event: MouseEvent) => {
      if (iconPickerRef.current?.contains(event.target as Node)) {
        return;
      }
      setIconPicker(null);
    };
    window.addEventListener("mousedown", close);
    return () => window.removeEventListener("mousedown", close);
  }, [iconPicker]);

  const attentionByPane = useMemo(() => {
    const map = new Map<string, number>();

    for (const notification of notifications) {
      if (notification.isRead) continue;
      const paneId = notification.panelId ?? notification.paneId;
      if (!paneId) continue;
      map.set(paneId, (map.get(paneId) ?? 0) + 1);
    }

    for (const approval of approvals) {
      if (approval.status === "pending" && approval.handledAt === null) {
        map.set(approval.paneId, (map.get(approval.paneId) ?? 0) + 1);
      }
    }
    return map;
  }, [approvals, notifications]);

  const filteredWorkspaces = useMemo(() => {
    const lower = query.trim().toLowerCase();
    if (!lower) return workspaces;

    return workspaces.filter((workspace) => {
      if (
        workspace.name.toLowerCase().includes(lower)
        || workspace.cwd.toLowerCase().includes(lower)
        || (workspace.gitBranch ?? "").toLowerCase().includes(lower)
      ) {
        return true;
      }

      return workspace.surfaces.some((surface) => {
        if (surface.name.toLowerCase().includes(lower) || surface.icon.toLowerCase().includes(lower)) {
          return true;
        }

        return allLeafIds(surface.layout).some((paneId) => {
          const paneName = (surface.paneNames[paneId] ?? paneId).toLowerCase();
          const paneIcon = normalizeIconId(surface.paneIcons[paneId]).toLowerCase();
          return paneName.includes(lower) || paneIcon.includes(lower);
        });
      });
    });
  }, [query, workspaces]);

  const paneMetaById = useMemo(() => {
    const map = new Map<string, {
      workspaceId: string;
      surfaceId: string;
      layoutMode: Surface["layoutMode"];
      paneName: string;
      paneIcon: string;
      sessionId: string | null;
      panel?: { x: number; y: number; width: number; height: number };
    }>();

    for (const workspace of workspaces) {
      for (const surface of workspace.surfaces) {
        const paneIds = allLeafIds(surface.layout);
        for (const paneId of paneIds) {
          const panel = surface.canvasPanels.find((entry) => entry.paneId === paneId);
          map.set(paneId, {
            workspaceId: workspace.id,
            surfaceId: surface.id,
            layoutMode: surface.layoutMode,
            paneName: surface.paneNames[paneId] ?? paneId,
            paneIcon: normalizeIconId(surface.paneIcons[paneId]),
            sessionId: panel?.sessionId ?? findLeaf(surface.layout, paneId)?.sessionId ?? null,
            panel: panel
              ? { x: panel.x, y: panel.y, width: panel.width, height: panel.height }
              : undefined,
          });
        }
      }
    }

    return map;
  }, [workspaces]);

  const surfaceMetaById = useMemo(() => {
    const map = new Map<string, { workspaceId: string; name: string }>();
    for (const workspace of workspaces) {
      for (const surface of workspace.surfaces) {
        map.set(surface.id, { workspaceId: workspace.id, name: surface.name });
      }
    }
    return map;
  }, [workspaces]);

  const paneOrderByWorkspace = useMemo(() => {
    const map = new Map<string, string[]>();
    for (const workspace of filteredWorkspaces) {
      if (collapsedWorkspaces[workspace.id] ?? false) {
        map.set(workspace.id, []);
        continue;
      }
      const order: string[] = [];
      for (const surface of workspace.surfaces) {
        if (collapsedSurfaces[surface.id] ?? false) {
          continue;
        }
        order.push(...allLeafIds(surface.layout));
      }
      map.set(workspace.id, order);
    }
    return map;
  }, [collapsedSurfaces, collapsedWorkspaces, filteredWorkspaces]);

  const surfaceOrderByWorkspace = useMemo(() => {
    const map = new Map<string, string[]>();
    for (const workspace of filteredWorkspaces) {
      if (collapsedWorkspaces[workspace.id] ?? false) {
        map.set(workspace.id, []);
        continue;
      }
      map.set(workspace.id, workspace.surfaces.map((surface) => surface.id));
    }
    return map;
  }, [collapsedWorkspaces, filteredWorkspaces]);

  useEffect(() => {
    const validPaneIds = new Set(paneMetaById.keys());
    const validSurfaceIds = new Set(surfaceMetaById.keys());
    setSelectedPaneIds((current) => current.filter((paneId) => validPaneIds.has(paneId)));
    setSelectedSurfaceIds((current) => current.filter((surfaceId) => validSurfaceIds.has(surfaceId)));
    setPaneSelectionAnchor((current) => (current && validPaneIds.has(current) ? current : null));
    setSurfaceSelectionAnchor((current) => (current && validSurfaceIds.has(current) ? current : null));
  }, [paneMetaById, surfaceMetaById]);

  const clearSelections = useCallback(() => {
    setSelectedPaneIds([]);
    setSelectedSurfaceIds([]);
    setSelectionWorkspaceId(null);
    setPaneSelectionAnchor(null);
    setSurfaceSelectionAnchor(null);
  }, []);

  const selectPaneInWorkspace = useCallback((workspaceId: string, paneId: string, opts?: {
    toggle?: boolean;
    range?: boolean;
    preserveIfAlreadySelected?: boolean;
  }) => {
    const inWorkspace = selectionWorkspaceId === workspaceId;
    const current = inWorkspace ? selectedPaneIds : [];
    const order = paneOrderByWorkspace.get(workspaceId) ?? [];
    const ordered = order.length > 0 ? order : [paneId];
    const toggle = Boolean(opts?.toggle);
    const range = Boolean(opts?.range);

    setSelectedSurfaceIds([]);
    setSurfaceSelectionAnchor(null);
    setSelectionWorkspaceId(workspaceId);

    if (range) {
      const anchor = paneSelectionAnchor && ordered.includes(paneSelectionAnchor) ? paneSelectionAnchor : paneId;
      const from = ordered.indexOf(anchor);
      const to = ordered.indexOf(paneId);
      if (from >= 0 && to >= 0) {
        const [start, end] = from < to ? [from, to] : [to, from];
        setSelectedPaneIds(ordered.slice(start, end + 1));
      } else {
        setSelectedPaneIds([paneId]);
      }
      setPaneSelectionAnchor(anchor);
      return;
    }

    if (toggle) {
      if (current.includes(paneId)) {
        const next = current.filter((id) => id !== paneId);
        setSelectedPaneIds(next);
        setPaneSelectionAnchor(next.length > 0 ? paneId : null);
      } else {
        setSelectedPaneIds([...current, paneId]);
        setPaneSelectionAnchor(paneId);
      }
      return;
    }

    if (opts?.preserveIfAlreadySelected && current.includes(paneId) && current.length > 1) {
      return;
    }

    setSelectedPaneIds([paneId]);
    setPaneSelectionAnchor(paneId);
  }, [paneOrderByWorkspace, paneSelectionAnchor, selectedPaneIds, selectionWorkspaceId]);

  const selectSurfaceInWorkspace = useCallback((workspaceId: string, surfaceId: string, opts?: {
    toggle?: boolean;
    range?: boolean;
    preserveIfAlreadySelected?: boolean;
  }) => {
    const inWorkspace = selectionWorkspaceId === workspaceId;
    const current = inWorkspace ? selectedSurfaceIds : [];
    const order = surfaceOrderByWorkspace.get(workspaceId) ?? [];
    const ordered = order.length > 0 ? order : [surfaceId];
    const toggle = Boolean(opts?.toggle);
    const range = Boolean(opts?.range);

    setSelectedPaneIds([]);
    setPaneSelectionAnchor(null);
    setSelectionWorkspaceId(workspaceId);

    if (range) {
      const anchor = surfaceSelectionAnchor && ordered.includes(surfaceSelectionAnchor) ? surfaceSelectionAnchor : surfaceId;
      const from = ordered.indexOf(anchor);
      const to = ordered.indexOf(surfaceId);
      if (from >= 0 && to >= 0) {
        const [start, end] = from < to ? [from, to] : [to, from];
        setSelectedSurfaceIds(ordered.slice(start, end + 1));
      } else {
        setSelectedSurfaceIds([surfaceId]);
      }
      setSurfaceSelectionAnchor(anchor);
      return;
    }

    if (toggle) {
      if (current.includes(surfaceId)) {
        const next = current.filter((id) => id !== surfaceId);
        setSelectedSurfaceIds(next);
        setSurfaceSelectionAnchor(next.length > 0 ? surfaceId : null);
      } else {
        setSelectedSurfaceIds([...current, surfaceId]);
        setSurfaceSelectionAnchor(surfaceId);
      }
      return;
    }

    if (opts?.preserveIfAlreadySelected && current.includes(surfaceId) && current.length > 1) {
      return;
    }

    setSelectedSurfaceIds([surfaceId]);
    setSurfaceSelectionAnchor(surfaceId);
  }, [selectedSurfaceIds, selectionWorkspaceId, surfaceOrderByWorkspace, surfaceSelectionAnchor]);

  const appendTerminalToSurface = (workspaceId: string, surfaceId: string, paneId?: string) => {
    setActiveWorkspace(workspaceId);
    setActiveSurface(surfaceId);
    if (paneId) {
      setActivePaneId(paneId);
    }

    const state = useWorkspaceStore.getState();
    const workspace = state.workspaces.find((entry) => entry.id === workspaceId);
    const surface = workspace?.surfaces.find((entry) => entry.id === surfaceId);
    if (!surface) return;

    if (surface.layoutMode === "canvas") {
      state.createCanvasPanel(surface.id);
    } else {
      state.splitActive("horizontal", "New Terminal");
    }
  };

  const appendTerminalToWorkspace = (workspaceId: string) => {
    const state = useWorkspaceStore.getState();
    const workspace = state.workspaces.find((entry) => entry.id === workspaceId);
    const targetSurface = workspace?.surfaces.find((entry) => entry.id === workspace.activeSurfaceId)
      ?? workspace?.surfaces[0];
    if (!targetSurface) return;

    appendTerminalToSurface(workspaceId, targetSurface.id, targetSurface.activePaneId ?? undefined);
  };

  const resolvePaneContextSelection = useCallback((workspaceId: string, paneId: string) => {
    if (selectionWorkspaceId === workspaceId && selectedPaneIds.includes(paneId) && selectedPaneIds.length > 0) {
      return selectedPaneIds;
    }
    return [paneId];
  }, [selectedPaneIds, selectionWorkspaceId]);

  const resolveSurfaceContextSelection = useCallback((workspaceId: string, surfaceId: string) => {
    if (selectionWorkspaceId === workspaceId && selectedSurfaceIds.includes(surfaceId) && selectedSurfaceIds.length > 0) {
      return selectedSurfaceIds;
    }
    return [surfaceId];
  }, [selectedSurfaceIds, selectionWorkspaceId]);

  const duplicatePaneIds = useCallback(async (paneIds: string[]) => {
    if (paneIds.length === 0) return;
    const createdPaneIds: string[] = [];
    let duplicateWorkspaceId: string | null = null;

    for (let index = 0; index < paneIds.length; index += 1) {
      const paneId = paneIds[index];
      const source = paneMetaById.get(paneId);
      if (!source) continue;

      duplicateWorkspaceId = source.workspaceId;
      setActiveWorkspace(source.workspaceId);
      setActiveSurface(source.surfaceId);
      setActivePaneId(paneId);
      const sourceSessionId = resolveDuplicateSourceSessionId(
        paneId,
        source.sessionId,
        operationalEvents,
      );
      const sourceWorkspace = workspaces.find((w) => w.id === source.workspaceId);
      const cloneResult = await cloneSessionForDuplication(paneId, sourceSessionId, {
        workspaceId: source.workspaceId,
        cwd: sourceWorkspace?.cwd || null,
      });

      if (source.layoutMode === "canvas") {
        const duplicatedPaneId = createCanvasPanel(source.surfaceId, {
          paneName: `${source.paneName} Copy`,
          paneIcon: source.paneIcon,
          sessionId: cloneResult?.sessionId ?? null,
          ...(source.panel
            ? {
              width: source.panel.width,
              height: source.panel.height,
              x: source.panel.x + 28 * (index + 1),
              y: source.panel.y + 28 * (index + 1),
            }
            : {}),
        });
        if (!duplicatedPaneId) continue;
        const bootstrapCommand =
          resolveDuplicateActiveBootstrapCommand(paneId, operationalEvents)
          ?? resolveDuplicateBootstrapCommand(paneId, operationalEvents)
          ?? cloneResult?.activeCommand;
        if (bootstrapCommand) {
          queuePaneBootstrapCommand(duplicatedPaneId, bootstrapCommand);
        }
        createdPaneIds.push(duplicatedPaneId);
        continue;
      }

      splitActive("horizontal", `${source.paneName} Copy`, {
        paneIcon: source.paneIcon,
        sessionId: cloneResult?.sessionId ?? null,
      });
      const duplicatedPaneId = useWorkspaceStore.getState().activePaneId();
      if (!duplicatedPaneId) continue;
      const bootstrapCommand =
        resolveDuplicateActiveBootstrapCommand(paneId, operationalEvents)
        ?? resolveDuplicateBootstrapCommand(paneId, operationalEvents)
        ?? cloneResult?.activeCommand;
      if (bootstrapCommand) {
        queuePaneBootstrapCommand(duplicatedPaneId, bootstrapCommand);
      }
      createdPaneIds.push(duplicatedPaneId);
    }

    if (createdPaneIds.length === 0) return;
    setSelectionWorkspaceId(duplicateWorkspaceId);
    setSelectedSurfaceIds([]);
    setSurfaceSelectionAnchor(null);
    setSelectedPaneIds(createdPaneIds);
    setPaneSelectionAnchor(createdPaneIds[createdPaneIds.length - 1] ?? null);
  }, [
    createCanvasPanel,
    paneMetaById,
    setActivePaneId,
    setActiveSurface,
    setActiveWorkspace,
    splitActive,
    operationalEvents,
  ]);

  const showConfirm = (state: ConfirmDialogState) => {
    setConfirmDialog(state);
  };

  const handleWorkspaceContextAction = (action: "rename" | "icon" | "append" | "new-canvas" | "close", workspaceId: string) => {
    setContextMenu(null);
    const workspace = useWorkspaceStore.getState().workspaces.find((entry) => entry.id === workspaceId);
    if (!workspace) return;

    if (action === "rename") {
      setEditingWorkspaceId(workspaceId);
      setWorkspaceNameDraft(workspace.name);
      return;
    }

    if (action === "icon") {
      setIconPicker({
        kind: "workspace",
        workspaceId,
        x: contextMenu?.x ?? 120,
        y: contextMenu?.y ?? 120,
      });
      return;
    }

    if (action === "append") {
      appendTerminalToWorkspace(workspaceId);
      return;
    }

    if (action === "new-canvas") {
      createSurface(workspaceId, { layoutMode: "canvas" });
      return;
    }

    showConfirm({
      title: `Close workspace '${workspace.name}'?`,
      message: "This will stop and close all terminals in this workspace.",
      confirmLabel: "Close Workspace",
      tone: "danger",
      action: () => closeWorkspace(workspaceId),
    });
    clearSelections();
  };

  const handleSurfaceContextAction = (action: "close", workspaceId: string, surfaceId: string) => {
    setContextMenu(null);
    const surfaceIds = resolveSurfaceContextSelection(workspaceId, surfaceId);
    if (action !== "close") return;

    if (surfaceIds.length > 1) {
      showConfirm({
        title: `Close ${surfaceIds.length} surfaces?`,
        message: "This will close all selected surfaces and stop their terminals.",
        confirmLabel: `Close ${surfaceIds.length} Surfaces`,
        tone: "danger",
        action: () => {
          for (const id of surfaceIds) {
            closeSurface(id);
          }
          clearSelections();
        },
      });
      return;
    }

    const surfaceMeta = surfaceMetaById.get(surfaceId);
    showConfirm({
      title: `Close surface '${surfaceMeta?.name ?? surfaceId}'?`,
      message: "This will close the selected surface and stop all of its terminals.",
      confirmLabel: "Close Surface",
      tone: "danger",
      action: () => {
        closeSurface(surfaceId);
        clearSelections();
      },
    });
  };

  const handlePaneContextAction = (action: "rename" | "icon" | "append" | "duplicate" | "close", workspaceId: string, surfaceId: string, paneId: string) => {
    setContextMenu(null);
    const paneIds = resolvePaneContextSelection(workspaceId, paneId);

    const state = useWorkspaceStore.getState();
    const workspace = state.workspaces.find((entry) => entry.id === workspaceId);
    const surface = workspace?.surfaces.find((entry) => entry.id === surfaceId);
    if (!surface) return;

    if (action === "rename" && paneIds.length === 1) {
      setEditingPaneId(paneId);
      setPaneNameDraft(surface.paneNames[paneId] ?? paneId);
      return;
    }

    if (action === "icon") {
      setIconPicker({
        kind: "pane",
        paneIds,
        x: contextMenu?.x ?? 120,
        y: contextMenu?.y ?? 120,
      });
      return;
    }

    if (action === "append" && paneIds.length === 1) {
      appendTerminalToSurface(workspaceId, surfaceId, paneId);
      return;
    }

    if (action === "duplicate") {
      void duplicatePaneIds(paneIds);
      return;
    }

    if (paneIds.length > 1) {
      showConfirm({
        title: `Close ${paneIds.length} terminals?`,
        message: "Terminal output and session state for selected panes will be closed.",
        confirmLabel: `Close ${paneIds.length} Terminals`,
        tone: "danger",
        action: () => {
          for (const id of paneIds) {
            closePane(id);
          }
          clearSelections();
        },
      });
      return;
    }

    const paneLabel = surface.paneNames[paneId] ?? paneId;
    const surfacePaneIds = allLeafIds(surface.layout);
    const shouldCloseWholeSurface = surface.layoutMode === "bsp"
      && surfacePaneIds.length === 1
      && (workspace?.surfaces.length ?? 0) > 1;

    if (shouldCloseWholeSurface) {
      showConfirm({
        title: `Close terminal surface '${surface.name}'?`,
        message: "This is the last terminal in this BSP surface. The entire surface will be closed.",
        confirmLabel: "Close Surface",
        tone: "danger",
        action: () => {
          closeSurface(surfaceId);
          clearSelections();
        },
      });
      return;
    }

    showConfirm({
      title: `Close terminal '${paneLabel}'?`,
      message: "Terminal output and session state for this pane will be closed.",
      confirmLabel: "Close Terminal",
      tone: "danger",
      action: () => {
        closePane(paneId);
        clearSelections();
      },
    });
  };

  const paneContextSelection = useMemo(() => {
    if (!contextMenu || contextMenu.kind !== "pane") return [];
    return resolvePaneContextSelection(contextMenu.workspaceId, contextMenu.paneId);
  }, [contextMenu, resolvePaneContextSelection]);

  const surfaceContextSelection = useMemo(() => {
    if (!contextMenu || contextMenu.kind !== "surface") return [];
    return resolveSurfaceContextSelection(contextMenu.workspaceId, contextMenu.surfaceId);
  }, [contextMenu, resolveSurfaceContextSelection]);

  return (
    <div
      ref={sidebarRef}
      style={{
        height: "100%",
        width: `${sidebarWidth}px`,
        minWidth: `${sidebarWidth}px`,
        maxWidth: `${sidebarWidth}px`,
        minHeight: 0,
        background: "var(--bg-primary)",
        borderRight: "1px solid var(--border)",
        display: "flex",
        flexDirection: "column",
        overflow: "hidden",
        position: "relative",
      }}
    >
      <SidebarHeader
        workspacesCount={workspaces.length}
        approvalsCount={approvals.filter((entry) => entry.status === "pending").length}
        reasoningCount={cognitiveEvents.length}
        createWorkspace={(layoutMode) => createWorkspace(undefined, layoutMode ? { layoutMode } : undefined)}
        query={query}
        setQuery={setQuery}
      />

      <div style={{ flex: 1, overflow: "auto", padding: "var(--space-2) var(--space-3)" }}>
        <div style={{ display: "flex", flexDirection: "column", gap: 2 }}>
          {filteredWorkspaces.map((workspace) => {
            const workspaceCollapsed = collapsedWorkspaces[workspace.id] ?? false;
            const workspaceActive = workspace.id === activeWorkspaceId;
            const workspaceContextActive = contextMenu?.kind === "workspace" && contextMenu.workspaceId === workspace.id;

            return (
              <div key={workspace.id} style={{ display: "grid", gap: 2 }}>
                <div
                  onContextMenu={(event) => {
                    event.preventDefault();
                    clearSelections();
                    setActiveWorkspace(workspace.id);
                    setContextMenu({
                      kind: "workspace",
                      workspaceId: workspace.id,
                      x: event.clientX,
                      y: event.clientY,
                    });
                  }}
                  style={{
                    display: "flex",
                    alignItems: "center",
                    gap: 6,
                    padding: "3px 4px",
                    borderRadius: "var(--radius-sm)",
                    background: workspaceActive || workspaceContextActive ? "var(--bg-secondary)" : "transparent",
                  }}
                >
                  <button
                    type="button"
                    onClick={() => {
                      setCollapsedWorkspaces((current) => ({
                        ...current,
                        [workspace.id]: !(current[workspace.id] ?? false),
                      }));
                    }}
                    style={treeToggleStyle}
                    title={workspaceCollapsed ? "Expand" : "Collapse"}
                  >
                    {workspaceCollapsed ? "▸" : "▾"}
                  </button>

                  <button
                    type="button"
                    onClick={() => {
                      clearSelections();
                      setActiveWorkspace(workspace.id);
                    }}
                    style={treeNodeButtonStyle(workspaceActive, workspace.accentColor)}
                  >
                    <span style={{ opacity: 0.9 }}>{iconGlyph(workspace.icon)}</span>
                    {editingWorkspaceId === workspace.id ? (
                      <input
                        autoFocus
                        value={workspaceNameDraft}
                        onChange={(event) => setWorkspaceNameDraft(event.target.value)}
                        onBlur={() => {
                          if (workspaceNameDraft.trim()) {
                            renameWorkspace(workspace.id, workspaceNameDraft);
                          }
                          setEditingWorkspaceId(null);
                        }}
                        onKeyDown={(event) => {
                          if (event.key === "Enter") {
                            if (workspaceNameDraft.trim()) {
                              renameWorkspace(workspace.id, workspaceNameDraft);
                            }
                            setEditingWorkspaceId(null);
                          }
                          if (event.key === "Escape") {
                            setEditingWorkspaceId(null);
                          }
                        }}
                        style={paneRenameInputStyle}
                      />
                    ) : (
                      <span style={{ overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap" }}>
                        {workspace.name}
                      </span>
                    )}
                  </button>

                  {getUnread(workspace.id) > 0 ? (
                    <span style={countBadgeStyle}>{getUnread(workspace.id)}</span>
                  ) : null}
                </div>

                {!workspaceCollapsed ? (
                  <div style={{ marginLeft: 16, borderLeft: "1px solid var(--border)", paddingLeft: 8, display: "grid", gap: 2 }}>
                    {workspace.surfaces.map((surface) => {
                      const surfaceCollapsed = collapsedSurfaces[surface.id] ?? false;
                      const paneIds = allLeafIds(surface.layout);
                      const surfaceSelected = selectionWorkspaceId === workspace.id && selectedSurfaceIds.includes(surface.id);

                      return (
                        <div key={surface.id} style={{ display: "grid", gap: 2 }}>
                          <div
                            onContextMenu={(event) => {
                              event.preventDefault();
                              const toggle = event.metaKey || event.ctrlKey;
                              const range = event.shiftKey;
                              selectSurfaceInWorkspace(workspace.id, surface.id, {
                                toggle,
                                range,
                                preserveIfAlreadySelected: !toggle && !range,
                              });
                              setContextMenu({
                                kind: "surface",
                                workspaceId: workspace.id,
                                surfaceId: surface.id,
                                x: event.clientX,
                                y: event.clientY,
                              });
                            }}
                            style={{ display: "flex", alignItems: "center", gap: 6, padding: "2px 0" }}
                          >
                            <button
                              type="button"
                              onClick={() => {
                                setCollapsedSurfaces((current) => ({
                                  ...current,
                                  [surface.id]: !(current[surface.id] ?? false),
                                }));
                              }}
                              style={treeToggleStyle}
                            >
                              {surfaceCollapsed ? "▸" : "▾"}
                            </button>

                            <button
                              type="button"
                              onClick={(event) => {
                                const toggle = event.metaKey || event.ctrlKey;
                                const range = event.shiftKey;
                                selectSurfaceInWorkspace(workspace.id, surface.id, { toggle, range });
                                if (toggle || range) {
                                  return;
                                }
                                setActiveWorkspace(workspace.id);
                                setActiveSurface(surface.id);
                              }}
                              style={surfaceNodeStyle(workspace.activeSurfaceId === surface.id || surfaceSelected)}
                            >
                              <span style={{ opacity: 0.9 }}>{iconGlyph(surface.icon)}</span>
                              <span style={{ overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap" }}>{surface.name}</span>
                              <span style={{ marginLeft: "auto", opacity: 0.65 }}>{surface.layoutMode}</span>
                            </button>
                          </div>

                          {!surfaceCollapsed ? (
                            <div style={{ marginLeft: 16, borderLeft: "1px dotted var(--border)", paddingLeft: 8, display: "grid", gap: 1 }}>
                              {paneIds.map((paneId) => {
                                const paneActive = workspaceActive && workspace.activeSurfaceId === surface.id && surface.activePaneId === paneId;
                                const paneSelected = selectionWorkspaceId === workspace.id && selectedPaneIds.includes(paneId);
                                const paneName = surface.paneNames[paneId] ?? paneId;
                                const paneIcon = normalizeIconId(surface.paneIcons[paneId]);
                                const paneAttentionCount = attentionByPane.get(paneId) ?? 0;
                                const needsApproval = paneAttentionCount > 0
                                  || surface.canvasPanels.find((panel) => panel.paneId === paneId)?.status === "needs_approval";
                                const editing = editingPaneId === paneId;

                                return (
                                  <div
                                    key={paneId}
                                    onContextMenu={(event) => {
                                      event.preventDefault();
                                      const toggle = event.metaKey || event.ctrlKey;
                                      const range = event.shiftKey;
                                      selectPaneInWorkspace(workspace.id, paneId, {
                                        toggle,
                                        range,
                                        preserveIfAlreadySelected: !toggle && !range,
                                      });
                                      setContextMenu({
                                        kind: "pane",
                                        workspaceId: workspace.id,
                                        surfaceId: surface.id,
                                        paneId,
                                        x: event.clientX,
                                        y: event.clientY,
                                      });
                                    }}
                                    style={{
                                      display: "flex",
                                      alignItems: "center",
                                      gap: 6,
                                      padding: "2px 4px",
                                      borderRadius: "var(--radius-sm)",
                                      background: paneSelected || paneActive ? "var(--bg-tertiary)" : "transparent",
                                    }}
                                  >
                                    <button
                                      type="button"
                                      onClick={(event) => {
                                        const toggle = event.metaKey || event.ctrlKey;
                                        const range = event.shiftKey;
                                        selectPaneInWorkspace(workspace.id, paneId, { toggle, range });
                                        if (toggle || range) {
                                          return;
                                        }
                                        setActiveWorkspace(workspace.id);
                                        setActiveSurface(surface.id);
                                        setActivePaneId(paneId);
                                        if (surface.layoutMode === "canvas") {
                                          focusCanvasPanel(paneId, { storePreviousView: true });
                                        }
                                      }}
                                      style={paneNodeButtonStyle(needsApproval)}
                                    >
                                      <span style={{ opacity: 0.9 }}>{iconGlyph(paneIcon)}</span>
                                      {needsApproval ? <span style={pendingDotStyle} /> : null}
                                      {editing ? (
                                        <input
                                          autoFocus
                                          value={paneNameDraft}
                                          onChange={(event) => setPaneNameDraft(event.target.value)}
                                          onBlur={() => {
                                            if (paneNameDraft.trim()) {
                                              setPaneName(paneId, paneNameDraft);
                                            }
                                            setEditingPaneId(null);
                                          }}
                                          onKeyDown={(event) => {
                                            if (event.key === "Enter") {
                                              if (paneNameDraft.trim()) {
                                                setPaneName(paneId, paneNameDraft);
                                              }
                                              setEditingPaneId(null);
                                            }
                                            if (event.key === "Escape") {
                                              setEditingPaneId(null);
                                            }
                                          }}
                                          style={paneRenameInputStyle}
                                        />
                                      ) : (
                                        <span
                                          onDoubleClick={() => {
                                            setEditingPaneId(paneId);
                                            setPaneNameDraft(paneName);
                                          }}
                                          style={{ overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap" }}
                                        >
                                          {paneName}
                                        </span>
                                      )}
                                      {paneAttentionCount > 0 ? (
                                        <span style={paneCountBadgeStyle(needsApproval)}>
                                          {paneAttentionCount > 9 ? "9+" : paneAttentionCount}
                                        </span>
                                      ) : null}
                                    </button>
                                  </div>
                                );
                              })}
                            </div>
                          ) : null}
                        </div>
                      );
                    })}
                  </div>
                ) : null}
              </div>
            );
          })}
        </div>
      </div>

      <SidebarActions
        workspacesCount={workspaces.length}
        toggleAgentPanel={toggleAgentPanel}
        toggleSystemMonitor={toggleSystemMonitor}
        toggleCommandPalette={toggleCommandPalette}
        toggleSearch={toggleSearch}
        toggleCommandHistory={toggleCommandHistory}
        toggleCommandLog={toggleCommandLog}
        toggleFileManager={toggleFileManager}
        toggleSessionVault={toggleSessionVault}
        toggleSettings={toggleSettings}
      />

      <SidebarResizeHandle />

      {contextMenu ? (
        <div
          ref={contextMenuRef}
          style={{
            position: "fixed",
            left: contextMenu.x,
            top: contextMenu.y,
            zIndex: 2500,
            minWidth: 190,
            border: "1px solid var(--glass-border)",
            borderRadius: "var(--radius-md)",
            background: "var(--bg-secondary)",
            boxShadow: "var(--shadow-sm)",
            padding: 4,
            display: "grid",
            gap: 2,
          }}
        >
          {contextMenu.kind === "workspace" ? (
            <>
              <button type="button" style={contextMenuItemStyle} onClick={() => handleWorkspaceContextAction("rename", contextMenu.workspaceId)}>Rename Workspace</button>
              <button type="button" style={contextMenuItemStyle} onClick={() => handleWorkspaceContextAction("icon", contextMenu.workspaceId)}>Change Icon</button>
              <button type="button" style={contextMenuItemStyle} onClick={() => handleWorkspaceContextAction("append", contextMenu.workspaceId)}>Append New Terminal</button>
              <button type="button" style={contextMenuItemStyle} onClick={() => handleWorkspaceContextAction("new-canvas", contextMenu.workspaceId)}>New Infinite Canvas</button>
              <button type="button" style={dangerContextMenuItemStyle} onClick={() => handleWorkspaceContextAction("close", contextMenu.workspaceId)}>Close Workspace</button>
            </>
          ) : contextMenu.kind === "surface" ? (
            <>
              <button
                type="button"
                style={dangerContextMenuItemStyle}
                onClick={() => handleSurfaceContextAction("close", contextMenu.workspaceId, contextMenu.surfaceId)}
              >
                {surfaceContextSelection.length > 1 ? `Close ${surfaceContextSelection.length} Surfaces` : "Close Surface"}
              </button>
            </>
          ) : (
            <>
              {paneContextSelection.length <= 1 ? (
                <button
                  type="button"
                  style={contextMenuItemStyle}
                  onClick={() => handlePaneContextAction("rename", contextMenu.workspaceId, contextMenu.surfaceId, contextMenu.paneId)}
                >
                  Rename Terminal
                </button>
              ) : null}
              <button
                type="button"
                style={contextMenuItemStyle}
                onClick={() => handlePaneContextAction("icon", contextMenu.workspaceId, contextMenu.surfaceId, contextMenu.paneId)}
              >
                {paneContextSelection.length > 1 ? `Change Icon (${paneContextSelection.length} Terminals)` : "Change Icon"}
              </button>
              {paneContextSelection.length <= 1 ? (
                <button
                  type="button"
                  style={contextMenuItemStyle}
                  onClick={() => handlePaneContextAction("append", contextMenu.workspaceId, contextMenu.surfaceId, contextMenu.paneId)}
                >
                  Append New Terminal
                </button>
              ) : null}
              <button
                type="button"
                style={contextMenuItemStyle}
                onClick={() => handlePaneContextAction("duplicate", contextMenu.workspaceId, contextMenu.surfaceId, contextMenu.paneId)}
              >
                {paneContextSelection.length > 1 ? `Duplicate ${paneContextSelection.length} Terminals` : "Duplicate Terminal"}
              </button>
              <button
                type="button"
                style={dangerContextMenuItemStyle}
                onClick={() => handlePaneContextAction("close", contextMenu.workspaceId, contextMenu.surfaceId, contextMenu.paneId)}
              >
                {paneContextSelection.length > 1 ? `Close ${paneContextSelection.length} Terminals` : "Close Terminal"}
              </button>
            </>
          )}
        </div>
      ) : null}

      {iconPicker ? (
        <div
          ref={iconPickerRef}
          style={{
            position: "fixed",
            left: iconPicker.x,
            top: iconPicker.y,
            zIndex: 2600,
            minWidth: 160,
            border: "1px solid var(--glass-border)",
            borderRadius: "var(--radius-md)",
            background: "var(--bg-primary)",
            boxShadow: "var(--shadow-sm)",
            padding: 4,
            display: "grid",
            gap: 2,
          }}
        >
          {iconChoices(iconPicker.kind === "workspace" ? WORKSPACE_ICON_IDS : PANE_ICON_IDS).map((icon) => (
            <button
              key={icon.id}
              type="button"
              style={contextMenuItemStyle}
              onClick={() => {
                if (iconPicker.kind === "workspace") {
                  setWorkspaceIcon(iconPicker.workspaceId, icon.id);
                } else {
                  for (const paneId of iconPicker.paneIds) {
                    setPaneIcon(paneId, icon.id);
                  }
                }
                setIconPicker(null);
              }}
            >
              <span style={{ display: "inline-flex", alignItems: "center", gap: 8 }}>
                <span style={{ minWidth: 24, textAlign: "center", fontFamily: "ui-monospace, SFMono-Regular, Menlo, Monaco, Consolas, monospace" }}>{icon.glyph}</span>
                <span>{icon.label}</span>
              </span>
            </button>
          ))}
        </div>
      ) : null}

      <AppConfirmDialog
        open={Boolean(confirmDialog)}
        title={confirmDialog?.title ?? ""}
        message={confirmDialog?.message ?? ""}
        confirmLabel={confirmDialog?.confirmLabel ?? "Confirm"}
        tone={confirmDialog?.tone ?? "danger"}
        onCancel={() => setConfirmDialog(null)}
        onConfirm={() => {
          if (!confirmDialog) return;
          confirmDialog.action();
          setConfirmDialog(null);
        }}
      />
    </div>
  );
}

const treeToggleStyle: CSSProperties = {
  border: "none",
  background: "transparent",
  color: "var(--text-muted)",
  cursor: "pointer",
  fontSize: "var(--text-xs)",
  width: 14,
  height: 14,
  padding: 0,
  lineHeight: 1,
};

function treeNodeButtonStyle(active: boolean, accent: string): CSSProperties {
  return {
    border: "none",
    background: "transparent",
    color: active ? "var(--text-primary)" : "var(--text-secondary)",
    cursor: "pointer",
    display: "flex",
    alignItems: "center",
    gap: 6,
    padding: 0,
    minWidth: 0,
    flex: 1,
    textAlign: "left",
    fontSize: "var(--text-xs)",
    fontWeight: active ? 600 : 500,
    borderLeft: active ? `2px solid ${accent}` : "2px solid transparent",
    paddingLeft: active ? 6 : 8,
  };
}

function surfaceNodeStyle(active: boolean): CSSProperties {
  return {
    border: "none",
    background: "transparent",
    color: active ? "var(--text-primary)" : "var(--text-muted)",
    cursor: "pointer",
    display: "flex",
    alignItems: "center",
    gap: 6,
    padding: 0,
    minWidth: 0,
    flex: 1,
    textAlign: "left",
    fontSize: "var(--text-xs)",
    fontWeight: active ? 600 : 500,
  };
}

function paneNodeButtonStyle(needsApproval: boolean): CSSProperties {
  return {
    border: "none",
    background: "transparent",
    color: needsApproval ? "var(--approval)" : "var(--text-secondary)",
    cursor: "pointer",
    display: "flex",
    alignItems: "center",
    gap: 6,
    padding: 0,
    minWidth: 0,
    flex: 1,
    textAlign: "left",
    fontSize: "var(--text-xs)",
  };
}

function paneCountBadgeStyle(needsApproval: boolean): CSSProperties {
  return {
    marginLeft: "auto",
    background: needsApproval ? "var(--approval-soft)" : "var(--bg-tertiary)",
    border: "1px solid",
    borderColor: needsApproval ? "var(--approval-border)" : "var(--glass-border)",
    color: needsApproval ? "var(--approval)" : "var(--text-muted)",
    borderRadius: "var(--radius-full)",
    fontSize: 10,
    fontWeight: 700,
    lineHeight: "16px",
    minWidth: 16,
    textAlign: "center",
    padding: "0 4px",
  };
}

const paneRenameInputStyle: CSSProperties = {
  width: "100%",
  background: "var(--bg-secondary)",
  border: "1px solid var(--glass-border)",
  borderRadius: "var(--radius-sm)",
  color: "var(--text-primary)",
  fontSize: "var(--text-xs)",
  padding: "2px 4px",
};

const pendingDotStyle: CSSProperties = {
  width: 6,
  height: 6,
  borderRadius: "50%",
  background: "var(--approval)",
  flexShrink: 0,
  animation: "agent-pulse 1.4s ease-in-out infinite",
};

const countBadgeStyle: CSSProperties = {
  background: "var(--accent)",
  color: "var(--bg-primary)",
  borderRadius: "var(--radius-full)",
  padding: "0 6px",
  fontSize: "var(--text-xs)",
  fontWeight: 700,
  minWidth: 18,
  textAlign: "center",
  lineHeight: "16px",
};

const contextMenuItemStyle: CSSProperties = {
  border: "none",
  background: "transparent",
  color: "var(--text-secondary)",
  cursor: "pointer",
  textAlign: "left",
  fontSize: "var(--text-xs)",
  padding: "6px 8px",
  borderRadius: "var(--radius-sm)",
};

const dangerContextMenuItemStyle: CSSProperties = {
  ...contextMenuItemStyle,
  color: "var(--danger)",
};
