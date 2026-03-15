use std::sync::Arc;

use amux_protocol::{ClientMessage, DaemonMessage};
use anyhow::Result;
use futures::SinkExt;
use futures::StreamExt;
use tokio::io::{AsyncRead, AsyncWrite};
use tokio::sync::broadcast;
use tokio_util::codec::Framed;

use crate::session_manager::SessionManager;

/// Socket path / pipe name for IPC.
#[cfg(unix)]
pub fn socket_path() -> std::path::PathBuf {
    let runtime_dir = std::env::var("XDG_RUNTIME_DIR").unwrap_or_else(|_| "/tmp".to_string());
    std::path::PathBuf::from(runtime_dir).join("tamux-daemon.sock")
}

/// Run the IPC server until a shutdown signal is received.
pub async fn run() -> Result<()> {
    let manager = SessionManager::new();
    let reaper_manager = manager.clone();

    tokio::spawn(async move {
        let mut interval = tokio::time::interval(std::time::Duration::from_secs(5));
        loop {
            interval.tick().await;
            reaper_manager.reap_dead().await;
        }
    });

    #[cfg(unix)]
    {
        run_unix(manager).await
    }

    #[cfg(windows)]
    {
        run_windows(manager).await
    }
}

// ---------------------------------------------------------------------------
// Unix Domain Socket implementation
// ---------------------------------------------------------------------------

#[cfg(unix)]
async fn run_unix(manager: Arc<SessionManager>) -> Result<()> {
    use tokio::net::UnixListener;

    let path = socket_path();

    // Remove stale socket file.
    let _ = std::fs::remove_file(&path);

    let listener = UnixListener::bind(&path)?;
    tracing::info!(?path, "daemon listening on Unix socket");

    // Graceful shutdown on SIGINT / SIGTERM.
    let shutdown = async {
        tokio::signal::ctrl_c().await.ok();
        tracing::info!("shutdown signal received");
    };

    tokio::select! {
        _ = accept_loop_unix(listener, manager) => {}
        _ = shutdown => {}
    }

    let _ = std::fs::remove_file(&path);
    tracing::info!("daemon shut down");
    Ok(())
}

