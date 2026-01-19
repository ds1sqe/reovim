//! Session driver for reovim.
//!
//! Provides traits and types for session management.
//!
//! # Overview
//!
//! This driver provides:
//!
//! - **Session types**: Per-client state ([`Session`], [`SessionId`], [`Window`])
//! - **Mode lifecycle**: Runtime mode hooks ([`SessionMode`], [`ModeError`])
//! - **Extension system**: Module per-session state ([`SessionExtension`], [`ExtensionMap`])
//! - **Empty session handling**: Startup behavior ([`EmptySessionHandler`])
//!
//! # Architecture
//!
//! ```text
//! ┌─────────────────────────────────────────────────────────────────┐
//! │ SESSION DRIVER (lib/drivers/session/) - PURE MECHANISM          │
//! │                                                                 │
//! │ Session: id, windows, mode_stack, pending_keys, extensions      │
//! │ SessionMode: id(), on_enter(), on_exit()                        │
//! │ SessionExtension: create() - module per-session state           │
//! │ EmptySessionHandler: handle() - startup behavior                │
//! └─────────────────────────────────────────────────────────────────┘
//! ```
//!
//! # Empty Session Handling
//!
//! The [`EmptySessionHandler`] trait defines how to handle sessions
//! with no buffers. Modules implement this to define policy (e.g.,
//! create a scratch buffer, show a welcome screen).
//!
//! ```ignore
//! use reovim_driver_session::{
//!     EmptySessionHandler, EmptySessionContext, EmptySessionAction
//! };
//!
//! struct MyHandler;
//!
//! impl EmptySessionHandler for MyHandler {
//!     fn handle(&self, ctx: &EmptySessionContext) -> EmptySessionAction {
//!         EmptySessionAction::CreateBuffer {
//!             name: None,
//!             content: String::new(),
//!         }
//!     }
//!     fn id(&self) -> &'static str { "my-module:handler" }
//!     fn description(&self) -> &'static str { "My handler" }
//! }
//! ```
//!
//! # Session Extension
//!
//! Modules store per-session policy state via [`SessionExtension`]:
//!
//! ```ignore
//! use reovim_driver_session::{SessionExtension, ExtensionMap};
//!
//! #[derive(Default)]
//! pub struct VimSessionState {
//!     pub pending_count: Option<usize>,
//! }
//!
//! impl SessionExtension for VimSessionState {
//!     fn create() -> Self { Self::default() }
//! }
//!
//! // Access in resolver
//! let vim = session.extensions.get_or_insert::<VimSessionState>();
//! vim.pending_count = Some(5);
//! ```

mod context;
mod empty_handler;
mod extension;
mod mode;
mod types;

// Empty session handling
pub use empty_handler::{EmptySessionAction, EmptySessionContext, EmptySessionHandler};

// Session extension system
pub use extension::{ExtensionMap, SessionExtension};

// Mode lifecycle
pub use mode::{ModeError, SessionMode};

// Session types
pub use types::{CursorPosition, KeySequence, Session, SessionId, Viewport, Window, WindowLayout};

// Session context
pub use context::SessionContext;
