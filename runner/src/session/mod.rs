//! Session management for the reovim server.
//!
//! This module provides the session abstraction - named editing contexts
//! that multiple clients can attach to (like tmux sessions).
//!
//! # Architecture
//!
//! ```text
//! ┌─────────────────────────────────────────────────────────┐
//! │                  SessionRegistry                         │
//! │  ├── Session "default"                                  │
//! │  │   └── SessionState (AppState + Registries)           │
//! │  ├── Session "project-x"                                │
//! │  │   └── SessionState (AppState + Registries)           │
//! │  └── next_client_id: AtomicU64                          │
//! └─────────────────────────────────────────────────────────┘
//! ```
//!
//! # Concurrency Model
//!
//! Following `docs/reference/concurrency.md`:
//!
//! | Level | Component | Lock Type |
//! |-------|-----------|-----------|
//! | 0 | `SessionRegistry` | Lock-free (`ArcSwap`) |
//! | 0 | Client ID gen | Lock-free (`AtomicU64`) |
//! | 1 | Session state | `tokio::sync::RwLock` |
//!
//! # Example
//!
//! ```ignore
//! use runner::session::{SessionRegistry, Session, SessionId};
//! use reovim_kernel::api::v1::{KernelContext, ModeId, ModuleId};
//!
//! // Create the session registry
//! let registry = SessionRegistry::new();
//!
//! // Create a new session
//! let session = Session::new(
//!     SessionId::new("default"),
//!     KernelContext::default(),
//!     ModeId::new(ModuleId::new("editor"), "normal"),
//! );
//!
//! // Register it
//! registry.insert(session);
//!
//! // Lock-free lookup
//! if let Some(session) = registry.get(&SessionId::default()) {
//!     // Process keys...
//!     let mode = session.current_mode().await;
//! }
//! ```

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
