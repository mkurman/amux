import React, { createContext, useContext, useEffect, useMemo, useRef, useState, type CSSProperties } from "react";
import { abortThreadStream, clearThreadAbortController, setThreadAbortController, useAgentStore } from "../../lib/agentStore";
import type { AgentMessage, AgentThread } from "../../lib/agentStore";
import { sendChatCompletion, messagesToApiFormat } from "../../lib/agentClient";
import { getAvailableTools, executeTool, getToolCapabilityDescription } from "../../lib/agentTools";
import { useAgentMissionStore } from "../../lib/agentMissionStore";
import { useSettingsStore } from "../../lib/settingsStore";
import { useSnippetStore } from "../../lib/snippetStore";
import { getTerminalController } from "../../lib/terminalRegistry";
import { useTranscriptStore } from "../../lib/transcriptStore";
import { useWorkspaceStore } from "../../lib/workspaceStore";
import { AgentExecutionGraph } from "../AgentExecutionGraph";
import { AITrainingView } from "./AITrainingView";
import { ChatView } from "./ChatView";
import { CodingAgentsView } from "./CodingAgentsView";
import { ContextView } from "./ContextView";
import { TasksView } from "./TasksView";
import { MetricRibbon, SectionTitle, iconButtonStyle } from "./shared";
import { ThreadList } from "./ThreadList";
import { TraceView } from "./TraceView";
import { UsageView } from "./UsageView";

const EMPTY_MESSAGES: AgentMessage[] = [];

export type AgentChatPanelView = "threads" | "chat" | "trace" | "usage" | "context" | "graph" | "coding-agents" | "ai-training" | "tasks";

type AgentStoreState = ReturnType<typeof useAgentStore.getState>;
type AgentMissionStoreState = ReturnType<typeof useAgentMissionStore.getState>;
type WorkspaceStoreState = ReturnType<typeof useWorkspaceStore.getState>;
type SnippetStoreState = ReturnType<typeof useSnippetStore.getState>;
type TranscriptStoreState = ReturnType<typeof useTranscriptStore.getState>;

type AgentChatPanelRuntimeValue = {
    togglePanel: () => void;
    activeWorkspace: ReturnType<WorkspaceStoreState["activeWorkspace"]>;
    threads: AgentThread[];
    activeThread: AgentThread | undefined;
    activeThreadId: string | null;
    createThread: AgentStoreState["createThread"];
    deleteThread: AgentStoreState["deleteThread"];
    setActiveThread: AgentStoreState["setActiveThread"];
    agentSettings: AgentStoreState["agentSettings"];
    searchQuery: string;
    setSearchQuery: (query: string) => void;
    messages: AgentMessage[];
    allMessagesByThread: Record<string, AgentMessage[]>;
    pendingApprovals: AgentMissionStoreState["approvals"];
    scopedOperationalEvents: AgentMissionStoreState["operationalEvents"];
    scopedCognitiveEvents: AgentMissionStoreState["cognitiveEvents"];
    latestContextSnapshot: AgentMissionStoreState["contextSnapshots"][number] | undefined;
    memory: AgentMissionStoreState["memory"];
    updateMemory: AgentMissionStoreState["updateMemory"];
    historySummary: AgentMissionStoreState["historySummary"];
    historyHits: AgentMissionStoreState["historyHits"];
    symbolHits: AgentMissionStoreState["symbolHits"];
    snapshots: AgentMissionStoreState["snapshots"];
    snippets: SnippetStoreState["snippets"];
    transcripts: TranscriptStoreState["transcripts"];
    scopePaneId: string | null;
    scopeController: ReturnType<typeof getTerminalController>;
    input: string;
    setInput: React.Dispatch<React.SetStateAction<string>>;
    historyQuery: string;
    setHistoryQuery: React.Dispatch<React.SetStateAction<string>>;
    symbolQuery: string;
    setSymbolQuery: React.Dispatch<React.SetStateAction<string>>;
    view: AgentChatPanelView;
    setView: React.Dispatch<React.SetStateAction<AgentChatPanelView>>;
    usageMessageCount: number;
    filteredThreads: AgentThread[];
    isStreamingResponse: boolean;
    messagesEndRef: React.RefObject<HTMLDivElement | null>;
    inputRef: React.RefObject<HTMLTextAreaElement | null>;
    sendMessage: (text: string) => void;
    stopStreaming: (threadId?: string | null) => void;
    handleSend: () => void;
    handleKeyDown: (event: React.KeyboardEvent) => void;
    tabItems: Array<{ id: AgentChatPanelView; label: string; count: number | null }>;
};

const AgentChatPanelRuntimeContext = createContext<AgentChatPanelRuntimeValue | null>(null);

