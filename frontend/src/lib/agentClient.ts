/**
 * LLM API client for agent chat.
 *
 * Supports two API formats:
 *  - OpenAI-compatible (most providers)
 *  - Anthropic Messages API
 *
 * All providers are called directly from the frontend via fetch().
 */

import type { AgentProviderId, AgentProviderConfig, AgentMessage } from "./agentStore";
import type { ToolDefinition, ToolCall } from "./agentTools";

export interface ApiChatMessage {
  role: string;
  content: string;
  tool_call_id?: string;
  name?: string;
  tool_calls?: ToolCall[];
}

export interface ChatRequest {
  provider: AgentProviderId;
  config: AgentProviderConfig;
  systemPrompt: string;
  messages: ApiChatMessage[];
  streaming: boolean;
  signal?: AbortSignal;
  tools?: ToolDefinition[];
}

export interface ChatChunk {
  type: "delta" | "done" | "error" | "tool_calls";
  content: string;
  reasoning?: string;
  inputTokens?: number;
  outputTokens?: number;
  totalTokens?: number;
  cost?: number;
  reasoningTokens?: number;
  audioTokens?: number;
  videoTokens?: number;
  toolCalls?: ToolCall[];
}

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/**
 * Send a chat completion request. Returns an async iterator of content chunks
 * for streaming, or a single chunk for non-streaming.
 */
export async function* sendChatCompletion(
  req: ChatRequest,
): AsyncGenerator<ChatChunk> {
  if (!req.config.apiKey && req.provider !== "ollama") {
    yield { type: "error", content: `No API key configured for ${req.provider}. Open Settings > Agent to add your key.` };
    return;
  }

  if (!req.config.baseUrl) {
    yield { type: "error", content: `No base URL configured for ${req.provider}.` };
    return;
  }

  try {
    if (req.provider === "anthropic") {
      yield* sendAnthropic(req);
    } else {
      yield* sendOpenAICompatible(req);
    }
  } catch (err: any) {
    if (err.name === "AbortError") {
      yield { type: "done", content: "" };
    } else {
      yield { type: "error", content: `API error: ${err.message || String(err)}` };
    }
  }
}

// ---------------------------------------------------------------------------
// OpenAI-compatible API (covers 15 of 16 providers)
// ---------------------------------------------------------------------------

function buildChatCompletionUrl(provider: AgentProviderId, baseUrl: string): string {
  const base = baseUrl.replace(/\/$/, "");
  const lowerBase = base.toLowerCase();

  // OpenRouter/Groq defaults already include /api/v1 or /openai/v1.
  if (provider === "openrouter" || provider === "groq") {
    return `${base}/chat/completions`;
  }

  // Most other providers in this app use base URLs ending with /v1.
  if (/(^|\/)api\/v1$/.test(lowerBase) || /(^|\/)v1$/.test(lowerBase)) {
    return `${base}/chat/completions`;
  }

  // For custom/unversioned endpoints, use explicit /v1 path.
  return `${base}/v1/chat/completions`;
}

async function* sendOpenAICompatible(req: ChatRequest): AsyncGenerator<ChatChunk> {
  const url = buildChatCompletionUrl(req.provider, req.config.baseUrl);

  const body: Record<string, unknown> = {
    model: req.config.model,
    messages: [
      { role: "system", content: req.systemPrompt },
      ...req.messages,
    ],
    stream: req.streaming,
  };

  // Include tool definitions if provided
  if (req.tools && req.tools.length > 0) {
    body.tools = req.tools;
    body.tool_choice = "auto";
  }

  const headers: Record<string, string> = {
    "Content-Type": "application/json",
  };

  if (req.config.apiKey) {
    headers["Authorization"] = `Bearer ${req.config.apiKey}`;
  }

  const response = await fetch(url, {
    method: "POST",
    headers,
    body: JSON.stringify(body),
    signal: req.signal,
  });

  if (!response.ok) {
    const text = await response.text().catch(() => "");
    yield { type: "error", content: `${req.provider} API returned ${response.status}: ${text.slice(0, 200)}` };
    return;
  }

  if (req.streaming && response.body) {
    yield* parseSSEStream(response.body, req.signal);
  } else {
    const json = await response.json();
    const msg = json.choices?.[0]?.message;
    const content = msg?.content ?? "";

    // Check for tool calls
    if (msg?.tool_calls && msg.tool_calls.length > 0) {
      yield {
        type: "tool_calls",
        content: content,
        toolCalls: msg.tool_calls.map((tc: any) => ({
          id: tc.id,
          type: "function",
          function: {
            name: tc.function.name,
            arguments: tc.function.arguments,
          },
        })),
        inputTokens: json.usage?.prompt_tokens ?? 0,
        outputTokens: json.usage?.completion_tokens ?? 0,
      };
    } else {
      yield {
        type: "done",
        content,
        inputTokens: json.usage?.prompt_tokens ?? 0,
        outputTokens: json.usage?.completion_tokens ?? 0,
      };
    }
  }
}

