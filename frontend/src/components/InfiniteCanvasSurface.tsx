import {
  useCallback,
  useEffect,
  useMemo,
  useRef,
  useState,
  type CSSProperties,
  type MouseEvent as ReactMouseEvent,
  type PointerEvent as ReactPointerEvent,
  type ReactNode,
} from "react";
import { allLeafIds, findLeaf } from "../lib/bspTree";
import { useAgentMissionStore } from "../lib/agentMissionStore";
import {
  cloneSessionForDuplication,
  queuePaneBootstrapCommand,
  resolveDuplicateActiveBootstrapCommand,
  resolveDuplicateBootstrapCommand,
  resolveDuplicateSourceSessionId,
} from "../lib/paneDuplication";
import type { Surface } from "../lib/types";
import { iconChoices, PANE_ICON_IDS } from "../lib/iconRegistry";
import { useWorkspaceStore } from "../lib/workspaceStore";
import { AppConfirmDialog } from "./AppConfirmDialog";
import { TerminalPane } from "./TerminalPane";

const CANVAS_GRID_SIZE = 32;
const MIN_CANVAS_ZOOM = 0.04;
const MAX_CANVAS_ZOOM = 2.2;

function clamp(value: number, min: number, max: number): number {
  return Math.min(max, Math.max(min, value));
}

function snapToGrid(value: number, gridSize: number): number {
  return Math.round(value / gridSize) * gridSize;
}

type InfiniteCanvasSurfaceProps = {
  surface: Surface;
};

type CanvasContextMenuState = {
  x: number;
  y: number;
  paneId: string;
};

type CanvasIconPickerState = {
  x: number;
  y: number;
  paneIds: string[];
};

type CanvasMarqueeState = {
  startX: number;
  startY: number;
  currentX: number;
  currentY: number;
  additive: boolean;
};