export function AgentChatPanelProvider({ children }: { children?: React.ReactNode }) {
    const open = useWorkspaceStore((s) => s.agentPanelOpen);
    const togglePanel = useWorkspaceStore((s) => s.toggleAgentPanel);
    const activePaneId = useWorkspaceStore((s) => s.activePaneId());
    const activeWorkspace = useWorkspaceStore((s) => s.activeWorkspace());

    const threads = useAgentStore((s) => s.threads);
    const activeThreadId = useAgentStore((s) => s.activeThreadId);
    const createThread = useAgentStore((s) => s.createThread);
    const deleteThread = useAgentStore((s) => s.deleteThread);
    const setActiveThread = useAgentStore((s) => s.setActiveThread);
    const addMessage = useAgentStore((s) => s.addMessage);
    const updateLastAssistantMessage = useAgentStore((s) => s.updateLastAssistantMessage);
    const agentSettings = useAgentStore((s) => s.agentSettings);
    const searchQuery = useAgentStore((s) => s.searchQuery);
    const setSearchQuery = useAgentStore((s) => s.setSearchQuery);
    const storeMessages = useAgentStore((s) => activeThreadId ? s.messages[activeThreadId] : undefined);
    const allMessagesByThread = useAgentStore((s) => s.messages);

    const operationalEvents = useAgentMissionStore((s) => s.operationalEvents);
    const cognitiveEvents = useAgentMissionStore((s) => s.cognitiveEvents);
    const contextSnapshots = useAgentMissionStore((s) => s.contextSnapshots);
    const approvals = useAgentMissionStore((s) => s.approvals);
    const memory = useAgentMissionStore((s) => s.memory);
    const updateMemory = useAgentMissionStore((s) => s.updateMemory);
    const historySummary = useAgentMissionStore((s) => s.historySummary);
    const historyHits = useAgentMissionStore((s) => s.historyHits);
    const symbolHits = useAgentMissionStore((s) => s.symbolHits);
    const snapshots = useAgentMissionStore((s) => s.snapshots);

    const snippets = useSnippetStore((s) => s.snippets);
    const transcripts = useTranscriptStore((s) => s.transcripts);

    const [input, setInput] = useState("");
    const [view, setView] = useState<AgentChatPanelView>("threads");
    const [historyQuery, setHistoryQuery] = useState("");
    const [symbolQuery, setSymbolQuery] = useState("");
    const messagesEndRef = useRef<HTMLDivElement>(null);
    const inputRef = useRef<HTMLTextAreaElement>(null);
    const abortRef = useRef<AbortController | null>(null);
    // Daemon mode: track the daemon's thread ID (for conversation continuity)
    // and which local frontend thread should receive daemon events.
    const daemonThreadIdRef = useRef<string | null>(null);
    const daemonLocalThreadRef = useRef<string | null>(null);
    // Buffer gateway messages that arrive before thread_created
    const pendingGatewayMessagesRef = useRef<Array<{ role: "user"; content: string; inputTokens: number; outputTokens: number; totalTokens: number; isCompactionSummary: boolean }>>([]);

    // Reset daemon thread refs when backend changes to avoid stale event routing
    useEffect(() => {
        daemonThreadIdRef.current = null;
        daemonLocalThreadRef.current = null;
    }, [agentSettings.agentBackend]);

    // Sync provider config to daemon whenever settings change in daemon mode
    useEffect(() => {
        // Sync config for daemon and external agent backends (not legacy)
        if (agentSettings.agentBackend === "legacy") return;
        const amux = (window as any).tamux ?? (window as any).amux;
        if (!amux?.agentSetConfig) return;

        const providerKey = agentSettings.activeProvider;
        const pc = agentSettings[providerKey] as { baseUrl: string; model: string; apiKey: string } | undefined;

        // For external agents, provider config may not be needed (agent uses its own),
        // but we still sync agent_backend and gateway settings
        const isExternalAgent = agentSettings.agentBackend === "openclaw" || agentSettings.agentBackend === "hermes";

        if (!isExternalAgent && !pc?.baseUrl) return;

        const appSettings = useSettingsStore.getState().settings;
        void amux.agentSetConfig({
            enabled: true,
            agent_backend: agentSettings.agentBackend,
            provider: providerKey,
            base_url: pc?.baseUrl || "",
            model: pc?.model || "",
            api_key: pc?.apiKey || "",
            system_prompt: agentSettings.systemPrompt,
            max_tool_loops: agentSettings.maxToolLoops,
            max_retries: agentSettings.maxRetries,
            retry_delay_ms: agentSettings.retryDelayMs,
            tools: {
                bash: agentSettings.enableBashTool,
                web_search: agentSettings.enableWebSearchTool,
                web_browse: agentSettings.enableWebBrowsingTool,
                vision: agentSettings.enableVisionTool,
                gateway_messaging: true,
                file_operations: true,
                system_info: true,
            },
            gateway: {
                enabled: appSettings.gatewayEnabled,
                slack_token: appSettings.slackToken || "",
                telegram_token: appSettings.telegramToken || "",
                discord_token: appSettings.discordToken || "",
                command_prefix: appSettings.gatewayCommandPrefix || "!tamux",
            },
        });
    }, [agentSettings]);

    // Subscribe to daemon agent events when in daemon or external agent mode
    useEffect(() => {
        if (agentSettings.agentBackend === "legacy") return;

        const amux = (window as any).tamux ?? (window as any).amux;
        if (!amux?.onAgentEvent) return;

        const unsubscribe = amux.onAgentEvent((event: any) => {
            if (!event?.type) return;

            // Route all daemon events to the local frontend thread
            const tid = daemonLocalThreadRef.current;

            switch (event.type) {
                case "delta": {
                    if (!tid) break;
                    const msgs = useAgentStore.getState().getThreadMessages(tid);
                    const last = msgs[msgs.length - 1];
                    if (last?.role === "assistant" && last.isStreaming) {
                        updateLastAssistantMessage(tid, (last.content || "") + (event.content || ""), true);
                    }
                    break;
                }
                case "done": {
                    if (!tid) break;
                    const msgs2 = useAgentStore.getState().getThreadMessages(tid);
                    const last2 = msgs2[msgs2.length - 1];
                    if (last2?.role === "assistant") {
                        updateLastAssistantMessage(tid, last2.content || "(empty)", false, {
                            inputTokens: event.input_tokens ?? 0,
                            outputTokens: event.output_tokens ?? 0,
                            totalTokens: (event.input_tokens ?? 0) + (event.output_tokens ?? 0),
                            provider: event.provider || undefined,
                            model: event.model || undefined,
                            tps: typeof event.tps === "number" ? event.tps : undefined,
                        });
                    }
                    break;
                }
                case "tool_call": {
                    if (!tid) break;
                    // Finalize the current streaming assistant message before adding tool call
                    const tcMsgs = useAgentStore.getState().getThreadMessages(tid);
                    const tcLast = tcMsgs[tcMsgs.length - 1];
                    if (tcLast?.role === "assistant" && tcLast.isStreaming) {
                        updateLastAssistantMessage(tid, tcLast.content || "Calling tools...", false);
                    }
                    addMessage(tid, {
                        role: "tool",
                        content: "",
                        toolName: event.name,
                        toolCallId: event.call_id,
                        toolArguments: event.arguments,
                        toolStatus: "requested",
                        inputTokens: 0,
                        outputTokens: 0,
                        totalTokens: 0,
                        isCompactionSummary: false,
                    });
                    break;
                }
                case "tool_result": {
                    if (!tid) break;
                    addMessage(tid, {
                        role: "tool",
                        content: event.content,
                        toolName: event.name,
                        toolCallId: event.call_id,
                        toolStatus: event.is_error ? "error" : "done",
                        inputTokens: 0,
                        outputTokens: 0,
                        totalTokens: 0,
                        isCompactionSummary: false,
                    });
                    // Add new streaming assistant message for the next LLM turn
                    const isExtAgent = agentSettings.agentBackend === "openclaw" || agentSettings.agentBackend === "hermes";
                    addMessage(tid, {
                        role: "assistant",
                        content: "",
                        provider: isExtAgent ? agentSettings.agentBackend : agentSettings.activeProvider,
                        model: isExtAgent ? agentSettings.agentBackend : ((agentSettings[agentSettings.activeProvider] as any)?.model || ""),
                        inputTokens: 0,
                        outputTokens: 0,
                        totalTokens: 0,
                        isCompactionSummary: false,
                        isStreaming: true,
                    });
                    break;
                }
                case "error": {
                    if (!tid) break;
                    updateLastAssistantMessage(tid, `Error: ${event.message}`, false);
                    break;
                }
                case "thread_created": {
                    if (event.thread_id) {
                        daemonThreadIdRef.current = event.thread_id;

                        // Auto-create a local thread if none exists (e.g. gateway message)
                        if (!daemonLocalThreadRef.current) {
                            const workspaceId = useWorkspaceStore.getState().activeWorkspaceId;
                            const surfaceId = useWorkspaceStore.getState().activeSurface()?.id ?? null;
                            const paneId = useWorkspaceStore.getState().activePaneId();
                            const localId = createThread({
                                workspaceId,
                                surfaceId,
                                paneId,
                                title: event.title || "Gateway conversation",
                            });
                            daemonLocalThreadRef.current = localId;
                            setActiveThread(localId);
                            setView("chat");

                            // Flush any buffered gateway messages
                            for (const buffered of pendingGatewayMessagesRef.current) {
                                addMessage(localId, buffered);
                            }
                            pendingGatewayMessagesRef.current = [];

                            // Add streaming assistant placeholder
                            addMessage(localId, {
                                role: "assistant",
                                content: "",
                                provider: "daemon",
                                model: "daemon",
                                inputTokens: 0,
                                outputTokens: 0,
                                totalTokens: 0,
                                isCompactionSummary: false,
                                isStreaming: true,
                            });
                        }
                    }
                    break;
                }
                case "workspace_command": {
                    // Execute workspace mutation on the frontend
                    const cmd = event.command;
                    const cmdArgs = event.args || {};
                    const ws = useWorkspaceStore.getState();
                    try {
                        switch (cmd) {
                            case "create_workspace": ws.createWorkspace(cmdArgs.name); break;
                            case "set_active_workspace": {
                                const w = ws.workspaces.find((x: any) => x.id === cmdArgs.workspace || x.name === cmdArgs.workspace);
                                if (w) ws.setActiveWorkspace(w.id);
                                break;
                            }
                            case "create_surface": ws.createSurface(undefined, undefined); break;
                            case "set_active_surface": {
                                const aw = ws.activeWorkspace();
                                if (aw) {
                                    const s = aw.surfaces.find((x: any) => x.id === cmdArgs.surface || x.name === cmdArgs.surface);
                                    if (s) ws.setActiveSurface(s.id);
                                }
                                break;
                            }
                            case "split_pane": ws.splitActive(cmdArgs.direction === "vertical" ? "vertical" : "horizontal"); break;
                            case "rename_pane": {
                                const paneId = cmdArgs.pane || ws.activePaneId();
                                if (paneId) ws.setPaneName(paneId, cmdArgs.name);
                                break;
                            }
                            case "set_layout_preset": break; // TODO
                            case "equalize_layout": break; // TODO
                            case "create_snippet": {
                                const snippetStore = useSnippetStore.getState();
                                snippetStore.addSnippet({ name: cmdArgs.name, content: cmdArgs.content, category: cmdArgs.category, description: cmdArgs.description, tags: cmdArgs.tags, owner: "assistant" });
                                break;
                            }
                            case "run_snippet": break; // TODO
                        }
                    } catch (e: any) {
                        console.warn("workspace command failed:", cmd, e);
                    }
                    break;
                }
                case "gateway_incoming": {
                    const gwMsg = {
                        role: "user" as const,
                        content: `[${event.platform} — ${event.sender}]: ${event.content}`,
                        inputTokens: 0,
                        outputTokens: 0,
                        totalTokens: 0,
                        isCompactionSummary: false,
                    };

                    const inTid = daemonLocalThreadRef.current;
                    if (inTid) {
                        // Thread exists — add message directly + streaming placeholder
                        addMessage(inTid, gwMsg);
                        addMessage(inTid, {
                            role: "assistant",
                            content: "",
                            provider: "daemon",
                            model: "daemon",
                            inputTokens: 0,
                            outputTokens: 0,
                            totalTokens: 0,
                            isCompactionSummary: false,
                            isStreaming: true,
                        });
                    } else {
                        // Thread not yet created — buffer for thread_created handler
                        pendingGatewayMessagesRef.current.push(gwMsg);
                    }
                    break;
                }
            }
        });

        return unsubscribe;
    }, [agentSettings.agentBackend, addMessage, updateLastAssistantMessage, setActiveThread]);

    const activeThread = threads.find((thread) => thread.id === activeThreadId);
    const messages = storeMessages ?? EMPTY_MESSAGES;
    const scopePaneId = activeThread?.paneId ?? activePaneId;
    const pendingApprovals = approvals.filter((approval) => approval.status === "pending");
    const scopeController = getTerminalController(scopePaneId);

    const usageMessageCount = useMemo(
        () => Object.values(allMessagesByThread)
            .flat()
            .filter((message) => message.role === "assistant" && ((message.totalTokens ?? 0) > 0 || message.cost !== undefined)).length,
        [allMessagesByThread],
    );

    const scopedOperationalEvents = useMemo(() => {
        if (!scopePaneId) return operationalEvents.slice(0, 30);
        return operationalEvents.filter((event) => event.paneId === scopePaneId).slice(0, 30);
    }, [operationalEvents, scopePaneId]);

    const scopedCognitiveEvents = useMemo(() => {
        if (!scopePaneId) return cognitiveEvents.slice(0, 20);
        return cognitiveEvents.filter((event) => event.paneId === scopePaneId).slice(0, 20);
    }, [cognitiveEvents, scopePaneId]);

    const latestContextSnapshot = useMemo(() => {
        if (!scopePaneId) return contextSnapshots[0];
        return contextSnapshots.find((snapshot) => snapshot.paneId === scopePaneId) ?? contextSnapshots[0];
    }, [contextSnapshots, scopePaneId]);

    useEffect(() => {
        messagesEndRef.current?.scrollIntoView({ behavior: "smooth" });
    }, [messages.length]);

    useEffect(() => {
        if (open && activeThreadId) {
            setView("chat");
            setTimeout(() => inputRef.current?.focus(), 100);
        } else if (open) {
            setView("threads");
        }
    }, [open, activeThreadId]);

    const filteredThreads = searchQuery
        ? threads.filter(
            (thread) => thread.title.toLowerCase().includes(searchQuery.toLowerCase())
                || thread.lastMessagePreview.toLowerCase().includes(searchQuery.toLowerCase()),
        )
        : threads;

    const isStreamingResponse = messages.some((message) => message.role === "assistant" && message.isStreaming);

    function stopStreaming(threadId?: string | null) {
        const targetThreadId = threadId ?? activeThreadId;
        if (!targetThreadId) return;

        if (agentSettings.agentBackend !== "legacy") {
            const amux = (window as any).tamux ?? (window as any).amux;
            const daemonTid = daemonThreadIdRef.current;
            if (daemonTid && amux?.agentStopStream) {
                void amux.agentStopStream(daemonTid);
            }
        }

        abortThreadStream(targetThreadId);
        if (abortRef.current) {
            abortRef.current.abort();
            abortRef.current = null;
        }

        const threadMessages = useAgentStore.getState().getThreadMessages(targetThreadId);
        const lastMessage = threadMessages[threadMessages.length - 1];
        if (lastMessage?.role === "assistant" && lastMessage.isStreaming) {
            updateLastAssistantMessage(targetThreadId, lastMessage.content || "(stopped)", false);
        }
    }

    function sendMessage(text: string) {
        if (!text) return;

        // Daemon mode (including external agents): send to daemon agent engine via IPC
        if (agentSettings.agentBackend !== "legacy") {
            const amux = (window as any).tamux ?? (window as any).amux;
            if (!amux?.agentSendMessage) {
                sendMessageLegacy(text);
                return;
            }

            // Use the daemon's thread ID for conversation continuity.
            // The daemon is the source of truth — we pass its ID so it
            // appends to the same thread across messages.
            const daemonTid = daemonThreadIdRef.current;

            // Ensure a frontend thread exists for rendering
            let threadId = activeThreadId;
            if (!threadId) {
                const workspaceId = useWorkspaceStore.getState().activeWorkspaceId;
                const surfaceId = useWorkspaceStore.getState().activeSurface()?.id ?? null;
                const paneId = useWorkspaceStore.getState().activePaneId();
                threadId = createThread({ workspaceId, surfaceId, paneId, title: text.slice(0, 50) });
                setView("chat");
            }

            addMessage(threadId, {
                role: "user",
                content: text,
                inputTokens: 0,
                outputTokens: 0,
                totalTokens: 0,
                isCompactionSummary: false,
            });

            const isExternalAgent = agentSettings.agentBackend === "openclaw" || agentSettings.agentBackend === "hermes";
            addMessage(threadId, {
                role: "assistant",
                content: "",
                provider: isExternalAgent ? agentSettings.agentBackend : agentSettings.activeProvider,
                model: isExternalAgent ? agentSettings.agentBackend : ((agentSettings[agentSettings.activeProvider] as any)?.model || "unknown"),
                inputTokens: 0,
                outputTokens: 0,
                totalTokens: 0,
                isCompactionSummary: false,
                isStreaming: true,
            });

            // Track which local thread receives daemon events
            daemonLocalThreadRef.current = threadId;

            // Send daemon's thread ID (null for first message → daemon creates thread)
            void amux.agentSendMessage(daemonTid, text);
            return;
        }

        // Legacy mode: run LLM in frontend
        sendMessageLegacy(text);
    }

    function sendMessageLegacy(text: string) {
        let threadId = activeThreadId;
        if (!threadId) {
            const workspaceId = useWorkspaceStore.getState().activeWorkspaceId;
            const surfaceId = useWorkspaceStore.getState().activeSurface()?.id ?? null;
            const paneId = useWorkspaceStore.getState().activePaneId();
            threadId = createThread({
                workspaceId,
                surfaceId,
                paneId,
                title: text.slice(0, 50),
            });
            setView("chat");
        }

        addMessage(threadId, {
            role: "user",
            content: text,
            inputTokens: 0,
            outputTokens: 0,
            totalTokens: 0,
            isCompactionSummary: false,
        });

        const providerConfig = agentSettings[agentSettings.activeProvider] as { baseUrl: string; model: string; apiKey: string };
        const currentThreadId = threadId;
        const gatewayEnabled = useSettingsStore.getState().settings.gatewayEnabled;
        const tools = getAvailableTools({
            enableBashTool: agentSettings.enableBashTool,
            gatewayEnabled,
            enableVisionTool: agentSettings.enableVisionTool,
            enableWebBrowsingTool: agentSettings.enableWebBrowsingTool,
        });
        const toolCapabilities = getToolCapabilityDescription(tools);
        const systemPrompt = agentSettings.systemPrompt + toolCapabilities;

        stopStreaming(currentThreadId);

        addMessage(currentThreadId, {
            role: "assistant",
            content: "",
            provider: agentSettings.activeProvider,
            model: providerConfig.model,
            inputTokens: 0,
            outputTokens: 0,
            totalTokens: 0,
            isCompactionSummary: false,
            isStreaming: true,
        });
        const controller = new AbortController();
        abortRef.current = controller;
        setThreadAbortController(currentThreadId, controller);

        (async () => {
            const maxToolLoops = Math.max(1, Math.min(100, Number(agentSettings.maxToolLoops ?? 25)));
            let loopCount = 0;
            let allCurrentMessages = useAgentStore.getState().getThreadMessages(currentThreadId);
            let apiMessages = messagesToApiFormat(allCurrentMessages.slice(0, -1));
            let lastPersistedReasoning: string | null = null;

            const persistReasoningTrace = (reasoning: string) => {
                const normalized = reasoning.trim();
                if (!normalized) return;
                if (normalized === lastPersistedReasoning) return;

                const thread = useAgentStore.getState().threads.find((entry) => entry.id === currentThreadId);
                const paneId = thread?.paneId ?? useWorkspaceStore.getState().activePaneId() ?? "agent";
                const workspaceId = thread?.workspaceId ?? useWorkspaceStore.getState().activeWorkspaceId;
                const surfaceId = thread?.surfaceId ?? useWorkspaceStore.getState().activeSurface()?.id ?? null;

                useAgentMissionStore.getState().recordCognitiveOutput({
                    paneId,
                    workspaceId,
                    surfaceId,
                    sessionId: null,
                    text: `<INNER_MONOLOGUE>\n${normalized}\n</INNER_MONOLOGUE>`,
                });
                lastPersistedReasoning = normalized;
            };

            while (loopCount < maxToolLoops) {
                loopCount += 1;
                let accumulated = "";
                let accumulatedReasoning = "";
                const responseStartedAt = Date.now();
                let receivedToolCalls = false;
                let roundToolCalls: Array<{ id: string; type: "function"; function: { name: string; arguments: string } }> = [];

                try {
                    for await (const chunk of sendChatCompletion({
                        provider: agentSettings.activeProvider,
                        config: providerConfig,
                        systemPrompt,
                        messages: apiMessages,
                        streaming: agentSettings.enableStreaming,
                        signal: controller.signal,
                        tools: tools.length > 0 ? tools : undefined,
                    })) {
                        if (chunk.type === "delta") {
                            accumulated += chunk.content;
                            if (chunk.reasoning) accumulatedReasoning += chunk.reasoning;
                            updateLastAssistantMessage(currentThreadId, accumulated, true, {
                                reasoning: accumulatedReasoning || undefined,
                            });
                            continue;
                        }

                        if (chunk.type === "done") {
                            if (chunk.content && chunk.content !== accumulated) accumulated = chunk.content;
                            if (chunk.reasoning) accumulatedReasoning = chunk.reasoning;

                            persistReasoningTrace(accumulatedReasoning);

                            const elapsedSeconds = Math.max(0.001, (Date.now() - responseStartedAt) / 1000);
                            const outputTokens = Number(chunk.outputTokens ?? 0);
                            const inputTokens = Number(chunk.inputTokens ?? 0);
                            const totalTokens = Number(chunk.totalTokens ?? (inputTokens + outputTokens));
                            const tps = outputTokens > 0 ? outputTokens / elapsedSeconds : undefined;

                            updateLastAssistantMessage(currentThreadId, accumulated || "(empty response)", false, {
                                inputTokens,
                                outputTokens,
                                totalTokens,
                                reasoning: accumulatedReasoning || undefined,
                                reasoningTokens: chunk.reasoningTokens,
                                audioTokens: chunk.audioTokens,
                                videoTokens: chunk.videoTokens,
                                cost: chunk.cost,
                                tps,
                            });
                            continue;
                        }

                        if (chunk.type === "error") {
                            updateLastAssistantMessage(currentThreadId, `Error: ${chunk.content}`, false);
                            continue;
                        }

                        if (chunk.type === "tool_calls" && chunk.toolCalls) {
                            receivedToolCalls = true;
                            roundToolCalls = chunk.toolCalls;
                            if (chunk.reasoning) accumulatedReasoning = chunk.reasoning;
                            if (chunk.content) accumulated = chunk.content;

                            persistReasoningTrace(accumulatedReasoning);

                            updateLastAssistantMessage(currentThreadId, accumulated || "Calling tools...", false, {
                                reasoning: accumulatedReasoning || undefined,
                                inputTokens: Number(chunk.inputTokens ?? 0),
                                outputTokens: Number(chunk.outputTokens ?? 0),
                                totalTokens: Number(chunk.totalTokens ?? ((chunk.inputTokens ?? 0) + (chunk.outputTokens ?? 0))),
                                reasoningTokens: chunk.reasoningTokens,
                                audioTokens: chunk.audioTokens,
                                videoTokens: chunk.videoTokens,
                                cost: chunk.cost,
                                toolCalls: roundToolCalls,
                            });

                            for (const toolCall of chunk.toolCalls) {
                                addMessage(currentThreadId, {
                                    role: "tool",
                                    content: "",
                                    toolName: toolCall.function.name,
                                    toolCallId: toolCall.id,
                                    toolArguments: toolCall.function.arguments,
                                    toolStatus: "requested",
                                    inputTokens: 0,
                                    outputTokens: 0,
                                    totalTokens: 0,
                                    isCompactionSummary: false,
                                });
                            }

                            const toolResults = [];
                            for (const toolCall of chunk.toolCalls) {
                                useAgentMissionStore.getState().recordToolCall({
                                    toolName: toolCall.function.name,
                                    arguments: toolCall.function.arguments,
                                });

                                const result = await executeTool(toolCall);
                                toolResults.push(result);

                                addMessage(currentThreadId, {
                                    role: "tool",
                                    content: result.content,
                                    toolName: result.name,
                                    toolCallId: result.toolCallId,
                                    toolArguments: toolCall.function.arguments,
                                    toolStatus: result.content.startsWith("Error:") ? "error" : "done",
                                    inputTokens: 0,
                                    outputTokens: 0,
                                    totalTokens: 0,
                                    isCompactionSummary: false,
                                });
                            }

                            updateLastAssistantMessage(currentThreadId, accumulated || "Tools executed.", false);

                            apiMessages = [
                                ...apiMessages,
                                {
                                    role: "assistant",
                                    content: accumulated || "",
                                    tool_calls: roundToolCalls,
                                },
                                ...toolResults.map((result) => ({
                                    role: "tool" as const,
                                    content: result.content,
                                    tool_call_id: result.toolCallId,
                                    name: result.name,
                                })),
                            ];

                            addMessage(currentThreadId, {
                                role: "assistant",
                                content: "",
                                provider: agentSettings.activeProvider,
                                model: providerConfig.model,
                                inputTokens: 0,
                                outputTokens: 0,
                                totalTokens: 0,
                                isCompactionSummary: false,
                                isStreaming: true,
                            });
                        }
                    }
                } catch (error: any) {
                    if (error.name !== "AbortError") {
                        updateLastAssistantMessage(currentThreadId, `Error: ${error.message || String(error)}`);
                    }
                    break;
                }

                if (!receivedToolCalls) break;
                allCurrentMessages = useAgentStore.getState().getThreadMessages(currentThreadId);
                apiMessages = messagesToApiFormat(allCurrentMessages.slice(0, -1));
            }

            if (loopCount >= maxToolLoops) {
                updateLastAssistantMessage(currentThreadId, "(Tool execution limit reached)", false);
            }

            if (abortRef.current === controller) {
                abortRef.current = null;
            }
            clearThreadAbortController(currentThreadId, controller);
        })();
    }

    function handleSend() {
        const text = input.trim();
        if (!text) return;
        sendMessage(text);
        setInput("");
    }

    function handleKeyDown(event: React.KeyboardEvent) {
        if (event.key === "Enter" && !event.shiftKey) {
            event.preventDefault();
            handleSend();
        }
    }

    const tabItems = [
        { id: "threads", label: "Threads", count: threads.length },
        { id: "chat", label: "Chat", count: null },
        { id: "trace", label: "Trace", count: scopedCognitiveEvents.length },
        { id: "usage", label: "Usage", count: usageMessageCount },
        { id: "context", label: "Context", count: null },
        { id: "graph", label: "Graph", count: null },
        { id: "coding-agents", label: "Coding Agents", count: null },
        { id: "ai-training", label: "AI Training", count: null },
        { id: "tasks", label: "Tasks", count: null },
    ] satisfies Array<{ id: AgentChatPanelView; label: string; count: number | null }>;

    const value = useMemo<AgentChatPanelRuntimeValue>(() => ({
        togglePanel,
        activeWorkspace,
        threads,
        activeThread,
        activeThreadId,
        createThread,
        deleteThread,
        setActiveThread,
        agentSettings,
        searchQuery,
        setSearchQuery,
        messages,
        allMessagesByThread,
        pendingApprovals,
        scopedOperationalEvents,
        scopedCognitiveEvents,
        latestContextSnapshot,
        memory,
        updateMemory,
        historySummary,
        historyHits,
        symbolHits,
        snapshots,
        snippets,
        transcripts,
        scopePaneId,
        scopeController,
        input,
        setInput,
        historyQuery,
        setHistoryQuery,
        symbolQuery,
        setSymbolQuery,
        view,
        setView,
        usageMessageCount,
        filteredThreads,
        isStreamingResponse,
        messagesEndRef,
        inputRef,
        sendMessage,
        stopStreaming,
        handleSend,
        handleKeyDown,
        tabItems,
    }), [
        togglePanel,
        activeWorkspace,
        threads,
        activeThread,
        activeThreadId,
        createThread,
        deleteThread,
        setActiveThread,
        agentSettings,
        searchQuery,
        setSearchQuery,
        messages,
        allMessagesByThread,
        pendingApprovals,
        scopedOperationalEvents,
        scopedCognitiveEvents,
        latestContextSnapshot,
        memory,
        updateMemory,
        historySummary,
        historyHits,
        symbolHits,
        snapshots,
        snippets,
        transcripts,
        scopePaneId,
        scopeController,
        input,
        historyQuery,
        symbolQuery,
        view,
        usageMessageCount,
        filteredThreads,
        isStreamingResponse,
        tabItems,
    ]);

    if (!open) {
        return null;
    }

    return (
        <AgentChatPanelRuntimeContext.Provider value={value}>
            {children}
        </AgentChatPanelRuntimeContext.Provider>
    );
}

