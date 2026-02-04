//! Session management for the server.
//!
//! A session is a named editing context (like tmux sessions).
//! Multiple clients can attach to the same session and share editor state.
//!
//! # Per-Client State (Phase 11.2)
//!
//! Each client in a session has a role defined by the [`Client`] enum:
//!
//! - **Owner**: Has own editing state (mode, cursor, etc.)
//! - **Follow**: Read-only spectator, sees target's state
//! - **Share**: Bidirectional co-edit with owner
//!
//! See the [`Client`] enum for details.

mod capture;
mod client;
pub mod crash_dump;
mod id;
mod presence;
mod registry;
pub mod ring_buffer;
#[allow(clippy::module_inception)]
mod session;
mod state;
mod syntax_state;

pub use {
    capture::{CaptureError, CaptureResult, CaptureTracker, wait_for_capture},
    client::{Client, ClientSelection, EditingState},
    id::{ClientId, SessionId},
    presence::{ClientPresence, PresenceMap, SyncMode},
    registry::SessionRegistry,
    ring_buffer::ClientRingBuffer,
    session::Session,
    state::SessionState,
    syntax_state::SyntaxSessionState,
};
