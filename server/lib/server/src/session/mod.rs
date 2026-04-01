//! Session management for the server.
//!
//! A session is a named editing context (like tmux sessions).
//! Multiple clients can attach to the same session and share editor state.
//!
//! # Per-Client State (#480 Client Architecture Unification)
//!
//! Each client in a session is represented by a [`Client`] struct with:
//!
//! - `relation`: `None` (independent), `Following`, or `Sharing`
//! - `state`: `EditingState` (always present - mode, cursor, windows)
//! - `metadata`: `ClientMetadata` (type, display name, join time)
//!
//! See the [`Client`] struct for details.

mod buffer_handle;
pub(crate) mod capture;
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
pub mod token_registry;

pub use {
    buffer_handle::BufferHandle,
    capture::{CaptureError, CaptureResult, CaptureTracker, wait_for_capture},
    client::{
        Client, ClientMetadata, ClientRelation, ClientSelection, EditingState, TransitionResult,
    },
    id::{ClientId, SessionId},
    // DEPRECATED: These will be removed in a future version.
    // Use Client struct with relation field instead.
    presence::{ClientPresence, PresenceMap, SyncMode},
    registry::SessionRegistry,
    ring_buffer::{ClientEventType, ClientRingBuffer},
    session::Session,
    state::SessionState,
    syntax_state::{
        SyntaxSessionState, SyntaxStreamState, build_token_update, modification_to_syntax_edit,
    },
    token_registry::{SessionToken, TokenRegistry},
};
