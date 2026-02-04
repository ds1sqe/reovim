//! Mode key resolver trait and types.
//!
//! This module defines how modes resolve key events into commands or transitions.
//! Resolvers provide the policy layer for key handling - the kernel/runner only
//! provides the mechanism (dispatch, mode stack, command execution).
//!
//! # Architecture
//!
//! The resolver system separates mechanism from policy:
//!
//! | Layer | Responsibility |
//! |-------|---------------|
//! | Kernel | Mechanism: `ModeId`, `ModeStack`, `CommandId`, `Position` |
//! | Input Driver (this) | Contract: `ModeKeyResolver` trait, result types |
//! | Modules | Policy: `VimNormalResolver`, `VimInsertResolver`, etc. |
//! | Runner | Dispatch: get resolver → `resolve()` → execute result |
//!
//! # Different Editing Styles
//!
//! The same kernel/runner supports different editing paradigms:
//!
//! | Style | Resolver Behavior |
//! |-------|-------------------|
//! | Vim | Counts, operators+motions, mode stacking |
//! | Emacs | C-x prefix chains, M-x commands |
//! | Kakoune | Object-verb (select then act) |
//! | CUA | Ctrl+C/V/X, Shift+arrows |
//!
//! # Example Flow: `dw` (delete word)
//!
//! ```text
//! 1. Normal mode receives 'd'
//!    └─ VimNormalResolver.resolve('d', state)
//!    └─ Returns: ModeTransition::Set { mode: "vim:delete" }
//!
//! 2. EventLoop sets delete mode
//!
//! 3. Delete mode receives 'w'
//!    └─ VimDeleteResolver.resolve('w', state)
//!    └─ Dispatches word motion, captures range
//!    └─ Returns: Delete text + ModeTransition::Set { mode: "vim:normal" }
//!
//! 4. EventLoop returns to normal mode
//! ```

use std::{any::TypeId, collections::HashMap};

use {
    reovim_driver_session::{ExtensionMap, SessionExtension, TextInputSink},
    reovim_kernel::api::v1::{BufferId, CommandId, ModeId, ModeStack, Position},
};

use crate::{KeyEvent, KeySequence, KeymapQuery};

// Re-export transition types and session API from session driver (canonical source)
pub use reovim_driver_session::{PopResult, SessionApi, SessionApiDyn, TransitionContext};

// ============================================================================
// OperatorArgs - Shared count/register fields (#391)
// ============================================================================

/// Common arguments for operator operations.
///
/// Extracted to avoid field duplication across context types (DRY principle).
/// Used by `TransitionContext` and `ResolveContext`.
///
/// # Example
///
/// ```
/// use reovim_driver_input::OperatorArgs;
///
/// // Create with defaults
/// let args = OperatorArgs::new();
/// assert_eq!(args.effective_count(), 1); // Default count is 1
///
/// // Builder pattern
/// let args = OperatorArgs::new().count(3).register('a');
/// assert_eq!(args.count, Some(3));
/// assert_eq!(args.register, Some('a'));
///
/// // From count shorthand
/// let args = OperatorArgs::with_count(5);
/// assert_eq!(args.effective_count(), 5);
/// ```
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct OperatorArgs {
    /// Count prefix (e.g., 3 in `3j` or `2d3w`).
    ///
    /// Counts can be combined across mode transitions: `2d3w` = 6 words.
    pub count: Option<usize>,

    /// Register for the operation (e.g., `a` in `"ayw`).
    ///
    /// In vim, registers store yanked/deleted text.
    pub register: Option<char>,
}

impl OperatorArgs {
    /// Create a new empty operator args.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            count: None,
            register: None,
        }
    }

    /// Create operator args with a count.
    #[must_use]
    pub const fn with_count(count: usize) -> Self {
        Self {
            count: Some(count),
            register: None,
        }
    }

    /// Set the count (builder pattern).
    #[must_use]
    pub const fn count(mut self, count: usize) -> Self {
        self.count = Some(count);
        self
    }

    /// Set the register (builder pattern).
    #[must_use]
    pub const fn register(mut self, reg: char) -> Self {
        self.register = Some(reg);
        self
    }

    /// Get the effective count (default to 1 if not set).
    ///
    /// In vim, an unspecified count means "1", not "none".
    #[must_use]
    pub const fn effective_count(&self) -> usize {
        match self.count {
            Some(c) => c,
            None => 1,
        }
    }

    /// Check if any arguments are set.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.count.is_none() && self.register.is_none()
    }
}

// ============================================================================
// ResolveInput - Input context for resolvers
// ============================================================================

/// Input context passed to resolvers for key resolution.
///
/// This struct provides resolvers with access to:
/// - The current key sequence being resolved
/// - The current mode
/// - Access to keymap queries (via `KeymapQuery` trait)
///
/// # Mechanism vs Policy
///
/// `ResolveInput` enables the mechanism/policy separation:
/// - **Mechanism**: The keymap registry reports FACTS via `keymap.query()`
/// - **Policy**: The resolver decides what to DO with those facts
///
/// For example, given `KeyLookupState::ExactWithLonger`:
/// - **Vim policy**: Wait for more keys (dd might follow d)
/// - **Eager policy**: Execute immediately
///
/// # Example
///
/// ```ignore
/// use reovim_driver_input::{ResolveInput, KeyLookupState, ResolveResult};
///
/// fn resolve(input: &ResolveInput<'_>) -> ResolveResult {
///     // Query the keymap for facts
///     let state = input.keymap.query(input.mode, input.keys);
///
///     // Apply policy to the facts
///     match state {
///         KeyLookupState::ExactWithLonger { exact } => {
///             // Vim: wait for more keys
///             ResolveResult::Pending
///         }
///         KeyLookupState::ExactOnly(cmd) => {
///             // Execute the command
///             ResolveResult::Execute(cmd, ResolveContext::new())
///         }
///         // ...
///     }
/// }
/// ```
pub struct ResolveInput<'a> {
    /// The accumulated key sequence being resolved.
    pub keys: &'a KeySequence,

    /// The current mode.
    pub mode: &'a ModeId,

    /// Access to keymap queries.
    ///
    /// Use `keymap.query(mode, keys)` to get facts about what bindings exist.
    pub keymap: &'a dyn KeymapQuery,

    /// Optional access to register bank (Epic #465 Phase 8D - Macro Playback).
    ///
    /// Provides read/write access to registers for macro recording/playback.
    /// This is optional because not all resolve contexts need register access.
    pub registers: Option<
        &'a std::sync::Arc<reovim_kernel::api::v1::RwLock<reovim_kernel::api::v1::RegisterBank>>,
    >,
}

