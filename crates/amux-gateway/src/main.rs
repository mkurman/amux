//! tamux-gateway: Chat platform gateway service.
//!
//! Bridges external messaging platforms (Slack, Telegram, Discord) to the tamux
//! daemon. Incoming chat messages that match a command prefix are translated
//! into `ClientMessage::ExecuteManagedCommand` requests, and daemon responses
//! are forwarded back through the originating chat provider.
//!
//! The gateway runs as a standalone daemon process alongside `tamux-daemon`.

mod discord;
mod router;
mod slack;
mod telegram;

use anyhow::{Context, Result};
use amux_protocol::{ClientMessage, AmuxCodec, DaemonMessage, SessionId};
use futures::{SinkExt, StreamExt};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{mpsc, Mutex};
use tokio_util::codec::Framed;

use crate::router::{GatewayMessage, GatewayResponse};

// ---------------------------------------------------------------------------
// GatewayProvider trait
// ---------------------------------------------------------------------------

/// Trait that each chat platform must implement to participate in the gateway.
///
/// Providers are responsible for:
/// 1. Establishing a connection to their platform's API
/// 2. Polling or streaming incoming messages
/// 3. Sending responses back to the originating channel
///
/// Methods return boxed futures to avoid requiring the `async_trait` crate.
/// Each provider implementation manually boxes its futures.
pub trait GatewayProvider: Send + 'static {
    /// Human-readable name of the provider (e.g. "Slack", "Telegram").
    fn name(&self) -> &str;

    /// Connect to the platform. Called once at startup.
    /// Returns an error if the connection cannot be established.
    fn connect(
        &mut self,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<()>> + Send + '_>>;

    /// Poll for the next incoming message. Returns `None` when the provider
    /// has no more messages available (i.e. the poll interval hasn't elapsed
    /// or the connection was closed).
    fn recv(
        &mut self,
    ) -> std::pin::Pin<
        Box<dyn std::future::Future<Output = Result<Option<GatewayMessage>>> + Send + '_>,
    >;

    /// Send a response back to the chat platform.
    fn send(
        &mut self,
        response: GatewayResponse,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<()>> + Send + '_>>;
}

// ---------------------------------------------------------------------------
// Daemon IPC connection
// ---------------------------------------------------------------------------

/// Connect to the tamux daemon and return a framed stream using `AmuxCodec`.
async fn connect_to_daemon(
) -> Result<Framed<impl tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin, AmuxCodec>> {
    #[cfg(unix)]
    {
        let runtime_dir =
            std::env::var("XDG_RUNTIME_DIR").unwrap_or_else(|_| "/tmp".to_string());
        let path = std::path::PathBuf::from(runtime_dir).join("tamux-daemon.sock");
        let stream = tokio::net::UnixStream::connect(&path)
            .await
            .with_context(|| format!("cannot connect to daemon at {}", path.display()))?;
        Ok(Framed::new(stream, AmuxCodec))
    }

    #[cfg(windows)]
    {
        let addr = amux_protocol::default_tcp_addr();
        let stream = tokio::net::TcpStream::connect(&addr)
            .await
            .with_context(|| format!("cannot connect to daemon on {addr}"))?;
        Ok(Framed::new(stream, AmuxCodec))
    }
}

/// Spawn a session via the daemon and return the session ID.
async fn spawn_daemon_session(
    framed: &mut Framed<impl tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin, AmuxCodec>,
) -> Result<SessionId> {
    framed
        .send(ClientMessage::SpawnSession {
            shell: None,
            cwd: None,
            env: None,
            workspace_id: Some("gateway".to_string()),
            cols: 120,
            rows: 40,
        })
        .await
        .context("failed to request session spawn")?;

    match framed.next().await {
        Some(Ok(DaemonMessage::SessionSpawned { id })) => {
            tracing::info!(session_id = %id, "daemon session spawned for gateway");
            Ok(id)
        }
        Some(Ok(DaemonMessage::Error { message })) => {
            anyhow::bail!("daemon error while spawning session: {message}")
        }
        Some(Ok(other)) => anyhow::bail!("unexpected daemon response: {other:?}"),
        Some(Err(e)) => Err(e.into()),
        None => anyhow::bail!("daemon closed connection while spawning session"),
    }
}

// ---------------------------------------------------------------------------
// Provider registry & event loop
// ---------------------------------------------------------------------------

/// Holds the set of active providers and coordinates message flow between
/// chat platforms and the daemon.
struct Gateway {
    /// Active providers keyed by name.
    providers: Vec<Box<dyn GatewayProvider>>,
}

impl Gateway {
    fn new() -> Self {
        Self {
            providers: Vec::new(),
        }
    }

    fn register(&mut self, provider: Box<dyn GatewayProvider>) {
        tracing::info!(provider = provider.name(), "registered gateway provider");
        self.providers.push(provider);
    }

    /// Run the main event loop.
    ///
    /// 1. Connect to the daemon and spawn a shared session.
    /// 2. Connect all registered providers.
    /// 3. Poll providers for incoming messages; translate them to daemon
    ///    commands via the router.
    /// 4. Forward daemon responses back through providers.
    async fn run(mut self) -> Result<()> {
        if self.providers.is_empty() {
            tracing::warn!(
                "no chat providers configured — gateway has nothing to do. \
                 Set AMUX_SLACK_TOKEN, AMUX_TELEGRAM_TOKEN, or AMUX_DISCORD_TOKEN \
                 to enable a provider."
            );
            // Keep running so the process doesn't exit immediately; wait for
            // signal so operators can see the logs.
            tracing::info!("gateway idle — waiting for shutdown signal (Ctrl+C)");
            tokio::signal::ctrl_c().await?;
            return Ok(());
        }

        // --- Connect to daemon ---
        tracing::info!("connecting to tamux daemon…");
        let mut framed = connect_to_daemon().await?;
        let session_id = spawn_daemon_session(&mut framed).await?;

        // Attach to the session so we receive output.
        framed
            .send(ClientMessage::AttachSession { id: session_id })
            .await?;
        match framed.next().await {
            Some(Ok(DaemonMessage::SessionAttached { .. })) => {
                tracing::info!(session_id = %session_id, "attached to daemon session");
            }
            Some(Ok(DaemonMessage::Error { message })) => {
                anyhow::bail!("daemon error attaching: {message}");
            }
            other => {
                anyhow::bail!("unexpected response while attaching: {other:?}");
            }
        }

        // --- Connect all providers ---
        for provider in &mut self.providers {
            match provider.connect().await {
                Ok(()) => tracing::info!(provider = provider.name(), "provider connected"),
                Err(e) => {
                    tracing::error!(
                        provider = provider.name(),
                        error = %e,
                        "failed to connect provider — skipping"
                    );
                }
            }
        }

        // Split the framed connection for concurrent read/write.
        let (daemon_tx, mut daemon_rx) = framed.split();
        let daemon_tx = Arc::new(Mutex::new(daemon_tx));

        // Channel for outbound responses that need to be sent back to chat
        // platforms.  Maps channel_id -> response text.
        let (response_tx, mut response_rx) =
            mpsc::unbounded_channel::<(String, GatewayResponse)>();

        // Map to track which channel_id belongs to which provider index.
        let pending_channels: Arc<Mutex<HashMap<String, usize>>> =
            Arc::new(Mutex::new(HashMap::new()));

        // --- Provider polling tasks ---
        // Each provider runs in its own task, sending received messages into a
        // shared channel.
        let (incoming_tx, mut incoming_rx) =
            mpsc::unbounded_channel::<(usize, GatewayMessage)>();

        let mut provider_tasks = Vec::new();
        // Move providers into individual tasks.
        let providers_vec: Vec<Box<dyn GatewayProvider>> =
            self.providers.into_iter().collect();

        for (idx, mut provider) in providers_vec.into_iter().enumerate() {
            let tx = incoming_tx.clone();
            let handle = tokio::spawn(async move {
                loop {
                    match provider.recv().await {
                        Ok(Some(msg)) => {
                            if tx.send((idx, msg)).is_err() {
                                break;
                            }
                        }
                        Ok(None) => {
                            // No message available; small backoff.
                            tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
                        }
                        Err(e) => {
                            tracing::error!(
                                provider = provider.name(),
                                error = %e,
                                "provider recv error"
                            );
                            tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;
                        }
                    }
                }
            });
            provider_tasks.push(handle);
        }
        // Drop the original sender so the channel can close when tasks end.
        drop(incoming_tx);

        // --- Main select loop ---
        let shutdown = tokio::signal::ctrl_c();
        tokio::pin!(shutdown);

        tracing::info!("gateway event loop running");

        loop {
            tokio::select! {
                // Incoming message from a chat provider.
                Some((provider_idx, msg)) = incoming_rx.recv() => {
                    tracing::info!(
                        provider_idx,
                        platform = %msg.platform,
                        user = %msg.user_id,
                        channel = %msg.channel_id,
                        text = %msg.text,
                        "received chat message"
                    );

                    // Track which provider owns this channel for response routing.
                    {
                        let mut map = pending_channels.lock().await;
                        map.insert(msg.channel_id.clone(), provider_idx);
                    }

                    // Route through the message router.
                    if let Some(request) = router::route_to_command(&msg) {
                        tracing::info!(
                            command = %request.command,
                            "routing message as managed command"
                        );
                        let mut tx = daemon_tx.lock().await;
                        if let Err(e) = tx
                            .send(ClientMessage::ExecuteManagedCommand {
                                id: session_id,
                                request,
                            })
                            .await
                        {
                            tracing::error!(error = %e, "failed to send command to daemon");
                        }
                    } else {
                        tracing::debug!(
                            "message did not match a command prefix — ignoring"
                        );
                    }
                }

                // Daemon response.
                Some(daemon_msg) = daemon_rx.next() => {
                    match daemon_msg {
                        Ok(DaemonMessage::ManagedCommandFinished {
                            execution_id,
                            command,
                            exit_code,
                            duration_ms,
                            ..
                        }) => {
                            let summary = format!(
                                "Command `{}` finished (exit {}){}\n[execution: {}]",
                                command,
                                exit_code.map_or("?".into(), |c| c.to_string()),
                                duration_ms
                                    .map(|ms| format!(", took {ms}ms"))
                                    .unwrap_or_default(),
                                execution_id,
                            );
                            tracing::info!(%summary, "managed command finished");

                            // Broadcast to all pending channels (simple strategy).
                            let map = pending_channels.lock().await;
                            for (channel_id, _) in map.iter() {
                                let _ = response_tx.send((
                                    channel_id.clone(),
                                    GatewayResponse {
                                        text: summary.clone(),
                                        channel_id: channel_id.clone(),
                                    },
                                ));
                            }
                        }
                        Ok(DaemonMessage::ManagedCommandStarted {
                            execution_id,
                            command,
                            ..
                        }) => {
                            tracing::info!(
                                execution_id,
                                command,
                                "managed command started on daemon"
                            );
                        }
                        Ok(DaemonMessage::ManagedCommandRejected { message, .. }) => {
                            tracing::warn!(message, "managed command rejected");
                            let map = pending_channels.lock().await;
                            for (channel_id, _) in map.iter() {
                                let _ = response_tx.send((
                                    channel_id.clone(),
                                    GatewayResponse {
                                        text: format!("Command rejected: {message}"),
                                        channel_id: channel_id.clone(),
                                    },
                                ));
                            }
                        }
                        Ok(DaemonMessage::Output { data, .. }) => {
                            if let Ok(text) = String::from_utf8(data) {
                                tracing::trace!(len = text.len(), "daemon output chunk");
                                // Output is streamed; we could buffer and send
                                // periodically. For now, just log.
                            }
                        }
                        Ok(DaemonMessage::Error { message }) => {
                            tracing::error!(message, "daemon error");
                        }
                        Ok(DaemonMessage::SessionExited { exit_code, .. }) => {
                            tracing::warn!(
                                ?exit_code,
                                "daemon session exited — gateway shutting down"
                            );
                            break;
                        }
                        Ok(other) => {
                            tracing::debug!(?other, "unhandled daemon message");
                        }
                        Err(e) => {
                            tracing::error!(error = %e, "daemon connection error");
                            break;
                        }
                    }
                }

                // Response delivery (placeholder — would call provider.send()).
                Some((_channel_id, response)) = response_rx.recv() => {
                    tracing::info!(
                        channel = %response.channel_id,
                        text = %response.text,
                        "would deliver response to chat platform"
                    );
                    // In a full implementation, we'd look up the provider by
                    // channel_id and call provider.send(response).await.
                }

                // Graceful shutdown.
                _ = &mut shutdown => {
                    tracing::info!("received shutdown signal");
                    break;
                }
            }
        }

        // Cleanup: abort provider tasks.
        for handle in provider_tasks {
            handle.abort();
        }

        // Kill the daemon session.
        {
            let mut tx = daemon_tx.lock().await;
            let _ = tx
                .send(ClientMessage::KillSession { id: session_id })
                .await;
        }

        tracing::info!("amux-gateway shut down");
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Entrypoint
// ---------------------------------------------------------------------------

#[tokio::main]
async fn main() -> Result<()> {
    // Initialise tracing with file-based log output alongside stderr.
    let log_path = amux_protocol::log_file_path("tamux-gateway.log");
    let log_dir = log_path.parent().unwrap_or_else(|| std::path::Path::new("."));
    let file_appender = tracing_appender::rolling::daily(log_dir, "tamux-gateway.log");
    let (non_blocking, _guard) = tracing_appender::non_blocking(file_appender);

    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_env("AMUX_GATEWAY_LOG")
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .with_writer(non_blocking)
        .with_ansi(false)
        .init();

    tracing::info!("tamux-gateway starting");
    tracing::info!(
        "supported platforms: Slack (AMUX_SLACK_TOKEN), \
         Telegram (AMUX_TELEGRAM_TOKEN), Discord (AMUX_DISCORD_TOKEN)"
    );

    let mut gateway = Gateway::new();

    // --- Register providers from environment ---
    match slack::SlackProvider::from_env() {
        Some(p) => {
            tracing::info!("Slack provider enabled");
            gateway.register(Box::new(p));
        }
        None => tracing::info!("Slack provider disabled (AMUX_SLACK_TOKEN not set)"),
    }

    match telegram::TelegramProvider::from_env() {
        Some(p) => {
            tracing::info!("Telegram provider enabled");
            gateway.register(Box::new(p));
        }
        None => tracing::info!("Telegram provider disabled (AMUX_TELEGRAM_TOKEN not set)"),
    }

    match discord::DiscordProvider::from_env() {
        Some(p) => {
            tracing::info!("Discord provider enabled");
            gateway.register(Box::new(p));
        }
        None => tracing::info!("Discord provider disabled (AMUX_DISCORD_TOKEN not set)"),
    }

    gateway.run().await
}
