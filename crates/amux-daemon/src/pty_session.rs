use std::collections::VecDeque;
use std::io::{Read, Write};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Instant;
use std::time::{SystemTime, UNIX_EPOCH};

use amux_protocol::{
    DaemonMessage, ManagedCommandRequest, ManagedCommandSource, SessionId, SnapshotInfo,
};
use anyhow::Result;
use base64::Engine;
use portable_pty::{native_pty_system, Child, CommandBuilder, MasterPty, PtySize};
use tokio::sync::broadcast;

use crate::history::{HistoryStore, ManagedHistoryRecord};
use crate::network;
use crate::osc::parse_osc_notifications;
use crate::sandbox;

/// Rolling scrollback buffer capacity (bytes).
const SCROLLBACK_CAPACITY: usize = 1024 * 1024; // 1 MiB

/// A single terminal session backed by a PTY.
pub struct PtySession {
    id: SessionId,
    master: Box<dyn MasterPty + Send>,
    master_write: Arc<std::sync::Mutex<Box<dyn Write + Send>>>,
    child: Arc<std::sync::Mutex<Box<dyn Child + Send + Sync>>>,
    tx: broadcast::Sender<DaemonMessage>,
    cols: u16,
    rows: u16,
    shell: Option<String>,
    cwd: Option<String>,
    workspace_id: Option<String>,
    created_at: u64,
    scrollback: Arc<std::sync::Mutex<Vec<u8>>>,
    dead: Arc<AtomicBool>,
    managed_lane: Arc<std::sync::Mutex<ManagedLaneState>>,
}

struct ManagedQueuedCommand {
    execution_id: String,
    request: ManagedCommandRequest,
    snapshot: Option<SnapshotInfo>,
}

struct ActiveManagedCommand {
    execution_id: String,
    request: ManagedCommandRequest,
    snapshot: Option<SnapshotInfo>,
    enqueued_at: Instant,
}

#[derive(Default)]
struct ManagedLaneState {
    active: Option<ActiveManagedCommand>,
    queue: VecDeque<ManagedQueuedCommand>,
}

enum CommandLifecycleMarker {
    Started(String),
    Finished(Option<i32>),
}

impl PtySession {
    /// Spawn a new PTY with the given shell and dimensions.
    pub fn spawn(
        id: SessionId,
        shell: Option<String>,
        cwd: Option<String>,
        workspace_id: Option<String>,
        env: Option<Vec<(String, String)>>,
        cols: u16,
        rows: u16,
        history: HistoryStore,
    ) -> Result<Self> {
        let pty_system = native_pty_system();

        let pair = pty_system.openpty(PtySize {
            rows,
            cols,
            pixel_width: 0,
            pixel_height: 0,
        })?;

        let configured_shell = shell;
        let shell_program = configured_shell.clone().unwrap_or_else(default_shell);
        tracing::info!(id = %id, shell = %shell_program, "starting PTY shell");
        let mut cmd = CommandBuilder::new(&shell_program);
        configure_shell_command(&shell_program, &mut cmd)?;

        if let Some(ref dir) = cwd {
            cmd.cwd(dir);
        }

        if let Some(vars) = &env {
            for (k, v) in vars {
                cmd.env(k, v);
            }
        }

        let child = Arc::new(std::sync::Mutex::new(pair.slave.spawn_command(cmd)?));
        // Drop the slave side — we only talk via the master.
        drop(pair.slave);

        let master_read = pair.master.try_clone_reader()?;
        let master_write = Arc::new(std::sync::Mutex::new(pair.master.take_writer()?));

        // Broadcast channel for output fanout to attached clients.
        let (tx, _) = broadcast::channel(256);

        let scrollback = Arc::new(std::sync::Mutex::new(Vec::with_capacity(
            SCROLLBACK_CAPACITY,
        )));

        let dead = Arc::new(AtomicBool::new(false));
        let managed_lane = Arc::new(std::sync::Mutex::new(ManagedLaneState::default()));

        let created_at = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        // Spawn a blocking read thread to pull output from the PTY.
        {
            let tx = tx.clone();
            let scrollback = scrollback.clone();
            let dead = dead.clone();
            let master_write = master_write.clone();
            let managed_lane = managed_lane.clone();
            let workspace_id = workspace_id.clone();
            let cwd = cwd.clone();
            std::thread::Builder::new()
                .name(format!("pty-reader-{id}"))
                .spawn(move || {
                    pty_reader_loop(
                        id,
                        master_read,
                        master_write,
                        tx,
                        scrollback,
                        dead,
                        managed_lane,
                        history,
                        workspace_id,
                        cwd,
                    );
                })?;
        }

        Ok(Self {
            id,
            master: pair.master,
            master_write,
            child,
            tx,
            cols,
            rows,
            shell: configured_shell.or_else(|| Some(shell_program)),
            cwd,
            workspace_id,
            created_at,
            scrollback,
            dead,
            managed_lane,
        })
    }