#[cfg(unix)]
async fn accept_loop_unix(listener: tokio::net::UnixListener, manager: Arc<SessionManager>) {
    loop {
        match listener.accept().await {
            Ok((stream, _addr)) => {
                let manager = manager.clone();
                tokio::spawn(async move {
                    if let Err(e) = handle_connection(stream, manager).await {
                        tracing::error!(error = %e, "client connection error");
                    }
                });
            }
            Err(e) => {
                tracing::error!(error = %e, "accept error");
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Windows IPC implementation
// ---------------------------------------------------------------------------

#[cfg(windows)]
async fn run_windows(manager: Arc<SessionManager>) -> Result<()> {
    let addr = amux_protocol::default_tcp_addr();
    tracing::info!(%addr, "daemon listening on TCP");
    run_tcp_fallback(manager).await
}

/// TCP server used for Windows IPC.
#[allow(dead_code)]
async fn run_tcp_fallback(manager: Arc<SessionManager>) -> Result<()> {
    use tokio::net::TcpListener;

    let addr = amux_protocol::default_tcp_addr();
    let listener = TcpListener::bind(&addr).await.map_err(|error| {
        if error.kind() == std::io::ErrorKind::AddrInUse {
            anyhow::anyhow!(
                "daemon is already running on {addr}; stop the existing process before starting another instance"
            )
        } else {
            anyhow::Error::new(error).context(format!("failed to bind daemon TCP listener on {addr}"))
        }
    })?;
    tracing::info!(%addr, "daemon ready on TCP");

    let shutdown = async {
        tokio::signal::ctrl_c().await.ok();
        tracing::info!("shutdown signal received");
    };

    tokio::select! {
        _ = accept_loop_tcp(listener, manager) => {}
        _ = shutdown => {}
    }

    tracing::info!("daemon shut down");
    Ok(())
}

#[allow(dead_code)]
async fn accept_loop_tcp(listener: tokio::net::TcpListener, manager: Arc<SessionManager>) {
    loop {
        match listener.accept().await {
            Ok((stream, addr)) => {
                tracing::info!(%addr, "client connected");
                let manager = manager.clone();
                tokio::spawn(async move {
                    if let Err(e) = handle_connection(stream, manager).await {
                        tracing::error!(%addr, error = %e, "client connection error");
                    }
                });
            }
            Err(e) => {
                tracing::error!(error = %e, "accept error");
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Connection handler — generic over any AsyncRead + AsyncWrite stream
// ---------------------------------------------------------------------------

async fn handle_connection<S>(stream: S, manager: Arc<SessionManager>) -> Result<()>
where
    S: AsyncRead + AsyncWrite + Unpin + Send + 'static,
{
    use amux_protocol::DaemonCodec;
    let mut framed = Framed::new(stream, DaemonCodec);

    // Track which sessions this client is attached to so we can fan-out output.
    let mut attached_rxs: Vec<(amux_protocol::SessionId, broadcast::Receiver<DaemonMessage>)> =
        Vec::new();

    loop {
        // We need to select between: incoming client messages and output from attached sessions.
        let msg = if attached_rxs.is_empty() {
            // No attached sessions — just wait for client input.
            match framed.next().await {
                Some(Ok(msg)) => Some(msg),
                Some(Err(e)) => return Err(e.into()),
                None => return Ok(()), // client disconnected
            }
        } else {
            // Select between client input and all attached session outputs.
            // For simplicity we drain all pending broadcast messages first.
            let mut forwarded = false;
            let mut closed_sessions = Vec::new();
            for (sid, rx) in attached_rxs.iter_mut() {
                loop {
                    match rx.try_recv() {
                        Ok(daemon_msg) => {
                            framed.send(daemon_msg).await?;
                            forwarded = true;
                        }
                        Err(broadcast::error::TryRecvError::Empty) => break,
                        Err(broadcast::error::TryRecvError::Lagged(n)) => {
                            tracing::warn!(session = %sid, skipped = n, "broadcast lagged");
                            break;
                        }
                        Err(broadcast::error::TryRecvError::Closed) => {
                            framed
                                .send(DaemonMessage::SessionExited {
                                    id: *sid,
                                    exit_code: None,
                                })
                                .await?;
                            closed_sessions.push(*sid);
                            forwarded = true;
                            break;
                        }
                    }
                }
            }
            if !closed_sessions.is_empty() {
                attached_rxs.retain(|(sid, _)| !closed_sessions.contains(sid));
            }

            // Now try to read one client message with a short timeout so we
            // keep draining output.
            match tokio::time::timeout(
                std::time::Duration::from_millis(if forwarded { 10 } else { 50 }),
                framed.next(),
            )
            .await
            {
                Ok(Some(Ok(msg))) => Some(msg),
                Ok(Some(Err(e))) => return Err(e.into()),
                Ok(None) => return Ok(()),
                Err(_) => None, // timeout — loop back to drain output
            }
        };

        if let Some(msg) = msg {
            match msg {
                ClientMessage::Ping => {
                    framed.send(DaemonMessage::Pong).await?;
                }

                ClientMessage::SpawnSession {
                    shell,
                    cwd,
                    env,
                    workspace_id,
                    cols,
                    rows,
                } => {
                    match manager
                        .spawn(shell, cwd, workspace_id, env, cols, rows)
                        .await
                    {
                        Ok((id, rx)) => {
                            attached_rxs.push((id, rx));
                            framed.send(DaemonMessage::SessionSpawned { id }).await?;
                        }
                        Err(e) => {
                            framed
                                .send(DaemonMessage::Error {
                                    message: e.to_string(),
                                })
                                .await?;
                        }
                    }
                }

                ClientMessage::CloneSession {
                    source_id,
                    workspace_id,
                    cols,
                    rows,
                    replay_scrollback,
                    cwd,
                } => {
                    match manager
                        .clone_session(source_id, workspace_id, cols, rows, replay_scrollback, cwd)
                        .await
                    {
                        Ok((id, rx, active_command)) => {
                            attached_rxs.push((id, rx));
                            framed
                                .send(DaemonMessage::SessionCloned { source_id, id, active_command })
                                .await?;
                        }
                        Err(e) => {
                            framed
                                .send(DaemonMessage::Error {
                                    message: e.to_string(),
                                })
                                .await?;
                        }
                    }
                }

                ClientMessage::AttachSession { id } => match manager.subscribe(id).await {
                    Ok((rx, alive)) => {
                        attached_rxs.push((id, rx));
                        framed.send(DaemonMessage::SessionAttached { id }).await?;
                        if !alive {
                            framed
                                .send(DaemonMessage::SessionExited {
                                    id,
                                    exit_code: None,
                                })
                                .await?;
                        }
                    }
                    Err(e) => {
                        framed
                            .send(DaemonMessage::Error {
                                message: e.to_string(),
                            })
                            .await?;
                    }
                },

                ClientMessage::DetachSession { id } => {
                    attached_rxs.retain(|(sid, _)| *sid != id);
                    framed.send(DaemonMessage::SessionDetached { id }).await?;
                }

                ClientMessage::KillSession { id } => {
                    attached_rxs.retain(|(sid, _)| *sid != id);
                    match manager.kill(id).await {
                        Ok(()) => {
                            framed.send(DaemonMessage::SessionKilled { id }).await?;
                        }
                        Err(e) => {
                            framed
                                .send(DaemonMessage::Error {
                                    message: e.to_string(),
                                })
                                .await?;
                        }
                    }
                }

                ClientMessage::Input { id, data } => {
                    if let Err(e) = manager.write_input(id, &data).await {
                        framed
                            .send(DaemonMessage::Error {
                                message: e.to_string(),
                            })
                            .await?;
                    }
                }

                ClientMessage::ExecuteManagedCommand { id, request } => {
                    match manager.execute_managed_command(id, request).await {
                        Ok(message) => {
                            framed.send(message).await?;
                        }
                        Err(e) => {
                            framed
                                .send(DaemonMessage::Error {
                                    message: e.to_string(),
                                })
                                .await?;
                        }
                    }
                }

                ClientMessage::ResolveApproval {
                    id,
                    approval_id,
                    decision,
                } => match manager.resolve_approval(id, &approval_id, decision).await {
                    Ok(messages) => {
                        for message in messages {
                            framed.send(message).await?;
                        }
                    }
                    Err(e) => {
                        framed
                            .send(DaemonMessage::Error {
                                message: e.to_string(),
                            })
                            .await?;
                    }
                },

                ClientMessage::Resize { id, cols, rows } => {
                    if let Err(e) = manager.resize(id, cols, rows).await {
                        framed
                            .send(DaemonMessage::Error {
                                message: e.to_string(),
                            })
                            .await?;
                    }
                }

                ClientMessage::ListSessions => {
                    let sessions = manager.list().await;
                    framed.send(DaemonMessage::SessionList { sessions }).await?;
                }

                ClientMessage::GetScrollback { id, max_lines } => {
                    match manager.get_scrollback(id, max_lines).await {
                        Ok(data) => {
                            framed.send(DaemonMessage::Scrollback { id, data }).await?;
                        }
                        Err(e) => {
                            framed
                                .send(DaemonMessage::Error {
                                    message: e.to_string(),
                                })
                                .await?;
                        }
                    }
                }

                ClientMessage::AnalyzeSession { id, max_lines } => {
                    match manager.get_analysis_text(id, max_lines).await {
                        Ok(text) => {
                            // TODO: Send to AI model. For now, return the raw text.
                            framed
                                .send(DaemonMessage::AnalysisResult { id, result: text })
                                .await?;
                        }
                        Err(e) => {
                            framed
                                .send(DaemonMessage::Error {
                                    message: e.to_string(),
                                })
                                .await?;
                        }
                    }
                }

                ClientMessage::SearchHistory { query, limit } => {
                    match manager.search_history(&query, limit.unwrap_or(8).max(1)) {
                        Ok((summary, hits)) => {
                            framed
                                .send(DaemonMessage::HistorySearchResult {
                                    query,
                                    summary,
                                    hits,
                                })
                                .await?;
                        }
                        Err(e) => {
                            framed
                                .send(DaemonMessage::Error {
                                    message: e.to_string(),
                                })
                                .await?;
                        }
                    }
                }

                ClientMessage::GenerateSkill { query, title } => {
                    match manager.generate_skill(query.as_deref(), title.as_deref()) {
                        Ok((title, path)) => {
                            framed
                                .send(DaemonMessage::SkillGenerated { title, path })
                                .await?;
                        }
                        Err(e) => {
                            framed
                                .send(DaemonMessage::Error {
                                    message: e.to_string(),
                                })
                                .await?;
                        }
                    }
                }

                ClientMessage::FindSymbol {
                    workspace_root,
                    symbol,
                    limit,
                } => {
                    let matches = manager.find_symbol_matches(
                        &workspace_root,
                        &symbol,
                        limit.unwrap_or(16).max(1),
                    );
                    framed
                        .send(DaemonMessage::SymbolSearchResult { symbol, matches })
                        .await?;
                }

                ClientMessage::ListSnapshots { workspace_id } => {
                    match manager.list_snapshots(workspace_id.as_deref()) {
                        Ok(snapshots) => {
                            framed
                                .send(DaemonMessage::SnapshotList { snapshots })
                                .await?;
                        }
                        Err(e) => {
                            framed
                                .send(DaemonMessage::Error {
                                    message: e.to_string(),
                                })
                                .await?;
                        }
                    }
                }

                ClientMessage::RestoreSnapshot { snapshot_id } => {
                    match manager.restore_snapshot(&snapshot_id) {
                        Ok((ok, message)) => {
                            framed
                                .send(DaemonMessage::SnapshotRestored {
                                    snapshot_id,
                                    ok,
                                    message,
                                })
                                .await?;
                        }
                        Err(e) => {
                            framed
                                .send(DaemonMessage::Error {
                                    message: e.to_string(),
                                })
                                .await?;
                        }
                    }
                }

                ClientMessage::ListWorkspaceSessions { workspace_id } => {
                    let sessions = manager.list_workspace(&workspace_id).await;
                    framed.send(DaemonMessage::SessionList { sessions }).await?;
                }

                ClientMessage::GetGitStatus { path } => {
                    let info = crate::git::get_git_status(&path);
                    framed.send(DaemonMessage::GitStatus { path, info }).await?;
                }

                ClientMessage::SubscribeNotifications => {
                    // Acknowledged. The client will receive OscNotification
                    // messages via the output broadcast channel.
                    // No explicit state change needed here.
                }

                ClientMessage::ScrubSensitive { text } => {
                    let scrubbed = crate::scrub::scrub_sensitive(&text);
                    framed
                        .send(DaemonMessage::ScrubResult { text: scrubbed })
                        .await?;
                }

                ClientMessage::CheckpointSession { id } => {
                    let dump_dir = crate::criu::dump_dir_for_session(&id.to_string())
                        .unwrap_or_else(|_| std::path::PathBuf::from("/tmp/amux-criu"));

                    if !crate::criu::is_available() {
                        framed
                            .send(DaemonMessage::SessionCheckpointed {
                                id,
                                ok: false,
                                path: None,
                                message: "CRIU is not available on this system".to_string(),
                            })
                            .await?;
                    } else {
                        // Get the PID from the session - for now report unavailable
                        // as we'd need to track the child PID in PtySession
                        framed
                            .send(DaemonMessage::SessionCheckpointed {
                                id,
                                ok: false,
                                path: Some(dump_dir.to_string_lossy().into_owned()),
                                message: "CRIU checkpoint: session PID tracking not yet integrated"
                                    .to_string(),
                            })
                            .await?;
                    }
                }

                ClientMessage::VerifyTelemetryIntegrity => {
                    match manager.verify_telemetry_integrity() {
                        Ok(results) => {
                            framed
                                .send(DaemonMessage::TelemetryIntegrityResult { results })
                                .await?;
                        }
                        Err(e) => {
                            framed
                                .send(DaemonMessage::Error {
                                    message: e.to_string(),
                                })
                                .await?;
                        }
                    }
                }
            }
        }
    }
}