// ---------------------------------------------------------------------------
// Anthropic Messages API
// ---------------------------------------------------------------------------

async function* sendAnthropic(req: ChatRequest): AsyncGenerator<ChatChunk> {
  const url = `${req.config.baseUrl.replace(/\/$/, "")}/v1/messages`;

  const body: Record<string, unknown> = {
    model: req.config.model,
    max_tokens: 4096,
    system: req.systemPrompt,
    messages: req.messages.map((m) => ({
      role: m.role === "system" ? "user" : (m.role === "tool" ? "user" : m.role),
      content: m.role === "tool"
        ? [{ type: "tool_result", tool_use_id: m.tool_call_id, content: m.content }]
        : m.content,
    })),
    stream: req.streaming,
  };

  // Include tool definitions in Anthropic format
  if (req.tools && req.tools.length > 0) {
    body.tools = req.tools.map((t) => ({
      name: t.function.name,
      description: t.function.description,
      input_schema: t.function.parameters,
    }));
  }

  const headers: Record<string, string> = {
    "Content-Type": "application/json",
    "x-api-key": req.config.apiKey,
    "anthropic-version": "2023-06-01",
    "anthropic-dangerous-direct-browser-access": "true",
  };

  const response = await fetch(url, {
    method: "POST",
    headers,
    body: JSON.stringify(body),
    signal: req.signal,
  });

  if (!response.ok) {
    const text = await response.text().catch(() => "");
    yield { type: "error", content: `Anthropic API returned ${response.status}: ${text.slice(0, 200)}` };
    return;
  }

  if (req.streaming && response.body) {
    yield* parseAnthropicSSE(response.body, req.signal);
  } else {
    const json = await response.json();

    // Check for tool_use blocks in Anthropic response
    const toolUseBlocks = json.content?.filter((b: any) => b.type === "tool_use") ?? [];
    if (toolUseBlocks.length > 0) {
      const textContent = json.content?.filter((b: any) => b.type === "text").map((b: any) => b.text).join("") ?? "";
      yield {
        type: "tool_calls",
        content: textContent,
        toolCalls: toolUseBlocks.map((b: any) => ({
          id: b.id,
          type: "function" as const,
          function: {
            name: b.name,
            arguments: JSON.stringify(b.input),
          },
        })),
        inputTokens: json.usage?.input_tokens ?? 0,
        outputTokens: json.usage?.output_tokens ?? 0,
      };
    } else {
      const content = json.content?.[0]?.text ?? "";
      yield {
        type: "done",
        content,
        inputTokens: json.usage?.input_tokens ?? 0,
        outputTokens: json.usage?.output_tokens ?? 0,
      };
    }
  }
}

// ---------------------------------------------------------------------------
// SSE parsers
// ---------------------------------------------------------------------------

