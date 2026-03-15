pub mod agent;
mod criu;
mod git;
mod history;
mod lsp_client;
mod network;
mod osc;
mod policy;
mod policy_external;
mod pty_session;
mod sandbox;
mod scrub;
mod server;
mod session_manager;
mod snapshot;
mod state;
mod validation;

use anyhow::Result;
use tracing_subscriber::EnvFilter;

fn init_logging() -> Result<tracing_appender::non_blocking::WorkerGuard> {
    let log_dir = amux_protocol::ensure_amux_data_dir()?;
    let log_path = amux_protocol::log_file_path("tamux-daemon.log");
    let file_appender = tracing_appender::rolling::never(log_dir, "tamux-daemon.log");
    let (writer, guard) = tracing_appender::non_blocking(file_appender);

    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_env("TAMUX_LOG")
                .or_else(|_| EnvFilter::try_from_env("AMUX_LOG"))
                .unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .with_writer(writer)
        .with_ansi(false)
        .init();

    std::panic::set_hook(Box::new(|panic_info| {
        tracing::error!(panic = %panic_info, "tamux-daemon panicked");
    }));

    tracing::info!(path = %log_path.display(), "daemon log file initialized");
    Ok(guard)
}

#[tokio::main]
async fn main() -> Result<()> {
    let _log_guard = init_logging()?;

    tracing::info!("tamux-daemon starting");

    // Restore any persisted state.
    let state_path = state::default_state_path();
    tracing::info!(?state_path, "state file location");
    match state::load_state(&state_path) {
        Ok(state) => {
            tracing::info!(previous_sessions = state.previous_sessions.len(), "loaded persisted daemon state");
        }
        Err(error) => {
            tracing::warn!(error = %error, path = %state_path.display(), "failed to load persisted daemon state");
        }
    }

    // Start the IPC server (blocks until shutdown signal).
    server::run().await?;

    Ok(())
}
