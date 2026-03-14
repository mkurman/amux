import { create } from "zustand";
import type { WorkspaceId, SurfaceId, PaneId } from "./types";
import type { ToolCall } from "./agentTools";
import {
  readPersistedJson,
  scheduleJsonWrite,
} from "./persistence";

// ---------------------------------------------------------------------------
// Types matching amux-windows AgentConversationThread/Message
// ---------------------------------------------------------------------------
export interface AgentThread {
  id: string;
  workspaceId: WorkspaceId | null;
  surfaceId: SurfaceId | null;
  paneId: PaneId | null;
  agentName: string;
  title: string;
  createdAt: number;
  updatedAt: number;
  messageCount: number;
  totalInputTokens: number;
  totalOutputTokens: number;
  totalTokens: number;
  compactionCount: number;
  lastMessagePreview: string;
}

export type AgentRole = "user" | "assistant" | "system" | "tool";

export type AgentProviderId =
  | "featherless"
  | "openai"
  | "anthropic"
  | "qwen"
  | "qwen-deepinfra"
  | "kimi"
  | "z.ai"
  | "openrouter"
  | "cerebras"
  | "together"
  | "groq"
  | "ollama"
  | "chutes"
  | "huggingface"
  | "minimax"
  | "custom";

const AGENT_PROVIDER_IDS: AgentProviderId[] = [
  "featherless",
  "openai",
  "anthropic",
  "qwen",
  "qwen-deepinfra",
  "kimi",
  "z.ai",
  "openrouter",
  "cerebras",
  "together",
  "groq",
  "ollama",
  "chutes",
  "huggingface",
  "minimax",
  "custom",
];

function normalizeAgentProviderId(value: unknown): AgentProviderId {
  if (typeof value !== "string") {
    return "openai";
  }
  const normalized = value.trim() as AgentProviderId;
  return AGENT_PROVIDER_IDS.includes(normalized) ? normalized : "openai";
}

export interface AgentProviderConfig {
  baseUrl: string;
  model: string;
  apiKey: string;
}

export interface AgentMessage {
  id: string;
  threadId: string;
  createdAt: number;
  role: AgentRole;
  content: string;
  provider?: string;
  model?: string;
  toolCalls?: ToolCall[];
  toolName?: string;
  toolCallId?: string;
  toolArguments?: string;
  toolStatus?: "requested" | "executing" | "done" | "error";
  inputTokens: number;
  outputTokens: number;
  totalTokens: number;
  reasoning?: string;
  reasoningTokens?: number;
  audioTokens?: number;
  videoTokens?: number;
  cost?: number;
  tps?: number;
  isCompactionSummary: boolean;
  isStreaming?: boolean;
}

// ---------------------------------------------------------------------------
// Agent Settings matching amux-windows
// ---------------------------------------------------------------------------
export interface AgentSettings {
  enabled: boolean;
  agentName: string;
  handler: string;
  additionalHandlers: string[];
  systemPrompt: string;

  activeProvider: AgentProviderId;
  featherless: AgentProviderConfig;
  openai: AgentProviderConfig;
  anthropic: AgentProviderConfig;
  qwen: AgentProviderConfig;
  "qwen-deepinfra": AgentProviderConfig;
  kimi: AgentProviderConfig;
  "z.ai": AgentProviderConfig;
  openrouter: AgentProviderConfig;
  cerebras: AgentProviderConfig;
  together: AgentProviderConfig;
  groq: AgentProviderConfig;
  ollama: AgentProviderConfig;
  chutes: AgentProviderConfig;
  huggingface: AgentProviderConfig;
  minimax: AgentProviderConfig;
  custom: AgentProviderConfig;

  enableBashTool: boolean;
  enableVisionTool: boolean;
  enableWebBrowsingTool: boolean;
  bashTimeoutSeconds: number;
  enableWebSearchTool: boolean;
  searchToolProvider: "none" | "firecrawl" | "exa" | "tavily";
  firecrawlApiKey: string;
  exaApiKey: string;
  tavilyApiKey: string;
  searchMaxResults: number;
  searchTimeoutSeconds: number;

  enableStreaming: boolean;
  enableConversationMemory: boolean;

  chatFontFamily: string;
  chatFontSize: number;