export function useAgentChatPanelRuntime(): AgentChatPanelRuntimeValue {
    const runtime = useContext(AgentChatPanelRuntimeContext);
    if (!runtime) {
        throw new Error("AgentChatPanel runtime is only available inside AgentChatPanelProvider.");
    }
    return runtime;
}

export function AgentChatPanelScaffold({ style, className }: { style?: CSSProperties; className?: string }) {
    return (
        <div
            style={{
                width: 560,
                minWidth: 380,
                maxWidth: 820,
                height: "100%",
                display: "flex",
                flexDirection: "column",
                background: "var(--bg-primary)",
                border: "1px solid var(--border)",
                borderRadius: "var(--radius-xl)",
                overflow: "hidden",
                ...(style ?? {}),
            }}
            className={className}
        >
            <AgentChatPanelHeader />
            <AgentChatPanelTabs />
            <div style={{ flex: 1, overflow: "hidden", position: "relative", display: "flex", flexDirection: "column" }}>
                <AgentChatPanelCurrentSurface />
            </div>
        </div>
    );
}

export function AgentChatPanelHeader() {
    const runtime = useAgentChatPanelRuntime();
    const { view, activeThread, setActiveThread, setView, togglePanel, createThread } = runtime;

    return (
        <div
            style={{
                padding: "var(--space-4)",
                borderBottom: "1px solid var(--border)",
                flexShrink: 0,
                background: "var(--bg-secondary)",
            }}
        >
            <div style={{ display: "flex", alignItems: "flex-start", justifyContent: "space-between", gap: "var(--space-3)" }}>
                <div style={{ display: "flex", flexDirection: "column", gap: "var(--space-1)" }}>
                    <div style={{ display: "flex", alignItems: "center", gap: "var(--space-2)" }}>
                        {view === "chat" && activeThread && (
                            <button
                                onClick={() => {
                                    setActiveThread(null);
                                    setView("threads");
                                }}
                                style={iconButtonStyle}
                                title="Back to threads"
                            >
                                ←
                            </button>
                        )}
                        <span
                            className="amux-agent-indicator"
                            style={{ background: "var(--mission-soft)", borderColor: "var(--mission-glow)", color: "var(--mission)" }}
                        >
                            Mission Console
                        </span>
                    </div>

                    <span style={{ fontSize: "var(--text-lg)", fontWeight: 700 }}>
                        {view === "threads" ? "Live Intelligence Surfaces" : activeThread?.title ?? "Conversation Lane"}
                    </span>

                    <span style={{ fontSize: "var(--text-sm)", color: "var(--text-muted)" }}>
                        Reasoning, execution context, recall memory, symbols, and snapshots
                    </span>
                </div>

                <div style={{ display: "flex", gap: "var(--space-1)" }}>
                    <button
                        onClick={() => {
                            const workspaceId = useWorkspaceStore.getState().activeWorkspaceId;
                            createThread({ workspaceId });
                            setView("chat");
                        }}
                        style={iconButtonStyle}
                        title="New thread"
                    >
                        +
                    </button>
                    <button onClick={togglePanel} style={iconButtonStyle} title="Close">
                        ✕
                    </button>
                </div>
            </div>
        </div>
    );
}

