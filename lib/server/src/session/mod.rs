//! Session management for the server.
//!
//! A session is a named editing context (like tmux sessions).
//! Multiple clients can attach to the same session and share editor state.

mod id;
mod registry;
#[allow(clippy::module_inception)]
mod session;
mod state;

pub use {
    id::{ClientId, SessionId},
    registry::SessionRegistry,
    session::Session,
    state::SessionState,
};
