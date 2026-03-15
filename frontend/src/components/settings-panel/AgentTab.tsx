import type { AgentProviderId, AgentSettings } from "../../lib/agentStore";
import { addBtnStyle, NumberInput, PasswordInput, Section, SelectInput, SettingRow, TextInput, Toggle, inputStyle } from "./shared";

export function AgentTab({
    settings, updateSetting, resetSettings,
}: {
    settings: AgentSettings;
    updateSetting: <K extends keyof AgentSettings>(key: K, value: AgentSettings[K]) => void;
    resetSettings: () => void;
}) {
    const providerOptions: { id: AgentProviderId; label: string }[] = [
        { id: "featherless", label: "Featherless" },
        { id: "openai", label: "OpenAI" },
        { id: "anthropic", label: "Anthropic" },
        { id: "qwen", label: "Qwen" },
        { id: "qwen-deepinfra", label: "Qwen (DeepInfra)" },
        { id: "kimi", label: "Kimi" },
        { id: "z.ai", label: "Z.AI" },
        { id: "openrouter", label: "OpenRouter" },
        { id: "cerebras", label: "Cerebras" },
        { id: "together", label: "Together" },
        { id: "groq", label: "Groq" },
        { id: "ollama", label: "Ollama" },
        { id: "chutes", label: "Chutes" },
        { id: "huggingface", label: "HuggingFace" },
        { id: "minimax", label: "MiniMax" },
        { id: "custom", label: "Custom" },
    ];

    const providerConfig = settings[settings.activeProvider] as { baseUrl: string; model: string; apiKey: string };

    return (
        <>
            <Section title="General">
                <SettingRow label="Enable Agent">
                    <Toggle value={settings.enabled} onChange={(value) => updateSetting("enabled", value)} />
                </SettingRow>
                <SettingRow label="Agent Backend">
                    <SelectInput value={settings.agentBackend}
                        options={["daemon", "legacy"]}
                        onChange={(value) => updateSetting("agentBackend", value as "daemon" | "legacy")} />
                </SettingRow>
                <SettingRow label="Agent Name">
                    <TextInput value={settings.agentName} onChange={(value) => updateSetting("agentName", value)} />
                </SettingRow>
                <SettingRow label="Handler Prefix">
                    <TextInput value={settings.handler} onChange={(value) => updateSetting("handler", value)} />
                </SettingRow>
                <SettingRow label="System Prompt">
                    <textarea value={settings.systemPrompt}
                        onChange={(event) => updateSetting("systemPrompt", event.target.value)}
                        rows={4}
                        style={{ ...inputStyle, width: "100%", resize: "vertical", fontFamily: "inherit" }} />
                </SettingRow>
            </Section>

            <Section title="Provider">
                <SettingRow label="Active Provider">
                    <SelectInput value={settings.activeProvider}
                        options={providerOptions.map((provider) => provider.id)}
                        onChange={(value) => updateSetting("activeProvider", value as AgentProviderId)} />
                </SettingRow>

                <div style={{ marginTop: 6, marginBottom: 6, fontSize: 11, color: "var(--text-secondary)" }}>
                    {providerOptions.find((provider) => provider.id === settings.activeProvider)?.label}
                </div>

                <SettingRow label="Base URL">
                    <TextInput value={providerConfig.baseUrl}
                        onChange={(value) => updateSetting(settings.activeProvider, { ...providerConfig, baseUrl: value })} />
                </SettingRow>
                <SettingRow label="Model">
                    <TextInput value={providerConfig.model}
                        onChange={(value) => updateSetting(settings.activeProvider, { ...providerConfig, model: value })} />
                </SettingRow>
                <SettingRow label="API Key">
                    <PasswordInput value={providerConfig.apiKey}
                        onChange={(value) => updateSetting(settings.activeProvider, { ...providerConfig, apiKey: value })}
                        placeholder="Provider API key" />
                </SettingRow>
            </Section>

            <Section title="Tools">
                <SettingRow label="Bash Tool">
                    <Toggle value={settings.enableBashTool} onChange={(value) => updateSetting("enableBashTool", value)} />
                </SettingRow>
                <SettingRow label="Vision Tool">
                    <Toggle value={settings.enableVisionTool} onChange={(value) => updateSetting("enableVisionTool", value)} />
                </SettingRow>
                <SettingRow label="Web Browsing Tool">
                    <Toggle value={settings.enableWebBrowsingTool} onChange={(value) => updateSetting("enableWebBrowsingTool", value)} />
                </SettingRow>
                <SettingRow label="Bash Timeout (s)">
                    <NumberInput value={settings.bashTimeoutSeconds} min={5} max={300}
                        onChange={(value) => updateSetting("bashTimeoutSeconds", value)} />
                </SettingRow>
                <SettingRow label="Web Search Tool">
                    <Toggle value={settings.enableWebSearchTool} onChange={(value) => updateSetting("enableWebSearchTool", value)} />
                </SettingRow>
                {settings.enableWebSearchTool ? (
                    <>
                        <SettingRow label="Search Provider">
                            <SelectInput
                                value={settings.searchToolProvider}
                                options={["none", "firecrawl", "exa", "tavily"]}
                                onChange={(value) => updateSetting("searchToolProvider", value as "none" | "firecrawl" | "exa" | "tavily")}
                            />
                        </SettingRow>
                        <SettingRow label="Firecrawl API Key">
                            <PasswordInput value={settings.firecrawlApiKey} onChange={(value) => updateSetting("firecrawlApiKey", value)} placeholder="fc-..." />
                        </SettingRow>
                        <SettingRow label="Exa API Key">
                            <PasswordInput value={settings.exaApiKey} onChange={(value) => updateSetting("exaApiKey", value)} placeholder="exa_..." />
                        </SettingRow>
                        <SettingRow label="Tavily API Key">
                            <PasswordInput value={settings.tavilyApiKey} onChange={(value) => updateSetting("tavilyApiKey", value)} placeholder="tvly-..." />
                        </SettingRow>
                        <SettingRow label="Search Max Results">
                            <NumberInput value={settings.searchMaxResults} min={1} max={20}
                                onChange={(value) => updateSetting("searchMaxResults", value)} />
                        </SettingRow>
                        <SettingRow label="Search Timeout (s)">
                            <NumberInput value={settings.searchTimeoutSeconds} min={3} max={120}
                                onChange={(value) => updateSetting("searchTimeoutSeconds", value)} />
                        </SettingRow>
                    </>
                ) : null}
            </Section>

            <Section title="Chat">
                <SettingRow label="Streaming">
                    <Toggle value={settings.enableStreaming} onChange={(value) => updateSetting("enableStreaming", value)} />
                </SettingRow>
                <SettingRow label="Conversation Memory">
                    <Toggle value={settings.enableConversationMemory} onChange={(value) => updateSetting("enableConversationMemory", value)} />
                </SettingRow>
                <SettingRow label="Chat Font Family">
                    <TextInput value={settings.chatFontFamily} onChange={(value) => updateSetting("chatFontFamily", value)} />
                </SettingRow>
                <SettingRow label="Chat Font Size">
                    <NumberInput value={settings.chatFontSize} min={10} max={24}
                        onChange={(value) => updateSetting("chatFontSize", value)} />
                </SettingRow>
            </Section>

            <Section title="Context Compaction">
                <SettingRow label="Auto Compact">
                    <Toggle value={settings.autoCompactContext} onChange={(value) => updateSetting("autoCompactContext", value)} />
                </SettingRow>
                <SettingRow label="Max Context Messages">
                    <NumberInput value={settings.maxContextMessages} min={10} max={500}
                        onChange={(value) => updateSetting("maxContextMessages", value)} />
                </SettingRow>
                <SettingRow label="Max Tool Loops">
                    <NumberInput value={settings.maxToolLoops} min={1} max={100}
                        onChange={(value) => updateSetting("maxToolLoops", value)} />
                </SettingRow>
                <SettingRow label="Budget Tokens">
                    <NumberInput value={settings.contextBudgetTokens} min={10000} max={500000} step={10000}
                        onChange={(value) => updateSetting("contextBudgetTokens", value)} />
                </SettingRow>
                <SettingRow label="Compact Threshold %">
                    <NumberInput value={settings.compactThresholdPercent} min={50} max={95}
                        onChange={(value) => updateSetting("compactThresholdPercent", value)} />
                </SettingRow>
                <SettingRow label="Keep Recent on Compact">
                    <NumberInput value={settings.keepRecentOnCompaction} min={1} max={50}
                        onChange={(value) => updateSetting("keepRecentOnCompaction", value)} />
                </SettingRow>
            </Section>

            <div style={{ marginTop: 12 }}>
                <button onClick={resetSettings} style={addBtnStyle}>Reset Agent Settings</button>
            </div>
        </>
    );
}