export function AgentChatPanelTabs() {
    const { tabItems, view, setView } = useAgentChatPanelRuntime();

    return (
        <div
            style={{
                display: "flex",
                gap: "var(--space-1)",
                padding: "var(--space-2) var(--space-3)",
                borderBottom: "1px solid var(--border)",
                background: "var(--bg-secondary)",
                overflowX: "auto",
            }}
        >
            {tabItems.map((tab) => (
                <button
                    key={tab.id}
                    type="button"
                    onClick={() => setView(tab.id)}
                    style={{
                        padding: "var(--space-1) var(--space-3)",
                        borderRadius: "var(--radius-full)",
                        border: "1px solid",
                        borderColor: view === tab.id ? "var(--accent-soft)" : "transparent",
                        background: view === tab.id ? "var(--accent-soft)" : "transparent",
                        color: view === tab.id ? "var(--accent)" : "var(--text-muted)",
                        fontSize: "var(--text-xs)",
                        fontWeight: 500,
                        cursor: "pointer",
                        whiteSpace: "nowrap",
                        transition: "all var(--transition-fast)",
                    }}
                >
                    {tab.label}
                    {tab.count !== null && <span style={{ marginLeft: "var(--space-1)", opacity: 0.7 }}>{tab.count}</span>}
                </button>
            ))}
        </div>
    );
}

