import { useState } from "react";
import { allLeafIds, findLeaf } from "../lib/bspTree";
import { useAgentMissionStore } from "../lib/agentMissionStore";
import {
  cloneSessionForDuplication,
  queuePaneBootstrapCommand,
  resolveDuplicateActiveBootstrapCommand,
  resolveDuplicateBootstrapCommand,
  resolveDuplicateSourceSessionId,
} from "../lib/paneDuplication";
import { useWorkspaceStore } from "../lib/workspaceStore";
import { AppConfirmDialog } from "./AppConfirmDialog";
import { SurfaceTabActions } from "./surface-tab-bar/SurfaceTabActions";
import { SurfaceCreateButton } from "./surface-tab-bar/SurfaceCreateButton";
import { SurfaceTabItem } from "./surface-tab-bar/SurfaceTabItem";
import { dividerStyle } from "./surface-tab-bar/shared";

export function SurfaceTabBar() {
  const ws = useWorkspaceStore((s) => s.activeWorkspace());
  const createSurface = useWorkspaceStore((s) => s.createSurface);
  const createCanvasPanel = useWorkspaceStore((s) => s.createCanvasPanel);
  const closeSurface = useWorkspaceStore((s) => s.closeSurface);
  const setActiveSurface = useWorkspaceStore((s) => s.setActiveSurface);
  const renameSurface = useWorkspaceStore((s) => s.renameSurface);
  const setSurfaceIcon = useWorkspaceStore((s) => s.setSurfaceIcon);
  const toggleSidebar = useWorkspaceStore((s) => s.toggleSidebar);
  const sidebarVisible = useWorkspaceStore((s) => s.sidebarVisible);
  const splitActive = useWorkspaceStore((s) => s.splitActive);
  const applyPresetLayout = useWorkspaceStore((s) => s.applyPresetLayout);
  const equalizeLayout = useWorkspaceStore((s) => s.equalizeLayout);
  const toggleZoom = useWorkspaceStore((s) => s.toggleZoom);
  const toggleWebBrowser = useWorkspaceStore((s) => s.toggleWebBrowser);
  const approvals = useAgentMissionStore((s) => s.approvals);
  const operationalEvents = useAgentMissionStore((s) => s.operationalEvents);
  const [pendingCloseSurface, setPendingCloseSurface] = useState<{ id: string; name: string } | null>(null);

  const surfaces = ws?.surfaces ?? [];
  const activeSurfaceId = ws?.activeSurfaceId ?? null;
  const activeSurface = ws?.surfaces.find((surface) => surface.id === activeSurfaceId);
  const activeLayoutMode = activeSurface?.layoutMode ?? "bsp";

  const duplicateSplit = async (direction: "horizontal" | "vertical") => {
    if (!ws || !activeSurface || activeSurface.layoutMode !== "bsp") return;
    const sourcePaneId = activeSurface.activePaneId;
    if (!sourcePaneId) return;

    const sourceSessionId = resolveDuplicateSourceSessionId(
      sourcePaneId,
      findLeaf(activeSurface.layout, sourcePaneId)?.sessionId ?? null,
      operationalEvents,
    );
    const cloneResult = await cloneSessionForDuplication(sourcePaneId, sourceSessionId, {
      workspaceId: ws.id,
      cwd: ws.cwd || null,
    });
    const sourcePaneName = activeSurface.paneNames[sourcePaneId] ?? sourcePaneId;
    const sourcePaneIcon = activeSurface.paneIcons[sourcePaneId] ?? "terminal";

    splitActive(direction, `${sourcePaneName} Copy`, {
      sessionId: cloneResult?.sessionId ?? null,
      paneIcon: sourcePaneIcon,
    });

    const duplicatedPaneId = useWorkspaceStore.getState().activePaneId();
    if (!duplicatedPaneId) return;
    const bootstrapCommand =
      resolveDuplicateActiveBootstrapCommand(sourcePaneId, operationalEvents)
      ?? resolveDuplicateBootstrapCommand(sourcePaneId, operationalEvents)
      ?? cloneResult?.activeCommand;
    if (bootstrapCommand) {
      queuePaneBootstrapCommand(duplicatedPaneId, bootstrapCommand);
    }
  };

  return (
    <div
      style={{
        display: "flex",
        alignItems: "center",
        background: "var(--bg-secondary)",
        borderBottom: "1px solid var(--border)",
        height: "var(--tab-height)",
        overflow: "hidden",
        padding: "0 var(--space-2)",
        gap: "var(--space-2)",
        flexShrink: 0,
      }}
    >
      <button
        onClick={toggleSidebar}
        style={{
          background: sidebarVisible ? "var(--accent-soft)" : "transparent",
          border: "1px solid",
          borderColor: sidebarVisible ? "var(--accent-soft)" : "var(--glass-border)",
          color: sidebarVisible ? "var(--accent)" : "var(--text-muted)",
          cursor: "pointer",
          fontSize: "var(--text-sm)",
          padding: "0 var(--space-2)",
          height: 26,
          minWidth: 28,
          borderRadius: "var(--radius-md)",
          transition: "all var(--transition-fast)",
        }}
        title="Toggle sidebar"
      >
        ☰
      </button>

      <div style={dividerStyle} />

      <SurfaceTabActions
        layoutMode={activeLayoutMode}
        splitActive={splitActive}
        duplicateSplit={(direction) => {
          void duplicateSplit(direction);
        }}
        applyPresetLayout={applyPresetLayout}
        equalizeLayout={equalizeLayout}
        toggleZoom={toggleZoom}
        toggleWebBrowser={toggleWebBrowser}
      />

      <div style={dividerStyle} />

      <div
        style={{
          display: "flex",
          alignItems: "center",
          flex: 1,
          overflow: "auto",
          gap: "var(--space-1)",
        }}
      >
        {surfaces.map((sf) => (
          <SurfaceTabItem
            key={sf.id}
            surface={sf}
            isActive={sf.id === activeSurfaceId}
            accentColor={ws?.accentColor ?? "var(--accent)"}
            approvalCount={approvals.filter((entry) => entry.surfaceId === sf.id && entry.status === "pending").length}
            paneCount={allLeafIds(sf.layout).length}
            onSelect={() => setActiveSurface(sf.id)}
            onClose={() => setPendingCloseSurface({ id: sf.id, name: sf.name })}
            onRename={(name) => renameSurface(sf.id, name)}
            onSetIcon={(icon) => setSurfaceIcon(sf.id, icon)}
          />
        ))}
      </div>

      <SurfaceCreateButton
        layoutMode={activeLayoutMode}
        createBspTerminal={() => {
          if (activeLayoutMode === "bsp") {
            splitActive("horizontal", "New Terminal");
            return;
          }
          createSurface(undefined, { layoutMode: "bsp" });
        }}
        createCanvasSurface={() => createSurface(undefined, { layoutMode: "canvas" })}
        createCanvasTerminal={() => {
          if (activeLayoutMode !== "canvas" || !activeSurfaceId) return;
          createCanvasPanel(activeSurfaceId);
        }}
      />

      <AppConfirmDialog
        open={Boolean(pendingCloseSurface)}
        title={pendingCloseSurface ? `Close surface '${pendingCloseSurface.name}'?` : ""}
        message="All terminals in this surface will be closed."
        confirmLabel="Close Surface"
        tone="danger"
        onCancel={() => setPendingCloseSurface(null)}
        onConfirm={() => {
          if (!pendingCloseSurface) return;
          closeSurface(pendingCloseSurface.id);
          setPendingCloseSurface(null);
        }}
      />
    </div>
  );
}
