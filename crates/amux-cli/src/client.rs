use amux_protocol::{
    AmuxCodec, ApprovalDecision, ApprovalPayload, ClientMessage, DaemonMessage, HistorySearchHit,
    ManagedCommandRequest, ManagedCommandSource, SecurityLevel, SessionInfo, SnapshotInfo,
    SymbolMatch,
};
use anyhow::{Context, Result};
use base64::Engine;
use futures::{SinkExt, StreamExt};
use serde::{Deserialize, Serialize};
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio_util::codec::Framed;

const BASE64_ENGINE: base64::engine::GeneralPurpose = base64::engine::general_purpose::STANDARD;

#[derive(Debug, Deserialize)]
#[serde(tag = "type", rename_all = "kebab-case")]
enum BridgeCommand {
    Input {
        data: String,
    },
    Resize {
        cols: u16,
        rows: u16,
    },
    ExecuteManaged {
        command: String,
        rationale: String,
        allow_network: bool,
        sandbox_enabled: Option<bool>,
        security_level: Option<String>,
        cwd: Option<String>,
        language_hint: Option<String>,
        source: Option<String>,
    },
    ApprovalDecision {
        approval_id: String,
        decision: String,
    },
    SearchHistory {
        query: String,
        limit: Option<usize>,
    },
    GenerateSkill {
        query: Option<String>,
        title: Option<String>,
    },
    FindSymbol {
        workspace_root: String,
        symbol: String,
        limit: Option<usize>,
    },
    ListSnapshots {
        workspace_id: Option<String>,
    },
    RestoreSnapshot {
        snapshot_id: String,
    },
    Shutdown,
    KillSession,
}

#[derive(Debug, Serialize)]
#[serde(tag = "type", rename_all = "kebab-case")]
enum BridgeEvent {
    Ready {
        session_id: String,
    },
    Output {
        session_id: String,
        data: String,
    },
    CommandStarted {
        session_id: String,
        command_b64: String,
    },
    CommandFinished {
        session_id: String,
        exit_code: Option<i32>,
    },
    ManagedQueued {
        session_id: String,
        execution_id: String,
        position: usize,
        snapshot: Option<SnapshotInfo>,
    },
    ApprovalRequired {
        session_id: String,
        approval: ApprovalPayload,
    },
    ApprovalResolved {
        session_id: String,
        approval_id: String,
        decision: ApprovalDecision,
    },
    ManagedStarted {
        session_id: String,
        execution_id: String,
        command: String,
        source: ManagedCommandSource,
    },
    ManagedFinished {
        session_id: String,
        execution_id: String,
        command: String,
        exit_code: Option<i32>,
        duration_ms: Option<u64>,
        snapshot: Option<SnapshotInfo>,
    },
    ManagedRejected {
        session_id: String,
        execution_id: Option<String>,
        message: String,
    },
    HistorySearchResult {
        query: String,
        summary: String,
        hits: Vec<HistorySearchHit>,
    },
    SkillGenerated {
        title: String,
        path: String,
    },
    SymbolSearchResult {
        symbol: String,
        matches: Vec<SymbolMatch>,
    },
    SnapshotList {
        snapshots: Vec<SnapshotInfo>,
    },
    SnapshotRestored {
        snapshot_id: String,
        ok: bool,
        message: String,
    },
    SessionExited {
        session_id: String,
        exit_code: Option<i32>,
    },
    Error {
        message: String,
    },
}

fn emit_bridge_event(event: BridgeEvent) -> Result<()> {
    println!("{}", serde_json::to_string(&event)?);
    Ok(())
}