export function AgentChatPanelCurrentSurface() {
    const { view } = useAgentChatPanelRuntime();

    if (view === "threads") return <AgentChatPanelThreadsSurface />;
    if (view === "chat") return <AgentChatPanelChatSurface />;
    if (view === "trace") return <AgentChatPanelTraceSurface />;
    if (view === "usage") return <AgentChatPanelUsageSurface />;
    if (view === "context") return <AgentChatPanelContextSurface />;
    if (view === "coding-agents") return <AgentChatPanelCodingAgentsSurface />;
    if (view === "ai-training") return <AgentChatPanelAITrainingSurface />;
    if (view === "tasks") return <TasksView />;
    return <AgentChatPanelGraphSurface />;
}

export function AgentChatPanelThreadsSurface() {
    const { filteredThreads, searchQuery, setSearchQuery, setActiveThread, setView, deleteThread } = useAgentChatPanelRuntime();

    return (
        <ThreadList
            threads={filteredThreads}
            searchQuery={searchQuery}
            onSearch={setSearchQuery}
            onSelect={(thread) => {
                setActiveThread(thread.id);
                setView("chat");
            }}
            onDelete={deleteThread}
        />
    );
}

export function AgentChatPanelChatSurface() {
    const runtime = useAgentChatPanelRuntime();
    return (
        <ChatView
            messages={runtime.messages}
            input={runtime.input}
            setInput={runtime.setInput}
            inputRef={runtime.inputRef}
            onKeyDown={runtime.handleKeyDown}
            agentSettings={runtime.agentSettings}
            isStreamingResponse={runtime.isStreamingResponse}
            activeThread={runtime.activeThread}
            messagesEndRef={runtime.messagesEndRef}
            onSendMessage={runtime.sendMessage}
            onStopStreaming={() => runtime.stopStreaming(runtime.activeThreadId)}
        />
    );
}

