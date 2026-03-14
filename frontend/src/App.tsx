import { Suspense, lazy, useCallback, useEffect, useMemo, useRef } from "react";
import { LayoutContainer } from "./components/LayoutContainer";
import { SurfaceTabBar } from "./components/SurfaceTabBar";
import { StatusBar } from "./components/StatusBar";
import { Sidebar } from "./components/Sidebar";
import { TitleBar } from "./components/TitleBar";
import { AgentApprovalOverlay } from "./components/AgentApprovalOverlay";
import { useAgentMissionStore } from "./lib/agentMissionStore";
import { clearThreadAbortController, setThreadAbortController, useAgentStore } from "./lib/agentStore";
import { applyAppShellTheme, getAppShellTheme } from "./lib/themes";
import { useSettingsStore } from "./lib/settingsStore";
import { useWorkspaceStore } from "./lib/workspaceStore";
import { useHotkeys } from "./hooks/useHotkeys";
import { saveSession, startAutoSave } from "./lib/sessionPersistence";
import { sendChatCompletion, messagesToApiFormat } from "./lib/agentClient";
import { executeTool, getAvailableTools, getToolCapabilityDescription } from "./lib/agentTools";
import { readPersistedJson, scheduleJsonWrite } from "./lib/persistence";

const GATEWAY_THREAD_MAP_FILE = "gateway-thread-map.json";

const CommandPalette = lazy(() => import("./components/CommandPalette").then((module) => ({ default: module.CommandPalette })));
const NotificationPanel = lazy(() => import("./components/NotificationPanel").then((module) => ({ default: module.NotificationPanel })));
const SettingsPanel = lazy(() => import("./components/SettingsPanel").then((module) => ({ default: module.SettingsPanel })));
const SessionVaultPanel = lazy(() => import("./components/SessionVaultPanel").then((module) => ({ default: module.SessionVaultPanel })));
const CommandLogPanel = lazy(() => import("./components/CommandLogPanel").then((module) => ({ default: module.CommandLogPanel })));
const CommandHistoryPicker = lazy(() => import("./components/CommandHistoryPicker").then((module) => ({ default: module.CommandHistoryPicker })));
const SearchOverlay = lazy(() => import("./components/SearchOverlay").then((module) => ({ default: module.SearchOverlay })));
const AgentChatPanel = lazy(() => import("./components/AgentChatPanel").then((module) => ({ default: module.AgentChatPanel })));
const SnippetPicker = lazy(() => import("./components/SnippetPicker").then((module) => ({ default: module.SnippetPicker })));
const SystemMonitorPanel = lazy(() => import("./components/SystemMonitorPanel").then((module) => ({ default: module.SystemMonitorPanel })));
const FileManagerPanel = lazy(() => import("./components/FileManagerPanel").then((module) => ({ default: module.FileManagerPanel })));
const TimeTravelSlider = lazy(() => import("./components/TimeTravelSlider").then((module) => ({ default: module.TimeTravelSlider })));
const ExecutionCanvas = lazy(() => import("./components/ExecutionCanvas").then((module) => ({ default: module.ExecutionCanvas })));