  autoCompactContext: boolean;
  maxContextMessages: number;
  maxToolLoops: number;
  contextBudgetTokens: number;
  compactThresholdPercent: number;
  keepRecentOnCompaction: number;
}

export const DEFAULT_AGENT_SETTINGS: AgentSettings = {
  enabled: false,
  agentName: "assistant",
  handler: "/agent",
  additionalHandlers: [],
  systemPrompt: "You are tamux, an agentic terminal multiplexer assistant. You can execute terminal commands, check system resources, and send messages to connected chat platforms (Slack, Discord, Telegram, WhatsApp) via the gateway. Use your tools proactively when the user asks you to perform actions. Be concise and direct.",

  activeProvider: "openai",
  featherless: { baseUrl: "https://api.featherless.ai/v1", model: "meta-llama/Llama-3.3-70B-Instruct", apiKey: "" },
  openai: { baseUrl: "https://api.openai.com/v1", model: "gpt-4o", apiKey: "" },
  anthropic: { baseUrl: "https://api.anthropic.com", model: "claude-sonnet-4-20250514", apiKey: "" },
  qwen: { baseUrl: "https://api.qwen.com/v1", model: "qwen-max", apiKey: "" },
  "qwen-deepinfra": { baseUrl: "https://api.deepinfra.com/v1/openai", model: "Qwen/Qwen2.5-72B-Instruct", apiKey: "" },
  kimi: { baseUrl: "https://api.moonshot.ai/v1", model: "moonshot-v1-32k", apiKey: "" },
  "z.ai": { baseUrl: "https://api.z.ai/api/paas/v4", model: "glm-4-plus", apiKey: "" },
  openrouter: { baseUrl: "https://openrouter.ai/api/v1", model: "anthropic/claude-sonnet-4", apiKey: "" },
  cerebras: { baseUrl: "https://api.cerebras.ai/v1", model: "llama-3.3-70b", apiKey: "" },
  together: { baseUrl: "https://api.together.xyz/v1", model: "meta-llama/Llama-3.3-70B-Instruct-Turbo", apiKey: "" },
  groq: { baseUrl: "https://api.groq.com/openai/v1", model: "llama-3.3-70b-versatile", apiKey: "" },
  ollama: { baseUrl: "http://localhost:11434/v1", model: "llama3.1", apiKey: "" },
  chutes: { baseUrl: "https://llm.chutes.ai/v1", model: "deepseek-ai/DeepSeek-V3", apiKey: "" },
  huggingface: { baseUrl: "https://api-inference.huggingface.co/v1", model: "meta-llama/Llama-3.3-70B-Instruct", apiKey: "" },
  minimax: { baseUrl: "https://api.minimax.io/anthropic", model: "MiniMax-M1-80k", apiKey: "" },
  custom: { baseUrl: "", model: "", apiKey: "" },

  enableBashTool: true,
  enableVisionTool: false,
  enableWebBrowsingTool: false,
  bashTimeoutSeconds: 30,
  enableWebSearchTool: false,
  searchToolProvider: "none",
  firecrawlApiKey: "",
  exaApiKey: "",
  tavilyApiKey: "",
  searchMaxResults: 8,
  searchTimeoutSeconds: 20,

  enableStreaming: true,
  enableConversationMemory: true,

  chatFontFamily: "Cascadia Code",
  chatFontSize: 13,

  autoCompactContext: true,
  maxContextMessages: 100,
  maxToolLoops: 25,
  contextBudgetTokens: 100000,
  compactThresholdPercent: 80,
  keepRecentOnCompaction: 10,
};

// ---------------------------------------------------------------------------
// Store
// ---------------------------------------------------------------------------
export interface AgentState {
  threads: AgentThread[];
  messages: Record<string, AgentMessage[]>; // threadId -> messages
  activeThreadId: string | null;
  agentPanelOpen: boolean;
  agentSettings: AgentSettings;
  searchQuery: string;

  // Thread actions
  createThread: (opts: {
    workspaceId?: WorkspaceId | null;
    surfaceId?: SurfaceId | null;
    paneId?: PaneId | null;
    title?: string;
  }) => string;
  deleteThread: (id: string) => void;
  setActiveThread: (id: string | null) => void;
  searchThreads: (query: string) => AgentThread[];