export function AgentChatPanelTraceSurface() {
    const { scopedOperationalEvents, scopedCognitiveEvents, pendingApprovals } = useAgentChatPanelRuntime();
    return (
        <TraceView
            operationalEvents={scopedOperationalEvents}
            cognitiveEvents={scopedCognitiveEvents}
            pendingApprovals={pendingApprovals}
        />
    );
}

export function AgentChatPanelUsageSurface() {
    const { threads, allMessagesByThread } = useAgentChatPanelRuntime();
    return <UsageView threads={threads} messagesByThread={allMessagesByThread} />;
}

export function AgentChatPanelContextSurface() {
    const runtime = useAgentChatPanelRuntime();
    return (
        <ContextView
            agentSettings={runtime.agentSettings}
            snippets={runtime.snippets}
            transcripts={runtime.transcripts}
            scopePaneId={runtime.scopePaneId}
            threads={runtime.threads}
            latestContextSnapshot={runtime.latestContextSnapshot}
            memory={runtime.memory}
            updateMemory={runtime.updateMemory}
            historyQuery={runtime.historyQuery}
            setHistoryQuery={runtime.setHistoryQuery}
            historySummary={runtime.historySummary}
            historyHits={runtime.historyHits}
            symbolQuery={runtime.symbolQuery}
            setSymbolQuery={runtime.setSymbolQuery}
            symbolHits={runtime.symbolHits}
            snapshots={runtime.snapshots}
            scopeController={runtime.scopeController}
            activeWorkspace={runtime.activeWorkspace}
        />
    );
}

export function AgentChatPanelGraphSurface() {
    const { scopedOperationalEvents, approvals, scopePaneId } = useAgentChatPanelGraphData();

    return (
        <div style={{ padding: "var(--space-4)", height: "100%", overflow: "auto" }}>
            <MetricRibbon
                items={[
                    { label: "Commands", value: String(scopedOperationalEvents.filter((event) => event.kind === "command-started").length) },
                    { label: "Approvals", value: String(approvals.length) },
                    { label: "Scope", value: scopePaneId ?? "all panes" },
                ]}
            />
            <SectionTitle title="Execution Graph" subtitle="Visualized command pipeline" />
            <AgentExecutionGraph paneId={scopePaneId} />
        </div>
    );
}

export function AgentChatPanelCodingAgentsSurface() {
    return <CodingAgentsView />;
}

export function AgentChatPanelAITrainingSurface() {
    return <AITrainingView />;
}

function useAgentChatPanelGraphData() {
    const { scopedOperationalEvents, scopePaneId } = useAgentChatPanelRuntime();
    const approvals = useAgentMissionStore((s) => s.approvals);
    return { scopedOperationalEvents, approvals, scopePaneId };
}