export default function App() {
  const createWorkspace = useWorkspaceStore((s) => s.createWorkspace);
  const createSurface = useWorkspaceStore((s) => s.createSurface);
  const splitActive = useWorkspaceStore((s) => s.splitActive);
  const toggleZoom = useWorkspaceStore((s) => s.toggleZoom);
  const toggleSidebar = useWorkspaceStore((s) => s.toggleSidebar);
  const toggleSettings = useWorkspaceStore((s) => s.toggleSettings);
  const toggleSearch = useWorkspaceStore((s) => s.toggleSearch);
  const toggleFileManager = useWorkspaceStore((s) => s.toggleFileManager);
  const toggleCommandPalette = useWorkspaceStore((s) => s.toggleCommandPalette);
  const toggleCommandHistory = useWorkspaceStore((s) => s.toggleCommandHistory);
  const toggleCommandLog = useWorkspaceStore((s) => s.toggleCommandLog);
  const toggleSystemMonitor = useWorkspaceStore((s) => s.toggleSystemMonitor);
  const toggleCanvas = useWorkspaceStore((s) => s.toggleCanvas);
  const toggleTimeTravel = useWorkspaceStore((s) => s.toggleTimeTravel);
  const workspaces = useWorkspaceStore((s) => s.workspaces);
  const sidebarVisible = useWorkspaceStore((s) => s.sidebarVisible);
  const sidebarWidth = useWorkspaceStore((s) => s.sidebarWidth);
  const agentPanelOpen = useWorkspaceStore((s) => s.agentPanelOpen);
  const commandPaletteOpen = useWorkspaceStore((s) => s.commandPaletteOpen);
  const notificationPanelOpen = useWorkspaceStore((s) => s.notificationPanelOpen);
  const settingsOpen = useWorkspaceStore((s) => s.settingsOpen);
  const sessionVaultOpen = useWorkspaceStore((s) => s.sessionVaultOpen);
  const commandLogOpen = useWorkspaceStore((s) => s.commandLogOpen);
  const commandHistoryOpen = useWorkspaceStore((s) => s.commandHistoryOpen);
  const searchOpen = useWorkspaceStore((s) => s.searchOpen);
  const snippetPickerOpen = useWorkspaceStore((s) => s.snippetPickerOpen);
  const systemMonitorOpen = useWorkspaceStore((s) => s.systemMonitorOpen);
  const fileManagerOpen = useWorkspaceStore((s) => s.fileManagerOpen);
  const canvasOpen = useWorkspaceStore((s) => s.canvasOpen);
  const timeTravelOpen = useWorkspaceStore((s) => s.timeTravelOpen);
  const settings = useSettingsStore((s) => s.settings);
  const activeWorkspace = useWorkspaceStore((s) => s.activeWorkspace());
  const activeSurface = useWorkspaceStore((s) => s.activeSurface());
  const activeProvider = useAgentStore((s) => s.agentSettings.activeProvider);
  const cognitiveEvents = useAgentMissionStore((s) => s.cognitiveEvents);
  const operationalEvents = useAgentMissionStore((s) => s.operationalEvents);
  const approvals = useAgentMissionStore((s) => s.approvals);
  const snapshots = useAgentMissionStore((s) => s.snapshots);
  const historyHits = useAgentMissionStore((s) => s.historyHits);
  const symbolHits = useAgentMissionStore((s) => s.symbolHits);
  const toggleAgentPanel = useWorkspaceStore((s) => s.toggleAgentPanel);
  const toggleSessionVault = useWorkspaceStore((s) => s.toggleSessionVault);
  const gatewayThreadMapRef = useRef<Record<string, string>>({});
  const gatewayInFlightRef = useRef<Set<string>>(new Set());

  const persistGatewayThreadMap = useCallback(() => {
    scheduleJsonWrite(GATEWAY_THREAD_MAP_FILE, gatewayThreadMapRef.current, 100);
  }, []);

  const traceCount = cognitiveEvents.length;
  const opsCount = operationalEvents.length;
  const snapshotCount = snapshots.length;
  const historyHitsCount = historyHits.length;
  const symbolHitsCount = symbolHits.length;
  const approvalCount = useMemo(
    () => approvals.filter((entry) => entry.status === "pending").length,
    [approvals],
  );

  useHotkeys();

  useEffect(() => {
    if (workspaces.length === 0) {
      createWorkspace("Default");
    }
  }, []);

  useEffect(() => startAutoSave(30_000), []);

  useEffect(() => {
    void readPersistedJson<Record<string, string>>(GATEWAY_THREAD_MAP_FILE).then((persisted) => {
      if (persisted && typeof persisted === "object") {
        gatewayThreadMapRef.current = persisted;
      }
    });
  }, []);

  useEffect(() => {
    const timeoutId = window.setTimeout(() => {
      saveSession();
    }, 500);

    return () => window.clearTimeout(timeoutId);
  }, [workspaces, sidebarVisible, sidebarWidth]);

  useEffect(() => {
    applyAppShellTheme(
      getAppShellTheme(
        settings.themeName,
        settings.useCustomTerminalColors,
        settings.customTerminalBackground,
        settings.customTerminalForeground,
        settings.customTerminalCursor,
        settings.customTerminalSelection,
      )
    );

    const amux = (window as any).tamux ?? (window as any).amux;
    void amux?.setWindowOpacity?.(settings.opacity);
  }, [
    settings.themeName,
    settings.useCustomTerminalColors,
    settings.customTerminalBackground,
    settings.customTerminalForeground,
    settings.customTerminalCursor,
    settings.customTerminalSelection,
    settings.opacity,
  ]);

  useEffect(() => {
    const amux = (window as any).tamux ?? (window as any).amux;
    if (!amux?.onAppCommand) return;

    return amux.onAppCommand((command: string) => {
      switch (command) {
        case "new-workspace":
          createWorkspace();
          break;
        case "new-surface":
          createSurface();
          break;
        case "toggle-settings":
          toggleSettings();
          break;
        case "toggle-command-palette":
          toggleCommandPalette();
          break;
        case "toggle-search":
          toggleSearch();
          break;
        case "toggle-file-manager":
          toggleFileManager();
          break;
        case "toggle-mission":
          toggleAgentPanel();
          break;
        case "toggle-command-history":
          toggleCommandHistory();
          break;
        case "toggle-command-log":
          toggleCommandLog();
          break;
        case "toggle-session-vault":
          toggleSessionVault();
          break;
        case "toggle-system-monitor":
          toggleSystemMonitor();
          break;
        case "toggle-canvas":
          toggleCanvas();
          break;
        case "toggle-time-travel":
          toggleTimeTravel();
          break;
        case "toggle-sidebar":
          toggleSidebar();
          break;
        case "split-right":
          splitActive("horizontal");
          break;
        case "split-down":
          splitActive("vertical");
          break;
        case "toggle-zoom":
          toggleZoom();
          break;
        case "about":
          if (!settingsOpen) {
            toggleSettings();
          }
          window.setTimeout(() => {
            window.dispatchEvent(new CustomEvent("tamux-open-settings-tab", {
              detail: { tab: "about" },
            }));
            window.dispatchEvent(new CustomEvent("amux-open-settings-tab", {
              detail: { tab: "about" },
            }));
          }, 50);
          break;
      }
    });
  }, [
    createWorkspace,
    createSurface,
    settingsOpen,
    splitActive,
    toggleAgentPanel,
    toggleCanvas,
    toggleCommandHistory,
    toggleCommandLog,
    toggleCommandPalette,
    toggleFileManager,
    toggleSearch,
    toggleSessionVault,
    toggleSettings,
    toggleSidebar,
    toggleSystemMonitor,
    toggleTimeTravel,
    toggleZoom,
  ]);

  const handleInboundGatewayMessage = useCallback(async (params: {
    provider: "discord" | "whatsapp" | "slack" | "telegram";
    channelId: string;
    userId: string;
    username: string;
    text: string;
    replyTarget: string;
  }) => {
    const amux = (window as any).tamux ?? (window as any).amux;
    const settingsState = useSettingsStore.getState().settings;
    const agentState = useAgentStore.getState();

    if (!settingsState.gatewayEnabled) return;

    const provider = params.provider;
    const channelId = params.channelId.trim();
    const userId = params.userId.trim();
    const username = params.username.trim() || `${provider}-user`;
    const rawContent = params.text;

    if (!channelId) return;

    if (provider === "discord") {
      if (!settingsState.discordToken) return;

      const allowedChannels = settingsState.discordChannelFilter
        .split(",")
        .map((entry) => entry.trim())
        .filter(Boolean)
        .map((entry) => (entry.match(/\d{17,20}/)?.[0] ?? entry));

      if (allowedChannels.length > 0 && !allowedChannels.includes(channelId)) {
        return;
      }

      const allowedUsers = settingsState.discordAllowedUsers
        .split(",")
        .map((entry) => entry.trim())
        .filter(Boolean)
        .map((entry) => (entry.match(/\d{17,20}/)?.[0] ?? entry));

      if (allowedUsers.length > 0 && userId && !allowedUsers.includes(userId)) {
        return;
      }
    }

    if (provider === "slack") {
      if (!settingsState.slackToken) return;

      const allowedChannels = settingsState.slackChannelFilter
        .split(",")
        .map((entry) => entry.trim())
        .filter(Boolean)
        .map((entry) => entry.toLowerCase());

      if (allowedChannels.length > 0) {
        const normalizedChannelId = channelId.toLowerCase();
        const normalizedReplyTarget = params.replyTarget.toLowerCase();
        const allowed = allowedChannels.some((entry) => (
          entry === normalizedChannelId || entry === normalizedReplyTarget
        ));
        if (!allowed) return;
      }
    }

    if (provider === "telegram") {
      if (!settingsState.telegramToken) return;

      const allowedChats = settingsState.telegramAllowedChats
        .split(",")
        .map((entry) => entry.trim())
        .filter(Boolean);
      if (allowedChats.length > 0 && !allowedChats.includes(channelId)) {
        return;
      }
    }

    if (provider === "whatsapp") {
      const allowedContacts = settingsState.whatsappAllowedContacts
        .split(",")
        .map((entry) => entry.trim())
        .filter(Boolean);
      if (allowedContacts.length > 0) {
        const normalizedTarget = params.replyTarget.replace(/\s+/g, "").toLowerCase();
        const allowed = allowedContacts.some((entry) => {
          const normalizedEntry = entry.replace(/\s+/g, "").toLowerCase();
          return normalizedTarget.includes(normalizedEntry);
        });
        if (!allowed) return;
      }
    }

    const cleaned = rawContent
      .replace(/<@!?\d{17,20}>/g, "")
      .replace(/^\s*amux[:,]?\s*/i, "")
      .trim();

    if (!cleaned) return;
    const routeKey = `${provider}:${channelId}`;
    if (gatewayInFlightRef.current.has(routeKey)) return;

    gatewayInFlightRef.current.add(routeKey);
    try {
      const threadPrefix = `${provider}:${channelId}:`;
      const activeThreadId = useAgentStore.getState().activeThreadId;
      let threadId = activeThreadId ?? gatewayThreadMapRef.current[routeKey];
      let threadExists = threadId
        ? agentState.threads.some((thread) => thread.id === threadId)
        : false;

      if (activeThreadId && threadExists) {
        gatewayThreadMapRef.current[routeKey] = activeThreadId;
        persistGatewayThreadMap();
      }

      if (!threadId || !threadExists) {
        const existingThread = agentState.threads.find((thread) => thread.title.startsWith(threadPrefix));
        if (existingThread) {
          threadId = existingThread.id;
          gatewayThreadMapRef.current[routeKey] = threadId;
          threadExists = true;
          persistGatewayThreadMap();
        }
      }

      if (!threadId || !threadExists) {
        threadId = agentState.createThread({
          workspaceId: useWorkspaceStore.getState().activeWorkspaceId,
          title: `${threadPrefix}${username || "user"}`,
        });
        gatewayThreadMapRef.current[routeKey] = threadId;
        persistGatewayThreadMap();
      }

      agentState.addMessage(threadId, {
        role: "user",
        content: cleaned,
        inputTokens: 0,
        outputTokens: 0,
        totalTokens: 0,
        isCompactionSummary: false,
      });

      const activeProvider = agentState.agentSettings.activeProvider;
      const providerConfig = agentState.agentSettings[activeProvider] as { baseUrl: string; model: string; apiKey: string };
      const tools = getAvailableTools({
        enableBashTool: agentState.agentSettings.enableBashTool,
        gatewayEnabled: settingsState.gatewayEnabled,
        enableVisionTool: agentState.agentSettings.enableVisionTool,
        enableWebBrowsingTool: agentState.agentSettings.enableWebBrowsingTool,
      });
      const toolCapabilities = getToolCapabilityDescription(tools);
      const hiddenGatewayContext = [
        "[Hidden Gateway Context]",
        `provider=${provider}`,
        `channel=${channelId}`,
        `user=${username}${userId ? ` (${userId})` : ""}`,
        "You are tamux's terminal agent running inside the user's tamux terminal environment.",
        "You can use tamux tools and terminal capabilities to help the user.",
        "Respond as the same assistant and keep continuity for this provider thread.",
      ].join("\n");

      const systemPrompt = `${agentState.agentSettings.systemPrompt}${toolCapabilities}\n\n${hiddenGatewayContext}`;

      agentState.addMessage(threadId, {
        role: "assistant",
        content: "",
        provider: activeProvider,
        model: providerConfig.model,
        inputTokens: 0,
        outputTokens: 0,
        totalTokens: 0,
        isCompactionSummary: false,
        isStreaming: true,
      });

      const maxToolLoops = Math.max(1, Math.min(100, Number(agentState.agentSettings.maxToolLoops ?? 25)));
      let loopCount = 0;
      let finalReply = "";
      let apiMessages = messagesToApiFormat(useAgentStore.getState().getThreadMessages(threadId).slice(0, -1));
      const controller = new AbortController();
      setThreadAbortController(threadId, controller);
      let lastPersistedReasoning: string | null = null;

      const persistReasoningTrace = (reasoning: string) => {
        const normalized = reasoning.trim();
        if (!normalized) return;
        if (normalized === lastPersistedReasoning) return;

        const thread = useAgentStore.getState().threads.find((entry) => entry.id === threadId);
        useAgentMissionStore.getState().recordCognitiveOutput({
          paneId: thread?.paneId ?? `gateway:${provider}:${channelId}`,
          workspaceId: thread?.workspaceId ?? null,
          surfaceId: thread?.surfaceId ?? null,
          sessionId: null,
          text: `<INNER_MONOLOGUE>\n${normalized}\n</INNER_MONOLOGUE>`,
        });
        lastPersistedReasoning = normalized;
      };

      try {
        while (loopCount < maxToolLoops) {
          loopCount += 1;
          let accumulated = "";
          let accumulatedReasoning = "";
          const responseStartedAt = Date.now();
          let receivedToolCalls = false;
          let roundToolCalls: Array<{ id: string; type: "function"; function: { name: string; arguments: string } }> = [];

          for await (const chunk of sendChatCompletion({
            provider: activeProvider,
            config: providerConfig,
            systemPrompt,
            messages: apiMessages,
            streaming: true,
            signal: controller.signal,
            tools: tools.length > 0 ? tools : undefined,
          })) {
            if (chunk.type === "delta") {
              accumulated += chunk.content;
              if (chunk.reasoning) {
                accumulatedReasoning += chunk.reasoning;
              }
              agentState.updateLastAssistantMessage(threadId, accumulated, true, {
                reasoning: accumulatedReasoning || undefined,
              });
            } else if (chunk.type === "done") {
              if (chunk.content && chunk.content !== accumulated) {
                accumulated = chunk.content;
              }
              if (chunk.reasoning) {
                accumulatedReasoning = chunk.reasoning;
              }

              persistReasoningTrace(accumulatedReasoning);

              const elapsedSeconds = Math.max(0.001, (Date.now() - responseStartedAt) / 1000);
              const outputTokens = Number(chunk.outputTokens ?? 0);
              const inputTokens = Number(chunk.inputTokens ?? 0);
              const totalTokens = Number(chunk.totalTokens ?? (inputTokens + outputTokens));
              const tps = outputTokens > 0 ? outputTokens / elapsedSeconds : undefined;

              agentState.updateLastAssistantMessage(threadId, accumulated || "(empty response)", false, {
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
            } else if (chunk.type === "error") {
              accumulated = `Error: ${chunk.content}`;
              agentState.updateLastAssistantMessage(threadId, accumulated, false);
            } else if (chunk.type === "tool_calls" && chunk.toolCalls) {
              receivedToolCalls = true;
              roundToolCalls = chunk.toolCalls;

              if (chunk.reasoning) {
                accumulatedReasoning = chunk.reasoning;
              }

              if (chunk.content) {
                accumulated = chunk.content;
              }

              persistReasoningTrace(accumulatedReasoning);
              agentState.updateLastAssistantMessage(threadId, accumulated || "Calling tools...", false, {
                reasoning: accumulatedReasoning || undefined,
                inputTokens: Number(chunk.inputTokens ?? 0),
                outputTokens: Number(chunk.outputTokens ?? 0),
                totalTokens: Number(chunk.totalTokens ?? ((chunk.inputTokens ?? 0) + (chunk.outputTokens ?? 0))),
                reasoningTokens: chunk.reasoningTokens,
                audioTokens: chunk.audioTokens,
                videoTokens: chunk.videoTokens,
                cost: chunk.cost,
              });

              const toolResults = [];
              for (const tc of chunk.toolCalls) {
                agentState.addMessage(threadId, {
                  role: "tool",
                  content: "",
                  toolName: tc.function.name,
                  toolCallId: tc.id,
                  toolArguments: tc.function.arguments,
                  toolStatus: "requested",
                  inputTokens: 0,
                  outputTokens: 0,
                  totalTokens: 0,
                  isCompactionSummary: false,
                });

                const result = await executeTool(tc);
                toolResults.push(result);
                agentState.addMessage(threadId, {
                  role: "tool",
                  content: result.content,
                  toolName: result.name,
                  toolCallId: result.toolCallId,
                  toolArguments: tc.function.arguments,
                  toolStatus: result.content.startsWith("Error:") ? "error" : "done",
                  inputTokens: 0,
                  outputTokens: 0,
                  totalTokens: 0,
                  isCompactionSummary: false,
                });
              }

              agentState.updateLastAssistantMessage(threadId, accumulated || "Tools executed.", false);

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

              agentState.addMessage(threadId, {
                role: "assistant",
                content: "",
                provider: activeProvider,
                model: providerConfig.model,
                inputTokens: 0,
                outputTokens: 0,
                totalTokens: 0,
                isCompactionSummary: false,
                isStreaming: true,
              });
            }
          }

          if (!receivedToolCalls) {
            finalReply = accumulated.trim();
            break;
          }
        }
      } finally {
        clearThreadAbortController(threadId, controller);
      }

      if (loopCount >= maxToolLoops) {
        agentState.updateLastAssistantMessage(threadId, "(Tool execution limit reached)", false);
        finalReply = "";
      }

      if (!finalReply) {
        return;
      }

      if (provider === "discord") {
        await amux?.sendDiscordMessage?.({
          token: settingsState.discordToken,
          channelId,
          message: finalReply,
        });
      } else if (provider === "slack") {
        await amux?.sendSlackMessage?.({
          token: settingsState.slackToken,
          channelId,
          message: finalReply,
        });
      } else if (provider === "telegram") {
        await amux?.sendTelegramMessage?.({
          token: settingsState.telegramToken,
          chatId: channelId,
          message: finalReply,
        });
      } else if (provider === "whatsapp") {
        await amux?.whatsappSend?.(params.replyTarget, finalReply);
      }
    } finally {
      gatewayInFlightRef.current.delete(routeKey);
    }
  }, [persistGatewayThreadMap]);

  useEffect(() => {
    const amux = (window as any).tamux ?? (window as any).amux;
    if (!amux?.ensureSlackConnected || !amux?.onSlackMessage) return;
    if (!settings.gatewayEnabled || !settings.slackToken) return;

    let disposed = false;
    void amux.ensureSlackConnected({ token: settings.slackToken }).then((result: any) => {
      if (disposed || result?.ok) return;
      console.warn("Slack bridge connection failed:", result?.error ?? "unknown error");
    });

    const unsubscribe = amux.onSlackMessage((payload: any) => {
      const channelId = String(payload?.channelId ?? "").trim();
      const channelName = String(payload?.channelName ?? "").trim();
      void handleInboundGatewayMessage({
        provider: "slack",
        channelId,
        userId: String(payload?.userId ?? ""),
        username: String(payload?.username ?? "slack-user"),
        text: String(payload?.content ?? ""),
        replyTarget: channelName || channelId,
      });
    });

    return () => {
      disposed = true;
      unsubscribe?.();
    };
  }, [handleInboundGatewayMessage, settings.gatewayEnabled, settings.slackToken]);

  useEffect(() => {
    const amux = (window as any).tamux ?? (window as any).amux;
    if (!amux?.ensureTelegramConnected || !amux?.onTelegramMessage) return;
    if (!settings.gatewayEnabled || !settings.telegramToken) return;

    let disposed = false;
    void amux.ensureTelegramConnected({ token: settings.telegramToken }).then((result: any) => {
      if (disposed || result?.ok) return;
      console.warn("Telegram bridge connection failed:", result?.error ?? "unknown error");
    });

    const unsubscribe = amux.onTelegramMessage((payload: any) => {
      void handleInboundGatewayMessage({
        provider: "telegram",
        channelId: String(payload?.chatId ?? ""),
        userId: String(payload?.userId ?? ""),
        username: String(payload?.username ?? "telegram-user"),
        text: String(payload?.content ?? ""),
        replyTarget: String(payload?.chatId ?? ""),
      });
    });

    return () => {
      disposed = true;
      unsubscribe?.();
    };
  }, [handleInboundGatewayMessage, settings.gatewayEnabled, settings.telegramToken]);

  useEffect(() => {
    const amux = (window as any).tamux ?? (window as any).amux;
    if (!amux?.ensureDiscordConnected || !amux?.onDiscordMessage) return;
    if (!settings.gatewayEnabled || !settings.discordToken) return;

    let disposed = false;
    void amux.ensureDiscordConnected({ token: settings.discordToken }).then((result: any) => {
      if (disposed || result?.ok) return;
      console.warn("Discord bridge connection failed:", result?.error ?? "unknown error");
    });

    const unsubscribe = amux.onDiscordMessage((payload: any) => {
      void handleInboundGatewayMessage({
        provider: "discord",
        channelId: String(payload?.channelId ?? ""),
        userId: String(payload?.userId ?? ""),
        username: String(payload?.username ?? "discord-user"),
        text: String(payload?.content ?? ""),
        replyTarget: String(payload?.channelId ?? ""),
      });
    });

    return () => {
      disposed = true;
      unsubscribe?.();
    };
  }, [handleInboundGatewayMessage, settings.discordToken, settings.gatewayEnabled]);

  useEffect(() => {
    const amux = (window as any).tamux ?? (window as any).amux;
    if (!amux?.onWhatsAppMessage || !amux?.whatsappStatus) return;
    if (!settings.gatewayEnabled) return;

    let disposed = false;
    void amux.whatsappStatus().then((status: any) => {
      if (disposed) return;
      if (status?.status !== "connected") return;
    });

    const unsubscribe = amux.onWhatsAppMessage((msg: any) => {
      void handleInboundGatewayMessage({
        provider: "whatsapp",
        channelId: String(msg?.from ?? ""),
        userId: String(msg?.from ?? ""),
        username: String(msg?.pushName ?? "whatsapp-user"),
        text: String(msg?.text ?? ""),
        replyTarget: String(msg?.from ?? ""),
      });
    });

    return () => {
      disposed = true;
      unsubscribe?.();
    };
  }, [handleInboundGatewayMessage, settings.gatewayEnabled]);

  return (
    <div
      style={{
        display: "flex",
        flexDirection: "column",
        width: "100%",
        height: "100%",
        background: "var(--bg-void)",
        overflow: "hidden",
      }}
    >
      <TitleBar />

      <div style={{ flex: 1, display: "flex", flexDirection: "column", minHeight: 0, gap: 0, padding: 0 }}>
        <MissionDeck
          workspaceName={activeWorkspace?.name ?? "No workspace"}
          surfaceName={activeSurface?.name ?? "No surface"}
          activeProvider={activeProvider}
          traceCount={traceCount}
          opsCount={opsCount}
          approvalCount={approvalCount}
          snapshotCount={snapshotCount}
          historyHitsCount={historyHitsCount}
          symbolHitsCount={symbolHitsCount}
          onOpenMission={toggleAgentPanel}
          onOpenVault={toggleSessionVault}
        />

        <SurfaceTabBar />

        <div style={{ flex: 1, display: "flex", overflow: "hidden", gap: 0, minHeight: 0, minWidth: 0 }}>
          {sidebarVisible && <Sidebar />}

          <div
            style={{
              flex: 1,
              display: "flex",
              flexDirection: "column",
              overflow: "hidden",
              minWidth: 0,
              minHeight: 0,
            }}
            className="amux-shell-card"
          >
            <LayoutContainer />

            <Suspense fallback={null}>
              {searchOpen && <SearchOverlay />}
              {timeTravelOpen && <TimeTravelSlider />}
            </Suspense>
          </div>

          <Suspense fallback={null}>
            {agentPanelOpen && <AgentChatPanel />}
            {settingsOpen && <SettingsPanel />}
            {sessionVaultOpen && <SessionVaultPanel />}
            {commandLogOpen && <CommandLogPanel />}
            {systemMonitorOpen && <SystemMonitorPanel />}
            {fileManagerOpen && <FileManagerPanel />}
          </Suspense>
        </div>

        <StatusBar />
      </div>

      <Suspense fallback={null}>
        {commandPaletteOpen && <CommandPalette />}
        {notificationPanelOpen && <NotificationPanel />}
        {commandHistoryOpen && <CommandHistoryPicker />}
        {snippetPickerOpen && <SnippetPicker />}
        {canvasOpen && <ExecutionCanvas />}
      </Suspense>

      <AgentApprovalOverlay />
    </div>
  );
}

function MissionDeck({
  workspaceName,
  surfaceName,
  activeProvider,
  traceCount,
  opsCount,
  approvalCount,
  snapshotCount,
  historyHitsCount,
  symbolHitsCount,
  onOpenMission,
  onOpenVault,
}: {
  workspaceName: string;
  surfaceName: string;
  activeProvider: string;
  traceCount: number;
  opsCount: number;
  approvalCount: number;
  snapshotCount: number;
  historyHitsCount: number;
  symbolHitsCount: number;
  onOpenMission: () => void;
  onOpenVault: () => void;
}) {
  const providerText = typeof activeProvider === "string" && activeProvider.trim().length > 0
    ? activeProvider
    : "unknown";

  return (
    <div
      className="amux-shell-card"
      style={{
        flexShrink: 0,
        padding: "6px 10px",
        minHeight: 52,
        display: "flex",
        alignItems: "center",
        justifyContent: "space-between",
        gap: "var(--space-2)",
        overflowX: "auto",
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
        <span className="amux-agent-indicator" style={{ fontSize: 10, padding: "2px 8px" }}>Mission</span>
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
          provider {providerText}
        </span>
      </div>

      <div style={{ display: "flex", alignItems: "center", gap: "var(--space-2)", whiteSpace: "nowrap" }}>
        <span className="amux-chip amux-chip--approval" style={{ fontSize: 10, padding: "2px 6px" }}>
          approvals {approvalCount}
        </span>
        <span className="amux-chip" style={{ fontSize: 10, padding: "2px 6px", color: "var(--reasoning)" }}>
          trace {traceCount}
        </span>
        <span className="amux-chip" style={{ fontSize: 10, padding: "2px 6px", color: "var(--agent)" }}>
          ops {opsCount}
        </span>
        <span className="amux-chip" style={{ fontSize: 10, padding: "2px 6px", color: "var(--timeline)" }}>
          recall {historyHitsCount + symbolHitsCount}
        </span>
        <span className="amux-chip" style={{ fontSize: 10, padding: "2px 6px" }}>
          snapshots {snapshotCount}
        </span>
      </div>

      <div style={{ display: "flex", alignItems: "center", gap: "var(--space-1)", whiteSpace: "nowrap" }}>
        <button
          type="button"
          onClick={onOpenMission}
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
          Mission
        </button>
        <button
          type="button"
          onClick={onOpenVault}
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
          Vault
        </button>
      </div>
    </div>
  );
}
