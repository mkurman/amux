mod client;
mod plugins;

use anyhow::Result;
use clap::{Parser, Subcommand};
use tracing_subscriber::EnvFilter;

#[derive(Parser)]
#[command(name = "tamux", about = "tamux terminal multiplexer CLI")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Debug, Subcommand)]
enum Commands {
    /// List all running sessions.
    #[command(alias = "ls")]
    List,

    /// Spawn a new terminal session.
    New {
        /// Shell to use (default: system shell).
        #[arg(short, long)]
        shell: Option<String>,
        /// Working directory.
        #[arg(short, long)]
        cwd: Option<String>,
        /// Workspace ID to associate with.
        #[arg(short, long)]
        workspace: Option<String>,
    },

    /// Attach to an existing session.
    Attach {
        /// Session ID.
        id: String,
    },

    /// Clone an existing session into a new independent session.
    Clone {
        /// Source session ID.
        #[arg(long)]
        source: String,
        /// Optional terminal columns override.
        #[arg(long)]
        cols: Option<u16>,
        /// Optional terminal rows override.
        #[arg(long)]
        rows: Option<u16>,
        /// Optional workspace override.
        #[arg(short, long)]
        workspace: Option<String>,
        /// Fallback CWD when the source session's CWD cannot be resolved.
        #[arg(long)]
        cwd: Option<String>,
    },

    /// Kill a session.
    Kill {
        /// Session ID.
        id: String,
    },

    /// Get git status for a directory.
    Git {
        /// Directory path (default: current directory).
        #[arg(default_value = ".")]
        path: String,
    },

    /// Scrub sensitive data from stdin or a string.
    Scrub {
        /// Text to scrub (reads from stdin if not provided).
        text: Option<String>,
    },

    /// Ping the daemon (health check).
    Ping,

    /// Start the daemon (if not already running).
    #[command(name = "daemon")]
    StartDaemon,

    /// Install runtime extensions.
    Install {
        #[command(subcommand)]
        target: InstallTarget,
    },

    /// Internal JSON bridge used by the Electron frontend.
    #[command(hide = true, name = "bridge")]
    Bridge {
        /// Attach to an existing session instead of spawning a new one.
        #[arg(long)]
        session: Option<String>,
        /// Shell to use when spawning a session.
        #[arg(long)]
        shell: Option<String>,
        /// Working directory when spawning a session.
        #[arg(long)]
        cwd: Option<String>,
        /// Workspace ID to associate with the spawned session.
        #[arg(long)]
        workspace: Option<String>,
        /// Initial terminal columns.
        #[arg(long, default_value_t = 80)]
        cols: u16,
        /// Initial terminal rows.
        #[arg(long, default_value_t = 24)]
        rows: u16,
    },
}

#[derive(Debug, Subcommand)]
enum InstallTarget {
    /// Install a tamux plugin from npm or a local package directory.
    Plugin {
        /// npm package spec or local package directory.
        package: String,
    },
}

fn init_logging(log_file_name: &str) -> Result<tracing_appender::non_blocking::WorkerGuard> {
    let log_dir = amux_protocol::ensure_amux_data_dir()?;
    let log_path = amux_protocol::log_file_path(log_file_name);
    let file_appender = tracing_appender::rolling::never(log_dir, log_file_name);
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
        tracing::error!(panic = %panic_info, "tamux-cli panicked");
    }));

    tracing::info!(path = %log_path.display(), "cli log file initialized");
    Ok(guard)
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();
    let log_file_name = match &cli.command {
        Commands::Bridge { .. } => "tamux-bridge.log",
        _ => "tamux-cli.log",
    };
    let _log_guard = init_logging(log_file_name)?;
    tracing::info!(command = ?cli.command, "tamux-cli starting");

    match cli.command {
        Commands::List => {
            let sessions = client::list_sessions().await?;
            if sessions.is_empty() {
                println!("No active sessions.");
            } else {
                println!(
                    "{:<38} {:>5} {:>5}  {:>5}  {}",
                    "ID", "COLS", "ROWS", "ALIVE", "CWD"
                );
                for s in sessions {
                    println!(
                        "{:<38} {:>5} {:>5}  {:>5}  {}",
                        s.id,
                        s.cols,
                        s.rows,
                        if s.is_alive { "yes" } else { "no" },
                        s.cwd.unwrap_or_default()
                    );
                }
            }
        }

        Commands::New {
            shell,
            cwd,
            workspace,
        } => {
            let id = client::spawn_session(shell, cwd, workspace).await?;
            println!("Session created: {id}");
        }

        Commands::Attach { id } => {
            println!("Attaching to session {id}...");
            client::attach_session(&id).await?;
        }

        Commands::Clone {
            source,
            cols,
            rows,
            workspace,
            cwd,
        } => {
            let (id, active_command) = client::clone_session(&source, workspace, cols, rows, cwd).await?;
            println!("{id}");
            if let Some(cmd) = active_command {
                println!("active_command:{cmd}");
            }
        }

        Commands::Kill { id } => {
            client::kill_session(&id).await?;
            println!("Session killed: {id}");
        }

        Commands::Git { path } => {
            let abs_path =
                std::fs::canonicalize(&path).unwrap_or_else(|_| std::path::PathBuf::from(&path));
            let info = client::get_git_status(abs_path.to_string_lossy().to_string()).await?;
            println!("Branch: {}", info.branch.as_deref().unwrap_or("(none)"));
            println!("Dirty:  {}", info.is_dirty);
            println!("Ahead:  {} Behind: {}", info.ahead, info.behind);
            println!(
                "Staged: {} Modified: {} Untracked: {}",
                info.staged, info.modified, info.untracked
            );
        }

        Commands::Scrub { text } => {
            let input = if let Some(t) = text {
                t
            } else {
                use std::io::Read;
                let mut buf = String::new();
                std::io::stdin().read_to_string(&mut buf)?;
                buf
            };
            let result = client::scrub_text(input).await?;
            print!("{result}");
        }

        Commands::Ping => {
            client::ping().await?;
            println!("Daemon is alive (pong).");
        }

        Commands::StartDaemon => {
            println!("Starting daemon...");
            let mut cmd = std::process::Command::new("tamux-daemon");
            cmd.stdin(std::process::Stdio::null())
                .stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::null());

            #[cfg(windows)]
            {
                use std::os::windows::process::CommandExt;
                cmd.creation_flags(0x08000000); // CREATE_NO_WINDOW
            }

            cmd.spawn()?;
            println!("Daemon started.");
        }

        Commands::Install { target } => match target {
            InstallTarget::Plugin { package } => {
                let installed = plugins::install_plugin(&package)?;
                println!(
                    "Installed plugin {}@{}\nentry: {}\nformat: {}",
                    installed.package_name,
                    installed.package_version,
                    installed.entry_path,
                    installed.format
                );
            }
        },

        Commands::Bridge {
            session,
            shell,
            cwd,
            workspace,
            cols,
            rows,
        } => {
            client::run_bridge(session, shell, cwd, workspace, cols, rows).await?;
        }
    }

    Ok(())
}