    /// Write raw bytes into the PTY's stdin.
    pub fn write(&mut self, data: &[u8]) -> Result<()> {
        let mut writer = self.master_write.lock().unwrap();
        writer.write_all(data)?;
        writer.flush()?;
        Ok(())
    }

    pub fn queue_managed_command(
        &mut self,
        execution_id: String,
        request: ManagedCommandRequest,
        snapshot: Option<SnapshotInfo>,
    ) -> Result<usize> {
        let mut lane = self.managed_lane.lock().unwrap();
        let immediate_dispatch = lane.active.is_some()
            && matches!(
                request.source,
                ManagedCommandSource::Agent | ManagedCommandSource::Gateway
            );

        if immediate_dispatch {
            // Assistant/gateway commands should remain responsive even if a
            // previously managed command is still running interactively.
            dispatch_managed_command(&self.master_write, &request, self.cwd.as_deref())?;
            return Ok(0);
        }

        if lane.active.is_none() {
            dispatch_managed_command(&self.master_write, &request, self.cwd.as_deref())?;
            lane.active = Some(ActiveManagedCommand {
                execution_id,
                request,
                snapshot,
                enqueued_at: Instant::now(),
            });
            Ok(0)
        } else {
            lane.queue.push_back(ManagedQueuedCommand {
                execution_id,
                request,
                snapshot,
            });
            Ok(lane.queue.len())
        }
    }

    /// Resize the PTY.
    pub fn resize(&mut self, cols: u16, rows: u16) -> Result<()> {
        self.cols = cols;
        self.rows = rows;
        self.master.resize(PtySize {
            rows,
            cols,
            pixel_width: 0,
            pixel_height: 0,
        })?;
        tracing::debug!(id = %self.id, cols, rows, "resize recorded");
        Ok(())
    }

    /// Kill the child process.
    pub fn kill(&mut self) -> Result<()> {
        self.child.lock().unwrap().kill()?;
        self.dead.store(true, Ordering::SeqCst);
        Ok(())
    }

    /// Subscribe to the output broadcast.
    pub fn subscribe(&self) -> broadcast::Receiver<DaemonMessage> {
        self.tx.subscribe()
    }

    /// Return a copy of the scrollback (optionally tail `max_lines` lines).
    pub fn scrollback(&self, max_lines: Option<usize>) -> Vec<u8> {
        let buf = self.scrollback.lock().unwrap();
        if let Some(n) = max_lines {
            // Walk backwards to find `n` newlines.
            let mut count = 0;
            let mut start = buf.len();
            for (i, &b) in buf.iter().enumerate().rev() {
                if b == b'\n' {
                    count += 1;
                    if count >= n {
                        start = i + 1;
                        break;
                    }
                }
            }
            buf[start..].to_vec()
        } else {
            buf.clone()
        }
    }

