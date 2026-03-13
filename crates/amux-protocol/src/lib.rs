//! tamux-protocol: Shared message types and codec for daemon <-> client IPC.
//!
//! This crate defines the wire protocol used between the tamux daemon and any
//! client (Tauri UI, CLI, future multiplayer connections). Messages are
//! length-prefixed bincode frames sent over the configured IPC transport.

mod codec;
mod config;
mod messages;

pub use codec::{AmuxCodec, DaemonCodec};
pub use config::{
	amux_data_dir,
	default_tcp_addr,
	ensure_amux_data_dir,
	log_file_path,
	AmuxConfig,
	DEFAULT_TCP_HOST,
	DEFAULT_TCP_PORT,
};
pub use messages::*;
