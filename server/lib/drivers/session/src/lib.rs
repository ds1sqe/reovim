//! Session driver for reovim.
//!
//! Provides traits and types for session management.
//!
//! # Overview
//!
//! This driver provides:
//!
//! - **Session infrastructure**: Shared state ([`SessionShared`]) and runtime ([`SessionRuntime`])
//! - **Client identification**: [`ClientId`] for explicit client binding
//! - **Window management**: [`Window`], [`WindowLayout`], [`CursorPosition`]
//! - **Mode lifecycle**: Runtime mode hooks ([`SessionMode`], [`ModeError`])
//! - **Extension system**: Module per-session state ([`SessionExtension`], [`ExtensionMap`])
//! - **Empty session handling**: Startup behavior ([`EmptySessionHandler`])
//!
//! # Architecture (#471, #491)
//!
//! ```text
//! ┌─────────────────────────────────────────────────────────────────┐
//! │ SESSION DRIVER (lib/drivers/session/) - PURE MECHANISM          │
//! │                                                                 │
//! │ SHARED INFRASTRUCTURE:                                          │
//! │   Session { id, shared: SessionShared }                         │
//! │   SessionShared { compositor, terminal_size, active_buffer,     │
//! │                   home_mode }                                   │
//! │                                                                 │
//! │ RUNTIME (borrows shared + per-client state):                    │
//! │   SessionRuntime::new(&session, &mode_stack, &windows, ...)     │
//! │                                                                 │
//! │ PER-CLIENT TYPES (defined here, owned by server::EditingState): │
//! │   ModeStack, WindowLayout, KeySequence, ExtensionMap, Viewport  │
//! │                                                                 │
//! │ LIFECYCLE TRAITS:                                               │
//! │   SessionMode: id(), on_enter(), on_exit()                      │
//! │   SessionExtension: create() - module state factory             │
//! │   EmptySessionHandler: handle() - startup behavior              │
//! └─────────────────────────────────────────────────────────────────┘
//! ```
//!
//! # Per-Client State (#471 Phase 0)
//!
//! Per-client state (mode, cursor, selection) lives in `server::EditingState`,
//! not in this driver. The driver provides shared infrastructure only.
//! Use [`SessionRuntime::new`] with per-client state, or [`SessionRuntime::with_owner`]
//! for explicit client binding. Per-client state is now REQUIRED (no Option wrappers).
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

pub mod api;
mod empty_handler;
mod extension;
mod handler_key;
mod handler_registry;
mod mode;
mod operator_state;
mod runtime;
pub mod testing;
mod transition;
mod types;

// Empty session handling
pub use empty_handler::{EmptySessionAction, EmptySessionContext, EmptySessionHandler};

// Session extension system
pub use extension::{ExtensionMap, SessionExtension, SessionExtensionDyn, TextInputSink};

// Operator-pending state for text object communication
pub use operator_state::OperatorPendingState;

// Mode lifecycle
pub use mode::{ModeError, SessionMode};

// Session types
pub use types::{
    BootstrapState, ClientId, CursorPosition, KeySequence, Session, SessionShared, TextObjRange,
    Viewport, Window, WindowLayout,
};

// SessionContext removed in #491 - use SessionRuntime instead

// Transition types
pub use transition::{PopResult, TransitionContext};

// Session runtime (implements all API traits)
pub use runtime::SessionRuntime;

// Session API traits (re-export for convenience)
pub use api::{
    BufferApi, BufferError, ChangeTracker, CmdlinePrompt, CmdlineState, CommandApi,
    CommandExecutor, CompositorApi, CompositorError, ExtensionApi, ModeApi,
    ModeError as ApiModeError, RegisterApi, RegisterContent, Selection, SelectionMode, SessionApi,
    SessionApiDyn, StateChanges, WindowApi, WindowError, YankType,
};

// Re-export typed key and registry (Epic #417 - UniqueProvider abstraction)
pub use {handler_key::SessionHandlerKey, handler_registry::SessionHandlerRegistry};