impl<'a> ResolveInput<'a> {
    /// Create a new resolve input context.
    #[must_use]
    pub const fn new(keys: &'a KeySequence, mode: &'a ModeId, keymap: &'a dyn KeymapQuery) -> Self {
        Self {
            keys,
            mode,
            keymap,
            registers: None,
        }
    }

    /// Create a resolve input context with register access.
    #[must_use]
    pub const fn with_registers(
        keys: &'a KeySequence,
        mode: &'a ModeId,
        keymap: &'a dyn KeymapQuery,
        registers: &'a std::sync::Arc<
            reovim_kernel::api::v1::RwLock<reovim_kernel::api::v1::RegisterBank>,
        >,
    ) -> Self {
        Self {
            keys,
            mode,
            keymap,
            registers: Some(registers),
        }
    }
}

// ============================================================================
// ModeKeyResolver Trait
// ============================================================================

/// Trait for resolving key events in a specific mode.
///
/// Resolvers have full control over how keys are interpreted:
/// - Count prefixes (1-9 in Vim, none in Emacs)
/// - Register selection (" in Vim)
/// - Multi-key sequences (gg, <C-w>h)
/// - Operator-pending state (d awaiting motion)
///
/// Each editing style implements different resolvers:
/// - Vim: `VimNormalResolver`, `VimInsertResolver`, `VimDeleteResolver`, `VimYankResolver`, `VimChangeResolver`
/// - Emacs: `EmacsResolver` (single mode with prefix handling)
/// - etc.
///
/// # Object Safety
///
/// This trait is object-safe and requires `Send + Sync` for use in
/// multi-threaded server contexts.
///
/// # Example
///
/// ```ignore
/// use reovim_driver_input::{ModeKeyResolver, ResolveResult, KeyEvent, ModeState};
/// use reovim_kernel::api::v1::ModeId;
///
/// struct MyResolver {
///     mode: ModeId,
/// }
///
/// impl ModeKeyResolver for MyResolver {
///     fn resolve(&self, key: &KeyEvent, state: &mut ModeState) -> ResolveResult {
///         // Handle key based on mode-specific logic
///         ResolveResult::NotHandled
///     }
///
///     fn mode_id(&self) -> &ModeId {
///         &self.mode
///     }
/// }
/// ```
pub trait ModeKeyResolver: Send + Sync {
    /// Process a key event in this mode's context (legacy API).
    ///
    /// **Deprecated**: Use `resolve_with_keymap` instead for access to keymap queries.
    ///
    /// The resolver has full control over interpretation:
    /// - Accumulate counts, registers, pending sequences
    /// - Execute commands
    /// - Request mode transitions
    ///
    /// # Arguments
    ///
    /// * `key` - The key event to process
    /// * `state` - Mutable access to shared mode state
    ///
    /// # Returns
    ///
    /// A `ResolveResult` indicating what action to take.
    #[deprecated(since = "0.9.5", note = "Override resolve_with_keymap() instead")]
    fn resolve(&self, _key: &KeyEvent, _state: &mut ModeState) -> ResolveResult {
        panic!("resolve() is removed - implement resolve_with_keymap() instead")
    }

    /// Process a key event with access to keymap queries.
    ///
    /// This is the preferred method for resolvers that need to query keybindings.
    /// It enables mechanism/policy separation:
    /// - Query `input.keymap.query()` to get FACTS about what bindings exist
    /// - Apply your own POLICY to decide what to do
    ///
    /// # Arguments
    ///
    /// * `key` - The key event to process
    /// * `state` - Mutable access to shared mode state
    /// * `input` - Input context with keymap access
    ///
    /// # Returns
    ///
    /// A `ResolveResult` indicating what action to take.
    ///
    /// # Default Implementation
    ///
    /// Falls back to `resolve()` for backward compatibility with existing resolvers.
    /// Override this method to use keymap queries.
    ///
    /// # Example
    ///
    /// ```ignore
    /// fn resolve_with_keymap(
    ///     &self,
    ///     key: &KeyEvent,
    ///     state: &mut ModeState,
    ///     input: &ResolveInput<'_>,
    /// ) -> ResolveResult {
    ///     // Get facts about what bindings exist
    ///     let lookup_state = input.keymap.query(input.mode, input.keys);
    ///
    ///     // Apply Vim policy: wait for longer sequences
    ///     match lookup_state {
    ///         KeyLookupState::ExactWithLonger { .. } => ResolveResult::Pending,
    ///         KeyLookupState::ExactOnly(cmd) => {
    ///             ResolveResult::Execute(cmd, ResolveContext::new())
    ///         }
    ///         // ...
    ///     }
    /// }
    /// ```
    #[allow(deprecated)]
    fn resolve_with_keymap(
        &self,
        key: &KeyEvent,
        state: &mut ModeState,
        _input: &ResolveInput<'_>,
    ) -> ResolveResult {
        // Default: delegate to legacy resolve() for backward compatibility
        // Note: Resolvers should override this method instead
        self.resolve(key, state)
    }

