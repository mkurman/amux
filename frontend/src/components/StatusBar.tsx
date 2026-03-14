import { useEffect, useMemo, useState } from "react";
import { allLeafIds } from "../lib/bspTree";
import { useWorkspaceStore } from "../lib/workspaceStore";
import { useNotificationStore } from "../lib/notificationStore";
import { useSettingsStore } from "../lib/settingsStore";
import { useAgentMissionStore } from "../lib/agentMissionStore";
import { InlineSystemMonitor } from "./status-bar/InlineSystemMonitor";
import { StatusBarMissionStats } from "./status-bar/StatusBarMissionStats";
import { StatusIndicator } from "./status-bar/StatusPrimitives";
import { dividerStyle, statusBarRootStyle } from "./status-bar/shared";

export function StatusBar() {
  const ws = useWorkspaceStore((s) => s.activeWorkspace());
  const surface = useWorkspaceStore((s) => s.activeSurface());
  const zoomedPaneId = useWorkspaceStore((s) => s.zoomedPaneId);
  const activePaneId = useWorkspaceStore((s) => s.activePaneId());
  const toggleNotificationPanel = useWorkspaceStore((s) => s.toggleNotificationPanel);
  const notifications = useNotificationStore((s) => s.notifications);
  const themeName = useSettingsStore((s) => s.settings.themeName);
  const sandboxEnabled = useSettingsStore((s) => s.settings.sandboxEnabled);
  const snapshotBackend = useSettingsStore((s) => s.settings.snapshotBackend);
  const gatewayEnabled = useSettingsStore((s) => s.settings.gatewayEnabled);
  const unreadCount = notifications.filter((n) => !n.isRead).length;
  const approvals = useAgentMissionStore((s) => s.approvals);
  const cognitiveEvents = useAgentMissionStore((s) => s.cognitiveEvents);
  const operationalEvents = useAgentMissionStore((s) => s.operationalEvents);
  const historyHits = useAgentMissionStore((s) => s.historyHits);
  const snapshots = useAgentMissionStore((s) => s.snapshots);
  const [daemonConnected, setDaemonConnected] = useState(false);
  const pendingApprovals = useMemo(() => approvals.filter((entry) => entry.status === "pending").length, [approvals]);
  const traceCount = cognitiveEvents.length;
  const opsCount = operationalEvents.length;
  const toolCallCount = useMemo(() => operationalEvents.filter((e) => e.kind === "tool-call").length, [operationalEvents]);

  useEffect(() => {
    async function check() {
      try {
        if (typeof window !== "undefined" && "amux" in window) {
          const ok = await ((window as any).tamux ?? (window as any).amux).checkDaemon();
          setDaemonConnected(ok);
        }
      } catch {
        setDaemonConnected(false);
      }
    }
    check();
    const interval = setInterval(check, 10_000);
    return () => clearInterval(interval);
  }, []);

  const paneCount = surface ? allLeafIds(surface.layout).length : 0;

  return (
    <div style={statusBarRootStyle}>
      <div style={{ display: "flex", alignItems: "center", gap: "var(--space-3)", minWidth: 0 }}>
        <StatusIndicator
          label={daemonConnected ? "daemon online" : "daemon offline"}
          status={daemonConnected ? "success" : "neutral"}
        />

        {ws && (
          <span style={{
            color: ws.accentColor,
            fontWeight: 600,
            letterSpacing: "0.02em",
            fontSize: "var(--text-sm)"
          }}>
            {ws.name}
          </span>
        )}

        {surface && (
          <span style={{ color: "var(--text-muted)" }}>
            {surface.name} · {paneCount} pane{paneCount !== 1 ? "s" : ""}
          </span>
        )}

        {activePaneId && (
          <span className="amux-code" style={{ color: "var(--text-muted)", opacity: 0.7 }}>
            {activePaneId}
          </span>
        )}

        {zoomedPaneId && (
          <StatusIndicator label="zoomed" status="warning" />
        )}

        {sandboxEnabled && (
          <StatusIndicator label="sandbox" status="success" />
        )}

        {snapshotBackend !== "tar" && (
          <StatusIndicator label={snapshotBackend} status="success" />
        )}

        {gatewayEnabled && (
          <StatusIndicator label="gateway" status="success" />
        )}

        {ws?.gitBranch && (
          <span style={{ opacity: 0.8 }}>
            ⎇ {ws.gitBranch}
            {ws.gitDirty && (
              <span style={{ color: "var(--warning)", marginLeft: 2 }}>●</span>
            )}
          </span>
        )}

        {ws && ws.listeningPorts.length > 0 && (
          <span style={{ opacity: 0.7 }}>
            :{ws.listeningPorts.join(",")}
          </span>
        )}
      </div>

      <div style={{ display: "flex", alignItems: "center", gap: "var(--space-2)" }}>
        <StatusBarMissionStats
          pendingApprovals={pendingApprovals}
          traceCount={traceCount}
          opsCount={opsCount}
          toolCallCount={toolCallCount}
          historyCount={historyHits.length}
          snapshotCount={snapshots.length}
        />

        <div style={dividerStyle} />

        <InlineSystemMonitor />

        <button
          type="button"
          onClick={toggleNotificationPanel}
          title="Open notifications"
          style={{
            border: "1px solid var(--glass-border)",
            background: unreadCount > 0 ? "var(--approval-soft)" : "transparent",
            color: unreadCount > 0 ? "var(--warning)" : "var(--text-secondary)",
            fontSize: "var(--text-xs)",
            fontWeight: 700,
            padding: "3px 8px",
            cursor: "pointer",
          }}
        >
          Alerts {unreadCount > 0 ? `(${unreadCount})` : ""}
        </button>

        {unreadCount > 0 && (
          <span
            style={{
              color: "var(--accent)",
              marginLeft: "var(--space-2)",
              fontSize: "var(--text-sm)",
            }}
          >
            {unreadCount}
          </span>
        )}

        <span style={{ marginLeft: "var(--space-3)", fontSize: "var(--text-xs)", color: "var(--text-muted)", textTransform: "uppercase", letterSpacing: "0.1em" }}>
          {themeName}
        </span>
      </div>
    </div>
  );
}
