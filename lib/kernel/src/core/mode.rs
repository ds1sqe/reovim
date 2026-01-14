//! Mode and command identity types.
//!
//! Linux equivalent: Mode/command identification (mechanism only)
//!
//! This module provides identity types for modes and commands. The kernel
//! only tracks WHAT modes and commands exist - HOW they behave is policy
//! defined by drivers and modules.
//!
//! # Architecture
//!
//! | Layer | Responsibility |
//! |-------|---------------|
//! | Kernel (this) | Identity: `ModeId`, `CommandId`, `Mode` trait |
//! | Display Driver | Display: `ModeDisplay`, `CursorStyle` |
//! | Input Driver | Input: `ModeInput`, `KeySequence`, `Keybinding` |
//! | Command Driver | Execution: `Command`, `CommandHandler` |
//! | Modules | Policy: actual mode/command implementations |
//!
//! # Usage
//!
//! ```
//! use reovim_kernel::api::v1::{Mode, ModeId, ModuleId, CommandId};
//!
//! // Define a module ID
//! const MY_MODULE: ModuleId = ModuleId::new("my-module");
//!
//! // Create mode and command IDs
//! let normal_mode = ModeId::new(MY_MODULE.clone(), "normal");
//! let insert_mode = ModeId::new(MY_MODULE.clone(), "insert");
//! let cursor_down = CommandId::new(MY_MODULE.clone(), "cursor-down");
//! ```

use std::fmt;

use crate::api::module::ModuleId;

// ============================================================================
// ModeId
// ============================================================================

/// Namespaced mode identifier.
///
/// Modes are identified by their owning module and a local name.
/// This prevents naming conflicts between modules.
///
/// # Example
///
/// ```
/// use reovim_kernel::api::v1::{ModeId, ModuleId};
///
/// let editor_module = ModuleId::new("editor");
/// let normal = ModeId::new(editor_module.clone(), "normal");
/// let insert = ModeId::new(editor_module, "insert");
///
/// assert_eq!(normal.name(), "normal");
/// assert_eq!(insert.name(), "insert");
/// assert_ne!(normal, insert);
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ModeId {
    /// The module that owns this mode.
    module: ModuleId,
    /// The local name within the module.
    name: &'static str,
}

impl ModeId {
    /// Create a new mode identifier.
    #[must_use]
    pub const fn new(module: ModuleId, name: &'static str) -> Self {
        Self { module, name }
    }

    /// Get the owning module.
    #[must_use]
    pub const fn module(&self) -> &ModuleId {
        &self.module
    }

    /// Get the local name.
    #[must_use]
    pub const fn name(&self) -> &'static str {
        self.name
    }
}

impl fmt::Display for ModeId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}:{}", self.module, self.name)
    }
}

// ============================================================================
// CommandId
// ============================================================================

/// Namespaced command identifier.
///
/// Commands are identified by their owning module and a local name.
/// This prevents naming conflicts between modules.
///
/// # Example
///
/// ```
/// use reovim_kernel::api::v1::{CommandId, ModuleId};
///
/// let editor_module = ModuleId::new("editor");
/// let cursor_down = CommandId::new(editor_module.clone(), "cursor-down");
/// let cursor_up = CommandId::new(editor_module, "cursor-up");
///
/// assert_eq!(cursor_down.name(), "cursor-down");
/// assert_ne!(cursor_down, cursor_up);
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct CommandId {
    /// The module that owns this command.
    module: ModuleId,
    /// The local name within the module.
    name: &'static str,
}

impl CommandId {
    /// Create a new command identifier.
    #[must_use]
    pub const fn new(module: ModuleId, name: &'static str) -> Self {
        Self { module, name }
    }

    /// Get the owning module.
    #[must_use]
    pub const fn module(&self) -> &ModuleId {
        &self.module
    }

    /// Get the local name.
    #[must_use]
    pub const fn name(&self) -> &'static str {
        self.name
    }
}

impl fmt::Display for CommandId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}:{}", self.module, self.name)
    }
}

// ============================================================================
// Mode Trait
// ============================================================================