    /// Process a key event with access to keymap queries AND session extensions.
    ///
    /// This is the preferred method for resolvers that need to access per-session
    /// module state (e.g., `VimSessionState` for pending operators, find-char state).
    ///
    /// # Architecture (Epic #385)
    ///
    /// This method enables the proper mechanism/policy separation:
    /// - **Mechanism**: Runner routes keys to resolvers, provides extension access
    /// - **Policy**: Vim resolver stores/reads `VimSessionState` in extensions
    ///
    /// By passing `extensions` as a mutable reference, resolvers can:
    /// - Read pending operator state
    /// - Set pending char state (for f/t/r commands)
    /// - Execute operators directly when motion provides range
    /// - All without returning vim-specific `CommandResult` variants
    ///
    /// # Arguments
    ///
    /// * `key` - The key event to process
    /// * `state` - Mutable access to shared mode state
    /// * `input` - Input context with keymap access
    /// * `extensions` - Per-session extension storage for module state
    ///
    /// # Returns
    ///
    /// A `ResolveResult` indicating what action to take.
    ///
    /// # Default Implementation
    ///
    /// Falls back to `resolve_with_keymap()` for backward compatibility.
    /// Override this method to use session extensions.
    ///
    /// # Example
    ///
    /// ```ignore
    /// use reovim_module_vim::VimSessionState;
    ///
    /// fn resolve_with_extensions(
    ///     &self,
    ///     key: &KeyEvent,
    ///     state: &mut ModeState,
    ///     input: &ResolveInput<'_>,
    ///     extensions: &mut ExtensionMap,
    /// ) -> ResolveResult {
    ///     // Get vim-specific state
    ///     let vim = extensions.get_or_insert::<VimSessionState>();
    ///
    ///     // Check for pending operator
    ///     if let Some(pending) = &vim.pending_operator {
    ///         // Handle operator-pending logic
    ///     }
    ///
    ///     // ... rest of resolution logic
    /// }
    /// ```
    fn resolve_with_extensions(
        &self,
        key: &KeyEvent,
        state: &mut ModeState,
        input: &ResolveInput<'_>,
        _extensions: &mut ExtensionMap,
    ) -> ResolveResult {
        // Default: delegate to resolve_with_keymap for backward compatibility
        self.resolve_with_keymap(key, state, input)
    }

    /// Process a key event with full session API access.
    ///
    /// This is the preferred method for resolvers that need direct state manipulation.
    /// Instead of returning commands for the runner to execute, resolvers can perform
    /// operations directly via the session API and return `ResolveResult::Completed`.
    ///
    /// # Architecture (Epic #393)
    ///
    /// This method completes the mechanism/policy separation:
    /// - **Mechanism (runner)**: Creates `SessionRuntime`, routes keys to resolvers
    /// - **Policy (resolvers)**: Perform actions directly via `session.*` methods
    ///
    /// # Arguments
    ///
    /// * `key` - The key event to process
    /// * `state` - Mutable access to shared mode state
    /// * `input` - Input context with keymap access
    /// * `session` - Dyn-compatible session API (mode, buffer, window, command, changes)
    /// * `extensions` - Per-session extension storage for module state
    ///
    /// # Why Separate `extensions`?
    ///
    /// `ExtensionApi` has generic methods (`ext<T>`, `ext_mut<T>`), making it
    /// incompatible with trait objects. By passing extensions separately, we preserve
    /// dyn-compatibility for `ModeKeyResolver` while still providing full access.
    ///
    /// # Returns
    ///
    /// A `ResolveResult` indicating what action to take:
    /// - `Completed` - Resolver handled everything via `SessionApi`
    /// - Other variants - Runner should handle as before
    ///
    /// # Default Implementation
    ///
    /// Falls back to `resolve_with_extensions()` for backward compatibility.
    ///
    /// # Example
    ///
    /// ```ignore
    /// use reovim_driver_session::{SessionApiDyn, ModeApi, BufferApi, ExtensionMap};
    ///
    /// fn resolve_with_session(
    ///     &self,
    ///     key: &KeyEvent,
    ///     state: &mut ModeState,
    ///     input: &ResolveInput<'_>,
    ///     session: &mut dyn SessionApiDyn,
    ///     extensions: &mut ExtensionMap,
    /// ) -> ResolveResult {
    ///     // Directly manipulate state via session
    ///     session.push_mode(insert_mode, TransitionContext::new());
    ///     session.move_cursor(buffer, Position::new(0, 5));
    ///
    ///     // Access module state via extensions
    ///     let vim = extensions.get_or_insert::<VimSessionState>();
    ///     vim.pending_count = None;
    ///
    ///     // Tell runner: "I'm done, just broadcast the changes"
    ///     ResolveResult::Completed
    /// }
    /// ```
    fn resolve_with_session(
        &self,
        key: &KeyEvent,
        state: &mut ModeState,
        input: &ResolveInput<'_>,
        _session: &mut dyn SessionApiDyn,
        extensions: &mut ExtensionMap,
    ) -> ResolveResult {
        // Default: delegate to resolve_with_extensions for backward compatibility.
        // Resolvers that need session access should override this method.
        self.resolve_with_extensions(key, state, input, extensions)
    }