    /// Preload output bytes into session scrollback and current subscribers.
    pub fn preload_output(&mut self, data: &[u8]) {
        if data.is_empty() {
            return;
        }

        {
            let mut sb = self.scrollback.lock().unwrap();
            sb.extend_from_slice(data);
            if sb.len() > SCROLLBACK_CAPACITY {
                let excess = sb.len() - SCROLLBACK_CAPACITY;
                sb.drain(..excess);
            }
        }

        let _ = self.tx.send(DaemonMessage::Output {
            id: self.id,
            data: data.to_vec(),
        });
    }

    pub fn title(&self) -> Option<&str> {
        None // TODO: Parse OSC title sequences
    }

    pub fn id(&self) -> SessionId {
        self.id
    }

    pub fn cwd(&self) -> Option<&str> {
        self.cwd.as_deref()
    }

    /// Resolve the most current working directory for this PTY process.
    /// Falls back to the startup cwd when process cwd cannot be queried.
    pub fn resolved_cwd(&self) -> Option<String> {
        #[cfg(unix)]
        {
            if let Some(pid) = self.child.lock().unwrap().process_id() {
                let proc_cwd = format!("/proc/{pid}/cwd");
                if let Ok(path) = std::fs::read_link(proc_cwd) {
                    return Some(path.to_string_lossy().into_owned());
                }
            }
        }

        self.cwd.clone()
    }

    pub fn shell(&self) -> Option<&str> {
        self.shell.as_deref()
    }

    pub fn workspace_id(&self) -> Option<&str> {
        self.workspace_id.as_deref()
    }

    pub fn cols(&self) -> u16 {
        self.cols
    }

    pub fn rows(&self) -> u16 {
        self.rows
    }

    pub fn created_at(&self) -> u64 {
        self.created_at
    }

    pub fn is_dead(&self) -> bool {
        self.dead.load(Ordering::SeqCst)
    }
}

