//! LLM API client with SSE streaming support.
//!
//! Supports two API formats:
//! - OpenAI-compatible (`/chat/completions` with Bearer auth) — covers most providers
//! - Anthropic Messages API (`/v1/messages` with `x-api-key` header)

use anyhow::{Context, Result};
use futures::Stream;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fmt;
use std::pin::Pin;
use std::task::Poll;
use std::time::Duration;
use tokio::sync::mpsc;

use super::types::{CompletionChunk, ProviderConfig, ToolCall, ToolDefinition, ToolFunction};

// ---------------------------------------------------------------------------
// API message types
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize)]
pub struct ApiMessage {
    pub role: String,
    pub content: ApiContent,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_call_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_calls: Option<Vec<ApiToolCall>>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(untagged)]
pub enum ApiContent {
    Text(String),
    Blocks(Vec<serde_json::Value>),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiToolCall {
    pub id: String,
    #[serde(rename = "type")]
    pub call_type: String,
    pub function: ApiToolCallFunction,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiToolCallFunction {
    pub name: String,
    pub arguments: String,
}

// ---------------------------------------------------------------------------
// Completion stream wrapper
// ---------------------------------------------------------------------------

pub struct CompletionStream {
    rx: mpsc::Receiver<Result<CompletionChunk>>,
}

#[derive(Debug)]
struct RateLimitError {
    provider: String,
    details: String,
}

impl fmt::Display for RateLimitError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} API returned 429: {}", self.provider, self.details)
    }
}

impl std::error::Error for RateLimitError {}

impl Stream for CompletionStream {
    type Item = Result<CompletionChunk>;

