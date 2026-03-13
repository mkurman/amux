import { useState } from "react";
import type { AmuxSettings } from "../../lib/types";
import { NumberInput, Section, SelectInput, SettingRow, type SettingsUpdater, TextInput, Toggle, inputStyle, smallBtnStyle } from "./shared";

type McpServerRow = {
    name: string;
    command: string;
    argsText: string;
};

export function BehaviorTab({
    settings, updateSetting,
}: {
    settings: AmuxSettings;
    updateSetting: SettingsUpdater;
}) {
    const [lspHealth, setLspHealth] = useState<Record<string, boolean> | null>(null);
    const [mcpHealth, setMcpHealth] = useState<Array<{ name: string; command: string; exists: boolean; error?: string }> | null>(null);
    const [mcpError, setMcpError] = useState<string | null>(null);

    const parseMcpServers = () => {
        try {
            const parsed = JSON.parse(settings.mcpServersJson || "{}");
            if (!parsed || typeof parsed !== "object" || Array.isArray(parsed)) {
                throw new Error("MCP servers must be a JSON object keyed by server name.");
            }
            return {
                servers: parsed as Record<string, { command?: string; args?: string[] }>,
                error: null as string | null,
            };
        } catch (error: any) {
            return {
                servers: null,
                error: error?.message || "Invalid MCP servers JSON",
            };
        }
    };

    const mcpRows: McpServerRow[] = (() => {
        const parsed = parseMcpServers();
        if (!parsed.servers) return [];

        return Object.entries(parsed.servers).map(([name, value]) => ({
            name,
            command: typeof value?.command === "string" ? value.command : "",
            argsText: Array.isArray(value?.args) ? value.args.join(" ") : "",
        }));
    })();
    const parsedMcp = parseMcpServers();

    const saveMcpRows = (rows: McpServerRow[]) => {
        const next: Record<string, { command: string; args?: string[] }> = {};

        for (const row of rows) {
            const key = row.name.trim();
            const command = row.command.trim();
            if (!key || !command) continue;

            const args = row.argsText
                .split(/\s+/)
                .map((value) => value.trim())
                .filter(Boolean);

            next[key] = args.length > 0 ? { command, args } : { command };
        }

        setMcpError(null);
        updateSetting("mcpServersJson", JSON.stringify(next, null, 2));
    };

    const updateMcpRow = (index: number, patch: Partial<McpServerRow>) => {
        const rows = mcpRows.map((row, rowIndex) => (rowIndex === index ? { ...row, ...patch } : row));
        saveMcpRows(rows);
    };

    const addMcpRow = () => {
        saveMcpRows([
            ...mcpRows,
            { name: `server${mcpRows.length + 1}`, command: "", argsText: "" },
        ]);
    };

    const removeMcpRow = (index: number) => {
        saveMcpRows(mcpRows.filter((_, rowIndex) => rowIndex !== index));
    };

    const runLspHealthCheck = async () => {
        try {
            const result = await ((window as any).tamux ?? (window as any).amux)?.checkLspHealth?.();
            setLspHealth(result ?? null);
        } catch {
            setLspHealth(null);
        }
    };

    const runMcpHealthCheck = async () => {
        const parsed = parseMcpServers();
        if (!parsed.servers) {
            setMcpError(parsed.error || "Invalid MCP servers JSON");
            return;
        }
        setMcpError(null);
        try {
            const result = await ((window as any).tamux ?? (window as any).amux)?.checkMcpHealth?.(parsed.servers);
            setMcpHealth(Array.isArray(result) ? result : null);
        } catch {
            setMcpHealth(null);
        }
    };

    const copyMcpConfig = async () => {
        const parsed = parseMcpServers();
        if (!parsed.servers) {
            setMcpError(parsed.error || "Invalid MCP servers JSON");
            return;
        }
        setMcpError(null);
        const payload = JSON.stringify({ mcpServers: parsed.servers }, null, 2);
        try {
            await navigator.clipboard.writeText(payload);
        } catch {
            setMcpError("Failed to copy MCP config to clipboard.");
        }
    };

    return (
        <>
            <Section title="Session">
                <SettingRow label="Restore on Startup">
                    <Toggle value={settings.restoreSessionOnStartup}
                        onChange={(value) => updateSetting("restoreSessionOnStartup", value)} />
                </SettingRow>
                <SettingRow label="Confirm on Close">
                    <Toggle value={settings.confirmOnClose}
                        onChange={(value) => updateSetting("confirmOnClose", value)} />
                </SettingRow>
                <SettingRow label="Auto-save Interval (s)">
                    <NumberInput value={settings.autoSaveIntervalSeconds} min={5} max={300}
                        onChange={(value) => updateSetting("autoSaveIntervalSeconds", value)} />
                </SettingRow>
            </Section>

            <Section title="Clipboard">
                <SettingRow label="Auto-copy on Select">
                    <Toggle value={settings.autoCopyOnSelect}
                        onChange={(value) => updateSetting("autoCopyOnSelect", value)} />
                </SettingRow>
                <SettingRow label="Ctrl+Click Opens URLs">
                    <Toggle value={settings.ctrlClickOpensUrls}
                        onChange={(value) => updateSetting("ctrlClickOpensUrls", value)} />
                </SettingRow>
            </Section>

            <Section title="Logging & Transcripts">
                <SettingRow label="Capture Transcripts on Close">
                    <Toggle value={settings.captureTranscriptsOnClose}
                        onChange={(value) => updateSetting("captureTranscriptsOnClose", value)} />
                </SettingRow>
                <SettingRow label="Capture Transcripts on Clear">
                    <Toggle value={settings.captureTranscriptsOnClear}
                        onChange={(value) => updateSetting("captureTranscriptsOnClear", value)} />
                </SettingRow>
                <SettingRow label="Command Log Retention (days)">
                    <NumberInput value={settings.commandLogRetentionDays} min={0} max={365}
                        onChange={(value) => updateSetting("commandLogRetentionDays", value)} />
                </SettingRow>
                <SettingRow label="Transcript Retention (days)">
                    <NumberInput value={settings.transcriptRetentionDays} min={0} max={365}
                        onChange={(value) => updateSetting("transcriptRetentionDays", value)} />
                </SettingRow>
            </Section>

            <Section title="Infrastructure">
                <SettingRow label="Security Level">
                    <SelectInput
                        value={settings.securityLevel}
                        options={["highest", "moderate", "lowest", "yolo"]}
                        onChange={(value) => updateSetting("securityLevel", value as AmuxSettings["securityLevel"])}
                    />
                </SettingRow>
                <SettingRow label="Sandbox Isolation">
                    <Toggle value={settings.sandboxEnabled}
                        onChange={(value) => updateSetting("sandboxEnabled", value)} />
                </SettingRow>
                <SettingRow label="Sandbox Network Access">
                    <Toggle value={settings.sandboxNetworkEnabled}
                        onChange={(value) => updateSetting("sandboxNetworkEnabled", value)} />
                </SettingRow>
                <SettingRow label="Snapshot Backend">
                    <SelectInput value={settings.snapshotBackend}
                        options={["tar", "zfs", "btrfs"]}
                        onChange={(value) => updateSetting("snapshotBackend", value as "tar" | "zfs" | "btrfs")} />
                </SettingRow>
                <SettingRow label="WORM Integrity Checks">
                    <Toggle value={settings.wormIntegrityEnabled}
                        onChange={(value) => updateSetting("wormIntegrityEnabled", value)} />
                </SettingRow>
                <SettingRow label="Cerbos Endpoint">
                    <TextInput value={settings.cerbosEndpoint}
                        onChange={(value) => updateSetting("cerbosEndpoint", value)}
                        placeholder="http://localhost:3593" />
                </SettingRow>

                <SettingRow label="MCP Servers">
                    <div style={{ display: "grid", gap: 8, width: "100%" }}>
                        {parsedMcp.error ? (
                            <div style={{ fontSize: 11, color: "var(--danger)" }}>
                                {parsedMcp.error}
                            </div>
                        ) : null}
                        {mcpRows.length === 0 ? (
                            <div style={{ fontSize: 11, color: "var(--text-secondary)" }}>No MCP servers configured yet.</div>
                        ) : null}

                        <div style={{ display: "grid", gap: 8 }}>
                            {mcpRows.map((row, index) => (
                                <div
                                    key={`mcp-row-${index}`}
                                    style={{
                                        display: "grid",
                                        gap: 6,
                                        padding: 8,
                                        border: "1px solid var(--glass-border)",
                                        borderRadius: "var(--radius-md)",
                                        background: "var(--bg-secondary)",
                                    }}
                                >
                                    <div style={{ display: "grid", gridTemplateColumns: "1fr 2fr auto", gap: 6 }}>
                                        <input
                                            value={row.name}
                                            onChange={(event) => updateMcpRow(index, { name: event.target.value })}
                                            placeholder="name"
                                            style={{ ...inputStyle, width: "100%" }}
                                        />
                                        <input
                                            value={row.command}
                                            onChange={(event) => updateMcpRow(index, { command: event.target.value })}
                                            placeholder="command"
                                            style={{ ...inputStyle, width: "100%" }}
                                        />
                                        <button onClick={() => removeMcpRow(index)} style={{ ...smallBtnStyle, color: "var(--danger)" }}>Remove</button>
                                    </div>
                                    <input
                                        value={row.argsText}
                                        onChange={(event) => updateMcpRow(index, { argsText: event.target.value })}
                                        placeholder="args (space separated)"
                                        style={{ ...inputStyle, width: "100%" }}
                                    />
                                </div>
                            ))}
                        </div>

                        <div style={{ display: "flex", gap: 8, flexWrap: "wrap" }}>
                            <button onClick={addMcpRow} style={smallBtnStyle}>Add MCP Server</button>
                            <button onClick={runLspHealthCheck} style={smallBtnStyle}>Check LSP Health</button>
                            <button onClick={runMcpHealthCheck} style={smallBtnStyle}>Check MCP Health</button>
                            <button onClick={copyMcpConfig} style={smallBtnStyle}>Copy MCP Config</button>
                        </div>
                        {mcpError ? <div style={{ fontSize: 11, color: "var(--danger)" }}>{mcpError}</div> : null}
                        {lspHealth ? (
                            <div style={{ fontSize: 11, color: "var(--text-secondary)" }}>
                                LSP: rust-analyzer={lspHealth.rustAnalyzer ? "ok" : "missing"}, typescript-language-server={lspHealth.typescriptLanguageServer ? "ok" : "missing"}, pyright-langserver={lspHealth.pyrightLangserver ? "ok" : "missing"}
                            </div>
                        ) : null}
                        {mcpHealth ? (
                            <div style={{ fontSize: 11, color: "var(--text-secondary)", display: "grid", gap: 2 }}>
                                {mcpHealth.map((row) => (
                                    <span key={`${row.name}:${row.command}`}>
                                        MCP {row.name}: {row.command || "(missing command)"} - {row.exists ? "ok" : (row.error || "missing")}
                                    </span>
                                ))}
                            </div>
                        ) : null}
                    </div>
                </SettingRow>
            </Section>
        </>
    );
}