/// Background thread that continuously reads PTY output and fans it out.
fn pty_reader_loop(
    id: SessionId,
    mut reader: Box<dyn Read + Send>,
    master_write: Arc<std::sync::Mutex<Box<dyn Write + Send>>>,
    tx: broadcast::Sender<DaemonMessage>,
    scrollback: Arc<std::sync::Mutex<Vec<u8>>>,
    dead: Arc<AtomicBool>,
    managed_lane: Arc<std::sync::Mutex<ManagedLaneState>>,
    history: HistoryStore,
    workspace_id: Option<String>,
    cwd: Option<String>,
) {
    let mut buf = [0u8; 4096];
    let mut marker_tail = Vec::<u8>::new();
    loop {
        match reader.read(&mut buf) {
            Ok(0) => {
                tracing::info!(%id, "PTY reader reached EOF");
                notify_session_exited(id, &tx, &dead, None);
                break;
            }
            Ok(n) => {
                let mut chunk = Vec::with_capacity(marker_tail.len() + n);
                if !marker_tail.is_empty() {
                    chunk.extend_from_slice(&marker_tail);
                    marker_tail.clear();
                }
                chunk.extend_from_slice(&buf[..n]);

                let (markers, after_markers, trailing_markers) = extract_command_markers(&chunk);
                marker_tail = trailing_markers;
                let (notifications, data) = parse_osc_notifications(&after_markers);

                // Append to scrollback, trimming if over capacity.
                {
                    let mut sb = scrollback.lock().unwrap();
                    sb.extend_from_slice(&data);
                    if sb.len() > SCROLLBACK_CAPACITY {
                        let excess = sb.len() - SCROLLBACK_CAPACITY;
                        sb.drain(..excess);
                    }
                }

                // Broadcast to all attached clients (ignore if no receivers).
                let _ = tx.send(DaemonMessage::Output { id, data });

                for notification in notifications {
                    let _ = tx.send(DaemonMessage::OscNotification { id, notification });
                }

                for marker in markers {
                    match marker {
                        CommandLifecycleMarker::Started(command) => {
                            let _ = tx.send(DaemonMessage::CommandStarted {
                                id,
                                command: command.clone(),
                            });
                            let lane = managed_lane.lock().unwrap();
                            if let Some(active) = lane.active.as_ref() {
                                let _ = tx.send(DaemonMessage::ManagedCommandStarted {
                                    id,
                                    execution_id: active.execution_id.clone(),
                                    command,
                                    source: active.request.source,
                                });
                            }
                        }
                        CommandLifecycleMarker::Finished(exit_code) => {
                            let _ = tx.send(DaemonMessage::CommandFinished { id, exit_code });

                            let completed = {
                                let mut lane = managed_lane.lock().unwrap();
                                let completed = lane.active.take();
                                if let Some(next) = lane.queue.pop_front() {
                                    if let Err(error) = dispatch_managed_command(
                                        &master_write,
                                        &next.request,
                                        cwd.as_deref(),
                                    ) {
                                        tracing::error!(%id, error = %error, "failed to dispatch queued managed command");
                                    } else {
                                        lane.active = Some(ActiveManagedCommand {
                                            execution_id: next.execution_id,
                                            request: next.request,
                                            snapshot: next.snapshot,
                                            enqueued_at: Instant::now(),
                                        });
                                    }
                                }
                                completed
                            };

                            if let Some(active) = completed {
                                let duration_ms = active.enqueued_at.elapsed().as_millis() as u64;
                                let _ = tx.send(DaemonMessage::ManagedCommandFinished {
                                    id,
                                    execution_id: active.execution_id.clone(),
                                    command: active.request.command.clone(),
                                    exit_code,
                                    duration_ms: Some(duration_ms),
                                    snapshot: active.snapshot.clone(),
                                });

                                let record = ManagedHistoryRecord {
                                    execution_id: active.execution_id,
                                    session_id: id.to_string(),
                                    workspace_id: workspace_id.clone(),
                                    command: active.request.command,
                                    rationale: active.request.rationale,
                                    source: format!("{:?}", active.request.source)
                                        .to_ascii_lowercase(),
                                    exit_code,
                                    duration_ms: Some(duration_ms),
                                    snapshot_path: active
                                        .snapshot
                                        .as_ref()
                                        .map(|snapshot| snapshot.path.clone()),
                                };
                                if let Err(error) = history.record_managed_finish(&record) {
                                    tracing::error!(%id, error = %error, cwd = ?cwd, "failed to persist managed history");
                                }

                                // Auto-generate skill if a successful workflow pattern is detected
                                if record.exit_code == Some(0) {
                                    if let Ok(candidates) = history.detect_skill_candidates() {
                                        for (title, _hits) in candidates.iter().take(1) {
                                            if let Ok((_title, path)) =
                                                history.generate_skill(Some(title), Some(title))
                                            {
                                                tracing::info!(skill_path = %path, "auto-generated skill from workflow pattern");
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
            Err(e) => {
                tracing::error!(%id, error = %e, "PTY read error");
                notify_session_exited(id, &tx, &dead, None);
                break;
            }
        }
    }
}

fn dispatch_managed_command(
    master_write: &Arc<std::sync::Mutex<Box<dyn Write + Send>>>,
    request: &ManagedCommandRequest,
    fallback_cwd: Option<&str>,
) -> Result<()> {
    let command_line = if request.sandbox_enabled {
        let workspace_root = request.cwd.as_deref().or(fallback_cwd).unwrap_or(".");
        let sandbox_impl = sandbox::detect_sandbox();
        let wrapped = sandbox_impl.wrap(&request.command, workspace_root, request.allow_network);
        tracing::info!(
            sandbox = sandbox_impl.name(),
            workspace_root,
            allow_network = request.allow_network,
            "dispatching managed command with sandbox"
        );
        shell_join(&wrapped.program, &wrapped.args)
    } else {
        if request.allow_network {
            tracing::warn!("sandbox disabled for managed command; dispatching raw command");
            request.command.clone()
        } else {
            let wrapped = network::wrap_network(&request.command, request.allow_network);
            tracing::warn!("sandbox disabled for managed command; using network wrapper only");
            shell_join(&wrapped.program, &wrapped.args)
        }
    };

    let mut writer = master_write.lock().unwrap();
    writer.write_all(command_line.as_bytes())?;
    writer.write_all(b"\r")?;
    writer.flush()?;
    Ok(())
}

fn shell_join(program: &str, args: &[String]) -> String {
    let mut parts = Vec::with_capacity(args.len() + 1);
    parts.push(shell_escape(program));
    for arg in args {
        parts.push(shell_escape(arg));
    }
    parts.join(" ")
}

fn shell_escape(value: &str) -> String {
    if value.is_empty() {
        return "''".to_string();
    }
    let escaped = value.replace('\'', "'\"'\"'");
    format!("'{}'", escaped)
}

fn extract_command_markers(data: &[u8]) -> (Vec<CommandLifecycleMarker>, Vec<u8>, Vec<u8>) {
    let mut markers = Vec::new();
    let mut cleaned = Vec::with_capacity(data.len());
    let mut trailing = Vec::new();
    let mut i = 0;

    while i < data.len() {
        if i + 1 < data.len() && data[i] == 0x1b && data[i + 1] == b']' {
            let osc_start = i + 2;
            let mut end = osc_start;
            let mut terminator_len = 0;

            while end < data.len() {
                if data[end] == 0x07 {
                    terminator_len = 1;
                    break;
                }
                if end + 1 < data.len() && data[end] == 0x1b && data[end + 1] == b'\\' {
                    terminator_len = 2;
                    break;
                }
                end += 1;
            }

            if terminator_len > 0 {
                let payload = &data[osc_start..end];
                if let Ok(text) = std::str::from_utf8(payload) {
                    if let Some(command_b64) = text.strip_prefix("133;C;") {
                        let command = base64::engine::general_purpose::STANDARD
                            .decode(command_b64)
                            .ok()
                            .and_then(|bytes| String::from_utf8(bytes).ok())
                            .unwrap_or_else(|| command_b64.to_string());
                        markers.push(CommandLifecycleMarker::Started(command));
                        i = end + terminator_len;
                        continue;
                    }

                    if let Some(rest) = text.strip_prefix("133;D") {
                        let exit_code = rest
                            .trim_start_matches(';')
                            .split(';')
                            .next()
                            .and_then(|value| value.parse::<i32>().ok());
                        markers.push(CommandLifecycleMarker::Finished(exit_code));
                        i = end + terminator_len;
                        continue;
                    }
                }
            } else {
                trailing.extend_from_slice(&data[i..]);
                break;
            }
        }

        cleaned.push(data[i]);
        i += 1;
    }

    (markers, cleaned, trailing)
}

fn notify_session_exited(
    id: SessionId,
    tx: &broadcast::Sender<DaemonMessage>,
    dead: &Arc<AtomicBool>,
    exit_code: Option<i32>,
) {
    if dead
        .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
        .is_ok()
    {
        let _ = tx.send(DaemonMessage::SessionExited { id, exit_code });
    }
}

/// Determine the default shell for the current platform.
fn default_shell() -> String {
    #[cfg(windows)]
    {
        detect_windows_shell()
    }
    #[cfg(not(windows))]
    {
        std::env::var("SHELL").unwrap_or_else(|_| "/bin/bash".to_string())
    }
}

#[cfg(windows)]
fn configure_shell_command(shell_program: &str, cmd: &mut CommandBuilder) -> Result<()> {
    let shell_name = std::path::Path::new(shell_program)
        .file_name()
        .and_then(|name| name.to_str())
        .map(|name| name.to_ascii_lowercase());

    match shell_name.as_deref() {
        Some("powershell.exe") | Some("pwsh.exe") => {
            cmd.arg("-NoLogo");
            cmd.arg("-NoExit");
        }
        _ => {}
    }

    Ok(())
}

#[cfg(not(windows))]
fn configure_shell_command(shell_program: &str, cmd: &mut CommandBuilder) -> Result<()> {
    let shell_name = std::path::Path::new(shell_program)
        .file_name()
        .and_then(|name| name.to_str())
        .map(|name| name.to_ascii_lowercase());

    if matches!(shell_name.as_deref(), Some("bash")) {
        let data_dir = amux_protocol::ensure_amux_data_dir()?;
        let integration_dir = data_dir.join("shell");
        std::fs::create_dir_all(&integration_dir)?;

        let bash_rc_path = integration_dir.join("bash_amux_rc.sh");
        let bash_rc = r#"# amux shell integration for command lifecycle markers
if [ -f /etc/bash.bashrc ]; then
    . /etc/bash.bashrc
fi

if [ -f ~/.bashrc ]; then
    . ~/.bashrc
fi

__amux_precmd() {
    local exit_code=$?
    printf '\033]133;D;%s\a' "$exit_code"
}

__amux_preexec() {
    local cmd="$BASH_COMMAND"

    # Ignore integration internals and prompt hooks.
    case "$cmd" in
        __amux_precmd*|__amux_preexec*|history*|fc*|"" )
            return
            ;;
    esac

    local cmd_b64
    cmd_b64="$(printf '%s' "$cmd" | base64 | tr -d '\r\n')"
    printf '\033]133;C;%s\a' "$cmd_b64"
}

trap '__amux_preexec' DEBUG

if [ -n "${PROMPT_COMMAND:-}" ]; then
    PROMPT_COMMAND="__amux_precmd;${PROMPT_COMMAND}"
else
    PROMPT_COMMAND="__amux_precmd"
fi
"#;
        std::fs::write(&bash_rc_path, bash_rc)?;

        cmd.arg("--rcfile");
        cmd.arg(bash_rc_path.to_string_lossy().to_string());
    }

    Ok(())
}

#[cfg(windows)]
fn detect_windows_shell() -> String {
    use std::path::PathBuf;

    let mut candidates: Vec<PathBuf> = Vec::new();

    if let Some(program_files) = std::env::var_os("ProgramFiles") {
        candidates.push(
            PathBuf::from(&program_files)
                .join("PowerShell")
                .join("7")
                .join("pwsh.exe"),
        );
    }

    if let Some(local_app_data) = std::env::var_os("LOCALAPPDATA") {
        candidates.push(
            PathBuf::from(&local_app_data)
                .join("Microsoft")
                .join("WindowsApps")
                .join("pwsh.exe"),
        );
    }

    if let Some(system_root) = std::env::var_os("SystemRoot") {
        let system_root = PathBuf::from(system_root);
        candidates.push(
            system_root
                .join("System32")
                .join("WindowsPowerShell")
                .join("v1.0")
                .join("powershell.exe"),
        );
        candidates.push(system_root.join("System32").join("cmd.exe"));
    }

    if let Some(comspec) = std::env::var_os("COMSPEC") {
        candidates.push(PathBuf::from(comspec));
    }

    if let Some(path_hit) = find_in_path("pwsh.exe") {
        candidates.insert(0, path_hit);
    }

    for candidate in candidates {
        if candidate.is_file() {
            return candidate.to_string_lossy().into_owned();
        }
    }

    if let Some(path_hit) = find_in_path("powershell.exe") {
        return path_hit.to_string_lossy().into_owned();
    }

    if let Some(path_hit) = find_in_path("cmd.exe") {
        return path_hit.to_string_lossy().into_owned();
    }

    "cmd.exe".to_string()
}

#[cfg(windows)]
fn find_in_path(binary: &str) -> Option<std::path::PathBuf> {
    let path_var = std::env::var_os("PATH")?;
    for dir in std::env::split_paths(&path_var) {
        let candidate = dir.join(binary);
        if candidate.is_file() {
            return Some(candidate);
        }
    }
    None
}