async function* parseSSEStream(
  body: ReadableStream<Uint8Array>,
  signal?: AbortSignal,
): AsyncGenerator<ChatChunk> {
  const reader = body.getReader();
  const decoder = new TextDecoder();
  let buffer = "";
  let totalContent = "";
  let totalReasoning = "";
  const pendingToolCalls: Map<number, { id: string; name: string; args: string }> = new Map();
  let usage: {
    inputTokens?: number;
    outputTokens?: number;
    totalTokens?: number;
    cost?: number;
    reasoningTokens?: number;
    audioTokens?: number;
    videoTokens?: number;
  } = {};

  try {
    while (true) {
      if (signal?.aborted) break;
      const { done, value } = await reader.read();
      if (done) break;

      buffer += decoder.decode(value, { stream: true });
      const lines = buffer.split("\n");
      buffer = lines.pop() ?? "";

      for (const line of lines) {
        if (!line.startsWith("data: ")) continue;
        const data = line.slice(6).trim();
        if (data === "[DONE]") {
          // If we accumulated tool calls, yield them
          if (pendingToolCalls.size > 0) {
            yield {
              type: "tool_calls",
              content: totalContent,
              reasoning: totalReasoning,
              toolCalls: Array.from(pendingToolCalls.values()).map((tc) => ({
                id: tc.id,
                type: "function" as const,
                function: { name: tc.name, arguments: tc.args },
              })),
              inputTokens: usage.inputTokens,
              outputTokens: usage.outputTokens,
              totalTokens: usage.totalTokens,
              cost: usage.cost,
              reasoningTokens: usage.reasoningTokens,
              audioTokens: usage.audioTokens,
              videoTokens: usage.videoTokens,
            };
          } else {
            yield {
              type: "done",
              content: totalContent,
              reasoning: totalReasoning,
              inputTokens: usage.inputTokens,
              outputTokens: usage.outputTokens,
              totalTokens: usage.totalTokens,
              cost: usage.cost,
              reasoningTokens: usage.reasoningTokens,
              audioTokens: usage.audioTokens,
              videoTokens: usage.videoTokens,
            };
          }
          return;
        }

        try {
          const parsed = JSON.parse(data);
          const delta = parsed.choices?.[0]?.delta;

          if (parsed.usage) {
            const parsedUsage = parsed.usage;
            usage = {
              inputTokens: Number(parsedUsage.prompt_tokens ?? usage.inputTokens ?? 0),
              outputTokens: Number(parsedUsage.completion_tokens ?? usage.outputTokens ?? 0),
              totalTokens: Number(parsedUsage.total_tokens ?? usage.totalTokens ?? 0),
              cost: parsedUsage.cost !== undefined ? Number(parsedUsage.cost) : usage.cost,
              reasoningTokens:
                parsedUsage.completion_tokens_details?.reasoning_tokens !== undefined
                  ? Number(parsedUsage.completion_tokens_details.reasoning_tokens)
                  : usage.reasoningTokens,
              audioTokens:
                parsedUsage.completion_tokens_details?.audio_tokens !== undefined
                  ? Number(parsedUsage.completion_tokens_details.audio_tokens)
                  : usage.audioTokens,
              videoTokens:
                parsedUsage.prompt_tokens_details?.video_tokens !== undefined
                  ? Number(parsedUsage.prompt_tokens_details.video_tokens)
                  : usage.videoTokens,
            };
          }

          // Handle content delta
          if (delta?.content) {
            totalContent += delta.content;
            yield { type: "delta", content: delta.content };
          }

          // Handle reasoning deltas (e.g. Qwen/OpenRouter style)
          if (delta?.reasoning) {
            totalReasoning += String(delta.reasoning);
            yield { type: "delta", content: "", reasoning: String(delta.reasoning) };
          } else if (Array.isArray(delta?.reasoning_details)) {
            for (const detail of delta.reasoning_details) {
              const piece = typeof detail?.text === "string" ? detail.text : "";
              if (!piece) continue;
              totalReasoning += piece;
              yield { type: "delta", content: "", reasoning: piece };
            }
          }

          // Handle tool call deltas (streamed incrementally)
          if (delta?.tool_calls) {
            for (const tc of delta.tool_calls) {
              const idx = tc.index ?? 0;
              if (!pendingToolCalls.has(idx)) {
                pendingToolCalls.set(idx, { id: tc.id || "", name: "", args: "" });
              }
              const pending = pendingToolCalls.get(idx)!;
              if (tc.id) pending.id = tc.id;
              if (tc.function?.name) pending.name += tc.function.name;
              if (tc.function?.arguments) pending.args += tc.function.arguments;
            }
          }
        } catch {
          // skip malformed JSON chunks
        }
      }
    }
  } finally {
    reader.releaseLock();
  }

  // Stream ended without [DONE] — yield accumulated tool calls or content
  if (pendingToolCalls.size > 0) {
    yield {
      type: "tool_calls",
      content: totalContent,
      reasoning: totalReasoning,
      toolCalls: Array.from(pendingToolCalls.values()).map((tc) => ({
        id: tc.id,
        type: "function" as const,
        function: { name: tc.name, arguments: tc.args },
      })),
      inputTokens: usage.inputTokens,
      outputTokens: usage.outputTokens,
      totalTokens: usage.totalTokens,
      cost: usage.cost,
      reasoningTokens: usage.reasoningTokens,
      audioTokens: usage.audioTokens,
      videoTokens: usage.videoTokens,
    };
  } else {
    yield {
      type: "done",
      content: totalContent,
      reasoning: totalReasoning,
      inputTokens: usage.inputTokens,
      outputTokens: usage.outputTokens,
      totalTokens: usage.totalTokens,
      cost: usage.cost,
      reasoningTokens: usage.reasoningTokens,
      audioTokens: usage.audioTokens,
      videoTokens: usage.videoTokens,
    };
  }
}

