import type { CSSProperties } from "react";
import { useAgentMissionStore } from "../lib/agentMissionStore";

type AgentApprovalOverlayProps = {
  style?: CSSProperties;
  className?: string;
};

export function AgentApprovalOverlay({ style, className }: AgentApprovalOverlayProps = {}) {
  const approval = useAgentMissionStore((s) =>
    s.approvals.find((entry) => entry.status === "pending" && entry.handledAt === null)
  );
  const resolveApproval = useAgentMissionStore((s) => s.resolveApproval);

  async function handleDecision(status: "approved-once" | "approved-session" | "denied") {
    if (!approval) return;

    const amux = (window as any).tamux ?? (window as any).amux;
    if (amux?.resolveManagedApproval) {
      const decision =
        status === "approved-session" ? "approve-session" : status === "denied" ? "deny" : "approve-once";
      await amux.resolveManagedApproval(approval.paneId, approval.id, decision);
    }

    resolveApproval(approval.id, status);
  }

  if (!approval) return null;

  const riskColorMap = {
    low: { bg: "var(--success-soft)", border: "rgba(74, 222, 128, 0.3)", text: "var(--success)" },
    medium: { bg: "var(--warning-soft)", border: "rgba(251, 191, 36, 0.3)", text: "var(--warning)" },
    high: { bg: "var(--danger-soft)", border: "rgba(248, 113, 113, 0.3)", text: "var(--danger)" },
    critical: { bg: "var(--risk-critical)", border: "rgba(239, 68, 68, 0.4)", text: "#f87171" },
  };
  const riskColors = riskColorMap[approval.riskLevel as keyof typeof riskColorMap] || riskColorMap.high;

  return (
    <div
      style={{
        position: "fixed",
        inset: 0,
        background: "var(--bg-overlay)",
        backdropFilter: "none",
        display: "flex",
        alignItems: "center",
        justifyContent: "center",
        zIndex: 5000,
        padding: "var(--space-6)",
        ...(style ?? {}),
      }}
      className={className}
    >
      <div
        style={{
          width: "min(640px, 92vw)",
          borderRadius: "var(--radius-2xl)",
          overflow: "hidden",
          border: "1px solid var(--border-strong)",
          background: "var(--bg-primary)",
          animation: "slideInUp var(--transition-base) ease",
        }}
      >
        <div
          style={{
            padding: "var(--space-5)",
            borderBottom: "1px solid var(--border)",
            display: "flex",
            justifyContent: "space-between",
            alignItems: "flex-start",
            gap: "var(--space-4)",
            background: "var(--bg-primary)",
          }}
        >
          <div>
            <div
              style={{
                fontSize: "var(--text-xs)",
                letterSpacing: "0.15em",
                textTransform: "uppercase",
                color: "var(--warning)",
                fontWeight: 600,
                display: "flex",
                alignItems: "center",
                gap: "var(--space-2)",
              }}
            >
              <span
                style={{
                  width: 6,
                  height: 6,
                  borderRadius: "50%",
                  background: "var(--warning)",
                  boxShadow: "none",
                  animation: "pulse 2s infinite",
                }}
              />
              Approval Required
            </div>

            <div style={{ fontSize: "var(--text-xl)", fontWeight: 700, marginTop: "var(--space-2)" }}>
              High-impact command intercepted
            </div>
          </div>

          <div
            style={{
              padding: "var(--space-1) var(--space-3)",
              borderRadius: "var(--radius-full)",
              fontSize: "var(--text-xs)",
              textTransform: "uppercase",
              letterSpacing: "0.1em",
              fontWeight: 600,
              color: riskColors.text,
              background: riskColors.bg,
              border: `1px solid ${riskColors.border}`,
            }}
          >
            {approval.riskLevel}
          </div>
        </div>

        <div style={{ padding: "var(--space-5)", display: "flex", flexDirection: "column", gap: "var(--space-4)" }}>
          <div style={{ display: "flex", flexDirection: "column", gap: "var(--space-2)" }}>
            <div style={{ fontSize: "var(--text-xs)", color: "var(--text-muted)", textTransform: "uppercase", letterSpacing: "0.1em" }}>
              Command
            </div>
            <div
              style={{
                padding: "var(--space-4)",
                borderRadius: "var(--radius-lg)",
                background: "var(--bg-secondary)",
                border: "1px solid var(--glass-border)",
                fontFamily: "var(--font-mono)",
                fontSize: "var(--text-sm)",
                whiteSpace: "pre-wrap",
                wordBreak: "break-word",
                color: "var(--text-primary)",
              }}
            >
              {approval.command}
            </div>
          </div>

          <div style={{ display: "grid", gridTemplateColumns: "repeat(2, 1fr)", gap: "var(--space-3)" }}>
            <InfoCard label="Blast Radius" value={approval.blastRadius} />
            <InfoCard label="Scope" value={approval.sessionId ?? approval.paneId} />
          </div>

          <div style={{ display: "flex", flexDirection: "column", gap: "var(--space-2)" }}>
            <div style={{ fontSize: "var(--text-xs)", color: "var(--text-muted)", textTransform: "uppercase", letterSpacing: "0.1em" }}>
              Risk Factors
            </div>

            <div style={{ display: "flex", flexWrap: "wrap", gap: "var(--space-2)" }}>
              {approval.reasons.map((reason) => (
                <span
                  key={reason}
                  style={{
                    padding: "var(--space-1) var(--space-2)",
                    borderRadius: "var(--radius-md)",
                    background: "var(--warning-soft)",
                    border: "1px solid rgba(251, 191, 36, 0.2)",
                    fontSize: "var(--text-xs)",
                    color: "var(--warning)",
                  }}
                >
                  {reason}
                </span>
              ))}
            </div>
          </div>
        </div>

        <div
          style={{
            padding: "var(--space-4) var(--space-5)",
            display: "flex",
            justifyContent: "flex-end",
            gap: "var(--space-2)",
            borderTop: "1px solid var(--border)",
            background: "var(--bg-secondary)",
          }}
        >
          <button
            type="button"
            onClick={() => void handleDecision("denied")}
            style={{
              padding: "var(--space-2) var(--space-4)",
              borderRadius: "var(--radius-md)",
              border: "1px solid var(--glass-border)",
              background: "transparent",
              color: "var(--text-secondary)",
              fontSize: "var(--text-sm)",
              fontWeight: 500,
              cursor: "pointer",
              transition: "all var(--transition-fast)",
            }}
            onMouseEnter={(e) => {
              e.currentTarget.style.background = "var(--danger-soft)";
              e.currentTarget.style.color = "var(--danger)";
              e.currentTarget.style.borderColor = "rgba(248, 113, 113, 0.3)";
            }}
            onMouseLeave={(e) => {
              e.currentTarget.style.background = "transparent";
              e.currentTarget.style.color = "var(--text-secondary)";
              e.currentTarget.style.borderColor = "var(--glass-border)";
            }}
          >
            Deny
          </button>

          <button
            type="button"
            onClick={() => void handleDecision("approved-once")}
            style={{
              padding: "var(--space-2) var(--space-4)",
              borderRadius: "var(--radius-md)",
              border: "1px solid var(--border)",
              background: "var(--bg-tertiary)",
              color: "var(--text-primary)",
              fontSize: "var(--text-sm)",
              fontWeight: 500,
              cursor: "pointer",
              transition: "all var(--transition-fast)",
            }}
            onMouseEnter={(e) => {
              e.currentTarget.style.background = "var(--bg-elevated)";
              e.currentTarget.style.borderColor = "var(--border-strong)";
            }}
            onMouseLeave={(e) => {
              e.currentTarget.style.background = "var(--bg-tertiary)";
              e.currentTarget.style.borderColor = "var(--border)";
            }}
          >
            Allow Once
          </button>

          <button
            type="button"
            onClick={() => void handleDecision("approved-session")}
            style={{
              padding: "var(--space-2) var(--space-4)",
              borderRadius: "var(--radius-md)",
              border: "1px solid var(--success-soft)",
              background: "var(--success-soft)",
              color: "var(--success)",
              fontSize: "var(--text-sm)",
              fontWeight: 600,
              cursor: "pointer",
              transition: "all var(--transition-fast)",
            }}
            onMouseEnter={(e) => {
              e.currentTarget.style.background = "rgba(74, 222, 128, 0.2)";
              e.currentTarget.style.borderColor = "rgba(74, 222, 128, 0.4)";
            }}
            onMouseLeave={(e) => {
              e.currentTarget.style.background = "var(--success-soft)";
              e.currentTarget.style.borderColor = "var(--success-soft)";
            }}
          >
            Allow For Session
          </button>
        </div>
      </div>
    </div>
  );
}

function InfoCard({ label, value }: { label: string; value: string }) {
  return (
    <div
      style={{
        padding: "var(--space-3)",
        borderRadius: "var(--radius-lg)",
        background: "var(--bg-secondary)",
        border: "1px solid var(--glass-border)",
      }}
    >
      <div className="amux-panel-title">{label}</div>
      <div style={{ fontSize: "var(--text-sm)", marginTop: "var(--space-1)", wordBreak: "break-word" }}>{value}</div>
    </div>
  );
}