    /// Hook called after a command executes successfully.
    ///
    /// Called by the runner after executing a command that returns `Success`.
    /// This allows resolvers to complete pending operations.
    ///
    /// # Architecture (Issue #388, Epic #415)
    ///
    /// This method keeps vim-specific operator completion logic in the vim module
    /// instead of the runner, maintaining the mechanism/policy separation:
    /// - **Runner (mechanism)**: Executes commands, calls this hook after success
    /// - **Resolver (policy)**: Checks its stored state, completes pending operations
    ///
    /// Per #388, `CommandResult` has no `Motion` variant. Instead, the resolver
    /// stores motion type info in `VimSessionState` BEFORE dispatching the motion,
    /// then uses this hook to complete the operator.
    ///
    /// # Example Use Case: `dw` (delete word)
    ///
    /// 1. `d` key → Normal resolver stores pending operator, pushes operator-pending mode
    /// 2. `w` key → Operator-pending resolver:
    ///    - Stores start position in `VimSessionState`
    ///    - Stores motion type (characterwise for 'w') in `VimSessionState`
    ///    - Returns `Execute(word-forward)`
    /// 3. Runner executes motion → `CommandResult::Success`
    /// 4. Runner calls `on_command_complete(session, extensions)`
    /// 5. Operator-pending resolver:
    ///    - Checks `VimSessionState` for pending motion
    ///    - Gets end position from session
    ///    - Builds `PopResult::ExecuteCommand` with operator and args
    ///    - Returns `Some(ModeTransition::Pop { result: ExecuteCommand })`
    /// 6. Runner handles the mode transition (executes the operator command)
    ///
    /// # Arguments
    ///
    /// * `session` - Session API for cursor/buffer access
    /// * `extensions` - Per-session extension storage (e.g., `VimSessionState`)
    ///
    /// # Returns
    ///
    /// * `Some(ModeTransition)` - Resolver wants to complete a pending operation
    /// * `None` - No pending operation to complete
    ///
    /// # Default Implementation
    ///
    /// Returns `None` - most modes don't have pending operations.
    fn on_command_complete(
        &self,
        _session: &mut dyn SessionApiDyn,
        _extensions: &mut ExtensionMap,
    ) -> Option<ModeTransition> {
        // Default: no pending operation to complete
        None
    }

    /// Which mode this resolver handles.
    fn mode_id(&self) -> &ModeId;

    /// Optional parent mode to try if we return `NotHandled`.
    ///
    /// This enables mode inheritance. For example, operator-pending mode
    /// might inherit from normal mode for motion keys.
    fn inherits_from(&self) -> Option<&ModeId> {
        None
    }

    /// Reset any internal state (counts, pending keys, etc.).
    ///
    /// Called when:
    /// - Mode is exited
    /// - Escape is pressed
    /// - An error occurs
    fn reset(&mut self) {}
}

// ============================================================================
// InputTarget - Where to route character input (#482)
// ============================================================================

/// Target for character input routing.
///
/// When a resolver returns `InsertChar`, this enum specifies where the
/// character should be inserted. This eliminates string-based mode detection
/// (like `mode_name.contains("command")`) in favor of explicit, type-safe routing.
///
/// # Architecture (#482)
///
/// | Layer | Responsibility |
/// |-------|----------------|
/// | Resolver | Returns `ResolveResult::InsertChar { char, target }` |
/// | Runner | Routes char to target (buffer or extension) |
/// | Extension | Receives char via `TextInputSink::insert_char()` |
///
/// # Example
///
/// ```ignore
/// use reovim_driver_input::{ResolveResult, InputTarget};
/// use reovim_driver_session::api::CmdlineState;
///
/// // Insert into buffer (default for insert mode)
/// ResolveResult::insert_char('x');
///
/// // Insert into command-line extension
/// ResolveResult::insert_char_to::<CmdlineState>('x');
/// ```
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum InputTarget {
    /// Insert into the active buffer at cursor position.
    ///
    /// This is the default target, used by insert mode, replace mode, etc.
    #[default]
    Buffer,

    /// Insert into a session extension that implements `TextInputSink`.
    ///
    /// The `TypeId` identifies the extension type. The runner looks up
    /// the extension in `ExtensionMap` and calls `insert_char()` on it.
    ///
    /// # Example
    ///
    /// ```ignore
    /// // Command-line mode routes to CmdlineState
    /// InputTarget::extension::<CmdlineState>()
    /// ```
    Extension(TypeId),
}

impl InputTarget {
    /// Create an extension target for the given type.
    ///
    /// # Example
    ///
    /// ```ignore
    /// use reovim_driver_session::api::CmdlineState;
    ///
    /// let target = InputTarget::extension::<CmdlineState>();
    /// ```
    #[must_use]
    pub const fn extension<T: SessionExtension + TextInputSink>() -> Self {
        Self::Extension(TypeId::of::<T>())
    }
}

// ============================================================================
// ResolveResult
// ============================================================================

/// Result of resolving a key event.
///
/// This enum represents all possible outcomes of key resolution:
/// - Execute a command with context
/// - Wait for more keys (pending sequence)
/// - Pass to parent mode
/// - Insert a character
/// - Request a mode transition
/// - Completed (resolver executed everything via `SessionApi`)
#[derive(Debug, Clone)]
pub enum ResolveResult {
    /// Execute this command with the given context.
    ///
    /// The command will be looked up in the command registry and executed
    /// with the provided context (count, register, etc.).
    Execute(CommandId, ResolveContext),

    /// Key is part of a pending sequence, wait for more.
    ///
    /// Used for multi-key commands like `gg`, `<C-w>h`, etc.
    /// The key has been accumulated; more keys are needed.
    Pending,

    /// This mode doesn't handle the key, try parent mode.
    ///
    /// If `inherits_from()` returns a parent mode, the runner should
    /// try that mode's resolver next.
    NotHandled,

