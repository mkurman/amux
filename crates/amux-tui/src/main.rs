mod app;
mod client;
mod projection;
mod providers;
mod state;
mod theme;
mod widgets;
mod wire;

use std::io;
use std::sync::mpsc;
use std::thread;
use std::time::Duration;

use anyhow::Result;
use crossterm::{
    event::{self, DisableBracketedPaste, DisableMouseCapture, EnableBracketedPaste, EnableMouseCapture, Event},
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
    ExecutableCommand,
};
use ratatui::prelude::*;
use tokio::sync::mpsc as tokio_mpsc;

use crate::app::TuiModel;
use crate::client::DaemonClient;
use crate::state::DaemonCommand;
use crate::wire::FetchedModel;

fn main() -> Result<()> {
    let home = std::env::var("HOME")
        .or_else(|_| std::env::var("USERPROFILE"))
        .unwrap_or_else(|_| "/tmp".to_string());
    let log_dir = format!("{}/.tamux", home);
    let _ = std::fs::create_dir_all(&log_dir);
    let log_file = std::fs::File::create(format!("{}/tamux-tui.log", log_dir))?;
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::DEBUG)
        .with_writer(log_file)
        .init();

    // Setup terminal
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    stdout.execute(EnterAlternateScreen)?;
    stdout.execute(EnableMouseCapture)?;
    stdout.execute(EnableBracketedPaste)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;
    terminal.clear()?;

    // Setup daemon bridge
    let (daemon_event_tx, daemon_event_rx) = mpsc::channel();
    let (daemon_cmd_tx, daemon_cmd_rx) = tokio_mpsc::unbounded_channel();
    start_daemon_bridge(daemon_event_tx, daemon_cmd_rx);

    // Create model
    let mut model = TuiModel::new(daemon_event_rx, daemon_cmd_tx);
    model.load_saved_settings();

    // Main loop
    let tick_rate = Duration::from_millis(50);
    let result = run_loop(&mut terminal, &mut model, tick_rate);

    // Restore terminal
    disable_raw_mode()?;
    terminal.backend_mut().execute(DisableBracketedPaste)?;
    terminal.backend_mut().execute(DisableMouseCapture)?;
    terminal.backend_mut().execute(LeaveAlternateScreen)?;
    terminal.show_cursor()?;

    result
}

fn run_loop(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    model: &mut TuiModel,
    tick_rate: Duration,
) -> Result<()> {
    loop {
        terminal.draw(|frame| {
            model.render(frame);
        })?;

        // Poll for events with timeout
        if event::poll(tick_rate)? {
            match event::read()? {
                Event::Key(key) if key.kind == crossterm::event::KeyEventKind::Press => {
                    if model.handle_key(key.code, key.modifiers) {
                        return Ok(());
                    }
                }
                Event::Paste(text) => {
                    model.handle_paste(text);
                }
                Event::Resize(w, h) => model.handle_resize(w, h),
                Event::Mouse(mouse) => model.handle_mouse(mouse),
                _ => {}
            }
        }

        model.pump_daemon_events();
    }
}