  // Message actions
  addMessage: (threadId: string, msg: Omit<AgentMessage, "id" | "threadId" | "createdAt">) => void;
  updateLastAssistantMessage: (
    threadId: string,
    content: string,
    streaming?: boolean,
    meta?: Partial<Pick<AgentMessage, "inputTokens" | "outputTokens" | "totalTokens" | "reasoning" | "reasoningTokens" | "audioTokens" | "videoTokens" | "cost" | "tps" | "toolCalls">>
  ) => void;
  getThreadMessages: (threadId: string) => AgentMessage[];

  // Panel
  toggleAgentPanel: () => void;
  setSearchQuery: (q: string) => void;

  // Settings
  updateAgentSetting: <K extends keyof AgentSettings>(key: K, value: AgentSettings[K]) => void;
  resetAgentSettings: () => void;

  // Derived
  getThreadsForPane: (paneId: PaneId) => AgentThread[];
}

const threadAbortControllers = new Map<string, AbortController>();

export function setThreadAbortController(threadId: string, controller: AbortController): void {
  threadAbortControllers.set(threadId, controller);
}

export function abortThreadStream(threadId: string): void {
  const controller = threadAbortControllers.get(threadId);
  if (!controller) return;
  controller.abort();
  threadAbortControllers.delete(threadId);
}

export function clearThreadAbortController(threadId: string, controller?: AbortController): void {
  const current = threadAbortControllers.get(threadId);
  if (!current) return;
  if (controller && current !== controller) return;
  threadAbortControllers.delete(threadId);
}

let _threadId = 0;
let _msgId = 0;

function loadAgentSettings(): AgentSettings {
  return { ...DEFAULT_AGENT_SETTINGS };
}

const AGENT_SETTINGS_FILE = "agent-settings.json";
const AGENT_CHAT_FILE = "agent-chat.json";

type AgentChatState = {
  threads: AgentThread[];
  messages: Record<string, AgentMessage[]>;
  activeThreadId: string | null;
};

function saveAgentSettings(s: AgentSettings) {
  scheduleJsonWrite(AGENT_SETTINGS_FILE, s);
}

function syncChatCounters(chat: AgentChatState) {
  let maxThread = 0;
  let maxMessage = 0;

  for (const thread of chat.threads) {
    const match = /^thread_(\d+)$/.exec(thread.id);
    if (match) {
      maxThread = Math.max(maxThread, Number(match[1]));
    }
  }

  for (const threadMessages of Object.values(chat.messages)) {
    for (const message of threadMessages) {
      const match = /^msg_(\d+)$/.exec(message.id);
      if (match) {
        maxMessage = Math.max(maxMessage, Number(match[1]));
      }
    }
  }

  _threadId = Math.max(_threadId, maxThread);
  _msgId = Math.max(_msgId, maxMessage);
}

function saveChatState(chat: AgentChatState) {
  scheduleJsonWrite(AGENT_CHAT_FILE, chat, 200);
}

