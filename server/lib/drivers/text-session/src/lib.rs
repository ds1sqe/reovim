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
//! use reovim_driver_text_session::{
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
//! use reovim_driver_text_session::{SessionExtension, ExtensionMap};
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
mod change_bridge;
mod jumplist;
mod key_dispatch;
mod mark;
mod notification_drain;
mod operator_state;
mod runtime;
mod snippet_expander;
pub mod tab;
pub mod testing;
mod text_client_state;
mod text_content;
mod text_cursor;
mod text_domain;
pub mod tick;
mod transition;
mod types;

// Re-export all subsys-session contracts (types + traits, zero domain deps).
// This provides full backwards compatibility: all items previously defined in
// this crate are still accessible at `reovim_driver_text_session::*`.
pub use reovim_subsys_session::*;

pub use jumplist::{JumpEntry, Jumplist, MAX_JUMPLIST_SIZE};

// Mark types (moved from kernel in #740)
pub use mark::{Mark, MarkBank, MarkResult, SpecialMark};

// Operator-pending state for text object communication
pub use operator_state::OperatorPendingState;

// Session types (remaining types not in subsys-session)
pub use types::{
    BootstrapState, ClientContext, CursorPosition, Session, SessionShared, TextObjRange, Viewport,
    Window, WindowLayout,
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

// Domain-text re-exports for server layer access via driver path.
// The server depends on this driver (not on reovim-domain-text directly).
pub use reovim_domain_text::{HistoryRing, RegisterBank};

// Tab page management (#401)
pub use tab::{TabPage, TabPageSet};

// Notification drain trait (#542 - decouple completion from notification module)
pub use notification_drain::{NotificationDrain, NotificationDrainRegistry};

// Snippet expander trait (#542 - decouple completion from snippet module)
pub use snippet_expander::{SnippetExpander, SnippetExpanderRegistry};

pub use buffer_access::BufferReadAccess;

// Key dispatch provider trait (sub-plan 05 Phase 1)
pub use key_dispatch::TextKeyDispatchProvider;

// StateChanges → ChangeSet bridge (sub-plan 03)
pub use change_bridge::{state_changes_from_change_set, state_changes_to_change_set};

// Text-domain client state bundle (sub-plan 03).
// Transitional: wired into TextDomainDriver internally. Sub-plan 05 migrates
// server's EditingState to use this type via ExtensionMap.
pub use text_client_state::TextClientState;

// Text-domain content provider (sub-plan 03)
pub use text_content::TextContentProvider;

// Text domain driver (sub-plan 03)
pub use text_domain::TextDomainDriver;

// Text-domain Position/Cursor implementations (sub-plan 03)
pub use text_cursor::{
    SelectionData, TEXT_CURSOR_INNER_ID, TEXT_POSITION_INNER_ID, TextCursor, TextCursorCodec,
    TextPosition, TextPositionCodec,
};

// Text buffer registry for session-layer text access (#740)
