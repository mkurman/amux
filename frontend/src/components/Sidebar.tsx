import { useEffect, useMemo, useRef, useState } from "react";
import { useAgentMissionStore } from "../lib/agentMissionStore";
import { useWorkspaceStore } from "../lib/workspaceStore";
import { useNotificationStore } from "../lib/notificationStore";
import { SidebarActions } from "./sidebar/SidebarActions";
import { SidebarHeader } from "./sidebar/SidebarHeader";
import { SidebarResizeHandle } from "./sidebar/SidebarResizeHandle";
import { WorkspaceItem } from "./sidebar/WorkspaceItem";

export function Sidebar() {
  const sidebarWidth = useWorkspaceStore((s) => s.sidebarWidth);
  const workspaces = useWorkspaceStore((s) => s.workspaces);
  const activeWorkspaceId = useWorkspaceStore((s) => s.activeWorkspaceId);
  const setActiveWorkspace = useWorkspaceStore((s) => s.setActiveWorkspace);
  const createWorkspace = useWorkspaceStore((s) => s.createWorkspace);
  const closeWorkspace = useWorkspaceStore((s) => s.closeWorkspace);
  const renameWorkspace = useWorkspaceStore((s) => s.renameWorkspace);
  const setWorkspaceIcon = useWorkspaceStore((s) => s.setWorkspaceIcon);
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
  const approvals = useAgentMissionStore((s) => s.approvals);
  const cognitiveEvents = useAgentMissionStore((s) => s.cognitiveEvents);
  const [query, setQuery] = useState("");
  const sidebarRef = useRef<HTMLDivElement>(null);

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

  const filteredWorkspaces = useMemo(() => {
    const lower = query.trim().toLowerCase();
    if (!lower) return workspaces;
    return workspaces.filter(
      (workspace) =>
        workspace.name.toLowerCase().includes(lower) ||
        workspace.cwd.toLowerCase().includes(lower) ||
        (workspace.gitBranch ?? "").toLowerCase().includes(lower)
    );
  }, [query, workspaces]);

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
        createWorkspace={createWorkspace}
        query={query}
        setQuery={setQuery}
      />

      <div style={{ flex: 1, overflow: "auto", padding: "var(--space-3)" }}>
        <div style={{ display: "flex", flexDirection: "column", gap: "var(--space-2)" }}>
          {filteredWorkspaces.map((ws, idx) => (
            <WorkspaceItem
              key={ws.id}
              workspace={ws}
              index={idx + 1}
              isActive={ws.id === activeWorkspaceId}
              unreadCount={getUnread(ws.id)}
              onSelect={() => setActiveWorkspace(ws.id)}
              onClose={() => closeWorkspace(ws.id)}
              onRename={(name) => renameWorkspace(ws.id, name)}
              onSetIcon={(icon) => setWorkspaceIcon(ws.id, icon)}
            />
          ))}
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
    </div>
  );
}