export const useAgentStore = create<AgentState>((set, get) => ({
  threads: [],
  messages: {},
  activeThreadId: null,
  agentPanelOpen: false,
  agentSettings: loadAgentSettings(),
  searchQuery: "",

  createThread: (opts) => {
    const id = `thread_${++_threadId}`;
    const now = Date.now();
    const thread: AgentThread = {
      id,
      workspaceId: opts.workspaceId ?? null,
      surfaceId: opts.surfaceId ?? null,
      paneId: opts.paneId ?? null,
      agentName: get().agentSettings.agentName,
      title: opts.title ?? "New Conversation",
      createdAt: now,
      updatedAt: now,
      messageCount: 0,
      totalInputTokens: 0,
      totalOutputTokens: 0,
      totalTokens: 0,
      compactionCount: 0,
      lastMessagePreview: "",
    };
    set((s) => {
      const next: AgentChatState = {
        threads: [thread, ...s.threads],
        messages: { ...s.messages, [id]: [] },
        activeThreadId: id,
      };
      saveChatState(next);
      return next;
    });
    return id;
  },

  deleteThread: (id) => {
    set((s) => {
      const { [id]: _, ...rest } = s.messages;
      const next: AgentChatState = {
        threads: s.threads.filter((t) => t.id !== id),
        messages: rest,
        activeThreadId: s.activeThreadId === id ? null : s.activeThreadId,
      };
      saveChatState(next);
      return next;
    });
  },

  setActiveThread: (id) => set((s) => {
    const next: AgentChatState = {
      threads: s.threads,
      messages: s.messages,
      activeThreadId: id,
    };
    saveChatState(next);
    return { activeThreadId: id };
  }),

  searchThreads: (query) => {
    const lower = query.toLowerCase();
    return get().threads.filter(
      (t) =>
        t.title.toLowerCase().includes(lower) ||
        t.lastMessagePreview.toLowerCase().includes(lower) ||
        t.agentName.toLowerCase().includes(lower)
    );
  },

  addMessage: (threadId, msg) => {
    const id = `msg_${++_msgId}`;
    const full: AgentMessage = {
      ...msg,
      id,
      threadId,
      createdAt: Date.now(),
    };
    set((s) => {
      const threadMsgs = [...(s.messages[threadId] ?? []), full];
      const _thread = s.threads.find((t) => t.id === threadId); void _thread;
      const next: AgentChatState = {
        messages: { ...s.messages, [threadId]: threadMsgs },
        threads: s.threads.map((t) =>
          t.id === threadId
            ? {
              ...t,
              messageCount: t.messageCount + 1,
              updatedAt: Date.now(),
              totalInputTokens: t.totalInputTokens + msg.inputTokens,
              totalOutputTokens: t.totalOutputTokens + msg.outputTokens,
              totalTokens: t.totalTokens + msg.totalTokens,
              lastMessagePreview: msg.content.slice(0, 100),
            }
            : t
        ),
        activeThreadId: s.activeThreadId,
      };
      saveChatState(next);
      return next;
    });
  },

  updateLastAssistantMessage: (threadId, content, streaming, meta) => {
    set((s) => {
      const msgs = s.messages[threadId];
      if (!msgs || msgs.length === 0) return s;
      const last = msgs[msgs.length - 1];
      if (last.role !== "assistant") return s;
      const nextInputTokens = meta?.inputTokens ?? last.inputTokens;
      const nextOutputTokens = meta?.outputTokens ?? last.outputTokens;
      const nextTotalTokens = meta?.totalTokens ?? last.totalTokens;
      const updatedLast: AgentMessage = {
        ...last,
        content,
        isStreaming: streaming ?? false,
        inputTokens: nextInputTokens,
        outputTokens: nextOutputTokens,
        totalTokens: nextTotalTokens,
        reasoning: meta?.reasoning ?? last.reasoning,
        reasoningTokens: meta?.reasoningTokens ?? last.reasoningTokens,
        audioTokens: meta?.audioTokens ?? last.audioTokens,
        videoTokens: meta?.videoTokens ?? last.videoTokens,
        cost: meta?.cost ?? last.cost,
        tps: meta?.tps ?? last.tps,
        toolCalls: meta?.toolCalls ?? last.toolCalls,
      };
      const updated = [...msgs.slice(0, -1), updatedLast];
      const tokenDeltaIn = nextInputTokens - last.inputTokens;
      const tokenDeltaOut = nextOutputTokens - last.outputTokens;
      const tokenDeltaTotal = nextTotalTokens - last.totalTokens;
      const nextThreads = s.threads.map((thread) =>
        thread.id === threadId
          ? {
            ...thread,
            totalInputTokens: thread.totalInputTokens + tokenDeltaIn,
            totalOutputTokens: thread.totalOutputTokens + tokenDeltaOut,
            totalTokens: thread.totalTokens + tokenDeltaTotal,
            updatedAt: Date.now(),
            lastMessagePreview: content.slice(0, 100),
          }
          : thread,
      );
      const next = { messages: { ...s.messages, [threadId]: updated }, threads: nextThreads };
      saveChatState({
        threads: nextThreads,
        messages: next.messages,
        activeThreadId: s.activeThreadId,
      });
      return next;
    });
  },

  getThreadMessages: (threadId) => get().messages[threadId] ?? [],

  toggleAgentPanel: () => set((s) => ({ agentPanelOpen: !s.agentPanelOpen })),
  setSearchQuery: (q) => set({ searchQuery: q }),

  updateAgentSetting: (key, value) => {
    set((s) => {
      const nextValue = key === "activeProvider"
        ? normalizeAgentProviderId(value)
        : value;
      const updated = { ...s.agentSettings, [key]: nextValue };
      saveAgentSettings(updated);
      return { agentSettings: updated };
    });
  },

  resetAgentSettings: () => {
    const def = { ...DEFAULT_AGENT_SETTINGS };
    saveAgentSettings(def);
    set({ agentSettings: def });
  },

  getThreadsForPane: (paneId) => get().threads.filter((t) => t.paneId === paneId),
}));

