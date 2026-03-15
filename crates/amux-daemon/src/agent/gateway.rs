//! Gateway: direct platform connections for receiving messages.
//!
//! Runs inside the daemon. No separate process, no Electron dependency.
//! - Telegram: long-poll getUpdates API
//! - Slack: poll conversations.history API
//! - Discord: poll channel messages REST API
//!
//! Incoming messages are routed to the agent engine for processing.

use std::collections::HashMap;
use std::sync::Arc;

use anyhow::Result;
use tokio::sync::RwLock;

use super::types::GatewayConfig;

/// State for tracking already-seen messages per platform.
pub struct GatewayState {
    pub config: GatewayConfig,
    pub telegram_offset: i64,
    pub slack_last_ts: HashMap<String, String>,
    pub discord_last_id: HashMap<String, String>,
    pub http_client: reqwest::Client,
}

impl GatewayState {
    pub fn new(config: GatewayConfig, http_client: reqwest::Client) -> Self {
        Self {
            config,
            telegram_offset: 0,
            slack_last_ts: HashMap::new(),
            discord_last_id: HashMap::new(),
            http_client,
        }
    }
}

/// Incoming message from any platform.
#[derive(Debug, Clone)]
pub struct IncomingMessage {
    pub platform: String,
    pub sender: String,
    pub content: String,
    pub channel: String,
}

// ---------------------------------------------------------------------------
// Telegram: long-poll getUpdates
// ---------------------------------------------------------------------------

pub async fn poll_telegram(state: &mut GatewayState) -> Vec<IncomingMessage> {
    let token = &state.config.telegram_token;
    if token.is_empty() {
        return vec![];
    }

    let url = format!(
        "https://api.telegram.org/bot{token}/getUpdates?offset={}&timeout=1&limit=10",
        state.telegram_offset
    );

    let resp = match state.http_client.get(&url).timeout(std::time::Duration::from_secs(5)).send().await {
        Ok(r) => r,
        Err(e) => {
            tracing::debug!("telegram poll error: {e}");
            return vec![];
        }
    };

    let body: serde_json::Value = match resp.json().await {
        Ok(v) => v,
        Err(_) => return vec![],
    };

    let mut messages = Vec::new();
    let is_first_poll = state.telegram_offset == 0;

    if let Some(results) = body.get("result").and_then(|v| v.as_array()) {
        for update in results {
            let update_id = update.get("update_id").and_then(|v| v.as_i64()).unwrap_or(0);
            if update_id >= state.telegram_offset {
                state.telegram_offset = update_id + 1;
            }

            // Skip historical messages on first poll
            if is_first_poll {
                continue;
            }

            let msg = update.get("message").or_else(|| update.get("channel_post"));
            if let Some(msg) = msg {
                let text = msg.get("text").and_then(|v| v.as_str()).unwrap_or("");
                if text.is_empty() {
                    continue;
                }

                let from = msg.get("from");
                let sender = from
                    .and_then(|f| f.get("username").and_then(|v| v.as_str()))
                    .or_else(|| from.and_then(|f| f.get("first_name").and_then(|v| v.as_str())))
                    .unwrap_or("unknown")
                    .to_string();

                let chat_id = msg
                    .get("chat")
                    .and_then(|c| c.get("id"))
                    .and_then(|v| v.as_i64())
                    .map(|id| id.to_string())
                    .unwrap_or_default();

                messages.push(IncomingMessage {
                    platform: "Telegram".into(),
                    sender,
                    content: text.into(),
                    channel: chat_id,
                });
            }
        }
    }

    messages
}

// ---------------------------------------------------------------------------
// Slack: poll conversations.history
// ---------------------------------------------------------------------------

