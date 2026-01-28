//! Port Manager for instance discovery and coordination.
//!
//! The manager daemon provides:
//! - Central registry for remote instance discovery
//! - Health checking and stale instance cleanup
//! - Auto-start capability when needed
//!
//! # Architecture
//!
//! The manager is **optional** for local usage. Basic `-L name` lookup works
//! via the file-based registry (Phase 2). The manager is needed for:
//! - `--host` flag (remote manager query)
//! - `reovim cli list` with enhanced info
//! - Periodic health checking
//!
//! # Auto-Start Flow
//!
//! ```text
//! Any command with --host flag
//!     ↓
//! ensure_manager_running()
//!     ↓
//! Manager alive on :12521?
//!     ├─ Yes → proceed
//!     └─ No → spawn manager process → wait for READY → proceed
//! ```

mod autostart;
mod client;
mod daemon;
mod protocol;

pub use {
    autostart::{ensure_manager_running, is_manager_alive},
    client::ManagerClient,
    daemon::ManagerDaemon,
    protocol::{ManagerMethod, ManagerRequest, ManagerResponse, ManagerResult},
};

/// Default port for the manager daemon.
pub const MANAGER_PORT: u16 = 12521;

/// Default host for the manager daemon.
pub const MANAGER_HOST: &str = "127.0.0.1";
