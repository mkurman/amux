import { useState } from "react";
import { isCDUIEnabled, setCDUIEnabled } from "../../lib/cduiMode";
import { Section, SettingRow, Toggle, smallBtnStyle } from "./shared";

export function AboutTab() {
    const [cduiEnabled, setCduiEnabledState] = useState<boolean>(() => isCDUIEnabled());

    const applyRuntimeMode = () => {
        setCDUIEnabled(cduiEnabled);
        window.location.reload();
    };

    return (
        <>
            <Section title="Runtime Mode">
                <SettingRow label="Use New CDUI">
                    <Toggle value={cduiEnabled} onChange={setCduiEnabledState} />
                </SettingRow>
                <div style={{ marginTop: 8, display: "flex", justifyContent: "space-between", alignItems: "center", gap: 12 }}>
                    <span style={{ fontSize: 11, color: "var(--text-secondary)" }}>
                        CDUI is now the default interface. Turn it off here to reload into the legacy UI, or turn it back on to use YAML-driven views from ~/.tamux/views.
                    </span>
                    <div style={{ display: "flex", alignItems: "center", gap: 8 }}>
                        <button
                            onClick={() => {
                                void ((window as any).tamux ?? (window as any).amux)?.revealDataPath?.("views");
                            }}
                            style={smallBtnStyle}
                        >
                            Open ~/.tamux/views
                        </button>
                        <button
                            onClick={() => {
                                window.dispatchEvent(new Event("tamux-cdui-views-reload"));
                                window.dispatchEvent(new Event("amux-cdui-views-reload"));
                            }}
                            style={smallBtnStyle}
                        >
                            Reload Views
                        </button>
                        <button onClick={applyRuntimeMode} style={smallBtnStyle}>
                            Apply & Reload
                        </button>
                    </div>
                </div>
            </Section>

            <Section title="About">
                <div style={{ fontSize: 13, lineHeight: 1.6 }}>
                    <p style={{ fontWeight: 600, marginBottom: 8 }}>tamux - Terminal Multiplexer</p>
                    <p>Version 0.1.3</p>
                    <p style={{ marginTop: 8, color: "var(--text-secondary)" }}>
                        A cross-platform terminal multiplexer with workspaces, surfaces, pane management,
                        AI agent integration, snippet library, and session persistence.
                    </p>
                    <p style={{ marginTop: 12, color: "var(--text-secondary)" }}>
                        Built with Electron, React, xterm.js, and a Rust daemon.
                    </p>
                </div>
            </Section>
        </>
    );
}