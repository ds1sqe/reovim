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
//!    └─ Returns: ModeTransition::Push {
//!         mode: "operator-pending",
//!         context: { pending_operator: "delete" }
//!       }
//!
//! 2. EventLoop pushes operator-pending mode
//!
//! 3. Operator-pending mode receives 'w'
//!    └─ OperatorPendingResolver.resolve('w', state)
//!    └─ Returns: ModeTransition::Pop {
//!         result: OperatorRange { start: (0,0), end: (0,5) }
//!       }
//!
//! 4. EventLoop pops mode, executes delete on range
//! ```

use std::collections::HashMap;

use reovim_kernel::api::v1::{BufferId, CommandId, ModeId, ModeStack, Position};

use crate::{KeyEvent, KeySequence};

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
/// - Vim: `VimNormalResolver`, `VimInsertResolver`, `VimOperatorPendingResolver`
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
    /// Process a key event in this mode's context.
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
    fn resolve(&self, key: &KeyEvent, state: &mut ModeState) -> ResolveResult;

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

    /// Insert this character directly.
    ///
    /// For input-accepting modes (insert, command-line, search).
    /// The character should be inserted at the cursor position.
    InsertChar(char),

    /// Request a mode transition.
    ///
    /// Push, pop, or replace the current mode on the mode stack.
    ModeTransition(ModeTransition),
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

// ============================================================================
// TransitionContext
// ============================================================================

/// Context passed when entering a new mode.
///
/// Contains state from the previous mode that the new mode needs:
/// - Pending operator (for operator-pending mode)
/// - Count prefix (inherited across mode transitions)
/// - Register selection
#[derive(Debug, Clone, Default)]
pub struct TransitionContext {
    /// Pending operator waiting for a motion/text-object.
    ///
    /// Set when entering operator-pending mode (e.g., after pressing `d`).
    pub pending_operator: Option<CommandId>,

    /// Count prefix to pass to the new mode.
    ///
    /// Counts can be combined: `2d3w` = delete 6 words.
    pub count: Option<usize>,

    /// Register for the operation.
    pub register: Option<char>,
}

impl TransitionContext {
    /// Create an empty context.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Create a context with a pending operator.
    #[must_use]
    pub fn with_operator(operator: CommandId) -> Self {
        Self {
            pending_operator: Some(operator),
            ..Default::default()
        }
    }

    /// Set the pending operator.
    #[must_use]
    pub fn operator(mut self, op: CommandId) -> Self {
        self.pending_operator = Some(op);
        self
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
}

// ============================================================================
// PopResult
// ============================================================================

/// Result returned when popping from a mode.
///
/// The parent mode uses this to complete its operation:
/// - Operator-pending returns range for the operator
/// - Search returns the search pattern
/// - Command-line returns the entered command
#[derive(Debug, Clone)]
pub enum PopResult {
    /// Operator completed with a range.
    ///
    /// The parent mode (normal) should execute the pending operator
    /// on this range.
    OperatorRange {
        /// Start position of the range.
        start: Position,
        /// End position of the range.
        end: Position,
        /// Whether the range is linewise (full lines).
        linewise: bool,
    },

    /// Text object selected.
    ///
    /// Similar to `OperatorRange` but explicitly marks text object selection.
    TextObject {
        /// Start position.
        start: Position,
        /// End position.
        end: Position,
        /// Whether the selection is linewise.
        linewise: bool,
        /// Whether this is an "inner" (i) or "around" (a) text object.
        inner: bool,
    },

    /// User cancelled the operation (Escape pressed).
    Cancelled,

    /// Search pattern entered.
    SearchPattern {
        /// The search pattern.
        pattern: String,
        /// Search direction (forward = true, backward = false).
        forward: bool,
    },

    /// Command-line command entered.
    CommandLine {
        /// The entered command string.
        command: String,
    },