export function InfiniteCanvasSurface({ surface }: InfiniteCanvasSurfaceProps) {
  const viewportRef = useRef<HTMLDivElement | null>(null);
  const rafRef = useRef<number | null>(null);
  const panCleanupRef = useRef<(() => void) | null>(null);
  const isPanningRef = useRef(false);
  const suppressContextMenuRef = useRef(false);
  const suppressContextMenuUntilRef = useRef(0);
  const activePaneRef = useRef<string | null>(surface.activePaneId ?? null);
  const lastFocusRequestNonceRef = useRef<number>(surface.canvasState.focusRequestNonce ?? 0);
  const viewRef = useRef({
    panX: surface.canvasState.panX,
    panY: surface.canvasState.panY,
    zoomLevel: surface.canvasState.zoomLevel,
  });
  const [isPanning, setIsPanning] = useState(false);
  const [animateTransform, setAnimateTransform] = useState(false);
  const [snapEnabled, setSnapEnabled] = useState(true);
  const [selectedPaneIds, setSelectedPaneIds] = useState<string[]>([]);
  const [contextMenu, setContextMenu] = useState<CanvasContextMenuState | null>(null);
  const [iconPicker, setIconPicker] = useState<CanvasIconPickerState | null>(null);
  const [confirmClosePaneIds, setConfirmClosePaneIds] = useState<string[]>([]);
  const [marquee, setMarquee] = useState<CanvasMarqueeState | null>(null);
  const dragGroupBaseRef = useRef<Map<string, { x: number; y: number }> | null>(null);
  const createCanvasPanel = useWorkspaceStore((s) => s.createCanvasPanel);
  const createSurface = useWorkspaceStore((s) => s.createSurface);
  const renameSurface = useWorkspaceStore((s) => s.renameSurface);
  const arrangeCanvasPanels = useWorkspaceStore((s) => s.arrangeCanvasPanels);
  const closePane = useWorkspaceStore((s) => s.closePane);
  const setPaneSessionId = useWorkspaceStore((s) => s.setPaneSessionId);
  const moveCanvasPanel = useWorkspaceStore((s) => s.moveCanvasPanel);
  const resizeCanvasPanel = useWorkspaceStore((s) => s.resizeCanvasPanel);
  const setPaneIcon = useWorkspaceStore((s) => s.setPaneIcon);
  const setPaneName = useWorkspaceStore((s) => s.setPaneName);
  const setCanvasPanelIcon = useWorkspaceStore((s) => s.setCanvasPanelIcon);
  const setCanvasView = useWorkspaceStore((s) => s.setCanvasView);
  const setCanvasPreviousView = useWorkspaceStore((s) => s.setCanvasPreviousView);
  const setActiveWorkspace = useWorkspaceStore((s) => s.setActiveWorkspace);
  const setActiveSurface = useWorkspaceStore((s) => s.setActiveSurface);
  const setActivePaneId = useWorkspaceStore((s) => s.setActivePaneId);
  const splitActive = useWorkspaceStore((s) => s.splitActive);
  const clearActivePaneFocus = useWorkspaceStore((s) => s.clearActivePaneFocus);
  const workspaces = useWorkspaceStore((s) => s.workspaces);
  const operationalEvents = useAgentMissionStore((s) => s.operationalEvents);

  const paneIds = useMemo(() => allLeafIds(surface.layout), [surface.layout]);
  const panelByPane = useMemo(() => new Map(surface.canvasPanels.map((panel) => [panel.paneId, panel])), [surface.canvasPanels]);
  const panels = useMemo(() => paneIds
    .map((paneId) => panelByPane.get(paneId))
    .filter((panel): panel is Surface["canvasPanels"][number] => Boolean(panel)), [paneIds, panelByPane]);
  const selectedPaneSet = useMemo(() => new Set(selectedPaneIds), [selectedPaneIds]);

  useEffect(() => {
    isPanningRef.current = isPanning;
  }, [isPanning]);

  useEffect(() => {
    activePaneRef.current = surface.activePaneId ?? null;
  }, [surface.activePaneId]);

  useEffect(() => {
    const validPaneIds = new Set(panels.map((panel) => panel.paneId));
    setSelectedPaneIds((current) => current.filter((paneId) => validPaneIds.has(paneId)));
  }, [panels]);

  useEffect(() => {
    if (!contextMenu && !iconPicker) return;
    const closeMenus = (event: MouseEvent) => {
      const target = event.target as HTMLElement | null;
      if (target?.closest("[data-canvas-menu='true']")) {
        return;
      }
      setContextMenu(null);
      setIconPicker(null);
    };
    window.addEventListener("mousedown", closeMenus);
    return () => window.removeEventListener("mousedown", closeMenus);
  }, [contextMenu, iconPicker]);

  useEffect(() => () => {
    if (panCleanupRef.current) {
      panCleanupRef.current();
      panCleanupRef.current = null;
    }
    if (rafRef.current !== null) {
      window.cancelAnimationFrame(rafRef.current);
      rafRef.current = null;
    }
  }, []);

  useEffect(() => {
    viewRef.current = {
      panX: surface.canvasState.panX,
      panY: surface.canvasState.panY,
      zoomLevel: surface.canvasState.zoomLevel,
    };
  }, [surface.canvasState.panX, surface.canvasState.panY, surface.canvasState.zoomLevel]);

  const queueViewUpdate = useCallback((nextPanX: number, nextPanY: number, nextZoom: number) => {
    viewRef.current = {
      panX: nextPanX,
      panY: nextPanY,
      zoomLevel: nextZoom,
    };
    if (rafRef.current !== null) {
      window.cancelAnimationFrame(rafRef.current);
    }
    rafRef.current = window.requestAnimationFrame(() => {
      setCanvasView(surface.id, {
        panX: nextPanX,
        panY: nextPanY,
        zoomLevel: nextZoom,
      });
      rafRef.current = null;
    });
  }, [setCanvasView, surface.id]);

  const centerOnPanel = useCallback((
    paneId: string,
    opts?: { storePrevious?: boolean; zoomIn?: boolean },
  ) => {
    const viewport = viewportRef.current;
    if (!viewport) return;
    const panel = surface.canvasPanels.find((entry) => entry.paneId === paneId);
    if (!panel) return;

    if (opts?.storePrevious) {
      setCanvasPreviousView(surface.id, {
        panX: surface.canvasState.panX,
        panY: surface.canvasState.panY,
        zoomLevel: surface.canvasState.zoomLevel,
      });
    }

    const rect = viewport.getBoundingClientRect();
    const requestedZoom = opts?.zoomIn
      ? Math.max(surface.canvasState.zoomLevel * 1.18, 1.05)
      : surface.canvasState.zoomLevel;
    const nextZoom = clamp(requestedZoom, MIN_CANVAS_ZOOM, MAX_CANVAS_ZOOM);
    const panelCenterX = panel.x + panel.width / 2;
    const panelCenterY = panel.y + panel.height / 2;
    const nextPanX = rect.width / 2 - panelCenterX * nextZoom;
    const nextPanY = rect.height / 2 - panelCenterY * nextZoom;

    setAnimateTransform(true);
    queueViewUpdate(nextPanX, nextPanY, nextZoom);
    window.setTimeout(() => setAnimateTransform(false), 260);
    setActivePaneId(paneId);
  }, [queueViewUpdate, setActivePaneId, setCanvasPreviousView, surface.canvasPanels, surface.canvasState.panX, surface.canvasState.panY, surface.canvasState.zoomLevel, surface.id]);

  const handleCreatePanel = useCallback(() => {
    const createdPaneId = createCanvasPanel(surface.id);
    if (!createdPaneId) return;
    window.setTimeout(() => centerOnPanel(createdPaneId), 0);
  }, [centerOnPanel, createCanvasPanel, surface.id]);

  const selectPanel = useCallback((paneId: string, opts?: {
    toggle?: boolean;
    additive?: boolean;
    preserveIfAlreadySelected?: boolean;
  }) => {
    const toggle = Boolean(opts?.toggle);
    const additive = Boolean(opts?.additive);
    const selected = selectedPaneSet.has(paneId);

    if (toggle) {
      setSelectedPaneIds((current) => (
        current.includes(paneId)
          ? current.filter((id) => id !== paneId)
          : [...current, paneId]
      ));
      return;
    }

    if (additive) {
      setSelectedPaneIds((current) => (
        current.includes(paneId) ? current : [...current, paneId]
      ));
      return;
    }

    if (opts?.preserveIfAlreadySelected && selected && selectedPaneIds.length > 1) {
      return;
    }

    setSelectedPaneIds([paneId]);
  }, [selectedPaneIds.length, selectedPaneSet]);

  const resolveContextPaneIds = useCallback((paneId: string) => {
    if (selectedPaneSet.has(paneId) && selectedPaneIds.length > 0) {
      return selectedPaneIds;
    }
    return [paneId];
  }, [selectedPaneIds, selectedPaneSet]);

  const requestClosePanes = useCallback((paneId: string) => {
    const paneIdsToClose = resolveContextPaneIds(paneId);
    setConfirmClosePaneIds(paneIdsToClose);
  }, [resolveContextPaneIds]);

  const runSnippetForPanes = useCallback((paneIdsToRun: string[]) => {
    if (paneIdsToRun.length === 0) return;
    setActivePaneId(paneIdsToRun[0]);
    const state = useWorkspaceStore.getState();
    if (!state.snippetPickerOpen) {
      state.toggleSnippetPicker();
    }
  }, [setActivePaneId]);

  const movePaneIdsToSurface = useCallback((paneIdsToMove: string[], targetWorkspaceId: string, targetSurfaceId: string) => {
    if (paneIdsToMove.length === 0) return;
    let targetSurfaceSeedConsumed = false;

    for (let index = 0; index < paneIdsToMove.length; index += 1) {
      const sourcePaneId = paneIdsToMove[index];
      const sourcePanel = panelByPane.get(sourcePaneId);
      const sourceSessionId = sourcePanel?.sessionId ?? findLeaf(surface.layout, sourcePaneId)?.sessionId ?? null;
      const sourceName = surface.paneNames[sourcePaneId] ?? sourcePaneId;
      const sourceIcon = surface.paneIcons[sourcePaneId] ?? "terminal";

      setActiveWorkspace(targetWorkspaceId);
      setActiveSurface(targetSurfaceId);

      const targetWorkspace = useWorkspaceStore.getState().workspaces.find((entry) => entry.id === targetWorkspaceId);
      const targetSurface = targetWorkspace?.surfaces.find((entry) => entry.id === targetSurfaceId);
      if (!targetSurface) continue;

      let newPaneId: string | null = null;
      if (targetSurface.layoutMode === "canvas") {
        newPaneId = createCanvasPanel(targetSurface.id, {
          paneName: sourceName,
          paneIcon: sourceIcon,
          sessionId: sourceSessionId,
        });
      } else {
        const targetPaneIds = allLeafIds(targetSurface.layout);
        const activeTargetPaneId = targetSurface.activePaneId ?? targetPaneIds[0] ?? null;
        const canReuseSeedPane = !targetSurfaceSeedConsumed && targetPaneIds.length === 1 && activeTargetPaneId;
        if (canReuseSeedPane && index === 0) {
          newPaneId = activeTargetPaneId;
          targetSurfaceSeedConsumed = true;
        } else {
          splitActive("horizontal", sourceName, {
            sessionId: sourceSessionId,
            paneIcon: sourceIcon,
          });
          newPaneId = useWorkspaceStore.getState().activePaneId();
          targetSurfaceSeedConsumed = true;
        }
      }

      if (!newPaneId) continue;
      setPaneName(newPaneId, sourceName);
      setPaneIcon(newPaneId, sourceIcon);
      if (sourceSessionId) {
        setPaneSessionId(newPaneId, sourceSessionId);
      }
      if (targetSurface.layoutMode === "canvas" && sourcePanel) {
        resizeCanvasPanel(newPaneId, sourcePanel.width, sourcePanel.height);
        const offset = 24 * (index + 1);
        moveCanvasPanel(newPaneId, sourcePanel.x + offset, sourcePanel.y + offset);
      }

      setActiveWorkspace(surface.workspaceId);
      setActiveSurface(surface.id);
      closePane(sourcePaneId, { stopSession: false, captureTranscript: false });
    }

    setSelectedPaneIds([]);
    setContextMenu(null);
  }, [
    closePane,
    createCanvasPanel,
    moveCanvasPanel,
    panelByPane,
    resizeCanvasPanel,
    setActiveSurface,
    setActiveWorkspace,
    setPaneIcon,
    setPaneName,
    setPaneSessionId,
    splitActive,
    surface.id,
    surface.paneIcons,
    surface.paneNames,
    surface.workspaceId,
  ]);

  const movePaneIdsToWorkspace = useCallback((paneIdsToMove: string[], workspaceId: string) => {
    const targetWorkspace = workspaces.find((entry) => entry.id === workspaceId);
    const targetSurface = targetWorkspace?.surfaces.find((entry) => entry.id === targetWorkspace.activeSurfaceId)
      ?? targetWorkspace?.surfaces[0];
    if (!targetSurface) return;
    movePaneIdsToSurface(paneIdsToMove, workspaceId, targetSurface.id);
  }, [movePaneIdsToSurface, workspaces]);

  const convertPaneIdsToBsp = useCallback((paneIdsToConvert: string[]) => {
    createSurface(surface.workspaceId, { layoutMode: "bsp" });
    const workspace = useWorkspaceStore.getState().workspaces.find((entry) => entry.id === surface.workspaceId);
    const targetSurfaceId = workspace?.activeSurfaceId;
    if (!targetSurfaceId) return;
    renameSurface(targetSurfaceId, "Converted from Canvas");
    movePaneIdsToSurface(paneIdsToConvert, surface.workspaceId, targetSurfaceId);
  }, [createSurface, movePaneIdsToSurface, renameSurface, surface.workspaceId]);

  const duplicatePaneIds = useCallback(async (paneIdsToDuplicate: string[]) => {
    if (paneIdsToDuplicate.length === 0) return;
    const createdPaneIds: string[] = [];

    for (let index = 0; index < paneIdsToDuplicate.length; index += 1) {
      const sourcePaneId = paneIdsToDuplicate[index];
      const sourcePanel = panelByPane.get(sourcePaneId);
      const sourceSessionId = resolveDuplicateSourceSessionId(
        sourcePaneId,
        sourcePanel?.sessionId ?? findLeaf(surface.layout, sourcePaneId)?.sessionId ?? null,
        operationalEvents,
      );
      const sourceWorkspace = useWorkspaceStore.getState().workspaces.find((w) => w.id === surface.workspaceId);
      const cloneResult = await cloneSessionForDuplication(sourcePaneId, sourceSessionId, {
        workspaceId: surface.workspaceId,
        cwd: sourceWorkspace?.cwd || null,
      });
      setActivePaneId(sourcePaneId);
      const duplicatedPaneId = createCanvasPanel(surface.id, {
        paneName: `${surface.paneNames[sourcePaneId] ?? sourcePaneId} Copy`,
        paneIcon: surface.paneIcons[sourcePaneId] ?? "terminal",
        sessionId: cloneResult?.sessionId ?? null,
        ...(sourcePanel
          ? {
            width: sourcePanel.width,
            height: sourcePanel.height,
            x: sourcePanel.x + 28 * (index + 1),
            y: sourcePanel.y + 28 * (index + 1),
          }
          : {}),
      });
      if (!duplicatedPaneId) continue;

      const bootstrapCommand =
        resolveDuplicateActiveBootstrapCommand(sourcePaneId, operationalEvents)
        ?? resolveDuplicateBootstrapCommand(sourcePaneId, operationalEvents)
        ?? cloneResult?.activeCommand;
      if (bootstrapCommand) {
        queuePaneBootstrapCommand(duplicatedPaneId, bootstrapCommand);
      }

      createdPaneIds.push(duplicatedPaneId);
    }

    if (createdPaneIds.length > 0) {
      setSelectedPaneIds(createdPaneIds);
      setActivePaneId(createdPaneIds[0]);
    }
  }, [
    createCanvasPanel,
    panelByPane,
    setActivePaneId,
    surface.id,
    surface.paneIcons,
    surface.paneNames,
    surface.layout,
    surface.workspaceId,
    operationalEvents,
  ]);

  useEffect(() => {
    if (!surface.activePaneId) {
      return;
    }
    const currentNonce = surface.canvasState.focusRequestNonce ?? 0;
    if (currentNonce === 0 || currentNonce === lastFocusRequestNonceRef.current) {
      return;
    }
    lastFocusRequestNonceRef.current = currentNonce;
    centerOnPanel(surface.activePaneId);
  }, [centerOnPanel, surface.activePaneId, surface.canvasState.focusRequestNonce]);

  const beginPan = useCallback((startX: number, startY: number, pointerId: number, target: HTMLElement, fromRightButton: boolean) => {
    if (panCleanupRef.current) {
      panCleanupRef.current();
      panCleanupRef.current = null;
    }

    setContextMenu(null);
    setIconPicker(null);

    if (fromRightButton) {
      suppressContextMenuRef.current = true;
    }

    const startPanX = viewRef.current.panX;
    const startPanY = viewRef.current.panY;
    const startZoom = viewRef.current.zoomLevel;
    setIsPanning(true);
    isPanningRef.current = true;

    const suppressContextMenu = (event: Event) => {
      event.preventDefault();
      event.stopPropagation();
    };

    const onMove = (moveEvent: PointerEvent) => {
      const dx = moveEvent.clientX - startX;
      const dy = moveEvent.clientY - startY;
      queueViewUpdate(startPanX + dx, startPanY + dy, startZoom);
    };

    const onStop = () => {
      setIsPanning(false);
      isPanningRef.current = false;
      window.removeEventListener("pointermove", onMove, true);
      window.removeEventListener("pointerup", onStop, true);
      window.removeEventListener("pointercancel", onStop, true);
      window.removeEventListener("contextmenu", suppressContextMenu, true);
      if (fromRightButton) {
        window.setTimeout(() => {
          suppressContextMenuRef.current = false;
        }, 120);
      }
      panCleanupRef.current = null;
    };

    target.setPointerCapture(pointerId);
    window.addEventListener("pointermove", onMove, true);
    window.addEventListener("pointerup", onStop, true);
    window.addEventListener("pointercancel", onStop, true);
    window.addEventListener("contextmenu", suppressContextMenu, true);

    panCleanupRef.current = onStop;
  }, [queueViewUpdate]);

  useEffect(() => {
    const viewport = viewportRef.current;
    if (!viewport) return;

    const onPointerDownCapture = (event: PointerEvent) => {
      const target = event.target as HTMLElement | null;
      const inToolbar = Boolean(target?.closest("[data-canvas-toolbar='true']"));
      const inPanel = Boolean(target?.closest("[data-canvas-panel='true']"));
      if (event.button === 0) {
        if (target?.closest("[data-canvas-toolbar='true'], [data-canvas-panel='true'], [data-canvas-menu='true']")) {
          return;
        }
        if (!event.ctrlKey && !event.metaKey) {
          setContextMenu(null);
          setIconPicker(null);
          return;
        }

        event.preventDefault();
        event.stopPropagation();
        setContextMenu(null);
        setIconPicker(null);

        const rect = viewport.getBoundingClientRect();
        const toWorld = (clientX: number, clientY: number) => {
          const current = viewRef.current;
          return {
            x: (clientX - rect.left - current.panX) / current.zoomLevel,
            y: (clientY - rect.top - current.panY) / current.zoomLevel,
          };
        };

        const worldStart = toWorld(event.clientX, event.clientY);
        setMarquee({
          startX: worldStart.x,
          startY: worldStart.y,
          currentX: worldStart.x,
          currentY: worldStart.y,
          additive: event.shiftKey,
        });

        const onMove = (moveEvent: PointerEvent) => {
          const worldPoint = toWorld(moveEvent.clientX, moveEvent.clientY);
          setMarquee((current) => (current
            ? { ...current, currentX: worldPoint.x, currentY: worldPoint.y }
            : current));
        };

        const onStop = () => {
          setMarquee((current) => {
            if (!current) return null;
            const minX = Math.min(current.startX, current.currentX);
            const minY = Math.min(current.startY, current.currentY);
            const maxX = Math.max(current.startX, current.currentX);
            const maxY = Math.max(current.startY, current.currentY);
            const nextSelection = panels
              .filter((panel) => (
                panel.x <= maxX
                && panel.x + panel.width >= minX
                && panel.y <= maxY
                && panel.y + panel.height >= minY
              ))
              .map((panel) => panel.paneId);

            setSelectedPaneIds((prev) => {
              if (current.additive) {
                return Array.from(new Set([...prev, ...nextSelection]));
              }
              return nextSelection;
            });

            return null;
          });
          window.removeEventListener("pointermove", onMove, true);
          window.removeEventListener("pointerup", onStop, true);
          window.removeEventListener("pointercancel", onStop, true);
        };

        window.addEventListener("pointermove", onMove, true);
        window.addEventListener("pointerup", onStop, true);
        window.addEventListener("pointercancel", onStop, true);
        return;
      }

      if (event.button === 2) {
        if (inToolbar || inPanel) {
          return;
        }

        setContextMenu(null);
        setIconPicker(null);

        const startX = event.clientX;
        const startY = event.clientY;
        const pointerId = event.pointerId;
        let startedPan = false;

        const cleanupProbe = () => {
          window.removeEventListener("pointermove", onProbeMove, true);
          window.removeEventListener("pointerup", onProbeEnd, true);
          window.removeEventListener("pointercancel", onProbeEnd, true);
        };

        const onProbeMove = (moveEvent: PointerEvent) => {
          if (startedPan) return;
          if (Math.abs(moveEvent.clientX - startX) < 4 && Math.abs(moveEvent.clientY - startY) < 4) {
            return;
          }
          startedPan = true;
          suppressContextMenuUntilRef.current = Date.now() + 420;
          moveEvent.preventDefault();
          moveEvent.stopPropagation();
          cleanupProbe();
          beginPan(startX, startY, pointerId, viewport, true);
        };

        const onProbeEnd = () => {
          cleanupProbe();
        };

        window.addEventListener("pointermove", onProbeMove, true);
        window.addEventListener("pointerup", onProbeEnd, true);
        window.addEventListener("pointercancel", onProbeEnd, true);
        return;
      }

      const canPan = event.button === 1;
      if (!canPan) {
        return;
      }

      if (inToolbar) {
        return;
      }

      event.preventDefault();
      event.stopPropagation();
      beginPan(event.clientX, event.clientY, event.pointerId, viewport, false);
    };

    viewport.addEventListener("pointerdown", onPointerDownCapture, true);
    return () => {
      viewport.removeEventListener("pointerdown", onPointerDownCapture, true);
    };
  }, [beginPan, panels]);

  useEffect(() => {
    const viewport = viewportRef.current;
    if (!viewport) return;

    const onWheel = (event: WheelEvent) => {
      if (!event.ctrlKey) {
        return;
      }
      event.preventDefault();
      const activeElement = document.activeElement;
      if (activeElement instanceof HTMLElement && viewport.contains(activeElement)) {
        activeElement.blur();
      }
      if (activePaneRef.current) {
        clearActivePaneFocus(surface.id);
        activePaneRef.current = null;
      }
      const rect = viewport.getBoundingClientRect();
      const pointerX = event.clientX - rect.left;
      const pointerY = event.clientY - rect.top;
      const zoomFactor = event.deltaY > 0 ? 0.92 : 1.08;
      const current = viewRef.current;
      const nextZoom = clamp(current.zoomLevel * zoomFactor, MIN_CANVAS_ZOOM, MAX_CANVAS_ZOOM);

      const worldX = (pointerX - current.panX) / current.zoomLevel;
      const worldY = (pointerY - current.panY) / current.zoomLevel;
      const nextPanX = pointerX - worldX * nextZoom;
      const nextPanY = pointerY - worldY * nextZoom;
      queueViewUpdate(nextPanX, nextPanY, nextZoom);
    };

    viewport.addEventListener("wheel", onWheel, { passive: false });
    return () => viewport.removeEventListener("wheel", onWheel);
  }, [clearActivePaneFocus, queueViewUpdate, surface.id]);

  const handleCenterView = useCallback(() => {
    const viewport = viewportRef.current;
    if (!viewport || surface.canvasPanels.length === 0) return;
    const rect = viewport.getBoundingClientRect();
    const zoom = viewRef.current.zoomLevel;
    const minX = Math.min(...surface.canvasPanels.map((panel) => panel.x));
    const minY = Math.min(...surface.canvasPanels.map((panel) => panel.y));
    const maxX = Math.max(...surface.canvasPanels.map((panel) => panel.x + panel.width));
    const maxY = Math.max(...surface.canvasPanels.map((panel) => panel.y + panel.height));
    const centerX = (minX + maxX) / 2;
    const centerY = (minY + maxY) / 2;
    const nextPanX = rect.width / 2 - centerX * zoom;
    const nextPanY = rect.height / 2 - centerY * zoom;
    setAnimateTransform(true);
    queueViewUpdate(nextPanX, nextPanY, zoom);
    window.setTimeout(() => setAnimateTransform(false), 220);
  }, [queueViewUpdate, surface.canvasPanels]);

  const gridMetrics = useMemo(() => {
    const size = Math.max(8, CANVAS_GRID_SIZE * surface.canvasState.zoomLevel);
    // Radial gradients render each dot in the center of a tile.
    // Shift by half a tile so world grid lines (snap origin) and visible dots align.
    const offsetX = (((surface.canvasState.panX - size / 2) % size) + size) % size;
    const offsetY = (((surface.canvasState.panY - size / 2) % size) + size) % size;
    return { size, offsetX, offsetY };
  }, [surface.canvasState.panX, surface.canvasState.panY, surface.canvasState.zoomLevel]);

  const handlePanelActivate = useCallback((paneId: string, event: ReactMouseEvent<HTMLDivElement>) => {
    dragGroupBaseRef.current = null;
    const toggle = event.metaKey || event.ctrlKey;
    const additive = event.shiftKey;
    selectPanel(paneId, { toggle, additive });
    if (toggle || additive) {
      return;
    }
    setActivePaneId(paneId);
    setContextMenu(null);
    setIconPicker(null);
  }, [selectPanel, setActivePaneId]);

  const handlePanelContextMenu = useCallback((paneId: string, event: ReactMouseEvent<HTMLDivElement>) => {
    event.preventDefault();
    dragGroupBaseRef.current = null;
    const toggle = event.metaKey || event.ctrlKey;
    const additive = event.shiftKey;
    selectPanel(paneId, { toggle, additive, preserveIfAlreadySelected: !toggle && !additive });
    setContextMenu({
      paneId,
      x: event.clientX,
      y: event.clientY,
    });
    setIconPicker(null);
  }, [selectPanel]);

  const handlePanelMoveStart = useCallback((paneId: string) => {
    setContextMenu(null);
    setIconPicker(null);
    if (selectedPaneSet.has(paneId) && selectedPaneIds.length > 1) {
      dragGroupBaseRef.current = new Map(
        panels
          .filter((panel) => selectedPaneSet.has(panel.paneId))
          .map((panel) => [panel.paneId, { x: panel.x, y: panel.y }]),
      );
      return;
    }
    dragGroupBaseRef.current = null;
  }, [panels, selectedPaneIds.length, selectedPaneSet]);

  const handlePanelMove = useCallback((paneId: string, nextX: number, nextY: number) => {
    const basePositions = dragGroupBaseRef.current;
    if (basePositions && basePositions.has(paneId)) {
      const sourceBase = basePositions.get(paneId);
      if (!sourceBase) return;
      const dx = nextX - sourceBase.x;
      const dy = nextY - sourceBase.y;
      for (const selectedPaneId of selectedPaneIds) {
        const base = basePositions.get(selectedPaneId);
        if (!base) continue;
        moveCanvasPanel(selectedPaneId, base.x + dx, base.y + dy);
      }
      return;
    }
    moveCanvasPanel(paneId, nextX, nextY);
  }, [moveCanvasPanel, selectedPaneIds]);

  const handlePanelMoveEnd = useCallback((paneId: string, nextX: number, nextY: number) => {
    const basePositions = dragGroupBaseRef.current;
    if (basePositions && basePositions.has(paneId)) {
      if (snapEnabled) {
        for (const selectedPaneId of selectedPaneIds) {
          const panel = panelByPane.get(selectedPaneId);
          if (!panel) continue;
          moveCanvasPanel(
            selectedPaneId,
            snapToGrid(panel.x, CANVAS_GRID_SIZE),
            snapToGrid(panel.y, CANVAS_GRID_SIZE),
          );
        }
      }
      dragGroupBaseRef.current = null;
      return;
    }

    if (snapEnabled) {
      moveCanvasPanel(
        paneId,
        snapToGrid(nextX, CANVAS_GRID_SIZE),
        snapToGrid(nextY, CANVAS_GRID_SIZE),
      );
    }
  }, [moveCanvasPanel, panelByPane, selectedPaneIds, snapEnabled]);

  const contextPaneIds = useMemo(() => {
    if (!contextMenu) return [];
    return resolveContextPaneIds(contextMenu.paneId);
  }, [contextMenu, resolveContextPaneIds]);

  const workspaceMoveTargets = useMemo(
    () => workspaces.filter((workspace) => workspace.id !== surface.workspaceId),
    [surface.workspaceId, workspaces],
  );

  return (
    <div
      ref={viewportRef}
      style={{
        width: "100%",
        height: "100%",
        position: "relative",
        overflow: "hidden",
        background: "radial-gradient(circle at top right, rgba(96, 165, 250, 0.09), transparent 58%), var(--bg-deep)",
        cursor: isPanning ? "grabbing" : "grab",
      }}
      onContextMenu={(event) => {
        if (event.buttons === 2) {
          event.preventDefault();
          event.stopPropagation();
          return;
        }
        if (Date.now() < suppressContextMenuUntilRef.current) {
          event.preventDefault();
          event.stopPropagation();
          return;
        }
        if (isPanningRef.current || suppressContextMenuRef.current) {
          event.preventDefault();
          event.stopPropagation();
          return;
        }
        const target = event.target as HTMLElement | null;
        if (!target?.closest("[data-canvas-panel='true'], [data-canvas-toolbar='true'], [data-canvas-menu='true']") && selectedPaneIds.length > 0) {
          event.preventDefault();
          setContextMenu({
            paneId: selectedPaneIds[0],
            x: event.clientX,
            y: event.clientY,
          });
        }
      }}
    >
      <div
        aria-hidden
        style={{
          position: "absolute",
          inset: 0,
          pointerEvents: "none",
          zIndex: 0,
          backgroundImage: snapEnabled
            ? "radial-gradient(circle, rgba(148, 163, 184, 0.26) 1px, transparent 1.2px)"
            : "none",
          backgroundSize: snapEnabled ? `${gridMetrics.size}px ${gridMetrics.size}px` : undefined,
          backgroundPosition: snapEnabled ? `${gridMetrics.offsetX}px ${gridMetrics.offsetY}px` : undefined,
        }}
      />

      <div
        style={{
          position: "absolute",
          inset: 0,
          zIndex: 1,
          transform: `translate3d(${surface.canvasState.panX}px, ${surface.canvasState.panY}px, 0) scale(${surface.canvasState.zoomLevel})`,
          transformOrigin: "0 0",
          transition: animateTransform ? "transform 240ms ease" : "none",
          willChange: "transform",
        }}
      >
        {panels.map((panel) => {
          const isActive = panel.paneId === surface.activePaneId;
          const isSelected = selectedPaneSet.has(panel.paneId);

          return (
            <CanvasPanelShell
              key={panel.paneId}
              panel={panel}
              zoomLevel={surface.canvasState.zoomLevel}
              active={isActive}
              selected={isSelected}
              onActivate={(event) => handlePanelActivate(panel.paneId, event)}
              onContextMenu={(event) => handlePanelContextMenu(panel.paneId, event)}
              onMoveStart={() => handlePanelMoveStart(panel.paneId)}
              onMove={(x, y) => handlePanelMove(panel.paneId, x, y)}
              onMoveEnd={(x, y) => handlePanelMoveEnd(panel.paneId, x, y)}
              onResize={(width, height) => resizeCanvasPanel(panel.paneId, width, height)}
              onDoubleClick={() => centerOnPanel(panel.paneId, { storePrevious: true, zoomIn: true })}
              onRequestClose={() => requestClosePanes(panel.paneId)}
            >
              <TerminalPane paneId={panel.paneId} sessionId={panel.sessionId ?? undefined} />
            </CanvasPanelShell>
          );
        })}
      </div>

      {marquee ? (
        <div
          aria-hidden
          style={{
            position: "absolute",
            zIndex: 25,
            pointerEvents: "none",
            left: Math.min(marquee.startX, marquee.currentX) * surface.canvasState.zoomLevel + surface.canvasState.panX,
            top: Math.min(marquee.startY, marquee.currentY) * surface.canvasState.zoomLevel + surface.canvasState.panY,
            width: Math.abs(marquee.currentX - marquee.startX) * surface.canvasState.zoomLevel,
            height: Math.abs(marquee.currentY - marquee.startY) * surface.canvasState.zoomLevel,
            border: "1px solid var(--accent)",
            background: "rgba(94, 231, 223, 0.14)",
          }}
        />
      ) : null}

      <div data-canvas-toolbar="true" style={{ position: "absolute", top: 10, left: 10, display: "flex", gap: 8, zIndex: 40 }}>
        <button
          type="button"
          onClick={handleCreatePanel}
          title="Add panel"
          style={{
            height: 30,
            minWidth: 32,
            borderRadius: "var(--radius-md)",
            border: "1px solid var(--accent)",
            background: "var(--accent-soft)",
            color: "var(--accent)",
            fontSize: 18,
            lineHeight: 1,
            cursor: "pointer",
          }}
        >
          +
        </button>

        <button
          type="button"
          onClick={() => arrangeCanvasPanels(surface.id)}
          title="Auto arrange panels"
          style={{
            height: 30,
            borderRadius: "var(--radius-md)",
            border: "1px solid var(--border)",
            background: "var(--bg-secondary)",
            color: "var(--text-secondary)",
            fontSize: 12,
            padding: "0 10px",
            cursor: "pointer",
          }}
        >
          Arrange
        </button>

        <button
          type="button"
          onClick={() => setSnapEnabled((value) => !value)}
          title="Toggle grid snap"
          style={{
            height: 30,
            borderRadius: "var(--radius-md)",
            border: snapEnabled ? "1px solid var(--accent)" : "1px solid var(--border)",
            background: snapEnabled ? "var(--accent-soft)" : "var(--bg-secondary)",
            color: snapEnabled ? "var(--accent)" : "var(--text-secondary)",
            fontSize: 12,
            padding: "0 10px",
            cursor: "pointer",
          }}
        >
          Snap
        </button>

        <button
          type="button"
          onClick={handleCenterView}
          title="Center view"
          style={{
            height: 30,
            borderRadius: "var(--radius-md)",
            border: "1px solid var(--border)",
            background: "var(--bg-secondary)",
            color: "var(--text-secondary)",
            fontSize: 12,
            padding: "0 10px",
            cursor: "pointer",
          }}
        >
          Center
        </button>

        {surface.canvasState.previousView ? (
          <button
            type="button"
            onClick={() => {
              setAnimateTransform(true);
              queueViewUpdate(
                surface.canvasState.previousView?.panX ?? 0,
                surface.canvasState.previousView?.panY ?? 0,
                surface.canvasState.previousView?.zoomLevel ?? 1,
              );
              setCanvasPreviousView(surface.id, null);
              window.setTimeout(() => setAnimateTransform(false), 260);
            }}
            title="Return to previous view"
            style={{
              height: 30,
              borderRadius: "var(--radius-md)",
              border: "1px solid var(--glass-border)",
              background: "var(--bg-secondary)",
              color: "var(--text-secondary)",
              fontSize: 12,
              padding: "0 10px",
              cursor: "pointer",
            }}
          >
            Back to previous
          </button>
        ) : null}
      </div>

      {contextMenu ? (
        <div
          data-canvas-menu="true"
          style={{
            position: "fixed",
            left: contextMenu.x,
            top: contextMenu.y,
            zIndex: 120,
            minWidth: 210,
            border: "1px solid var(--glass-border)",
            borderRadius: "var(--radius-md)",
            background: "var(--bg-primary)",
            boxShadow: "var(--shadow-sm)",
            padding: 4,
            display: "grid",
            gap: 2,
          }}
        >
          <button
            type="button"
            style={contextMenuItemStyle}
            onClick={() => {
              if (contextPaneIds.length > 0) {
                centerOnPanel(contextPaneIds[0], { storePrevious: true, zoomIn: true });
              }
              setContextMenu(null);
            }}
          >
            Zoom In
          </button>
          <button
            type="button"
            style={contextMenuItemStyle}
            onClick={() => {
              void duplicatePaneIds(contextPaneIds);
              setContextMenu(null);
            }}
          >
            {contextPaneIds.length > 1
              ? `Duplicate ${contextPaneIds.length} Panels`
              : "Duplicate Panel"}
          </button>
          <button
            type="button"
            style={contextMenuItemStyle}
            onClick={() => {
              convertPaneIdsToBsp(contextPaneIds);
            }}
          >
            {contextPaneIds.length > 1 ? `Convert ${contextPaneIds.length} to BSP` : "Convert to BSP"}
          </button>
          <button
            type="button"
            style={contextMenuItemStyle}
            onClick={() => {
              runSnippetForPanes(contextPaneIds);
              setContextMenu(null);
            }}
          >
            Run Snippet
          </button>
          <button
            type="button"
            style={contextMenuItemStyle}
            onClick={() => {
              setIconPicker({
                x: contextMenu.x,
                y: contextMenu.y,
                paneIds: contextPaneIds,
              });
              setContextMenu(null);
            }}
          >
            {contextPaneIds.length > 1
              ? `Change Icon (${contextPaneIds.length} Panels)`
              : "Change Icon"}
          </button>
          {workspaceMoveTargets.length > 0 ? (
            <>
              <div style={contextMenuSectionLabelStyle}>Move to workspace</div>
              {workspaceMoveTargets.map((workspace) => (
                <button
                  key={workspace.id}
                  type="button"
                  style={contextMenuItemStyle}
                  onClick={() => {
                    movePaneIdsToWorkspace(contextPaneIds, workspace.id);
                  }}
                >
                  {workspace.name}
                </button>
              ))}
            </>
          ) : null}
          <button
            type="button"
            style={dangerContextMenuItemStyle}
            onClick={() => {
              requestClosePanes(contextMenu.paneId);
              setContextMenu(null);
            }}
          >
            {contextPaneIds.length > 1
              ? `Close ${contextPaneIds.length} Panels`
              : "Close Panel"}
          </button>
        </div>
      ) : null}

      {iconPicker ? (
        <div
          data-canvas-menu="true"
          style={{
            position: "fixed",
            left: iconPicker.x,
            top: iconPicker.y,
            zIndex: 130,
            minWidth: 180,
            border: "1px solid var(--glass-border)",
            borderRadius: "var(--radius-md)",
            background: "var(--bg-secondary)",
            boxShadow: "var(--shadow-sm)",
            padding: 4,
            display: "grid",
            gap: 2,
          }}
        >
          {iconChoices(PANE_ICON_IDS).map((icon) => (
            <button
              key={icon.id}
              type="button"
              style={contextMenuItemStyle}
              onClick={() => {
                for (const paneId of iconPicker.paneIds) {
                  setCanvasPanelIcon(paneId, icon.id);
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
        open={confirmClosePaneIds.length > 0}
        title={confirmClosePaneIds.length > 1
          ? `Close ${confirmClosePaneIds.length} panels?`
          : confirmClosePaneIds.length === 1
            ? `Close '${surface.paneNames[confirmClosePaneIds[0]] ?? "panel"}'?`
            : ""}
        message={confirmClosePaneIds.length > 1
          ? "All selected terminal panels will be closed."
          : "This terminal panel will be closed."}
        confirmLabel={confirmClosePaneIds.length > 1 ? `Close ${confirmClosePaneIds.length} Panels` : "Close Panel"}
        tone="danger"
        onCancel={() => setConfirmClosePaneIds([])}
        onConfirm={() => {
          if (confirmClosePaneIds.length === 0) return;
          for (const paneId of confirmClosePaneIds) {
            closePane(paneId);
          }
          setSelectedPaneIds((current) => current.filter((paneId) => !confirmClosePaneIds.includes(paneId)));
          setConfirmClosePaneIds([]);
        }}
      />
    </div>
  );
}

function CanvasPanelShell({
  panel,
  zoomLevel,
  active,
  selected,
  onActivate,
  onContextMenu,
  onMoveStart,
  onMove,
  onMoveEnd,
  onResize,
  onDoubleClick,
  onRequestClose,
  children,
}: {
  panel: {
    paneId: string;
    title: string;
    x: number;
    y: number;
    width: number;
    height: number;
    status: string;
  };
  zoomLevel: number;
  active: boolean;
  selected: boolean;
  onActivate: (event: ReactMouseEvent<HTMLDivElement>) => void;
  onContextMenu: (event: ReactMouseEvent<HTMLDivElement>) => void;
  onMoveStart: () => void;
  onMove: (x: number, y: number) => void;
  onMoveEnd: (x: number, y: number) => void;
  onResize: (width: number, height: number) => void;
  onDoubleClick: () => void;
  onRequestClose: () => void;
  children: ReactNode;
}) {
  const dragRafRef = useRef<number | null>(null);
  const suppressActivationRef = useRef(false);
  const interactionActiveRef = useRef(false);
  const suppressContextMenuUntilRef = useRef(0);

  const handleDragStart = useCallback((event: ReactPointerEvent<HTMLDivElement>) => {
    if (event.button !== 0) return;
    const target = event.target as HTMLElement | null;
    if (target?.closest("button, input, select, textarea, [data-no-drag='true']")) {
      return;
    }
    event.preventDefault();
    event.stopPropagation();

    const pointerId = event.pointerId;
    const startX = event.clientX;
    const startY = event.clientY;
    const baseX = panel.x;
    const baseY = panel.y;
    let lastX = baseX;
    let lastY = baseY;
    let moved = false;
    interactionActiveRef.current = true;
    const host = event.currentTarget;
    onMoveStart();

    const onMovePointer = (moveEvent: PointerEvent) => {
      if (!moved && (Math.abs(moveEvent.clientX - startX) > 3 || Math.abs(moveEvent.clientY - startY) > 3)) {
        moved = true;
      }
      const zoom = Math.max(0.001, zoomLevel);
      const nextX = baseX + (moveEvent.clientX - startX) / zoom;
      const nextY = baseY + (moveEvent.clientY - startY) / zoom;
      lastX = nextX;
      lastY = nextY;
      if (dragRafRef.current !== null) {
        window.cancelAnimationFrame(dragRafRef.current);
      }
      dragRafRef.current = window.requestAnimationFrame(() => {
        onMove(nextX, nextY);
        dragRafRef.current = null;
      });
    };

    const onPointerUp = () => {
      if (moved) {
        suppressActivationRef.current = true;
        suppressContextMenuUntilRef.current = Date.now() + 220;
        window.setTimeout(() => {
          suppressActivationRef.current = false;
        }, 120);
        onMoveEnd(lastX, lastY);
      }
      interactionActiveRef.current = false;
      if (dragRafRef.current !== null) {
        window.cancelAnimationFrame(dragRafRef.current);
        dragRafRef.current = null;
      }
      window.removeEventListener("pointermove", onMovePointer);
      window.removeEventListener("pointerup", onPointerUp);
      window.removeEventListener("pointercancel", onPointerUp);
    };

    host.setPointerCapture(pointerId);
    window.addEventListener("pointermove", onMovePointer);
    window.addEventListener("pointerup", onPointerUp);
    window.addEventListener("pointercancel", onPointerUp);
  }, [onMove, onMoveEnd, onMoveStart, panel.x, panel.y, zoomLevel]);

  const handleResizeStart = useCallback((event: ReactPointerEvent<HTMLDivElement>) => {
    if (event.button !== 0) return;
    event.preventDefault();
    event.stopPropagation();

    const pointerId = event.pointerId;
    const startX = event.clientX;
    const startY = event.clientY;
    const baseWidth = panel.width;
    const baseHeight = panel.height;
    let moved = false;
    interactionActiveRef.current = true;
    const host = event.currentTarget;

    const onMovePointer = (moveEvent: PointerEvent) => {
      if (!moved && (Math.abs(moveEvent.clientX - startX) > 2 || Math.abs(moveEvent.clientY - startY) > 2)) {
        moved = true;
      }
      const zoom = Math.max(0.001, zoomLevel);
      const nextWidth = Math.max(320, baseWidth + (moveEvent.clientX - startX) / zoom);
      const nextHeight = Math.max(220, baseHeight + (moveEvent.clientY - startY) / zoom);
      if (dragRafRef.current !== null) {
        window.cancelAnimationFrame(dragRafRef.current);
      }
      dragRafRef.current = window.requestAnimationFrame(() => {
        onResize(nextWidth, nextHeight);
        dragRafRef.current = null;
      });
    };

    const onPointerUp = () => {
      if (moved) {
        suppressActivationRef.current = true;
        suppressContextMenuUntilRef.current = Date.now() + 220;
        window.setTimeout(() => {
          suppressActivationRef.current = false;
        }, 120);
      }
      interactionActiveRef.current = false;
      if (dragRafRef.current !== null) {
        window.cancelAnimationFrame(dragRafRef.current);
        dragRafRef.current = null;
      }
      window.removeEventListener("pointermove", onMovePointer);
      window.removeEventListener("pointerup", onPointerUp);
      window.removeEventListener("pointercancel", onPointerUp);
    };

    host.setPointerCapture(pointerId);
    window.addEventListener("pointermove", onMovePointer);
    window.addEventListener("pointerup", onPointerUp);
    window.addEventListener("pointercancel", onPointerUp);
  }, [onResize, panel.height, panel.width, zoomLevel]);

  return (
    <div
      data-canvas-panel="true"
      onClickCapture={(event) => {
        if (suppressActivationRef.current) {
          event.preventDefault();
          event.stopPropagation();
        }
      }}
      onClick={(event) => {
        if (event.defaultPrevented || event.button !== 0) return;
        if (suppressActivationRef.current) {
          return;
        }
        onActivate(event);
      }}
      onContextMenu={(event) => {
        if (suppressActivationRef.current || interactionActiveRef.current || Date.now() < suppressContextMenuUntilRef.current) {
          event.preventDefault();
          event.stopPropagation();
          return;
        }
        onContextMenu(event);
      }}
      onDoubleClick={(event) => {
        if (event.button !== 0) return;
        if (suppressActivationRef.current) return;
        event.preventDefault();
        event.stopPropagation();
        onDoubleClick();
      }}
      style={{
        position: "absolute",
        left: panel.x,
        top: panel.y,
        width: panel.width,
        height: panel.height,
        border: "1px solid var(--glass-border)",
        boxShadow: panel.status === "needs_approval"
          ? "0 0 0 1px var(--approval), 0 0 24px rgba(251, 191, 36, 0.22)"
          : "none",
        background: "var(--bg-primary)",
        borderRadius: "8px",
        overflow: "hidden",
        willChange: "transform",
      }}
    >
      <div
        onPointerDown={handleDragStart}
        style={{
          height: 28,
          display: "flex",
          alignItems: "center",
          justifyContent: "space-between",
          padding: "0 10px",
          borderBottom: "1px solid var(--glass-border)",
          background: panel.status === "needs_approval"
            ? "var(--approval-soft)"
            : (selected || active)
              ? "var(--human-soft)"
              : "var(--bg-secondary)",
          color: panel.status === "needs_approval"
            ? "var(--approval)"
            : (selected || active)
              ? "var(--human)"
              : "var(--text-secondary)",
          cursor: "grab",
          userSelect: "none",
          fontSize: "var(--text-xs)",
          letterSpacing: "0.04em",
          textTransform: "uppercase",
        }}
      >
        <span style={{ overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap" }}>{panel.title}</span>
        <div style={{ display: "flex", alignItems: "center", gap: 8 }}>
          {panel.status === "needs_approval" ? (
            <span style={{ color: "var(--approval)", fontWeight: 700 }}>action required</span>
          ) : null}
          <button
            type="button"
            onClick={(event) => {
              event.preventDefault();
              event.stopPropagation();
              onRequestClose();
            }}
            title="Close panel"
            aria-label="Close panel"
            style={{
              border: "none",
              background: "transparent",
              color: panel.status === "needs_approval"
                ? "var(--approval)"
                : (selected || active)
                  ? "var(--human)"
                  : "var(--text-muted)",
              cursor: "pointer",
              fontSize: "var(--text-sm)",
              lineHeight: 1,
              width: 22,
              height: 22,
              borderRadius: 6,
              padding: 0,
              display: "inline-flex",
              alignItems: "center",
              justifyContent: "center",
              flexShrink: 0,
            }}
          >
            ×
          </button>
        </div>
      </div>

      <div style={{ width: "100%", height: "calc(100% - 28px)" }}>{children}</div>

      <div
        onPointerDown={handleResizeStart}
        style={{
          position: "absolute",
          right: 0,
          bottom: 0,
          width: 14,
          height: 14,
          cursor: "nwse-resize",
          background: "linear-gradient(135deg, transparent 45%, var(--glass-border) 45%, var(--glass-border) 60%, transparent 60%)",
        }}
      />
    </div>
  );
}

const contextMenuItemStyle: CSSProperties = {
  border: "none",
  background: "transparent",
  color: "var(--text-primary)",
  padding: "6px 8px",
  borderRadius: "var(--radius-sm)",
  cursor: "pointer",
  textAlign: "left",
  fontSize: "var(--text-sm)",
};

const dangerContextMenuItemStyle: CSSProperties = {
  ...contextMenuItemStyle,
  color: "var(--danger)",
};

const contextMenuSectionLabelStyle: CSSProperties = {
  padding: "6px 8px 2px",
  color: "var(--text-muted)",
  fontSize: "var(--text-2xs)",
  textTransform: "uppercase",
  letterSpacing: "0.04em",
};