async function* parseAnthropicSSE(
  body: ReadableStream<Uint8Array>,
  signal?: AbortSignal,
): AsyncGenerator<ChatChunk> {
  const reader = body.getReader();
  const decoder = new TextDecoder();
  let buffer = "";
  let totalContent = "";
  let inputTokens = 0;
  let outputTokens = 0;

  try {
    while (true) {
      if (signal?.aborted) break;
      const { done, value } = await reader.read();
      if (done) break;

      buffer += decoder.decode(value, { stream: true });
      const lines = buffer.split("\n");
      buffer = lines.pop() ?? "";

      for (const line of lines) {
        if (!line.startsWith("data: ")) continue;
        const data = line.slice(6).trim();

        try {
          const parsed = JSON.parse(data);

          if (parsed.type === "content_block_delta") {
            const delta = parsed.delta?.text ?? "";
            if (delta) {
              totalContent += delta;
              yield { type: "delta", content: delta };
            }
          } else if (parsed.type === "message_start") {
            inputTokens = parsed.message?.usage?.input_tokens ?? 0;
          } else if (parsed.type === "message_delta") {
            outputTokens = parsed.usage?.output_tokens ?? 0;
          } else if (parsed.type === "message_stop") {
            yield { type: "done", content: totalContent, inputTokens, outputTokens };
            return;
          }
        } catch {
          // skip malformed chunks
        }
      }
    }
  } finally {
    reader.releaseLock();
  }

  yield { type: "done", content: totalContent, inputTokens, outputTokens };
}

/**
 * Convert AgentMessage history to the format needed for API calls.
 */
export function messagesToApiFormat(
  messages: AgentMessage[],
): ApiChatMessage[] {
  return messages
    .filter((m) => !m.isCompactionSummary)
    .filter((m) => {
      if (m.role === "user" || m.role === "assistant") {
        return true;
      }

      if (m.role === "tool") {
        return Boolean(m.toolCallId) && (m.toolStatus === "done" || m.toolStatus === "error");
      }

      return false;
    })
    .map((m) => {
      if (m.role === "tool") {
        return {
          role: "tool",
          content: m.content,
          tool_call_id: m.toolCallId,
        } satisfies ApiChatMessage;
      }

      return {
        role: m.role,
        content: m.content,
        tool_calls: m.role === "assistant" ? m.toolCalls : undefined,
      } satisfies ApiChatMessage;
    });
}