    /// Character input completed (for f/t/r commands).
    CharInput {
        /// The entered character.
        char: char,
    },
}

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
    use reovim_kernel::api::v1::ModuleId;

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
        let result = ResolveResult::InsertChar('x');
        if let ResolveResult::InsertChar(c) = result {
            assert_eq!(c, 'x');
        } else {
            panic!("expected InsertChar variant");
        }
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
        let trans = ModeTransition::Pop {
            result: Some(PopResult::OperatorRange {
                start: Position::new(0, 0),
                end: Position::new(0, 5),
                linewise: false,
            }),
        };

        if let ModeTransition::Pop { result: Some(r) } = trans {
            if let PopResult::OperatorRange {
                start,
                end,
                linewise,
            } = r
            {
                assert_eq!(start, Position::new(0, 0));
                assert_eq!(end, Position::new(0, 5));
                assert!(!linewise);
            } else {
                panic!("expected OperatorRange");
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

    // ========================================================================
    // TransitionContext tests
    // ========================================================================

    #[test]
    fn test_transition_context_new() {
        let ctx = TransitionContext::new();
        assert!(ctx.pending_operator.is_none());
        assert!(ctx.count.is_none());
        assert!(ctx.register.is_none());
    }

    #[test]
    fn test_transition_context_builder() {
        let op = test_command();
        let ctx = TransitionContext::new()
            .operator(op.clone())
            .count(2)
            .register('b');

        assert_eq!(ctx.pending_operator, Some(op));
        assert_eq!(ctx.count, Some(2));
        assert_eq!(ctx.register, Some('b'));
    }

    #[test]
    fn test_transition_context_with_operator() {
        let op = test_command();
        let ctx = TransitionContext::with_operator(op.clone());
        assert_eq!(ctx.pending_operator, Some(op));
    }

    // ========================================================================
    // PopResult tests
    // ========================================================================

    #[test]
    fn test_pop_result_operator_range() {
        let result = PopResult::OperatorRange {
            start: Position::new(1, 0),
            end: Position::new(3, 0),
            linewise: true,
        };

        if let PopResult::OperatorRange {
            start,
            end,
            linewise,
        } = result
        {
            assert_eq!(start, Position::new(1, 0));
            assert_eq!(end, Position::new(3, 0));
            assert!(linewise);
        } else {
            panic!("expected OperatorRange");
        }
    }

    #[test]
    fn test_pop_result_text_object() {
        let result = PopResult::TextObject {
            start: Position::new(0, 5),
            end: Position::new(0, 10),
            linewise: false,
            inner: true,
        };

        if let PopResult::TextObject {
            start,
            end,
            linewise,
            inner,
        } = result
        {
            assert_eq!(start, Position::new(0, 5));
            assert_eq!(end, Position::new(0, 10));
            assert!(!linewise);
            assert!(inner);
        } else {
            panic!("expected TextObject");
        }
    }

    #[test]
    fn test_pop_result_cancelled() {
        let result = PopResult::Cancelled;
        assert!(matches!(result, PopResult::Cancelled));
    }

    #[test]
    fn test_pop_result_search_pattern() {
        let result = PopResult::SearchPattern {
            pattern: "foo".to_string(),
            forward: true,
        };

        if let PopResult::SearchPattern { pattern, forward } = result {
            assert_eq!(pattern, "foo");
            assert!(forward);
        } else {
            panic!("expected SearchPattern");
        }
    }

    #[test]
    fn test_pop_result_command_line() {
        let result = PopResult::CommandLine {
            command: "wq".to_string(),
        };

        if let PopResult::CommandLine { command } = result {
            assert_eq!(command, "wq");
        } else {
            panic!("expected CommandLine");
        }
    }

    #[test]
    fn test_pop_result_char_input() {
        let result = PopResult::CharInput { char: 'x' };

        if let PopResult::CharInput { char: c } = result {
            assert_eq!(c, 'x');
        } else {
            panic!("expected CharInput");
        }
    }

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
}