    /// Insert this character into the specified target.
    ///
    /// For input-accepting modes (insert, command-line, search).
    /// The `target` specifies where to insert:
    /// - `InputTarget::Buffer` - Insert at cursor position in active buffer
    /// - `InputTarget::Extension(type_id)` - Insert into session extension
    ///
    /// # Helpers
    ///
    /// Use the helper methods for ergonomic construction:
    /// - `ResolveResult::insert_char(c)` - Insert into buffer (default)
    /// - `ResolveResult::insert_char_to::<T>(c)` - Insert into extension
    ///
    /// # Example
    ///
    /// ```ignore
    /// use reovim_driver_input::ResolveResult;
    /// use reovim_driver_session::api::CmdlineState;
    ///
    /// // Insert mode → buffer
    /// ResolveResult::insert_char('x');
    ///
    /// // Command-line mode → CmdlineState extension
    /// ResolveResult::insert_char_to::<CmdlineState>(':');
    /// ```
    InsertChar {
        /// The character to insert.
        char: char,
        /// Where to insert (buffer or extension).
        target: InputTarget,
    },

    /// Request a mode transition.
    ///
    /// Push, pop, or replace the current mode on the mode stack.
    ModeTransition(ModeTransition),

    /// Resolver already executed everything via `SessionApi`.
    ///
    /// The resolver used `SessionRuntime` methods directly to perform
    /// all necessary operations (mode changes, cursor moves, buffer edits, etc.).
    /// Runner should just take accumulated changes via `take_changes()` and
    /// broadcast notifications to clients.
    ///
    /// This variant is used with the new `resolve_with_session` pattern
    /// where resolvers receive `&mut impl SessionApi` and can act directly.
    ///
    /// # Example
    ///
    /// ```ignore
    /// fn resolve_with_session<S: ModeApi + BufferApi>(
    ///     &self,
    ///     key: &KeyEvent,
    ///     state: &mut ModeState,
    ///     session: &mut S,
    /// ) -> ResolveResult {
    ///     // Directly modify state via SessionApi
    ///     session.push_mode(insert_mode, TransitionContext::new());
    ///
    ///     // Tell runner: "I'm done, just broadcast the changes"
    ///     ResolveResult::Completed
    /// }
    /// ```
    Completed,

    /// Inject keys into the input queue (Epic #465 Phase 8D - Macro Playback).
    ///
    /// Used by macro playback to inject recorded key sequences. The runner
    /// should process these keys as if they were typed by the user.
    ///
    /// # Fields
    ///
    /// - `keys`: The key sequence to inject
    /// - `exit_macro_playback`: If true, call `VimSessionState::exit_macro_playback()`
    ///   after all injected keys are processed
    ///
    /// # Example
    ///
    /// ```ignore
    /// // Play macro from register 'a'
    /// ResolveResult::InjectKeys {
    ///     keys: vec![KeyEvent::new(KeyCode::Char('d')), KeyEvent::new(KeyCode::Char('w'))],
    ///     exit_macro_playback: true,
    /// }
    /// ```
    InjectKeys {
        /// Keys to inject into the input queue.
        keys: Vec<crate::KeyEvent>,
        /// Whether to call `exit_macro_playback()` after processing.
        exit_macro_playback: bool,
    },
}

impl ResolveResult {
    /// Create an `InsertChar` result that inserts into the buffer (default).
    ///
    /// Use this for insert mode, replace mode, and other modes that edit
    /// the active buffer.
    ///
    /// # Example
    ///
    /// ```ignore
    /// use reovim_driver_input::ResolveResult;
    ///
    /// // In insert mode resolver
    /// ResolveResult::insert_char('x')
    /// ```
    #[must_use]
    pub const fn insert_char(c: char) -> Self {
        Self::InsertChar {
            char: c,
            target: InputTarget::Buffer,
        }
    }

    /// Create an `InsertChar` result that inserts into a session extension.
    ///
    /// Use this for modes that input into extension state, such as command-line
    /// mode (into `CmdlineState`) or search mode (into search state).
    ///
    /// The extension must implement both `SessionExtension` and `TextInputSink`.
    ///
    /// # Example
    ///
    /// ```ignore
    /// use reovim_driver_input::ResolveResult;
    /// use reovim_driver_session::api::CmdlineState;
    ///
    /// // In command-line resolver
    /// ResolveResult::insert_char_to::<CmdlineState>(':')
    /// ```
    #[must_use]
    pub const fn insert_char_to<T: SessionExtension + TextInputSink>(c: char) -> Self {
        Self::InsertChar {
            char: c,
            target: InputTarget::extension::<T>(),
        }
    }
}

// ============================================================================
// ResolveContext
// ============================================================================

/// Context passed when executing a command.
///
/// Contains all the information gathered during key resolution:
/// - Count prefix (e.g., 3 in `3j`)
/// - Register (e.g., `a` in `"ayw`)
/// - Key sequence that triggered the command
/// - Arbitrary metadata for complex commands
#[derive(Debug, Clone, Default)]
pub struct ResolveContext {
    /// Count prefix (e.g., 3 in `3j`).
    pub count: Option<usize>,

    /// Register for the operation (e.g., `a` in `"ayw`).
    pub register: Option<char>,

    /// The key sequence that triggered this command.
    pub keys: KeySequence,

    /// Arbitrary metadata for command-specific data.
    ///
    /// Used for passing operator-specific information, motion flags, etc.
    pub metadata: HashMap<String, ArgValue>,
}

impl ResolveContext {
    /// Create a new empty context.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Create a context with just a count.
    #[must_use]
    pub fn with_count(count: usize) -> Self {
        Self {
            count: Some(count),
            ..Default::default()
        }
    }

    /// Set the count.
    #[must_use]
    pub const fn count(mut self, count: usize) -> Self {
        self.count = Some(count);
        self
    }

    /// Set the register.
    #[must_use]
    pub const fn register(mut self, register: char) -> Self {
        self.register = Some(register);
        self
    }

