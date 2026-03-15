import { useEffect, useState, type CSSProperties } from "react";
import { useWorkspaceStore } from "../lib/workspaceStore";
import { useSettingsStore } from "../lib/settingsStore";
import { useAgentStore } from "../lib/agentStore";
import { useAgentMissionStore } from "../lib/agentMissionStore";
import { AboutTab } from "./settings-panel/AboutTab";
import { AppearanceTab } from "./settings-panel/AppearanceTab";
import { AgentTab } from "./settings-panel/AgentTab";
import { BehaviorTab } from "./settings-panel/BehaviorTab";
import { GatewayTab } from "./settings-panel/GatewayTab";
import { KeyboardTab } from "./settings-panel/KeyboardTab";
import { TerminalTab } from "./settings-panel/TerminalTab";
import {
  headerBtnStyle,
} from "./settings-panel/shared";

type SettingsTab = "appearance" | "terminal" | "behavior" | "agent" | "gateway" | "keyboard" | "about";

type SettingsPanelProps = {
  style?: CSSProperties;
  className?: string;
};

/**
 * Full settings panel matching tamux-windows SettingsWindow.
 * 6 sections: Appearance, Terminal, Behavior, Agent, Keyboard, About.
 */
export function SettingsPanel({ style, className }: SettingsPanelProps = {}) {
  const open = useWorkspaceStore((s) => s.settingsOpen);
  const toggle = useWorkspaceStore((s) => s.toggleSettings);
  const settings = useSettingsStore((s) => s.settings);
  const updateSetting = useSettingsStore((s) => s.updateSetting);
  const resetSettings = useSettingsStore((s) => s.resetSettings);
  const profiles = useSettingsStore((s) => s.profiles);
  const addProfile = useSettingsStore((s) => s.addProfile);
  const removeProfile = useSettingsStore((s) => s.removeProfile);
  const updateProfile = useSettingsStore((s) => s.updateProfile);
  const setDefaultProfile = useSettingsStore((s) => s.setDefaultProfile);
  const agentSettings = useAgentStore((s) => s.agentSettings);
  const updateAgentSetting = useAgentStore((s) => s.updateAgentSetting);
  const resetAgentSettings = useAgentStore((s) => s.resetAgentSettings);
  const approvals = useAgentMissionStore((s) => s.approvals);
  const snapshots = useAgentMissionStore((s) => s.snapshots);

  const [tab, setTab] = useState<SettingsTab>("appearance");
  const [systemFonts, setSystemFonts] = useState<string[]>([]);
  const [availableShells, setAvailableShells] = useState<{ name: string; path: string; args?: string }[]>([]);
  const [isFullscreen, setIsFullscreen] = useState(true);

  useEffect(() => {
    if (open && systemFonts.length === 0) {
      // Load system fonts via Electron IPC
      if (typeof window !== "undefined" && ("tamux" in window || "amux" in window)) {
        const bridge = (window as any).tamux ?? (window as any).amux;
        bridge.getSystemFonts().then((fonts: string[]) => {
          setSystemFonts(fonts);
        });
        bridge.getAvailableShells?.().then(
          (shells: { name: string; path: string; args?: string }[]) => setAvailableShells(shells),
        ).catch(() => {});
      }
    }
  }, [open]);

  useEffect(() => {
    if (open) {
      setIsFullscreen(true);
    }
  }, [open]);

  useEffect(() => {
    const handleOpenTab = (event: Event) => {
      const customEvent = event as CustomEvent<{ tab?: SettingsTab }>;
      const requestedTab = customEvent.detail?.tab;
      if (!requestedTab) return;

      if (["appearance", "terminal", "behavior", "agent", "gateway", "keyboard", "about"].includes(requestedTab)) {
        setTab(requestedTab);
      }
    };

    window.addEventListener("tamux-open-settings-tab", handleOpenTab as EventListener);
    window.addEventListener("amux-open-settings-tab", handleOpenTab as EventListener);
    return () => {
      window.removeEventListener("tamux-open-settings-tab", handleOpenTab as EventListener);
      window.removeEventListener("amux-open-settings-tab", handleOpenTab as EventListener);
    };
  }, []);

  if (!open) return null;

  const tabs: { id: SettingsTab; label: string }[] = [
    { id: "appearance", label: "Interface" },
    { id: "terminal", label: "Execution" },
    { id: "behavior", label: "Operator" },
    { id: "agent", label: "Agent" },
    { id: "gateway", label: "Gateway" },
    { id: "keyboard", label: "Bindings" },
    { id: "about", label: "Runtime" },
  ];

  return (
    <div
      style={{
        ...(isFullscreen
          ? {
            position: "fixed" as const,
            inset: 0,
            zIndex: 3600,
            width: "100vw",
            height: "100vh",
            borderRadius: 0,
          }
          : {
            width: 720,
            minWidth: 420,
            maxWidth: 1100,
            height: "100%",
            resize: "horizontal" as const,
          }),
        display: "flex",
        flexDirection: "column",
        padding: "20px",
        background: "rgba(0,0,0,0.8)",
        border: "1px solid var(--border)",
        borderRadius: isFullscreen ? 0 : "var(--radius-xl)",
        overflow: "hidden",
        ...(style ?? {}),
      }}
      className={className}
    >
      <div
        style={{
          background: "var(--bg-primary)",
          borderBottom: "1px solid var(--border)",
          display: "flex",
          height: "100%",
          flexDirection: "column",
          overflow: "hidden",
        }}
      >
        <div style={{
          display: "grid", gap: 14,
          padding: "18px 22px 16px", borderBottom: "1px solid rgba(255,255,255,0.08)",
        }}>
          <div style={{ display: "flex", alignItems: "start", justifyContent: "space-between", gap: 16 }}>
            <div style={{ display: "grid", gap: 6 }}>
              <span className="amux-panel-title" style={{ color: "var(--mission)" }}>Operator Configuration</span>
              <span style={{ fontSize: 22, fontWeight: 800 }}>Mission Runtime Settings</span>
              <span style={{ fontSize: 12, color: "var(--text-secondary)", lineHeight: 1.45 }}>
                Tune visual hierarchy, terminal execution, providers, bindings, and runtime ergonomics from one control plane.
              </span>
            </div>
            <div style={{ display: "flex", gap: 8 }}>
              <button
                onClick={() => setIsFullscreen((prev) => !prev)}
                style={headerBtnStyle}
                title={isFullscreen ? "Switch to resizable window" : "Expand fullscreen"}
              >
                {isFullscreen ? "Window" : "Fullscreen"}
              </button>
              <button onClick={() => { resetSettings(); resetAgentSettings(); }} style={headerBtnStyle} title="Reset all">Reset</button>
              <button onClick={toggle} style={headerBtnStyle}>✕</button>
            </div>
          </div>
          <div style={{ display: "grid", gridTemplateColumns: "repeat(4, minmax(0, 1fr))", gap: 10 }}>
            <SettingsMetric label="Theme" value={settings.themeName} />
            <SettingsMetric label="Provider" value={agentSettings.activeProvider} />
            <SettingsMetric label="Approvals" value={String(approvals.filter((entry) => entry.status === "pending").length)} />
            <SettingsMetric label="Snapshots" value={String(snapshots.length)} />
          </div>
        </div>

        <div style={{
          display: "flex", borderBottom: "1px solid rgba(255,255,255,0.08)", padding: "0 16px",
          gap: 0, overflow: "auto",
        }}>
          {tabs.map((t) => (
            <button key={t.id} onClick={() => setTab(t.id)} style={{
              background: tab === t.id ? "rgba(97, 197, 255, 0.1)" : "none", border: "none", borderBottom: tab === t.id ? "2px solid var(--accent)" : "2px solid transparent",
              color: tab === t.id ? "var(--text-primary)" : "var(--text-secondary)",
              padding: "12px 14px", fontSize: 12, cursor: "pointer", fontWeight: tab === t.id ? 700 : 500,
              fontFamily: "inherit", whiteSpace: "nowrap",
            }}>
              {t.label}
            </button>
          ))}
        </div>

        <div style={{ flex: 1, overflow: "auto", padding: "18px 22px" }}>
          {tab === "appearance" && (
            <AppearanceTab settings={settings} updateSetting={updateSetting} systemFonts={systemFonts} />
          )}
          {tab === "terminal" && (
            <TerminalTab
              settings={settings}
              updateSetting={updateSetting}
              availableShells={availableShells}
              profiles={profiles}
              addProfile={addProfile}
              removeProfile={removeProfile}
              updateProfile={updateProfile}
              setDefaultProfile={setDefaultProfile}
            />
          )}
          {tab === "behavior" && (
            <BehaviorTab settings={settings} updateSetting={updateSetting} />
          )}
          {tab === "agent" && (
            <AgentTab settings={agentSettings} updateSetting={updateAgentSetting} resetSettings={resetAgentSettings} />
          )}
          {tab === "gateway" && (
            <GatewayTab settings={settings} updateSetting={updateSetting} />
          )}
          {tab === "keyboard" && <KeyboardTab />}
          {tab === "about" && <AboutTab />}
        </div>
      </div>
    </div>
  );
}

function SettingsMetric({ label, value }: { label: string; value: string }) {
  return (
    <div style={{ borderRadius: 0, padding: "10px 12px", border: "1px solid rgba(255,255,255,0.06)", background: "rgba(18, 33, 47, 0.8)", display: "grid", gap: 4 }}>
      <span className="amux-panel-title">{label}</span>
      <span style={{ fontSize: 15, fontWeight: 700 }}>{value}</span>
    </div>
  );
}