export async function hydrateAgentStore(): Promise<void> {
  const diskState = await readPersistedJson<AgentSettings>(AGENT_SETTINGS_FILE);
  if (diskState) {
    const merged: AgentSettings = {
      ...DEFAULT_AGENT_SETTINGS,
      ...diskState,
      activeProvider: normalizeAgentProviderId(diskState.activeProvider),
      featherless: { ...DEFAULT_AGENT_SETTINGS.featherless, ...(diskState.featherless ?? {}) },
      openai: { ...DEFAULT_AGENT_SETTINGS.openai, ...(diskState.openai ?? {}) },
      anthropic: { ...DEFAULT_AGENT_SETTINGS.anthropic, ...(diskState.anthropic ?? {}) },
      qwen: { ...DEFAULT_AGENT_SETTINGS.qwen, ...(diskState.qwen ?? {}) },
      "qwen-deepinfra": { ...DEFAULT_AGENT_SETTINGS["qwen-deepinfra"], ...(diskState["qwen-deepinfra"] ?? {}) },
      kimi: { ...DEFAULT_AGENT_SETTINGS.kimi, ...(diskState.kimi ?? {}) },
      "z.ai": { ...DEFAULT_AGENT_SETTINGS["z.ai"], ...(diskState["z.ai"] ?? {}) },
      openrouter: { ...DEFAULT_AGENT_SETTINGS.openrouter, ...(diskState.openrouter ?? {}) },
      cerebras: { ...DEFAULT_AGENT_SETTINGS.cerebras, ...(diskState.cerebras ?? {}) },
      together: { ...DEFAULT_AGENT_SETTINGS.together, ...(diskState.together ?? {}) },
      groq: { ...DEFAULT_AGENT_SETTINGS.groq, ...(diskState.groq ?? {}) },
      ollama: { ...DEFAULT_AGENT_SETTINGS.ollama, ...(diskState.ollama ?? {}) },
      chutes: { ...DEFAULT_AGENT_SETTINGS.chutes, ...(diskState.chutes ?? {}) },
      huggingface: { ...DEFAULT_AGENT_SETTINGS.huggingface, ...(diskState.huggingface ?? {}) },
      minimax: { ...DEFAULT_AGENT_SETTINGS.minimax, ...(diskState.minimax ?? {}) },
      custom: { ...DEFAULT_AGENT_SETTINGS.custom, ...(diskState.custom ?? {}) },
    };
    useAgentStore.setState({ agentSettings: merged });
  }

  const diskChat = await readPersistedJson<AgentChatState>(AGENT_CHAT_FILE);
  const chat = diskChat;
  if (!chat || !Array.isArray(chat.threads) || !chat.messages || typeof chat.messages !== "object") {
    return;
  }

  const messages: Record<string, AgentMessage[]> = {};
  for (const thread of chat.threads) {
    const threadMessages = Array.isArray(chat.messages[thread.id]) ? chat.messages[thread.id] : [];
    messages[thread.id] = threadMessages
      .filter((entry) => entry && typeof entry.id === "string" && typeof entry.content === "string")
      .map((entry) => ({
        ...entry,
        threadId: thread.id,
        createdAt: Number(entry.createdAt) || Date.now(),
      }));
  }

  const hydrated: AgentChatState = {
    threads: chat.threads
      .filter((thread) => thread && typeof thread.id === "string" && thread.id)
      .map((thread) => ({
        ...thread,
        messageCount: messages[thread.id]?.length ?? 0,
        lastMessagePreview: messages[thread.id]?.[messages[thread.id].length - 1]?.content?.slice(0, 100) ?? thread.lastMessagePreview ?? "",
      })),
    messages,
    activeThreadId: typeof chat.activeThreadId === "string" ? chat.activeThreadId : null,
  };

  syncChatCounters(hydrated);
  useAgentStore.setState(hydrated);
  saveChatState(hydrated);
}