    /// Set the keys.
    #[must_use]
    pub fn keys(mut self, keys: KeySequence) -> Self {
        self.keys = keys;
        self
    }

    /// Add metadata.
    #[must_use]
    pub fn with_metadata(mut self, key: impl Into<String>, value: ArgValue) -> Self {
        self.metadata.insert(key.into(), value);
        self
    }

    /// Get the effective count (default to 1 if not set).
    #[must_use]
    pub fn effective_count(&self) -> usize {
        self.count.unwrap_or(1)
    }
}

// ============================================================================
// ArgValue
// ============================================================================

/// Dynamic argument value for command metadata.
///
/// Supports common types used in command arguments.
#[derive(Debug, Clone, PartialEq)]
pub enum ArgValue {
    /// Boolean flag.
    Bool(bool),
    /// Integer value.
    Int(i64),
    /// Unsigned integer.
    Uint(u64),
    /// Floating point.
    Float(f64),
    /// String value.
    String(String),
    /// Character value.
    Char(char),
    /// Position in buffer.
    Position(Position),
    /// Range (start, end, linewise).
    Range {
        start: Position,
        end: Position,
        linewise: bool,
    },
}

impl From<bool> for ArgValue {
    fn from(v: bool) -> Self {
        Self::Bool(v)
    }
}

impl From<i64> for ArgValue {
    fn from(v: i64) -> Self {
        Self::Int(v)
    }
}

impl From<u64> for ArgValue {
    fn from(v: u64) -> Self {
        Self::Uint(v)
    }
}

impl From<f64> for ArgValue {
    fn from(v: f64) -> Self {
        Self::Float(v)
    }
}

impl From<String> for ArgValue {
    fn from(v: String) -> Self {
        Self::String(v)
    }
}

impl From<&str> for ArgValue {
    fn from(v: &str) -> Self {
        Self::String(v.to_owned())
    }
}

impl From<char> for ArgValue {
    fn from(v: char) -> Self {
        Self::Char(v)
    }
}

impl From<Position> for ArgValue {
    fn from(v: Position) -> Self {
        Self::Position(v)
    }
}

// ============================================================================
// ModeTransition
// ============================================================================

/// Mode transition request.
///
/// Resolvers return this to request changes to the mode stack.
#[derive(Debug, Clone)]
pub enum ModeTransition {
    /// Push a mode onto the stack with context.
    ///
    /// The new mode becomes current. The previous mode remains on the stack.
    /// Used for operator-pending, command-line, search, etc.
    Push {
        /// The mode to push.
        mode: ModeId,
        /// Context to pass to the new mode.
        context: TransitionContext,
    },

    /// Pop the current mode, passing result to parent.
    ///
    /// The current mode is removed from the stack. The parent mode
    /// receives the result (e.g., operator range, cancelled).
    Pop {
        /// Result to pass to the parent mode.
        result: Option<PopResult>,
    },

    /// Replace the current mode (equivalent to pop + push, but atomic).
    ///
    /// Used for mode changes that don't stack (normal → insert).
    Set {
        /// The mode to switch to.
        mode: ModeId,
        /// Context for the new mode.
        context: TransitionContext,
    },
}

// TransitionContext and PopResult are re-exported from session driver above.

// ============================================================================
// ModeState
// ============================================================================

/// Shared state passed to all resolvers.
///
/// This is the mechanism layer - it provides access to kernel state
/// without encoding any policy about how modes should behave.
///
/// # Note on Resolver-Owned State
///
/// Mode-specific state (counts, pending keys, pending operators) is
/// owned by each resolver, NOT by `ModeState`. This allows:
/// - Different editing styles to have different state needs
/// - Unit testing resolvers without full runner
/// - Clean hot-reload (replace resolver, state resets)
#[derive(Debug, Clone)]
pub struct ModeState {
    /// Keys accumulated for multi-key sequence matching.
    ///
    /// Cleared when a sequence completes or times out.
    pub pending_keys: KeySequence,

    /// Current mode stack.
    ///
    /// Read-only access; transitions happen via `ResolveResult::ModeTransition`.
    pub mode_stack: ModeStack,

    /// Currently active buffer (if any).
    pub active_buffer: Option<BufferId>,

    /// Context from the last mode transition.
    ///
    /// Contains operator, count, register from the parent mode.
    pub transition_context: Option<TransitionContext>,
}

impl ModeState {
    /// Create a new mode state with the given initial mode.
    #[must_use]
    pub fn new(initial_mode: ModeId) -> Self {
        Self {
            pending_keys: KeySequence::new(),
            mode_stack: ModeStack::new(initial_mode),
            active_buffer: None,
            transition_context: None,
        }
    }

    /// Get the current mode.
    #[must_use]
    pub fn current_mode(&self) -> &ModeId {
        self.mode_stack.current()
    }

    /// Check if keys are pending.
    #[must_use]
    pub const fn has_pending_keys(&self) -> bool {
        !self.pending_keys.is_empty()
    }

    /// Clear pending keys.
    pub fn clear_pending_keys(&mut self) {
        self.pending_keys.clear();
    }

    /// Add a key to the pending sequence.
    pub fn push_pending_key(&mut self, key: KeyEvent) {
        self.pending_keys.push(key);
    }