pub async fn poll_slack(state: &mut GatewayState, channels: &[String]) -> Vec<IncomingMessage> {
    let token = &state.config.slack_token;
    if token.is_empty() {
        return vec![];
    }

    let mut messages = Vec::new();

    for channel in channels {
        let oldest = state
            .slack_last_ts
            .get(channel)
            .cloned()
            .unwrap_or_default();

        let mut url = format!(
            "https://slack.com/api/conversations.history?channel={channel}&limit=5"
        );
        if !oldest.is_empty() {
            url.push_str(&format!("&oldest={oldest}"));
        }

        let resp = match state
            .http_client
            .get(&url)
            .bearer_auth(token)
            .timeout(std::time::Duration::from_secs(5))
            .send()
            .await
        {
            Ok(r) => r,
            Err(e) => {
                tracing::debug!("slack poll error for {channel}: {e}");
                continue;
            }
        };

        let body: serde_json::Value = match resp.json().await {
            Ok(v) => v,
            Err(_) => continue,
        };

        if body.get("ok").and_then(|v| v.as_bool()) != Some(true) {
            continue;
        }

        if let Some(msgs) = body.get("messages").and_then(|v| v.as_array()) {
            for msg in msgs {
                let ts = msg.get("ts").and_then(|v| v.as_str()).unwrap_or("");
                let text = msg.get("text").and_then(|v| v.as_str()).unwrap_or("");
                let user = msg
                    .get("user")
                    .and_then(|v| v.as_str())
                    .unwrap_or("unknown");
                let subtype = msg.get("subtype").and_then(|v| v.as_str());

                // Skip bot messages and empty
                if text.is_empty() || subtype == Some("bot_message") {
                    continue;
                }

                // Skip if we've already seen this timestamp
                if !oldest.is_empty() && ts <= oldest.as_str() {
                    continue;
                }

                if !ts.is_empty() {
                    state
                        .slack_last_ts
                        .entry(channel.clone())
                        .and_modify(|v| {
                            if ts > v.as_str() {
                                *v = ts.to_string();
                            }
                        })
                        .or_insert_with(|| ts.to_string());
                }

                messages.push(IncomingMessage {
                    platform: "Slack".into(),
                    sender: user.into(),
                    content: text.into(),
                    channel: channel.clone(),
                });
            }
        }
    }

    messages
}

// ---------------------------------------------------------------------------
// Discord: poll channel messages REST API
// ---------------------------------------------------------------------------

pub async fn poll_discord(state: &mut GatewayState, channel_ids: &[String]) -> Vec<IncomingMessage> {
    let token = &state.config.discord_token;
    if token.is_empty() {
        return vec![];
    }

    let mut messages = Vec::new();

    for channel_id in channel_ids {
        let had_last_id = state.discord_last_id.contains_key(channel_id);

        let mut url = format!(
            "https://discord.com/api/v10/channels/{channel_id}/messages?limit=5"
        );

        if let Some(after) = state.discord_last_id.get(channel_id) {
            url.push_str(&format!("&after={after}"));
        }

        let resp = match state
            .http_client
            .get(&url)
            .header("Authorization", format!("Bot {token}"))
            .timeout(std::time::Duration::from_secs(5))
            .send()
            .await
        {
            Ok(r) => r,
            Err(e) => {
                tracing::debug!("discord poll error for {channel_id}: {e}");
                continue;
            }
        };

        if !resp.status().is_success() {
            continue;
        }

        let body: serde_json::Value = match resp.json().await {
            Ok(v) => v,
            Err(_) => continue,
        };

        let msgs = match body.as_array() {
            Some(a) => a,
            None => continue,
        };

        // Track the newest message ID (for ALL messages including bot) so we
        // never re-fetch. Discord returns newest first.
        let newest_id = msgs.first()
            .and_then(|m| m.get("id").and_then(|v| v.as_str()))
            .unwrap_or("");
        if !newest_id.is_empty() {
            state.discord_last_id.insert(channel_id.clone(), newest_id.to_string());
        }

        // On first poll (no prior last_id), skip historical messages —
        // only process messages from subsequent polls
        if !had_last_id {
            tracing::info!(
                channel = %channel_id,
                skipped = msgs.len(),
                newest_id = %newest_id,
                "discord: initialized, skipping historical messages"
            );
            continue;
        }

        // Process messages oldest first (reverse since Discord returns newest first)
        for msg in msgs.iter().rev() {
            let content = msg.get("content").and_then(|v| v.as_str()).unwrap_or("");
            let author = msg.get("author");
            let is_bot = author
                .and_then(|a| a.get("bot").and_then(|v| v.as_bool()))
                .unwrap_or(false);
            let username = author
                .and_then(|a| a.get("username").and_then(|v| v.as_str()))
                .unwrap_or("unknown");

            // Skip bot messages (our own replies) and empty
            if content.is_empty() || is_bot {
                continue;
            }

            messages.push(IncomingMessage {
                platform: "Discord".into(),
                sender: username.into(),
                content: content.into(),
                channel: channel_id.clone(),
            });
        }
    }

    messages
}