fn start_daemon_bridge(
    daemon_event_tx: mpsc::Sender<client::ClientEvent>,
    mut daemon_cmd_rx: tokio_mpsc::UnboundedReceiver<DaemonCommand>,
) {
    thread::spawn(move || {
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build();

        let Ok(runtime) = runtime else {
            let _ = daemon_event_tx.send(client::ClientEvent::Error(
                "failed to create tokio runtime".to_string(),
            ));
            return;
        };

        runtime.block_on(async move {
            let (client_event_tx, mut client_event_rx) = tokio_mpsc::channel(512);
            let client = DaemonClient::new(client_event_tx);

            if let Err(err) = client.connect().await {
                let _ = daemon_event_tx.send(client::ClientEvent::Error(format!(
                    "daemon connection init failed: {}",
                    err
                )));
            }

            loop {
                tokio::select! {
                    incoming = client_event_rx.recv() => {
                        match incoming {
                            Some(event) => {
                                let _ = daemon_event_tx.send(event);
                            }
                            None => break,
                        }
                    }
                    command = daemon_cmd_rx.recv() => {
                        let Some(command) = command else {
                            break;
                        };

                        match command {
                            DaemonCommand::Refresh => {
                                let _ = client.refresh();
                            }
                            DaemonCommand::RefreshServices => {
                                let _ = client.refresh_services();
                            }
                            DaemonCommand::RequestThread(thread_id) => {
                                let _ = client.request_thread(thread_id);
                            }
                            DaemonCommand::RequestGoalRunDetail(goal_run_id) => {
                                let _ = client.request_goal_run(goal_run_id);
                            }
                            DaemonCommand::SendMessage { thread_id, content, session_id } => {
                                let _ = client.send_message(thread_id, content, session_id);
                            }
                            DaemonCommand::FetchModels { provider_id: _, base_url, api_key } => {
                                // Fetch models directly from provider API
                                let tx = daemon_event_tx.clone();
                                tokio::spawn(async move {
                                    match fetch_models_http(&base_url, &api_key).await {
                                        Ok(models) => {
                                            let _ = tx.send(client::ClientEvent::ModelsFetched(models));
                                        }
                                        Err(err) => {
                                            tracing::warn!("Model fetch failed: {}", err);
                                            // Don't send error — hardcoded fallback will be used
                                        }
                                    }
                                });
                            }
                            DaemonCommand::SetConfigJson(config_json) => {
                                let _ = client.set_config_json(config_json);
                            }
                            DaemonCommand::ControlGoalRun { goal_run_id, action } => {
                                let _ = client.control_goal_run(goal_run_id, action);
                            }
                            DaemonCommand::ResolveTaskApproval { approval_id, decision } => {
                                let _ = client.resolve_task_approval(approval_id, decision);
                            }
                            DaemonCommand::SpawnSession { shell, cwd, cols, rows } => {
                                let _ = client.spawn_session(shell, cwd, cols, rows);
                            }
                        }
                    }
                }
            }
        });
    });
}

/// Fetch models from a provider's /models API endpoint.
/// Most OpenAI-compatible providers expose GET /models (or /v1/models).
async fn fetch_models_http(base_url: &str, api_key: &str) -> Result<Vec<FetchedModel>> {
    // Normalize URL: ensure it ends with /models
    let url = if base_url.ends_with("/models") {
        base_url.to_string()
    } else {
        let base = base_url.trim_end_matches('/');
        if base.ends_with("/v1") {
            format!("{}/models", base)
        } else {
            format!("{}/v1/models", base)
        }
    };

    // ureq is sync — run in blocking task
    let api_key = api_key.to_string();
    let result = tokio::task::spawn_blocking(move || -> Result<Vec<FetchedModel>> {
        let mut resp = ureq::get(&url)
            .header("Authorization", &format!("Bearer {}", api_key))
            .header("Accept", "application/json")
            .call()
            .map_err(|e| anyhow::anyhow!("HTTP request failed: {}", e))?;

        let body: serde_json::Value = resp.body_mut()
            .read_json()
            .map_err(|e| anyhow::anyhow!("JSON parse failed: {}", e))?;

        let mut models = Vec::new();
        if let Some(data) = body.get("data").and_then(|d| d.as_array()) {
            for item in data {
                let id = item.get("id").and_then(|v| v.as_str()).unwrap_or("").to_string();
                if id.is_empty() {
                    continue;
                }
                // Skip embedding/tts/whisper/dall-e models
                if id.contains("embedding") || id.contains("tts") || id.contains("whisper")
                    || id.contains("dall-e") || id.contains("davinci") || id.contains("babbage")
                    || id.contains("moderation")
                {
                    continue;
                }
                let name = item.get("name").and_then(|v| v.as_str()).map(String::from);
                let context_window = item
                    .get("context_window")
                    .and_then(|v| v.as_u64())
                    .map(|v| v as u32);
                models.push(FetchedModel {
                    id,
                    name,
                    context_window,
                });
            }
        }

        // Sort: longer context windows first, then alphabetical
        models.sort_by(|a, b| {
            b.context_window.unwrap_or(0).cmp(&a.context_window.unwrap_or(0))
                .then(a.id.cmp(&b.id))
        });

        Ok(models)
    }).await??;

    Ok(result)
}