    /// Take the transition context (clears it).
    pub const fn take_transition_context(&mut self) -> Option<TransitionContext> {
        self.transition_context.take()
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use {reovim_driver_command_types::ArgValue as CmdArgValue, reovim_kernel::api::v1::ModuleId};

    use super::*;

    fn test_module() -> ModuleId {
        ModuleId::new("test")
    }

    fn test_mode() -> ModeId {
        ModeId::new(test_module(), "normal")
    }

    fn test_command() -> CommandId {
        CommandId::new(test_module(), "delete")
    }

    // ========================================================================
    // Trait object safety tests
    // ========================================================================

    #[test]
    fn test_mode_key_resolver_is_object_safe() {
        // Verify the trait is object-safe by accepting trait objects
        fn _accepts_ref(_: &dyn ModeKeyResolver) {}
        fn _accepts_box(_: Box<dyn ModeKeyResolver>) {}
        fn _accepts_arc(_: std::sync::Arc<dyn ModeKeyResolver>) {}
    }

    // ========================================================================
    // ResolveResult tests
    // ========================================================================

    #[test]
    fn test_resolve_result_execute() {
        let cmd = test_command();
        let ctx = ResolveContext::new().count(3);
        let result = ResolveResult::Execute(cmd.clone(), ctx);

        if let ResolveResult::Execute(c, context) = result {
            assert_eq!(c, cmd);
            assert_eq!(context.count, Some(3));
        } else {
            panic!("expected Execute variant");
        }
    }

    #[test]
    fn test_resolve_result_pending() {
        let result = ResolveResult::Pending;
        assert!(matches!(result, ResolveResult::Pending));
    }

    #[test]
    fn test_resolve_result_not_handled() {
        let result = ResolveResult::NotHandled;
        assert!(matches!(result, ResolveResult::NotHandled));
    }

    #[test]
    fn test_resolve_result_insert_char() {
        // Test the helper method (buffer target)
        let result = ResolveResult::insert_char('x');
        if let ResolveResult::InsertChar { char: c, target } = result {
            assert_eq!(c, 'x');
            assert_eq!(target, InputTarget::Buffer);
        } else {
            panic!("expected InsertChar variant");
        }
    }

    #[test]
    fn test_input_target_default_is_buffer() {
        let target = InputTarget::default();
        assert_eq!(target, InputTarget::Buffer);
    }

    #[test]
    fn test_input_target_extension_type_id() {
        // Use TestExtension as a stand-in (it needs SessionExtension + TextInputSink)
        // For now, just test that Extension variant works
        let type_id = std::any::TypeId::of::<String>();
        let target = InputTarget::Extension(type_id);
        assert!(matches!(target, InputTarget::Extension(_)));
    }

    #[test]
    fn test_insert_char_helper_creates_buffer_target() {
        let result = ResolveResult::insert_char('a');
        if let ResolveResult::InsertChar { char: c, target } = result {
            assert_eq!(c, 'a');
            assert_eq!(target, InputTarget::Buffer);
        } else {
            panic!("expected InsertChar with Buffer target");
        }
    }

    #[test]
    fn test_insert_char_to_creates_extension_target() {
        // Test that insert_char_to creates an Extension target with correct TypeId
        // We need a type that implements both SessionExtension and TextInputSink
        // Use CmdlineState from the session driver
        use reovim_driver_session::CmdlineState;

        let result = ResolveResult::insert_char_to::<CmdlineState>('x');
        if let ResolveResult::InsertChar { char: c, target } = result {
            assert_eq!(c, 'x');
            // Verify it's an Extension target with the correct TypeId
            let expected_type_id = std::any::TypeId::of::<CmdlineState>();
            assert_eq!(target, InputTarget::Extension(expected_type_id));
        } else {
            panic!("expected InsertChar with Extension target");
        }
    }

    #[test]
    fn test_input_target_extension_creates_correct_type_id() {
        // Test that InputTarget::extension::<T>() creates correct TypeId
        use reovim_driver_session::CmdlineState;

        let target = InputTarget::extension::<CmdlineState>();
        let expected_type_id = std::any::TypeId::of::<CmdlineState>();
        assert_eq!(target, InputTarget::Extension(expected_type_id));
    }

    #[test]
    fn test_resolve_result_mode_transition() {
        let mode = test_mode();
        let result = ResolveResult::ModeTransition(ModeTransition::Set {
            mode: mode.clone(),
            context: TransitionContext::new(),
        });

        if let ResolveResult::ModeTransition(ModeTransition::Set { mode: m, .. }) = result {
            assert_eq!(m, mode);
        } else {
            panic!("expected ModeTransition::Set variant");
        }
    }

    // ========================================================================
    // ResolveContext tests
    // ========================================================================

    #[test]
    fn test_resolve_context_new() {
        let ctx = ResolveContext::new();
        assert!(ctx.count.is_none());
        assert!(ctx.register.is_none());
        assert!(ctx.keys.is_empty());
        assert!(ctx.metadata.is_empty());
    }

    #[test]
    fn test_resolve_context_builder() {
        let ctx = ResolveContext::new()
            .count(5)
            .register('a')
            .with_metadata("linewise", ArgValue::Bool(true));

        assert_eq!(ctx.count, Some(5));
        assert_eq!(ctx.register, Some('a'));
        assert_eq!(ctx.metadata.get("linewise"), Some(&ArgValue::Bool(true)));
    }

    #[test]
    fn test_resolve_context_effective_count() {
        let ctx_none = ResolveContext::new();
        assert_eq!(ctx_none.effective_count(), 1);

        let ctx_some = ResolveContext::with_count(7);
        assert_eq!(ctx_some.effective_count(), 7);
    }

    // ========================================================================
    // ArgValue tests
    // ========================================================================

    #[test]
    fn test_arg_value_from_bool() {
        let v: ArgValue = true.into();
        assert_eq!(v, ArgValue::Bool(true));
    }

    #[test]
    fn test_arg_value_from_int() {
        let v: ArgValue = 42i64.into();
        assert_eq!(v, ArgValue::Int(42));
    }

    #[test]
    fn test_arg_value_from_string() {
        let v: ArgValue = "hello".into();
        assert_eq!(v, ArgValue::String("hello".to_string()));
    }

    #[test]
    fn test_arg_value_from_position() {
        let pos = Position::new(5, 10);
        let v: ArgValue = pos.into();
        assert_eq!(v, ArgValue::Position(Position::new(5, 10)));
    }

    #[test]
    fn test_arg_value_range() {
        let v = ArgValue::Range {
            start: Position::new(0, 0),
            end: Position::new(5, 10),
            linewise: true,
        };

        if let ArgValue::Range {
            start,
            end,
            linewise,
        } = v
        {
            assert_eq!(start, Position::new(0, 0));
            assert_eq!(end, Position::new(5, 10));
            assert!(linewise);
        } else {
            panic!("expected Range variant");
        }
    }

    // ========================================================================
    // ModeTransition tests
    // ========================================================================

    #[test]
    fn test_mode_transition_push() {
        let mode = test_mode();
        let op = test_command();
        let trans = ModeTransition::Push {
            mode: mode.clone(),
            context: TransitionContext::with_operator(op.clone()),
        };

        if let ModeTransition::Push { mode: m, context } = trans {
            assert_eq!(m, mode);
            assert_eq!(context.pending_operator, Some(op));
        } else {
            panic!("expected Push variant");
        }
    }

    #[test]
    fn test_mode_transition_pop() {
        let mut args = HashMap::new();
        args.insert("linewise".to_string(), CmdArgValue::Bang(false));
        args.insert("count".to_string(), CmdArgValue::Count(2));
        args.insert("register".to_string(), CmdArgValue::Register('a'));

        let trans = ModeTransition::Pop {
            result: Some(PopResult::ExecuteCommand {
                command: test_command(),
                args,
            }),
        };

        if let ModeTransition::Pop { result: Some(r) } = trans {
            if let PopResult::ExecuteCommand { command, args } = r {
                assert_eq!(command, test_command());
                assert_eq!(args.get("linewise"), Some(&CmdArgValue::Bang(false)));
                assert_eq!(args.get("count"), Some(&CmdArgValue::Count(2)));
                assert_eq!(args.get("register"), Some(&CmdArgValue::Register('a')));
            } else {
                panic!("expected ExecuteCommand");
            }
        } else {
            panic!("expected Pop with result");
        }
    }

    #[test]
    fn test_mode_transition_set() {
        let mode = test_mode();
        let trans = ModeTransition::Set {
            mode: mode.clone(),
            context: TransitionContext::new().count(3),
        };

        if let ModeTransition::Set { mode: m, context } = trans {
            assert_eq!(m, mode);
            assert_eq!(context.count, Some(3));
        } else {
            panic!("expected Set variant");
        }
    }

    // TransitionContext and PopResult tests are in reovim-driver-session::transition (canonical source)

    // ========================================================================
    // ModeState tests
    // ========================================================================

    #[test]
    fn test_mode_state_new() {
        let mode = test_mode();
        let state = ModeState::new(mode.clone());

        assert_eq!(state.current_mode(), &mode);
        assert!(!state.has_pending_keys());
        assert!(state.active_buffer.is_none());
        assert!(state.transition_context.is_none());
    }

    #[test]
    fn test_mode_state_pending_keys() {
        let mode = test_mode();
        let mut state = ModeState::new(mode);

        assert!(!state.has_pending_keys());

        state.push_pending_key(crate::KeyEvent::new(crate::KeyCode::Char('g')));
        assert!(state.has_pending_keys());
        assert_eq!(state.pending_keys.len(), 1);

        state.clear_pending_keys();
        assert!(!state.has_pending_keys());
    }

    #[test]
    fn test_mode_state_take_transition_context() {
        let mode = test_mode();
        let mut state = ModeState::new(mode);

        state.transition_context = Some(TransitionContext::new().count(5));
        assert!(state.transition_context.is_some());

        let ctx = state.take_transition_context();
        assert!(ctx.is_some());
        assert_eq!(ctx.unwrap().count, Some(5));
        assert!(state.transition_context.is_none());
    }

    // ========================================================================
    // OperatorArgs tests (#391)
    // ========================================================================

    #[test]
    fn test_operator_args_new() {
        let args = OperatorArgs::new();
        assert!(args.count.is_none());
        assert!(args.register.is_none());
        assert!(args.is_empty());
    }

    #[test]
    fn test_operator_args_with_count() {
        let args = OperatorArgs::with_count(5);
        assert_eq!(args.count, Some(5));
        assert!(args.register.is_none());
        assert!(!args.is_empty());
    }

    #[test]
    fn test_operator_args_builder() {
        let args = OperatorArgs::new().count(3).register('a');
        assert_eq!(args.count, Some(3));
        assert_eq!(args.register, Some('a'));
        assert!(!args.is_empty());
    }

    #[test]
    fn test_operator_args_effective_count() {
        let args_none = OperatorArgs::new();
        assert_eq!(args_none.effective_count(), 1);

        let args_some = OperatorArgs::with_count(7);
        assert_eq!(args_some.effective_count(), 7);
    }

    #[test]
    fn test_operator_args_is_empty() {
        let empty = OperatorArgs::new();
        assert!(empty.is_empty());

        let with_count = OperatorArgs::new().count(1);
        assert!(!with_count.is_empty());

        let with_register = OperatorArgs::new().register('b');
        assert!(!with_register.is_empty());
    }

    #[test]
    fn test_operator_args_equality() {
        let args1 = OperatorArgs::new().count(3).register('a');
        let args2 = OperatorArgs::new().count(3).register('a');
        let args3 = OperatorArgs::new().count(3).register('b');

        assert_eq!(args1, args2);
        assert_ne!(args1, args3);
    }

    #[test]
    fn test_operator_args_default() {
        let args = OperatorArgs::default();
        assert!(args.is_empty());
        assert_eq!(args.effective_count(), 1);
    }
}