    fn poll_next(
        mut self: Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> Poll<Option<Self::Item>> {
        self.rx.poll_recv(cx)
    }
}

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// Send a chat completion request. Returns a stream of `CompletionChunk`.
///
/// The `provider` string determines the API format:
/// - `"anthropic"` uses the Anthropic Messages API
/// - Everything else uses OpenAI-compatible format
pub fn send_chat_completion(
    client: &reqwest::Client,
    provider: &str,
    config: &ProviderConfig,
    system_prompt: &str,
    messages: &[ApiMessage],
    tools: &[ToolDefinition],
    max_retries: u32,
    retry_delay_ms: u64,
) -> CompletionStream {
    let (tx, rx) = mpsc::channel(64);
    let client = client.clone();
    let provider = provider.to_string();
    let config = config.clone();
    let system_prompt = system_prompt.to_string();
    let messages = messages.to_vec();
    let tools = tools.to_vec();

    tokio::spawn(async move {
        let mut retry_attempt = 0u32;
        loop {
            let result = if provider == "anthropic" {
                run_anthropic(&client, &config, &system_prompt, &messages, &tools, &tx).await
            } else {
                run_openai_compatible(
                    &client,
                    &provider,
                    &config,
                    &system_prompt,
                    &messages,
                    &tools,
                    &tx,
                )
                .await
            };

            match result {
                Ok(()) => break,
                Err(e) => {
                    let is_rate_limited = e.downcast_ref::<RateLimitError>().is_some();
                    if is_rate_limited && retry_attempt < max_retries {
                        retry_attempt += 1;
                        let _ = tx
                            .send(Ok(CompletionChunk::Retry {
                                attempt: retry_attempt,
                                max_retries,
                                delay_ms: retry_delay_ms,
                            }))
                            .await;
                        tokio::time::sleep(Duration::from_millis(retry_delay_ms)).await;
                        continue;
                    }

                    let message = if let Some(rate_limit) = e.downcast_ref::<RateLimitError>() {
                        rate_limit.to_string()
                    } else {
                        format!("API error: {e}")
                    };
                    let _ = tx.send(Ok(CompletionChunk::Error { message })).await;
                    break;
                }
            }
        }
    });

    CompletionStream { rx }
}

/// Convert `AgentMessage` history to API format.
pub fn messages_to_api_format(messages: &[super::types::AgentMessage]) -> Vec<ApiMessage> {
    messages
        .iter()
        .filter(|m| {
            matches!(
                m.role,
                super::types::MessageRole::User
                    | super::types::MessageRole::Assistant
                    | super::types::MessageRole::Tool
            )
        })
        .map(|m| {
            let tool_calls = m.tool_calls.as_ref().map(|tcs| {
                tcs.iter()
                    .map(|tc| ApiToolCall {
                        id: tc.id.clone(),
                        call_type: "function".into(),
                        function: ApiToolCallFunction {
                            name: tc.function.name.clone(),
                            arguments: tc.function.arguments.clone(),
                        },
                    })
                    .collect()
            });

            ApiMessage {
                role: match m.role {
                    super::types::MessageRole::System => "system".into(),
                    super::types::MessageRole::User => "user".into(),
                    super::types::MessageRole::Assistant => "assistant".into(),
                    super::types::MessageRole::Tool => "tool".into(),
                },
                content: ApiContent::Text(m.content.clone()),
                tool_call_id: m.tool_call_id.clone(),
                name: m.tool_name.clone(),
                tool_calls,
            }
        })
        .collect()
}

// ---------------------------------------------------------------------------
// OpenAI-compatible implementation
// ---------------------------------------------------------------------------

fn build_chat_completion_url(provider: &str, base_url: &str) -> String {
    let base = base_url.trim_end_matches('/');
    let lower = base.to_lowercase();

    if provider == "openrouter" || provider == "groq" {
        return format!("{base}/chat/completions");
    }

    if lower.ends_with("/v1") || lower.ends_with("/api/v1") {
        return format!("{base}/chat/completions");
    }

    format!("{base}/v1/chat/completions")
}

async fn run_openai_compatible(
    client: &reqwest::Client,
    provider: &str,
    config: &ProviderConfig,
    system_prompt: &str,
    messages: &[ApiMessage],
    tools: &[ToolDefinition],
    tx: &mpsc::Sender<Result<CompletionChunk>>,
) -> Result<()> {
    let url = build_chat_completion_url(provider, &config.base_url);

    let mut all_messages = vec![ApiMessage {
        role: "system".into(),
        content: ApiContent::Text(system_prompt.into()),
        tool_call_id: None,
        name: None,
        tool_calls: None,
    }];
    all_messages.extend_from_slice(messages);

    let mut body = serde_json::json!({
        "model": config.model,
        "messages": all_messages,
        "stream": true,
        "stream_options": { "include_usage": true },
    });

    if !tools.is_empty() {
        body["tools"] = serde_json::to_value(tools)?;
        body["tool_choice"] = serde_json::json!("auto");
    }

    let mut req = client.post(&url).header("Content-Type", "application/json");

    if !config.api_key.is_empty() {
        req = req.header("Authorization", format!("Bearer {}", config.api_key));
    }

    let response = req.body(body.to_string()).send().await?;

    if !response.status().is_success() {
        let status = response.status();
        let text = response
            .text()
            .await
            .unwrap_or_default()
            .chars()
            .take(200)
            .collect::<String>();
        if status == reqwest::StatusCode::TOO_MANY_REQUESTS {
            return Err(RateLimitError {
                provider: provider.to_string(),
                details: text,
            }
            .into());
        }
        anyhow::bail!("{provider} API returned {status}: {text}");
    }

    parse_openai_sse(response, tx).await
}

async fn parse_openai_sse(
    response: reqwest::Response,
    tx: &mpsc::Sender<Result<CompletionChunk>>,
) -> Result<()> {
    use futures::StreamExt;

    let mut stream = response.bytes_stream();
    let mut buffer = String::new();
    let mut total_content = String::new();
    let mut total_reasoning = String::new();
    let mut pending_tool_calls: HashMap<u32, PendingToolCall> = HashMap::new();
    let mut input_tokens: u64 = 0;
    let mut output_tokens: u64 = 0;

    while let Some(chunk_result) = stream.next().await {
        let chunk = chunk_result.context("failed to read SSE chunk")?;
        buffer.push_str(&String::from_utf8_lossy(&chunk));

        let mut remaining = String::new();
        for line in buffer.split('\n') {
            if !line.starts_with("data: ") {
                // Keep incomplete lines in the buffer
                if !line.is_empty() && !line.starts_with(':') && !line.starts_with("event:") {
                    remaining.push_str(line);
                    remaining.push('\n');
                }
                continue;
            }

            let data = line[6..].trim();
            if data == "[DONE]" {
                // Emit final chunk
                if !pending_tool_calls.is_empty() {
                    let tool_calls = drain_tool_calls(&mut pending_tool_calls);
                    let _ = tx
                        .send(Ok(CompletionChunk::ToolCalls {
                            tool_calls,
                            content: if total_content.is_empty() {
                                None
                            } else {
                                Some(total_content.clone())
                            },
                            reasoning: if total_reasoning.is_empty() {
                                None
                            } else {
                                Some(total_reasoning.clone())
                            },
                            input_tokens: Some(input_tokens),
                            output_tokens: Some(output_tokens),
                        }))
                        .await;
                } else {
                    let _ = tx
                        .send(Ok(CompletionChunk::Done {
                            content: total_content.clone(),
                            reasoning: if total_reasoning.is_empty() {
                                None
                            } else {
                                Some(total_reasoning.clone())
                            },
                            input_tokens,
                            output_tokens,
                        }))
                        .await;
                }
                return Ok(());
            }

            let parsed: serde_json::Value = match serde_json::from_str(data) {
                Ok(v) => v,
                Err(_) => continue,
            };

            // Extract usage
            if let Some(usage) = parsed.get("usage") {
                input_tokens = usage
                    .get("prompt_tokens")
                    .and_then(|v| v.as_u64())
                    .unwrap_or(input_tokens);
                output_tokens = usage
                    .get("completion_tokens")
                    .and_then(|v| v.as_u64())
                    .unwrap_or(output_tokens);
            }

            let delta = match parsed.pointer("/choices/0/delta") {
                Some(d) => d,
                None => continue,
            };

            // Content delta
            if let Some(content) = delta.get("content").and_then(|v| v.as_str()) {
                total_content.push_str(content);
                let _ = tx
                    .send(Ok(CompletionChunk::Delta {
                        content: content.into(),
                        reasoning: None,
                    }))
                    .await;
            }

            // Reasoning delta
            if let Some(reasoning) = delta.get("reasoning").and_then(|v| v.as_str()) {
                total_reasoning.push_str(reasoning);
                let _ = tx
                    .send(Ok(CompletionChunk::Delta {
                        content: String::new(),
                        reasoning: Some(reasoning.into()),
                    }))
                    .await;
            }

            // Tool call deltas (streamed incrementally)
            if let Some(tcs) = delta.get("tool_calls").and_then(|v| v.as_array()) {
                for tc in tcs {
                    let idx = tc.get("index").and_then(|v| v.as_u64()).unwrap_or(0) as u32;
                    let entry = pending_tool_calls
                        .entry(idx)
                        .or_insert_with(|| PendingToolCall {
                            id: String::new(),
                            name: String::new(),
                            arguments: String::new(),
                        });

                    if let Some(id) = tc.get("id").and_then(|v| v.as_str()) {
                        entry.id = id.into();
                    }
                    if let Some(name) = tc.pointer("/function/name").and_then(|v| v.as_str()) {
                        entry.name.push_str(name);
                    }
                    if let Some(args) = tc.pointer("/function/arguments").and_then(|v| v.as_str()) {
                        entry.arguments.push_str(args);
                    }
                }
            }
        }
        buffer = remaining;
    }

    // Stream ended without [DONE]
    if !pending_tool_calls.is_empty() {
        let tool_calls = drain_tool_calls(&mut pending_tool_calls);
        let _ = tx
            .send(Ok(CompletionChunk::ToolCalls {
                tool_calls,
                content: if total_content.is_empty() {
                    None
                } else {
                    Some(total_content)
                },
                reasoning: if total_reasoning.is_empty() {
                    None
                } else {
                    Some(total_reasoning)
                },
                input_tokens: Some(input_tokens),
                output_tokens: Some(output_tokens),
            }))
            .await;
    } else {
        let _ = tx
            .send(Ok(CompletionChunk::Done {
                content: total_content,
                reasoning: if total_reasoning.is_empty() {
                    None
                } else {
                    Some(total_reasoning)
                },
                input_tokens,
                output_tokens,
            }))
            .await;
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// Anthropic Messages API implementation
// ---------------------------------------------------------------------------

async fn run_anthropic(
    client: &reqwest::Client,
    config: &ProviderConfig,
    system_prompt: &str,
    messages: &[ApiMessage],
    tools: &[ToolDefinition],
    tx: &mpsc::Sender<Result<CompletionChunk>>,
) -> Result<()> {
    let url = format!("{}/v1/messages", config.base_url.trim_end_matches('/'));

    // Convert messages to Anthropic format
    let anthropic_messages: Vec<serde_json::Value> = messages
        .iter()
        .map(|m| {
            let role = match m.role.as_str() {
                "tool" | "system" => "user",
                r => r,
            };

            let content = if m.role == "tool" {
                serde_json::json!([{
                    "type": "tool_result",
                    "tool_use_id": m.tool_call_id,
                    "content": match &m.content {
                        ApiContent::Text(t) => t.as_str(),
                        _ => "",
                    }
                }])
            } else {
                match &m.content {
                    ApiContent::Text(t) => serde_json::json!(t),
                    ApiContent::Blocks(b) => serde_json::json!(b),
                }
            };

            serde_json::json!({
                "role": role,
                "content": content,
            })
        })
        .collect();

    let mut body = serde_json::json!({
        "model": config.model,
        "max_tokens": 4096,
        "system": system_prompt,
        "messages": anthropic_messages,
        "stream": true,
    });

    if !tools.is_empty() {
        let anthropic_tools: Vec<serde_json::Value> = tools
            .iter()
            .map(|t| {
                serde_json::json!({
                    "name": t.function.name,
                    "description": t.function.description,
                    "input_schema": t.function.parameters,
                })
            })
            .collect();
        body["tools"] = serde_json::json!(anthropic_tools);
    }

    let response = client
        .post(&url)
        .header("Content-Type", "application/json")
        .header("x-api-key", &config.api_key)
        .header("anthropic-version", "2023-06-01")
        .body(body.to_string())
        .send()
        .await?;

    if !response.status().is_success() {
        let status = response.status();
        let text = response
            .text()
            .await
            .unwrap_or_default()
            .chars()
            .take(200)
            .collect::<String>();
        if status == reqwest::StatusCode::TOO_MANY_REQUESTS {
            return Err(RateLimitError {
                provider: "Anthropic".to_string(),
                details: text,
            }
            .into());
        }
        anyhow::bail!("Anthropic API returned {status}: {text}");
    }

    parse_anthropic_sse(response, tx).await
}

async fn parse_anthropic_sse(
    response: reqwest::Response,
    tx: &mpsc::Sender<Result<CompletionChunk>>,
) -> Result<()> {
    use futures::StreamExt;

    let mut stream = response.bytes_stream();
    let mut buffer = String::new();
    let mut total_content = String::new();
    let mut input_tokens: u64 = 0;
    let mut output_tokens: u64 = 0;

    // Anthropic tool use tracking
    let mut pending_tool_calls: Vec<PendingToolCall> = Vec::new();
    let mut current_tool_input = String::new();

    while let Some(chunk_result) = stream.next().await {
        let chunk = chunk_result.context("failed to read Anthropic SSE chunk")?;
        buffer.push_str(&String::from_utf8_lossy(&chunk));

        let mut remaining = String::new();
        for line in buffer.split('\n') {
            if !line.starts_with("data: ") {
                if !line.is_empty() && !line.starts_with(':') && !line.starts_with("event:") {
                    remaining.push_str(line);
                    remaining.push('\n');
                }
                continue;
            }

            let data = line[6..].trim();
            let parsed: serde_json::Value = match serde_json::from_str(data) {
                Ok(v) => v,
                Err(_) => continue,
            };

            let event_type = parsed.get("type").and_then(|v| v.as_str()).unwrap_or("");

            match event_type {
                "message_start" => {
                    input_tokens = parsed
                        .pointer("/message/usage/input_tokens")
                        .and_then(|v| v.as_u64())
                        .unwrap_or(0);
                }
                "content_block_start" => {
                    // Check if this is a tool_use block
                    if let Some(cb) = parsed.get("content_block") {
                        if cb.get("type").and_then(|v| v.as_str()) == Some("tool_use") {
                            let id = cb
                                .get("id")
                                .and_then(|v| v.as_str())
                                .unwrap_or("")
                                .to_string();
                            let name = cb
                                .get("name")
                                .and_then(|v| v.as_str())
                                .unwrap_or("")
                                .to_string();
                            pending_tool_calls.push(PendingToolCall {
                                id,
                                name,
                                arguments: String::new(),
                            });
                            current_tool_input.clear();
                        }
                    }
                }
                "content_block_delta" => {
                    let delta_type = parsed
                        .pointer("/delta/type")
                        .and_then(|v| v.as_str())
                        .unwrap_or("");

                    if delta_type == "text_delta" {
                        let text = parsed
                            .pointer("/delta/text")
                            .and_then(|v| v.as_str())
                            .unwrap_or("");
                        if !text.is_empty() {
                            total_content.push_str(text);
                            let _ = tx
                                .send(Ok(CompletionChunk::Delta {
                                    content: text.into(),
                                    reasoning: None,
                                }))
                                .await;
                        }
                    } else if delta_type == "input_json_delta" {
                        let partial = parsed
                            .pointer("/delta/partial_json")
                            .and_then(|v| v.as_str())
                            .unwrap_or("");
                        current_tool_input.push_str(partial);
                    }
                }
                "content_block_stop" => {
                    // Finalize tool call arguments if we're in a tool block
                    if let Some(tc) = pending_tool_calls.last_mut() {
                        if tc.arguments.is_empty() && !current_tool_input.is_empty() {
                            tc.arguments = current_tool_input.clone();
                            current_tool_input.clear();
                        }
                    }
                }
                "message_delta" => {
                    output_tokens = parsed
                        .pointer("/usage/output_tokens")
                        .and_then(|v| v.as_u64())
                        .unwrap_or(output_tokens);
                }
                "message_stop" => {
                    if !pending_tool_calls.is_empty() {
                        let tool_calls: Vec<ToolCall> = pending_tool_calls
                            .drain(..)
                            .map(|tc| ToolCall {
                                id: tc.id,
                                function: ToolFunction {
                                    name: tc.name,
                                    arguments: tc.arguments,
                                },
                            })
                            .collect();
                        let _ = tx
                            .send(Ok(CompletionChunk::ToolCalls {
                                tool_calls,
                                content: if total_content.is_empty() {
                                    None
                                } else {
                                    Some(total_content.clone())
                                },
                                reasoning: None,
                                input_tokens: Some(input_tokens),
                                output_tokens: Some(output_tokens),
                            }))
                            .await;
                    } else {
                        let _ = tx
                            .send(Ok(CompletionChunk::Done {
                                content: total_content.clone(),
                                reasoning: None,
                                input_tokens,
                                output_tokens,
                            }))
                            .await;
                    }
                    return Ok(());
                }
                _ => {}
            }
        }
        buffer = remaining;
    }

    // Stream ended without message_stop
    let _ = tx
        .send(Ok(CompletionChunk::Done {
            content: total_content,
            reasoning: None,
            input_tokens,
            output_tokens,
        }))
        .await;

    Ok(())
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

struct PendingToolCall {
    id: String,
    name: String,
    arguments: String,
}

fn drain_tool_calls(map: &mut HashMap<u32, PendingToolCall>) -> Vec<ToolCall> {
    let mut entries: Vec<(u32, PendingToolCall)> = map.drain().collect();
    entries.sort_by_key(|(idx, _)| *idx);
    entries
        .into_iter()
        .map(|(_, tc)| ToolCall {
            id: tc.id,
            function: ToolFunction {
                name: tc.name,
                arguments: tc.arguments,
            },
        })
        .collect()
}