/// Mode identity trait.
///
/// This trait provides identity only - no behavior. Mode behavior is defined
/// by driver-level traits (`ModeDisplay`, `ModeInput`) and module implementations.
///
/// # Example
///
/// ```ignore
/// use reovim_kernel::api::v1::{Mode, ModeId, ModuleId};
///
/// const MY_MODULE: ModuleId = ModuleId::new("my-module");
///
/// #[derive(Debug, Clone, Copy)]
/// enum MyMode {
///     Normal,
///     Insert,
/// }
///
/// impl Mode for MyMode {
///     fn id(&self) -> ModeId {
///         ModeId::new(MY_MODULE.clone(), match self {
///             Self::Normal => "normal",
///             Self::Insert => "insert",
///         })
///     }
/// }
/// ```
pub trait Mode: Send + Sync + 'static {
    /// Get the mode's unique identifier.
    fn id(&self) -> ModeId;
}

// ============================================================================
// ModeStack
// ============================================================================

/// Mode stack for push/pop mode switching.
///
/// Supports vim-style mode stacking where modes can be pushed and popped.
/// For example, entering operator-pending mode pushes onto the stack,
/// and completing/canceling the operation pops back.
///
/// # Example
///
/// ```
/// use reovim_kernel::api::v1::{ModeId, ModeStack, ModuleId};
///
/// let module = ModuleId::new("editor");
/// let normal = ModeId::new(module.clone(), "normal");
/// let insert = ModeId::new(module.clone(), "insert");
/// let op_pending = ModeId::new(module, "operator-pending");
///
/// let mut stack = ModeStack::new(normal.clone());
/// assert_eq!(stack.current(), &normal);
///
/// // Push operator-pending mode
/// stack.push(op_pending.clone());
/// assert_eq!(stack.current(), &op_pending);
///
/// // Pop back to normal
/// assert_eq!(stack.pop(), Some(op_pending));
/// assert_eq!(stack.current(), &normal);
///
/// // Set directly to insert (replaces current)
/// stack.set(insert.clone());
/// assert_eq!(stack.current(), &insert);
/// ```
#[derive(Debug, Clone)]
pub struct ModeStack {
    /// The mode stack. Always has at least one element.
    stack: Vec<ModeId>,
}

impl ModeStack {
    /// Create a new mode stack with an initial mode.
    #[must_use]
    pub fn new(initial: ModeId) -> Self {
        Self {
            stack: vec![initial],
        }
    }

    /// Get the current (top) mode.
    ///
    /// # Panics
    ///
    /// This function will never panic in normal use. It only panics if the internal
    /// invariant (stack has at least one element) is violated, which indicates a bug.
    #[must_use]
    pub fn current(&self) -> &ModeId {
        // Safety: stack always has at least one element (invariant maintained by all methods)
        self.stack.last().expect("mode stack is never empty")
    }

    /// Push a new mode onto the stack.
    pub fn push(&mut self, mode: ModeId) {
        self.stack.push(mode);
    }

    /// Pop the top mode from the stack.
    ///
    /// Returns `None` if only one mode remains (cannot pop the base mode).
    pub fn pop(&mut self) -> Option<ModeId> {
        if self.stack.len() > 1 {
            self.stack.pop()
        } else {
            None
        }
    }

    /// Set the current mode, replacing the top of the stack.
    ///
    /// This is equivalent to pop + push, but works even when only one mode exists.
    pub fn set(&mut self, mode: ModeId) {
        if let Some(last) = self.stack.last_mut() {
            *last = mode;
        }
    }

    /// Get the stack depth.
    #[must_use]
    pub const fn depth(&self) -> usize {
        self.stack.len()
    }

    /// Check if we're in the base mode (stack depth is 1).
    #[must_use]
    pub const fn is_base(&self) -> bool {
        self.stack.len() == 1
    }

