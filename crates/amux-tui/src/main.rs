mod app;
mod client;
mod projection;
mod state;
mod theme;
mod widgets;
mod wire;

use std::{sync::mpsc, thread};

use anyhow::Result;
use ftui_runtime::{string_model::StringModelAdapter, App, ScreenMode};
use tokio::sync::mpsc as tokio_mpsc;

use crate::app::TuiModel;
use crate::client::DaemonClient;
use crate::state::DaemonCommand;

fn main() -> Result<()> {
    let log_file = std::fs::File::create(std::env::temp_dir().join("tamux-tui.log"))?;
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .with_writer(log_file)
        .init();

    let (daemon_event_tx, daemon_event_rx) = mpsc::channel();
    let (daemon_cmd_tx, daemon_cmd_rx) = tokio_mpsc::unbounded_channel();

    start_daemon_bridge(daemon_event_tx, daemon_cmd_rx);

    let model = TuiModel::new(daemon_event_rx, daemon_cmd_tx);

    App::new(StringModelAdapter::new(model))
        .screen_mode(ScreenMode::AltScreen)
        .with_mouse()
        .run()?;

    Ok(())
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
                            DaemonCommand::FetchModels { provider_id, base_url, api_key } => {
                                let _ = client.fetch_models(provider_id, base_url, api_key);
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
