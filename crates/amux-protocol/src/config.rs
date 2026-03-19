use serde::{Deserialize, Serialize};
use std::path::PathBuf;

pub const DEFAULT_TCP_HOST: &str = "127.0.0.1";
pub const DEFAULT_TCP_PORT: u16 = 17563;

pub fn default_tcp_addr() -> String {
    format!("{DEFAULT_TCP_HOST}:{DEFAULT_TCP_PORT}")
}

fn legacy_amux_data_dir() -> PathBuf {
    #[cfg(windows)]
    {
        dirs::data_local_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("amux")
    }

    #[cfg(not(windows))]
    {
        dirs::home_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join(".amux")
    }
}

pub fn tamux_data_dir() -> PathBuf {
    #[cfg(windows)]
    {
        dirs::data_local_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("tamux")
    }

    #[cfg(not(windows))]
    {
        dirs::home_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join(".tamux")
    }
}

pub fn amux_data_dir() -> PathBuf {
    tamux_data_dir()
}

fn migrate_legacy_data_dir() -> std::io::Result<()> {
    let target = tamux_data_dir();
    let legacy = legacy_amux_data_dir();
    if target.exists() || !legacy.exists() {
        return Ok(());
    }

    if let Some(parent) = target.parent() {
        std::fs::create_dir_all(parent)?;
    }

    let _ = std::fs::rename(&legacy, &target);
    Ok(())
}

fn legacy_config_path() -> PathBuf {
    let base = dirs::config_dir().unwrap_or_else(|| PathBuf::from("."));
    base.join("amux").join("config.json")
}

fn tamux_config_path() -> PathBuf {
    let base = dirs::config_dir().unwrap_or_else(|| PathBuf::from("."));
    base.join("tamux").join("config.json")
}

pub fn ensure_tamux_data_dir() -> std::io::Result<PathBuf> {
    migrate_legacy_data_dir()?;
    let dir = tamux_data_dir();
    std::fs::create_dir_all(&dir)?;
    Ok(dir)
}

pub fn ensure_amux_data_dir() -> std::io::Result<PathBuf> {
    ensure_tamux_data_dir()
}

pub fn log_file_path(file_name: &str) -> PathBuf {
    tamux_data_dir().join(file_name)
}

/// User-configurable settings for tamux.
///
/// Config file locations:
/// - Linux:   ~/.config/tamux/config.json
/// - macOS:   ~/Library/Application Support/tamux/config.json
/// - Windows: %APPDATA%\tamux\config.json
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct AmuxConfig {
    /// Default shell to spawn (None = system default).
    pub default_shell: Option<String>,

    /// Default terminal dimensions.
    pub default_cols: u16,
    pub default_rows: u16,

    /// Scrollback buffer size in bytes.
    pub scrollback_bytes: usize,

    /// IPC mode: "socket" on Unix platforms or "tcp" on Windows.
    pub ipc_mode: String,

    /// TCP port for IPC when TCP mode is active.
    pub tcp_port: u16,

    /// Font settings (used by the frontend).
    pub font_family: String,
    pub font_size: u16,

    /// Theme name (loaded by frontend).
    pub theme: String,

    /// Whether to auto-start the daemon when the UI launches.
    pub auto_start_daemon: bool,

    /// Whether workspace sandboxing is enabled for managed commands.
    pub sandbox_enabled: bool,

    /// Snapshot backend: "auto", "tar", "zfs", "btrfs". Default is "auto".
    pub snapshot_backend: Option<String>,

    /// Cerbos PDP endpoint for external policy evaluation (e.g. "http://localhost:3592").
    pub cerbos_endpoint: Option<String>,
}

impl Default for AmuxConfig {
    fn default() -> Self {
        Self {
            default_shell: None,
            default_cols: 80,
            default_rows: 24,
            scrollback_bytes: 1024 * 1024, // 1 MiB
            ipc_mode: if cfg!(unix) {
                "socket".into()
            } else {
                "tcp".into()
            },
            tcp_port: DEFAULT_TCP_PORT,
            font_family: "Cascadia Code".into(),
            font_size: 14,
            theme: "catppuccin-mocha".into(),
            auto_start_daemon: true,
            sandbox_enabled: true,
            snapshot_backend: None,
            cerbos_endpoint: None,
        }
    }
}

impl AmuxConfig {
    /// Load config from the default location, or return defaults.
    pub fn load() -> Self {
        for path in [Self::config_path(), legacy_config_path()] {
            if !path.exists() {
                continue;
            }

            match std::fs::read_to_string(&path) {
                Ok(data) => match serde_json::from_str(&data) {
                    Ok(cfg) => return cfg,
                    Err(e) => {
                        eprintln!("warning: invalid config file: {e}");
                    }
                },
                Err(e) => {
                    eprintln!("warning: cannot read config file: {e}");
                }
            }
        }
        Self::default()
    }

    /// Save the current config to disk.
    pub fn save(&self) -> std::io::Result<()> {
        let path = Self::config_path();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let data = serde_json::to_string_pretty(self)
            .map_err(std::io::Error::other)?;
        std::fs::write(&path, data)
    }

    /// Platform-specific config file path.
    pub fn config_path() -> PathBuf {
        tamux_config_path()
    }
}
