import { useMemo } from "react";
import { useAgentMissionStore } from "../../lib/agentMissionStore";
import { useAgentStore } from "../../lib/agentStore";
import { useWorkspaceStore } from "../../lib/workspaceStore";
import { executeCommand } from "../../registry/commandRegistry";
import type { MissionDeckProps } from "./shared";

export const MissionDeck: React.FC<MissionDeckProps> = ({
    style,
    className,
    children,
    missionTagLabel = "Mission",
    missionButtonLabel = "Mission",
    vaultButtonLabel = "Vault",
    providerLabelPrefix = "provider",
    approvalsLabel = "approvals",
    traceLabel = "trace",
    opsLabel = "ops",
    recallLabel = "recall",
    snapshotsLabel = "snapshots",
    missionCommand = "view.toggleMission",
    vaultCommand = "view.toggleSessionVault",
}) => {
    const asText = (value: unknown, fallback: string): string => {
        if (typeof value === "string") {
            const trimmed = value.trim();
            return trimmed.length > 0 ? trimmed : fallback;
        }
        if (typeof value === "number") {
            return String(value);
        }
        return fallback;
    };

    const activeWorkspace = useWorkspaceStore((s) => s.activeWorkspace());
    const activeSurface = useWorkspaceStore((s) => s.activeSurface());
    const activeProvider = useAgentStore((s) => s.agentSettings.activeProvider);
    const cognitiveEvents = useAgentMissionStore((s) => s.cognitiveEvents);
    const operationalEvents = useAgentMissionStore((s) => s.operationalEvents);
    const approvals = useAgentMissionStore((s) => s.approvals);
    const snapshots = useAgentMissionStore((s) => s.snapshots);
    const historyHits = useAgentMissionStore((s) => s.historyHits);
    const symbolHits = useAgentMissionStore((s) => s.symbolHits);

    const approvalCount = useMemo(
        () => approvals.filter((entry) => entry.status === "pending").length,
        [approvals],
    );
    const workspaceName = asText(activeWorkspace?.name, "No workspace");
    const surfaceName = asText(activeSurface?.name, "No surface");
    const missionTag = asText(missionTagLabel, "Mission");
    const missionButton = asText(missionButtonLabel, "Mission");
    const vaultButton = asText(vaultButtonLabel, "Vault");
    const providerPrefix = asText(providerLabelPrefix, "provider");
    const providerText = asText(activeProvider, "unknown");
    const approvalsText = asText(approvalsLabel, "approvals");
    const traceText = asText(traceLabel, "trace");
    const opsText = asText(opsLabel, "ops");
    const recallText = asText(recallLabel, "recall");
    const snapshotsText = asText(snapshotsLabel, "snapshots");

    return (
        <div
            className={`amux-shell-card ${className ?? ""}`.trim()}
            style={{
                flexShrink: 0,
                padding: "6px 10px",
                minHeight: 52,
                display: "flex",
                alignItems: "center",
                justifyContent: "space-between",
                gap: "var(--space-2)",
                overflowX: "auto",
                ...style,
            }}
        >
            <div
                style={{
                    display: "flex",
                    alignItems: "center",
                    gap: "var(--space-2)",
                    minWidth: 0,
                }}
            >
                <span className="amux-agent-indicator" style={{ fontSize: 10, padding: "2px 8px" }}>
                    {missionTag}
                </span>
                <span
                    style={{
                        fontSize: "var(--text-sm)",
                        fontWeight: 600,
                        whiteSpace: "nowrap",
                        overflow: "hidden",
                        textOverflow: "ellipsis",
                        maxWidth: 240,
                    }}
                    title={`${workspaceName} - ${surfaceName}`}
                >
                    {workspaceName}
                </span>
                <span style={{ color: "var(--text-muted)", fontSize: "var(--text-xs)", whiteSpace: "nowrap" }}>
                    {surfaceName}
                </span>
                <span className="amux-chip" style={{ fontSize: 10, padding: "2px 6px" }}>
                    {providerPrefix} {providerText}
                </span>
            </div>

            <div style={{ display: "flex", alignItems: "center", gap: "var(--space-2)", whiteSpace: "nowrap" }}>
                <span className="amux-chip amux-chip--approval" style={{ fontSize: 10, padding: "2px 6px" }}>
                    {approvalsText} {approvalCount}
                </span>
                <span className="amux-chip" style={{ fontSize: 10, padding: "2px 6px", color: "var(--reasoning)" }}>
                    {traceText} {cognitiveEvents.length}
                </span>
                <span className="amux-chip" style={{ fontSize: 10, padding: "2px 6px", color: "var(--agent)" }}>
                    {opsText} {operationalEvents.length}
                </span>
                <span className="amux-chip" style={{ fontSize: 10, padding: "2px 6px", color: "var(--timeline)" }}>
                    {recallText} {historyHits.length + symbolHits.length}
                </span>
                <span className="amux-chip" style={{ fontSize: 10, padding: "2px 6px" }}>
                    {snapshotsText} {snapshots.length}
                </span>
            </div>

            <div style={{ display: "flex", alignItems: "center", gap: "var(--space-1)", whiteSpace: "nowrap" }}>
                <button
                    type="button"
                    onClick={() => {
                        void executeCommand(missionCommand);
                    }}
                    style={{
                        padding: "4px 8px",
                        border: "1px solid var(--accent-soft)",
                        background: "var(--accent-soft)",
                        color: "var(--accent)",
                        fontSize: 11,
                        fontWeight: 500,
                        cursor: "pointer",
                    }}
                >
                    {missionButton}
                </button>
                <button
                    type="button"
                    onClick={() => {
                        void executeCommand(vaultCommand);
                    }}
                    style={{
                        padding: "4px 8px",
                        border: "1px solid var(--border)",
                        background: "transparent",
                        color: "var(--text-secondary)",
                        fontSize: 11,
                        fontWeight: 500,
                        cursor: "pointer",
                    }}
                >
                    {vaultButton}
                </button>
            </div>
            {children}
        </div>
    );
};
