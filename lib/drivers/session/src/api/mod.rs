//! Session API traits for resolvers and commands.
//!
//! This module provides focused traits that resolvers and commands
//! use to interact with session state. Each trait does ONE thing well
//! (Unix philosophy).
//!
//! # Traits
//!
//! | Trait | Purpose |
//! |-------|---------|
//! | [`ModeApi`] | Mode stack operations |
//! | [`BufferApi`] | Buffer content and lifecycle |
//! | [`WindowApi`] | Window management and focus |
//! | [`CommandApi`] | Command execution |
//! | [`ExtensionApi`] | Module per-session state |
//! | [`ChangeTracker`] | Accumulated change collection |
//!
//! # Architecture
//!
//! ```text
//! ┌──────────────────────────────────────────────────────┐
//! │ MODULES (resolvers, commands)              POLICY    │
//! │   - Call session.push_mode(), session.move_cursor()  │
//! │   - Decide WHAT actions to perform                   │
//! └──────────────────────────────────────────────────────┘
//!          │ calls methods on
//!          ↓
//! ┌──────────────────────────────────────────────────────┐
//! │ SESSION API (this module)                 MECHANISM  │
//! │   - Focused traits: ModeApi, BufferApi, etc.         │
//! │   - SessionRuntime implements all traits             │
//! │   - Returns: StateChanges (what changed)             │
//! └──────────────────────────────────────────────────────┘
//!          │ returns changes to
//!          ↓
//! ┌──────────────────────────────────────────────────────┐
//! │ RUNNER                                    MECHANISM  │
//! │   - Passes SessionRuntime to resolvers               │
//! │   - Receives StateChanges                            │
//! │   - Broadcasts notifications to clients              │
//! └──────────────────────────────────────────────────────┘
//! ```
//!
//! # Usage
//!
//! Resolvers specify which APIs they need via trait bounds:
//!
//! ```ignore
//! fn resolve_with_session<S>(
//!     &self,
//!     key: &KeyEvent,
//!     state: &mut ModeState,
//!     session: &mut S,
//! ) -> ResolveResult
//! where
//!     S: ModeApi + BufferApi + CommandApi + ExtensionApi,
//! {
//!     // Only has access to the traits specified in bounds
//! }
//! ```

mod buffer;
mod changes;
mod command;
mod extension;
mod mode;
mod window;

// Buffer API
pub use buffer::{BufferApi, BufferError, Selection, SelectionMode};

// Change tracking
pub use changes::{ChangeTracker, StateChanges};

// Command API
pub use command::{CommandApi, CommandExecutor};

// Extension API
pub use extension::ExtensionApi;

// Mode API
pub use mode::{ModeApi, ModeError};

// Window API
pub use window::{WindowApi, WindowError};

// ============================================================================
// SessionApi - Combined trait for resolvers
// ============================================================================

/// Dyn-compatible session API (without extensions).
///
/// This trait bundles the dyn-compatible API traits for use with trait objects.
/// `ExtensionApi` is excluded because it has generic methods.
///
/// # Why Separate from `SessionApi`?
///
/// Rust's trait object (`dyn Trait`) requires all methods to be dyn-compatible.
/// `ExtensionApi` has generic methods (`ext<T>`, `ext_mut<T>`), so it can't be
/// part of a dyn-compatible trait.
///
/// Use `SessionApiDyn` when you need `&mut dyn SessionApiDyn` (e.g., in resolver traits).
/// Extensions are passed separately as `&mut ExtensionMap`.
///
/// # Example
///
/// ```ignore
/// fn resolve_with_session(
///     &self,
///     key: &KeyEvent,
///     state: &mut ModeState,
///     input: &ResolveInput<'_>,
///     session: &mut dyn SessionApiDyn,
///     extensions: &mut ExtensionMap,
/// ) -> ResolveResult {
///     session.push_mode(insert_mode, TransitionContext::new());
///     session.move_cursor(buffer, Position::new(0, 5));
///     ResolveResult::Completed
/// }
/// ```
pub trait SessionApiDyn: ModeApi + BufferApi + WindowApi + CommandApi + ChangeTracker {}

// Blanket implementation for SessionApiDyn
impl<T> SessionApiDyn for T where T: ModeApi + BufferApi + WindowApi + CommandApi + ChangeTracker {}

/// Full session API for generic bounds (not dyn-compatible).
///
/// This trait includes `ExtensionApi` and is used for generic bounds like
/// `where S: SessionApi`. It cannot be used as `dyn SessionApi` because
/// `ExtensionApi` has generic methods.
///
/// # Example
///
/// ```ignore
/// fn do_something<S: SessionApi>(session: &mut S) {
///     // Full access including extensions
///     let ext = session.ext_mut::<MyExtension>();
/// }
/// ```
pub trait SessionApi: SessionApiDyn + ExtensionApi {}

// Blanket implementation for SessionApi
impl<T> SessionApi for T where T: SessionApiDyn + ExtensionApi {}
