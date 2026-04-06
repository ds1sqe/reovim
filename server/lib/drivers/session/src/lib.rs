#![cfg_attr(coverage_nightly, allow(unused_features))]
#![cfg_attr(coverage_nightly, feature(coverage_attribute))]
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
//! │ SESSION DRIVER (server/lib/drivers/session/) - PURE MECHANISM   │
//! │                                                                 │
//! │ SHARED INFRASTRUCTURE:                                          │
//! │   Session { id, shared: SessionShared }                         │
//! │   SessionShared { compositor, global_marks, home_mode }         │
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
pub mod bridges;
mod buffer_access;
mod empty_handler;
mod extension;
mod handler_key;
mod handler_registry;
mod initial_mode;
mod jumplist;
mod leader_key;
mod mark;
mod mode;
mod notification_drain;
mod notification_queue;
mod operator_state;
mod runtime;
mod snippet_expander;
pub mod tab;
pub mod testing;
pub mod tick;
mod transition;
mod types;

pub use jumplist::{JumpEntry, Jumplist, MAX_JUMPLIST_SIZE};

// Mark types (moved from kernel in #740)
pub use mark::{Mark, MarkBank, MarkResult, SpecialMark};

// Empty session handling
pub use empty_handler::{EmptySessionAction, EmptySessionContext, EmptySessionHandler};

// Initial mode provider for cross-personality support (#623)
pub use initial_mode::InitialModeProvider;

// Leader key provider for personality-aware binding expansion (#700)
pub use leader_key::{LeaderKeyProvider, expand_leader};

// Session extension system
pub use extension::{ExtensionMap, SessionExtension, SessionExtensionDyn, TextInputSink};

// Operator-pending state for text object communication
pub use operator_state::OperatorPendingState;

// Mode lifecycle
pub use mode::{ModeError, SessionMode};

// Session types
pub use types::{
    BootstrapState, ClientContext, ClientId, CursorPosition, CursorSnapshot, KeySequence, Session,
    SessionShared, TextObjRange, Viewport, Window, WindowLayout,
};

// SessionContext removed in #491 - use SessionRuntime instead

// Transition types
pub use transition::{PopResult, TransitionContext};

// Session runtime (implements all API traits)
pub use runtime::SessionRuntime;

// Session API traits (re-export for convenience)
pub use api::{
    BufferApi, BufferError, ChangeTracker, ClipboardApi, CommandApi, CommandExecutor,
    CommandHandle, CompositorApi, CompositorError, ExtensionApi, FindCharRecord, FindCharState,
    ModeApi, ModeError as ApiModeError, RegisterApi, RegisterContent, Selection, SelectionMode,
    SessionApi, SessionApiDyn, StateChanges, WindowApi, WindowError, YankType,
};

// Tab page management (#401)
pub use tab::{TabPage, TabPageSet};

// Re-export typed key and registry (Epic #417 - UniqueProvider abstraction)
pub use {handler_key::SessionHandlerKey, handler_registry::SessionHandlerRegistry};

// Pending notification queue (#542 - cross-module decoupling, #691 - progress ops)
pub use notification_queue::{
    PendingEntry, PendingLevel, PendingNotification, PendingNotificationQueue, PendingOp,
};

// Notification drain trait (#542 - decouple completion from notification module)
pub use notification_drain::{NotificationDrain, NotificationDrainRegistry};

// Snippet expander trait (#542 - decouple completion from snippet module)
pub use snippet_expander::{SnippetExpander, SnippetExpanderRegistry};

// Tick scheduler (#546 - periodic state advancement)
pub use tick::{TickScheduler, TickSchedulerHandle};

pub use buffer_access::BufferReadAccess;