    /// Get all modes in the stack (bottom to top).
    #[must_use]
    pub fn as_slice(&self) -> &[ModeId] {
        &self.stack
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    fn test_module() -> ModuleId {
        ModuleId::new("test-module")
    }

    // ========================================================================
    // ModeId tests
    // ========================================================================

    #[test]
    fn test_mode_id_new() {
        let module = test_module();
        let mode = ModeId::new(module.clone(), "normal");
        assert_eq!(mode.module(), &module);
        assert_eq!(mode.name(), "normal");
    }

    #[test]
    fn test_mode_id_display() {
        let module = test_module();
        let mode = ModeId::new(module, "normal");
        assert_eq!(format!("{mode}"), "test-module:normal");
    }

    #[test]
    fn test_mode_id_equality() {
        let module = test_module();
        let mode1 = ModeId::new(module.clone(), "normal");
        let mode2 = ModeId::new(module.clone(), "normal");
        let mode3 = ModeId::new(module, "insert");
        assert_eq!(mode1, mode2);
        assert_ne!(mode1, mode3);
    }

    #[test]
    fn test_mode_id_hash() {
        use std::collections::HashSet;
        let module = test_module();
        let mode1 = ModeId::new(module.clone(), "normal");
        let mode2 = ModeId::new(module.clone(), "normal");
        let mode3 = ModeId::new(module, "insert");

        let mut set = HashSet::new();
        set.insert(mode1);
        assert!(set.contains(&mode2));
        assert!(!set.contains(&mode3));
    }

    // ========================================================================
    // CommandId tests
    // ========================================================================

    #[test]
    fn test_command_id_new() {
        let module = test_module();
        let cmd = CommandId::new(module.clone(), "cursor-down");
        assert_eq!(cmd.module(), &module);
        assert_eq!(cmd.name(), "cursor-down");
    }

    #[test]
    fn test_command_id_display() {
        let module = test_module();
        let cmd = CommandId::new(module, "cursor-down");
        assert_eq!(format!("{cmd}"), "test-module:cursor-down");
    }

    #[test]
    fn test_command_id_equality() {
        let module = test_module();
        let cmd1 = CommandId::new(module.clone(), "cursor-down");
        let cmd2 = CommandId::new(module.clone(), "cursor-down");
        let cmd3 = CommandId::new(module, "cursor-up");
        assert_eq!(cmd1, cmd2);
        assert_ne!(cmd1, cmd3);
    }

    // ========================================================================
    // ModeStack tests
    // ========================================================================

    #[test]
    fn test_mode_stack_new() {
        let module = test_module();
        let normal = ModeId::new(module, "normal");
        let stack = ModeStack::new(normal.clone());
        assert_eq!(stack.current(), &normal);
        assert_eq!(stack.depth(), 1);
        assert!(stack.is_base());
    }

    #[test]
    fn test_mode_stack_push_pop() {
        let module = test_module();
        let normal = ModeId::new(module.clone(), "normal");
        let op_pending = ModeId::new(module, "operator-pending");

        let mut stack = ModeStack::new(normal.clone());
        stack.push(op_pending.clone());

        assert_eq!(stack.current(), &op_pending);
        assert_eq!(stack.depth(), 2);
        assert!(!stack.is_base());

        assert_eq!(stack.pop(), Some(op_pending));
        assert_eq!(stack.current(), &normal);
        assert_eq!(stack.depth(), 1);
        assert!(stack.is_base());
    }

    #[test]
    fn test_mode_stack_pop_base() {
        let module = test_module();
        let normal = ModeId::new(module, "normal");
        let mut stack = ModeStack::new(normal.clone());

        // Cannot pop the base mode
        assert_eq!(stack.pop(), None);
        assert_eq!(stack.current(), &normal);
        assert_eq!(stack.depth(), 1);
    }

    #[test]
    fn test_mode_stack_set() {
        let module = test_module();
        let normal = ModeId::new(module.clone(), "normal");
        let insert = ModeId::new(module, "insert");

        let mut stack = ModeStack::new(normal);
        stack.set(insert.clone());

        assert_eq!(stack.current(), &insert);
        assert_eq!(stack.depth(), 1);
    }

    #[test]
    fn test_mode_stack_as_slice() {
        let module = test_module();
        let normal = ModeId::new(module.clone(), "normal");
        let op_pending = ModeId::new(module, "operator-pending");

        let mut stack = ModeStack::new(normal.clone());
        stack.push(op_pending.clone());

        let slice = stack.as_slice();
        assert_eq!(slice.len(), 2);
        assert_eq!(slice[0], normal);
        assert_eq!(slice[1], op_pending);
    }

    // ========================================================================
    // Mode trait tests
    // ========================================================================

    #[test]
    fn test_mode_trait_object_safety() {
        // Verify Mode trait is object-safe
        fn _accepts_mode_ref(_: &dyn Mode) {}
        fn _accepts_boxed_mode(_: Box<dyn Mode>) {}
        fn _accepts_arc_mode(_: std::sync::Arc<dyn Mode>) {}
    }

    // Test implementation of Mode trait
    struct TestMode {
        name: &'static str,
    }

    impl Mode for TestMode {
        fn id(&self) -> ModeId {
            ModeId::new(ModuleId::new("test"), self.name)
        }
    }

    #[test]
    fn test_mode_implementation() {
        let mode = TestMode { name: "normal" };
        assert_eq!(mode.id().name(), "normal");
        assert_eq!(mode.id().module().as_str(), "test");
    }
}