/// Connect to the daemon and return a framed stream.
async fn connect(
) -> Result<Framed<impl tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin, AmuxCodec>> {
    #[cfg(unix)]
    {
        let runtime_dir = std::env::var("XDG_RUNTIME_DIR").unwrap_or_else(|_| "/tmp".to_string());
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

/// Send a message and receive exactly one response.
async fn roundtrip(msg: ClientMessage) -> Result<DaemonMessage> {
    let mut framed = connect().await?;
    framed.send(msg).await?;
    let resp = framed
        .next()
        .await
        .ok_or_else(|| anyhow::anyhow!("daemon closed connection"))??;
    Ok(resp)
}

pub async fn ping() -> Result<()> {
    match roundtrip(ClientMessage::Ping).await? {
        DaemonMessage::Pong => Ok(()),
        other => anyhow::bail!("unexpected response: {other:?}"),
    }
}

pub async fn list_sessions() -> Result<Vec<SessionInfo>> {
    match roundtrip(ClientMessage::ListSessions).await? {
        DaemonMessage::SessionList { sessions } => Ok(sessions),
        DaemonMessage::Error { message } => anyhow::bail!("daemon error: {message}"),
        other => anyhow::bail!("unexpected response: {other:?}"),
    }
}

pub async fn spawn_session(
    shell: Option<String>,
    cwd: Option<String>,
    workspace_id: Option<String>,
) -> Result<String> {
    match roundtrip(ClientMessage::SpawnSession {
        shell,
        cwd,
        env: None,
        workspace_id,
        cols: 80,
        rows: 24,
    })
    .await?
    {
        DaemonMessage::SessionSpawned { id } => Ok(id.to_string()),
        DaemonMessage::Error { message } => anyhow::bail!("daemon error: {message}"),
        other => anyhow::bail!("unexpected response: {other:?}"),
    }
}

pub async fn clone_session(
    source_id: &str,
    workspace_id: Option<String>,
    cols: Option<u16>,
    rows: Option<u16>,
    cwd: Option<String>,
) -> Result<(String, Option<String>)> {
    let source_uuid = source_id.parse().context("invalid source session ID")?;
    match roundtrip(ClientMessage::CloneSession {
        source_id: source_uuid,
        workspace_id,
        cols,
        rows,
        replay_scrollback: false,
        cwd,
    })
    .await?
    {
        DaemonMessage::SessionCloned { id, active_command, .. } => Ok((id.to_string(), active_command)),
        DaemonMessage::Error { message } => anyhow::bail!("daemon error: {message}"),
        other => anyhow::bail!("unexpected response: {other:?}"),
    }
}

pub async fn kill_session(id: &str) -> Result<()> {
    let uuid = id.parse().context("invalid session ID")?;
    match roundtrip(ClientMessage::KillSession { id: uuid }).await? {
        DaemonMessage::SessionKilled { .. } => Ok(()),
        DaemonMessage::Error { message } => anyhow::bail!("daemon error: {message}"),
        other => anyhow::bail!("unexpected response: {other:?}"),
    }
}

pub async fn attach_session(id: &str) -> Result<()> {
    let uuid: uuid::Uuid = id.parse().context("invalid session ID")?;
    let mut framed = connect().await?;

    // Attach.
    framed
        .send(ClientMessage::AttachSession { id: uuid })
        .await?;

    // Stream output to stdout.
    while let Some(msg) = framed.next().await {
        match msg? {
            DaemonMessage::Output { data, .. } => {
                use std::io::Write;
                std::io::stdout().write_all(&data)?;
                std::io::stdout().flush()?;
            }
            DaemonMessage::SessionExited { exit_code, .. } => {
                println!(
                    "\r\nSession exited (code: {})",
                    exit_code.map_or("unknown".to_string(), |c| c.to_string())
                );
                break;
            }
            DaemonMessage::Error { message } => {
                eprintln!("Error: {message}");
                break;
            }
            _ => {}
        }
    }

    Ok(())
}

pub async fn get_git_status(path: String) -> Result<amux_protocol::GitInfo> {
    match roundtrip(ClientMessage::GetGitStatus { path }).await? {
        DaemonMessage::GitStatus { info, .. } => Ok(info),
        DaemonMessage::Error { message } => anyhow::bail!("daemon error: {message}"),
        other => anyhow::bail!("unexpected response: {other:?}"),
    }
}

pub async fn scrub_text(text: String) -> Result<String> {
    match roundtrip(ClientMessage::ScrubSensitive { text }).await? {
        DaemonMessage::ScrubResult { text } => Ok(text),
        DaemonMessage::Error { message } => anyhow::bail!("daemon error: {message}"),
        other => anyhow::bail!("unexpected response: {other:?}"),
    }
}

pub async fn run_bridge(
    session: Option<String>,
    shell: Option<String>,
    cwd: Option<String>,
    workspace: Option<String>,
    cols: u16,
    rows: u16,
) -> Result<()> {
    let mut framed = connect().await?;
    let session_id = if let Some(session) = session {
        let id = session.parse().context("invalid session ID")?;
        match attach_bridge_session(&mut framed, id).await {
            Ok(attached_id) => attached_id,
            Err(error) if is_missing_session_error(&error) => {
                tracing::warn!(requested_session = %id, error = %error, "saved session missing; spawning replacement session");
                spawn_bridge_session(
                    &mut framed,
                    shell.clone(),
                    cwd.clone(),
                    workspace.clone(),
                    cols,
                    rows,
                )
                .await?
            }
            Err(error) => return Err(error),
        }
    } else {
        spawn_bridge_session(
            &mut framed,
            shell.clone(),
            cwd.clone(),
            workspace.clone(),
            cols,
            rows,
        )
        .await?
    };

    emit_bridge_event(BridgeEvent::Ready {
        session_id: session_id.to_string(),
    })?;

    // Replay recent daemon scrollback so renderer reloads can reconstruct terminal state
    // even when the Electron-side bridge process is recreated.
    framed
        .send(ClientMessage::GetScrollback {
            id: session_id,
            max_lines: None,
        })
        .await
        .ok();

    // Nudge the PTY with a resize so that shells (especially wsl.exe on Windows)
    // redraw their prompt even if the initial output was produced before we subscribed.
    framed
        .send(ClientMessage::Resize {
            id: session_id,
            cols,
            rows,
        })
        .await
        .ok();

    let mut stdin_lines = BufReader::new(tokio::io::stdin()).lines();

    loop {
        tokio::select! {
            maybe_line = stdin_lines.next_line() => {
                match maybe_line? {
                    Some(line) => {
                        let command: BridgeCommand = match serde_json::from_str(&line) {
                            Ok(command) => command,
                            Err(error) => {
                                emit_bridge_event(BridgeEvent::Error {
                                    message: format!("invalid bridge command: {error}"),
                                })?;
                                continue;
                            }
                        };

                        match command {
                            BridgeCommand::Input { data } => {
                                let bytes = BASE64_ENGINE
                                    .decode(data)
                                    .context("invalid input payload")?;
                                framed
                                    .send(ClientMessage::Input { id: session_id, data: bytes })
                                    .await?;
                            }
                            BridgeCommand::Resize { cols, rows } => {
                                framed
                                    .send(ClientMessage::Resize { id: session_id, cols, rows })
                                    .await?;
                            }
                            BridgeCommand::ExecuteManaged {
                                command,
                                rationale,
                                allow_network,
                                sandbox_enabled,
                                security_level,
                                cwd,
                                language_hint,
                                source,
                            } => {
                                let source = match source.as_deref() {
                                    Some("human") => ManagedCommandSource::Human,
                                    Some("replay") => ManagedCommandSource::Replay,
                                    Some("gateway") => ManagedCommandSource::Gateway,
                                    _ => ManagedCommandSource::Agent,
                                };
                                let security_level = match security_level.as_deref() {
                                    Some("highest") => SecurityLevel::Highest,
                                    Some("lowest") => SecurityLevel::Lowest,
                                    Some("yolo") => SecurityLevel::Yolo,
                                    _ => SecurityLevel::Moderate,
                                };
                                framed
                                    .send(ClientMessage::ExecuteManagedCommand {
                                        id: session_id,
                                        request: ManagedCommandRequest {
                                            command,
                                            rationale,
                                            allow_network,
                                            sandbox_enabled: sandbox_enabled.unwrap_or(false),
                                            security_level,
                                            cwd,
                                            language_hint,
                                            source,
                                        },
                                    })
                                    .await?;
                            }
                            BridgeCommand::ApprovalDecision { approval_id, decision } => {
                                let decision = match decision.as_str() {
                                    "approve-session" => ApprovalDecision::ApproveSession,
                                    "deny" => ApprovalDecision::Deny,
                                    _ => ApprovalDecision::ApproveOnce,
                                };
                                framed
                                    .send(ClientMessage::ResolveApproval {
                                        id: session_id,
                                        approval_id,
                                        decision,
                                    })
                                    .await?;
                            }
                            BridgeCommand::SearchHistory { query, limit } => {
                                framed
                                    .send(ClientMessage::SearchHistory { query, limit })
                                    .await?;
                            }
                            BridgeCommand::GenerateSkill { query, title } => {
                                framed
                                    .send(ClientMessage::GenerateSkill { query, title })
                                    .await?;
                            }
                            BridgeCommand::FindSymbol { workspace_root, symbol, limit } => {
                                framed
                                    .send(ClientMessage::FindSymbol {
                                        workspace_root,
                                        symbol,
                                        limit,
                                    })
                                    .await?;
                            }
                            BridgeCommand::ListSnapshots { workspace_id } => {
                                framed
                                    .send(ClientMessage::ListSnapshots { workspace_id })
                                    .await?;
                            }
                            BridgeCommand::RestoreSnapshot { snapshot_id } => {
                                framed
                                    .send(ClientMessage::RestoreSnapshot { snapshot_id })
                                    .await?;
                            }
                            BridgeCommand::Shutdown => {
                                break;
                            }
                            BridgeCommand::KillSession => {
                                framed
                                    .send(ClientMessage::KillSession { id: session_id })
                                    .await?;
                                break;
                            }
                        }
                    }
                    None => break,
                }
            }
            maybe_message = framed.next() => {
                match maybe_message {
                    Some(Ok(DaemonMessage::Output { id, data })) if id == session_id => {
                        if !data.is_empty() {
                            emit_bridge_event(BridgeEvent::Output {
                                session_id: id.to_string(),
                                data: BASE64_ENGINE.encode(data),
                            })?;
                        }
                    }
                    Some(Ok(DaemonMessage::CommandStarted { id, command })) if id == session_id => {
                        emit_bridge_event(BridgeEvent::CommandStarted {
                            session_id: id.to_string(),
                            command_b64: BASE64_ENGINE.encode(command),
                        })?;
                    }
                    Some(Ok(DaemonMessage::CommandFinished { id, exit_code })) if id == session_id => {
                        emit_bridge_event(BridgeEvent::CommandFinished {
                            session_id: id.to_string(),
                            exit_code,
                        })?;
                    }
                    Some(Ok(DaemonMessage::ManagedCommandQueued { id, execution_id, position, snapshot })) if id == session_id => {
                        emit_bridge_event(BridgeEvent::ManagedQueued {
                            session_id: id.to_string(),
                            execution_id,
                            position,
                            snapshot,
                        })?;
                    }
                    Some(Ok(DaemonMessage::ApprovalRequired { id, approval })) if id == session_id => {
                        emit_bridge_event(BridgeEvent::ApprovalRequired {
                            session_id: id.to_string(),
                            approval,
                        })?;
                    }
                    Some(Ok(DaemonMessage::ApprovalResolved { id, approval_id, decision })) if id == session_id => {
                        emit_bridge_event(BridgeEvent::ApprovalResolved {
                            session_id: id.to_string(),
                            approval_id,
                            decision,
                        })?;
                    }
                    Some(Ok(DaemonMessage::ManagedCommandStarted { id, execution_id, command, source })) if id == session_id => {
                        emit_bridge_event(BridgeEvent::ManagedStarted {
                            session_id: id.to_string(),
                            execution_id,
                            command,
                            source,
                        })?;
                    }
                    Some(Ok(DaemonMessage::ManagedCommandFinished { id, execution_id, command, exit_code, duration_ms, snapshot })) if id == session_id => {
                        emit_bridge_event(BridgeEvent::ManagedFinished {
                            session_id: id.to_string(),
                            execution_id,
                            command,
                            exit_code,
                            duration_ms,
                            snapshot,
                        })?;
                    }
                    Some(Ok(DaemonMessage::ManagedCommandRejected { id, execution_id, message })) if id == session_id => {
                        emit_bridge_event(BridgeEvent::ManagedRejected {
                            session_id: id.to_string(),
                            execution_id,
                            message,
                        })?;
                    }
                    Some(Ok(DaemonMessage::HistorySearchResult { query, summary, hits })) => {
                        emit_bridge_event(BridgeEvent::HistorySearchResult { query, summary, hits })?;
                    }
                    Some(Ok(DaemonMessage::SkillGenerated { title, path })) => {
                        emit_bridge_event(BridgeEvent::SkillGenerated { title, path })?;
                    }
                    Some(Ok(DaemonMessage::SymbolSearchResult { symbol, matches })) => {
                        emit_bridge_event(BridgeEvent::SymbolSearchResult { symbol, matches })?;
                    }
                    Some(Ok(DaemonMessage::SnapshotList { snapshots })) => {
                        emit_bridge_event(BridgeEvent::SnapshotList { snapshots })?;
                    }
                    Some(Ok(DaemonMessage::SnapshotRestored { snapshot_id, ok, message })) => {
                        emit_bridge_event(BridgeEvent::SnapshotRestored { snapshot_id, ok, message })?;
                    }
                    Some(Ok(DaemonMessage::Scrollback { id, data })) if id == session_id => {
                        if !data.is_empty() {
                            emit_bridge_event(BridgeEvent::Output {
                                session_id: id.to_string(),
                                data: BASE64_ENGINE.encode(data),
                            })?;
                        }
                    }
                    Some(Ok(DaemonMessage::SessionExited { id, exit_code })) if id == session_id => {
                        emit_bridge_event(BridgeEvent::SessionExited {
                            session_id: id.to_string(),
                            exit_code,
                        })?;
                        break;
                    }
                    Some(Ok(DaemonMessage::SessionKilled { id })) if id == session_id => {
                        emit_bridge_event(BridgeEvent::SessionExited {
                            session_id: id.to_string(),
                            exit_code: None,
                        })?;
                        break;
                    }
                    Some(Ok(DaemonMessage::Error { message })) => {
                        emit_bridge_event(BridgeEvent::Error { message })?;
                    }
                    Some(Ok(_)) => {}
                    Some(Err(error)) => return Err(error.into()),
                    None => {
                        emit_bridge_event(BridgeEvent::Error {
                            message: "daemon connection closed".to_string(),
                        })?;
                        break;
                    }
                }
            }
        }
    }

    Ok(())
}

async fn attach_bridge_session(
    framed: &mut Framed<impl tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin, AmuxCodec>,
    id: uuid::Uuid,
) -> Result<uuid::Uuid> {
    framed
        .send(ClientMessage::AttachSession { id })
        .await
        .context("failed to attach to daemon session")?;

    match framed.next().await {
        Some(Ok(DaemonMessage::SessionAttached { id })) => Ok(id),
        Some(Ok(DaemonMessage::Error { message })) => anyhow::bail!(message),
        Some(Ok(other)) => anyhow::bail!("unexpected response: {other:?}"),
        Some(Err(error)) => Err(error.into()),
        None => anyhow::bail!("daemon closed connection while attaching"),
    }
}

async fn spawn_bridge_session(
    framed: &mut Framed<impl tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin, AmuxCodec>,
    shell: Option<String>,
    cwd: Option<String>,
    workspace: Option<String>,
    cols: u16,
    rows: u16,
) -> Result<uuid::Uuid> {
    framed
        .send(ClientMessage::SpawnSession {
            shell,
            cwd,
            env: None,
            workspace_id: workspace,
            cols,
            rows,
        })
        .await
        .context("failed to request session spawn")?;

    match framed.next().await {
        Some(Ok(DaemonMessage::SessionSpawned { id })) => Ok(id),
        Some(Ok(DaemonMessage::Error { message })) => anyhow::bail!(message),
        Some(Ok(other)) => anyhow::bail!("unexpected response: {other:?}"),
        Some(Err(error)) => Err(error.into()),
        None => anyhow::bail!("daemon closed connection while spawning session"),
    }
}

fn is_missing_session_error(error: &anyhow::Error) -> bool {
    error
        .to_string()
        .to_ascii_lowercase()
        .contains("session not found")
}
