//! Session management for the server.
//!
//! A session is a named editing context (like tmux sessions).
//! Multiple clients can attach to the same session and share editor state.

mod capture;
mod id;
mod registry;
#[allow(clippy::module_inception)]
mod session;
mod state;
mod syntax_state;

pub use {
    capture::{CaptureError, CaptureResult, CaptureTracker, wait_for_capture},
    id::{ClientId, SessionId},
    registry::SessionRegistry,
    session::Session,
    state::SessionState,
    syntax_state::SyntaxSessionState,
};
