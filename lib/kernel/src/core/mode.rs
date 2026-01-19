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
//! | Kernel (this) | Identity: `ModeId`, `CommandId`, `Mode` trait, `CursorStyle` |
//! | Display Driver | Display: `ModeDisplay` (uses kernel `CursorStyle`) |
//! | Input Driver | Input: `ModeInput`, `KeySequence`, `Keybinding` |
//! | Command Driver | Execution: `Command`, `CommandHandler` |
//! | Modules | Policy: actual mode/command implementations |
//!
//! # Mode Ownership
//!
//! The `Mode` trait is designed for type-safe, compile-time enforced mode ownership:
//!
//! - Policy modules (e.g., vim) define their own Mode enums
//! - Mode enums implement the `Mode` trait
//! - `ModeId` provides runtime identity for storage in `ModeStack`
//! - Blanket impl `From<M> for ModeId` allows ergonomic conversion
//!
//! # Usage
//!
//! ```
//! use reovim_kernel::api::v1::{Mode, ModeId, ModuleId, CommandId, CursorStyle};
//!
//! // Define a module ID
//! const MY_MODULE: ModuleId = ModuleId::new("my-module");
//!
//! // Create mode and command IDs
//! let normal_mode = ModeId::new(MY_MODULE.clone(), "normal");
//! let insert_mode = ModeId::with_discriminant(MY_MODULE.clone(), "insert", 1);
//! let cursor_down = CommandId::new(MY_MODULE.clone(), "cursor-down");
//!
//! assert_eq!(insert_mode.discriminant(), 1);
//! ```

use std::{fmt, hash::Hash};

use crate::api::module::ModuleId;

// ============================================================================
// CursorStyle
// ============================================================================

/// Cursor display style.
///
/// Different modes typically use different cursor styles to provide
/// visual feedback about the current mode. This enum is defined in the
/// kernel so that the `Mode` trait can include cursor style information.
///
/// # Variants
///
/// - `Block`: Full cell cursor, typical for Normal mode
/// - `Bar`: Thin vertical line, typical for Insert mode
/// - `Underline`: Horizontal line under the character
/// - `Hidden`: Cursor not visible
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum CursorStyle {
    /// Block cursor (full cell, typical for Normal mode).
    #[default]
    Block,
    /// Vertical bar cursor (thin line, typical for Insert mode).
    Bar,
    /// Underline cursor (horizontal line under the character).
    Underline,
    /// Hidden cursor (cursor not visible).
    Hidden,
}

impl CursorStyle {
    /// Check if the cursor is visible.
    #[must_use]
    pub const fn is_visible(&self) -> bool {
        !matches!(self, Self::Hidden)
    }

    /// Get the style name for display.
    #[must_use]
    pub const fn name(&self) -> &'static str {
        match self {
            Self::Block => "block",
            Self::Bar => "bar",
            Self::Underline => "underline",
            Self::Hidden => "hidden",
        }
    }
}

// ============================================================================
// ModeId
// ============================================================================

/// Namespaced mode identifier with numeric discriminant.
///
/// Modes are identified by their owning module, a local name, and a numeric
/// discriminant for O(1) identity comparison. The discriminant provides
/// compile-time type safety when used with the `Mode` trait.
///
/// # Identity
///
/// Two `ModeId`s are equal if and only if their module AND discriminant match.
/// The name is for display purposes only and does not affect equality.
///
/// # Example
///
/// ```
/// use reovim_kernel::api::v1::{ModeId, ModuleId};
///
/// let editor_module = ModuleId::new("editor");
/// let normal = ModeId::with_discriminant(editor_module.clone(), "NORMAL", 0);
/// let insert = ModeId::with_discriminant(editor_module, "INSERT", 1);
///
/// assert_eq!(normal.name(), "NORMAL");
/// assert_eq!(normal.discriminant(), 0);
/// assert_eq!(insert.discriminant(), 1);
/// assert_ne!(normal, insert);
/// ```
#[derive(Debug, Clone)]
pub struct ModeId {
    /// The module that owns this mode.
    module: ModuleId,
    /// The display name (for statusline, etc.).
    name: &'static str,
    /// Numeric discriminant for O(1) identity comparison.
    discriminant: u16,
}

impl ModeId {
    /// Create a new mode identifier with default discriminant (0).
    ///
    /// This constructor is provided for backward compatibility.
    /// Prefer `with_discriminant` for new code.
    #[must_use]
    pub const fn new(module: ModuleId, name: &'static str) -> Self {
        Self {
            module,
            name,
            discriminant: 0,
        }
    }

    /// Create a new mode identifier with explicit discriminant.
    ///
    /// The discriminant should be unique within the module and stable
    /// across versions for serialization compatibility.
    #[must_use]
    pub const fn with_discriminant(
        module: ModuleId,
        name: &'static str,
        discriminant: u16,
    ) -> Self {
        Self {
            module,
            name,
            discriminant,
        }
    }

    /// Get the owning module.
    #[must_use]
    pub const fn module(&self) -> &ModuleId {
        &self.module
    }

    /// Get the display name.
    #[must_use]
    pub const fn name(&self) -> &'static str {
        self.name
    }

    /// Get the numeric discriminant.
    #[must_use]
    pub const fn discriminant(&self) -> u16 {
        self.discriminant
    }
}

// Manual PartialEq: equality based on module + discriminant only
impl PartialEq for ModeId {
    fn eq(&self, other: &Self) -> bool {
        self.module == other.module && self.discriminant == other.discriminant
    }
}

impl Eq for ModeId {}

// Manual Hash: hash based on module + discriminant only
impl std::hash::Hash for ModeId {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.module.hash(state);
        self.discriminant.hash(state);
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

    /// Create a command identifier from a qualified string like "module:command".
    ///
    /// This method is intended for dynamic use cases like FFI where command IDs
    /// are specified as strings at runtime. The strings are leaked to get
    /// `'static` lifetime, so this should only be used for long-lived commands.
    ///
    /// If the string doesn't contain ':', the entire string is treated as the
    /// command name with "unknown" as the module.
    ///
    /// # Example
    ///
    /// ```
    /// use reovim_kernel::api::v1::CommandId;
    ///
    /// let cmd = CommandId::from_qualified_leaked("editor:cursor-down".to_string());
    /// assert_eq!(cmd.module().as_str(), "editor");
    /// assert_eq!(cmd.name(), "cursor-down");
    /// ```
    #[must_use]
    pub fn from_qualified_leaked(qualified: String) -> Self {
        let (module_str, name_str) = if let Some(idx) = qualified.find(':') {
            (qualified[..idx].to_string(), qualified[idx + 1..].to_string())
        } else {
            ("unknown".to_string(), qualified)
        };

        let module_static: &'static str = Box::leak(module_str.into_boxed_str());
        let name_static: &'static str = Box::leak(name_str.into_boxed_str());

        Self {
            module: ModuleId::new(module_static),
            name: name_static,
        }
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

// Note: OperatorId was removed from kernel (Epic #385).
// Operators are vim-specific policy - OperatorId now lives in modules/vim/src/ids.rs

// ============================================================================
// Mode Trait
// ============================================================================

/// Mode trait for type-safe, compile-time enforced mode ownership.
///
/// Policy modules (e.g., vim, emacs) define their own Mode enums and implement
/// this trait. The trait provides both identity and behavior information,
/// allowing the kernel and drivers to query mode properties without knowing
/// the concrete type.
///
/// # Type Safety
///
/// The trait requires `Copy + Clone + PartialEq + Eq + Hash` to ensure modes
/// are lightweight value types that can be efficiently compared and stored.
/// This also makes the trait **not object-safe**, which is intentional:
/// runtime storage uses `ModeId`, not `dyn Mode`.
///
/// # Blanket Implementation
///
/// All `Mode` types automatically implement `From<M> for ModeId`, allowing
/// ergonomic conversion when storing modes in `ModeStack` or `ModeRegistry`.
///
/// # Example
///
/// ```ignore
/// use reovim_kernel::api::v1::{Mode, ModeId, ModuleId, CursorStyle};
///
/// const VIM_MODULE: ModuleId = ModuleId::new("vim");
///
/// #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
/// #[repr(u16)]
/// enum VimMode {
///     Normal = 0,
///     Insert = 1,
///     Visual = 2,
/// }
///
/// impl Mode for VimMode {
///     fn module() -> ModuleId { VIM_MODULE }
///
///     fn discriminant(&self) -> u16 { *self as u16 }
///
///     fn display_name(&self) -> &'static str {
///         match self {
///             Self::Normal => "NORMAL",
///             Self::Insert => "INSERT",
///             Self::Visual => "VISUAL",
///         }
///     }
///
///     fn cursor_style(&self) -> CursorStyle {
///         match self {
///             Self::Insert => CursorStyle::Bar,
///             _ => CursorStyle::Block,
///         }
///     }
///
///     fn accepts_char_input(&self) -> bool {
///         matches!(self, Self::Insert)
///     }
/// }
/// ```
pub trait Mode: Copy + Clone + PartialEq + Eq + Hash + Send + Sync + 'static {
    /// The module that owns this mode type.
    ///
    /// This is a type-level constant, not an instance method.
    fn module() -> ModuleId
    where
        Self: Sized;

    /// Get the unique discriminant for this mode variant.
    ///
    /// For `#[repr(u16)]` enums, this is typically `*self as u16`.
    /// Discriminants must be stable across versions for serialization.
    fn discriminant(&self) -> u16;

    /// Convert to the runtime storage type.
    ///
    /// Default implementation creates a `ModeId` from module, `display_name`,
    /// and discriminant. Override only if custom behavior is needed.
    fn id(&self) -> ModeId
    where
        Self: Sized,
    {
        ModeId::with_discriminant(Self::module(), self.display_name(), self.discriminant())
    }

    /// Get the display name for this mode.
    ///
    /// This is shown in the statusline (e.g., "NORMAL", "INSERT", "VISUAL").
    fn display_name(&self) -> &'static str;

    /// Get the cursor style for this mode.
    ///
    /// Different modes typically use different cursor styles:
    /// - Normal mode: Block cursor
    /// - Insert mode: Bar cursor
    /// - Replace mode: Underline cursor
    fn cursor_style(&self) -> CursorStyle;

    /// Whether this mode accepts direct character input.
    ///
    /// Returns `true` for modes like Insert, `CommandLine`, Replace.
    /// Returns `false` for Normal, Visual, `OperatorPending`.
    fn accepts_char_input(&self) -> bool;

    /// Whether this mode has an active selection.
    ///
    /// Returns `true` for Visual, Select modes.
    /// Default is `false`.
    fn has_selection(&self) -> bool {
        false
    }

    /// Get the parent mode for keybinding inheritance.
    ///
    /// For example, `VisualLine` might inherit from Visual, which inherits
    /// from Normal. Returns `None` for root modes.
    fn inherits_from(&self) -> Option<Self>
    where
        Self: Sized,
    {
        None
    }

    /// Whether this is the entry/default mode for new sessions.
    ///
    /// Only one mode per module should return true. The first mode
    /// registered with `is_entry() = true` becomes the session's initial mode.
    ///
    /// Default is `false`.
    fn is_entry(&self) -> bool {
        false
    }
}

/// Blanket implementation: all Mode types convert to `ModeId`.
///
/// This allows ergonomic usage:
/// ```ignore
/// let stack = ModeStack::new(VimMode::Normal);  // No .into() needed
/// stack.push(VimMode::Insert);
/// ```
impl<M: Mode> From<M> for ModeId {
    fn from(mode: M) -> Self {
        mode.id()
    }
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
/// # Generic Mode Support
///
/// All methods accept `impl Into<ModeId>`, allowing both `ModeId` and
/// any type implementing `Mode` to be used directly:
///
/// ```ignore
/// let mut stack = ModeStack::new(VimMode::Normal);  // VimMode implements Mode
/// stack.push(VimMode::OperatorPending);
/// ```
///
/// # Example
///
/// ```
/// use reovim_kernel::api::v1::{ModeId, ModeStack, ModuleId};
///
/// let module = ModuleId::new("editor");
/// let normal = ModeId::with_discriminant(module.clone(), "NORMAL", 0);
/// let insert = ModeId::with_discriminant(module.clone(), "INSERT", 1);
/// let op_pending = ModeId::with_discriminant(module, "OP-PEND", 5);
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
    ///
    /// Accepts any type that implements `Into<ModeId>`, including
    /// `ModeId` itself and any type implementing `Mode`.
    #[must_use]
    pub fn new<M: Into<ModeId>>(initial: M) -> Self {
        Self {
            stack: vec![initial.into()],
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
    ///
    /// Accepts any type that implements `Into<ModeId>`.
    pub fn push<M: Into<ModeId>>(&mut self, mode: M) {
        self.stack.push(mode.into());
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
    /// Accepts any type that implements `Into<ModeId>`.
    pub fn set<M: Into<ModeId>>(&mut self, mode: M) {
        if let Some(last) = self.stack.last_mut() {
            *last = mode.into();
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

    /// Get the home (base) mode.
    ///
    /// The home mode is the first mode pushed onto the stack and cannot be popped.
    ///
    /// # Panics
    ///
    /// This function will never panic in normal use. It only panics if the internal
    /// invariant (stack has at least one element) is violated, which indicates a bug.
    #[must_use]
    pub fn home(&self) -> &ModeId {
        // Safety: stack always has at least one element (invariant maintained by all methods)
        self.stack.first().expect("mode stack is never empty")
    }

    /// Check if a mode is anywhere in the stack.
    #[must_use]
    pub fn contains(&self, mode_id: &ModeId) -> bool {
        self.stack.contains(mode_id)
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
    // CursorStyle tests
    // ========================================================================

    #[test]
    fn test_cursor_style_default() {
        let style = CursorStyle::default();
        assert_eq!(style, CursorStyle::Block);
    }

    #[test]
    fn test_cursor_style_is_visible() {
        assert!(CursorStyle::Block.is_visible());
        assert!(CursorStyle::Bar.is_visible());
        assert!(CursorStyle::Underline.is_visible());
        assert!(!CursorStyle::Hidden.is_visible());
    }

    #[test]
    fn test_cursor_style_name() {
        assert_eq!(CursorStyle::Block.name(), "block");
        assert_eq!(CursorStyle::Bar.name(), "bar");
        assert_eq!(CursorStyle::Underline.name(), "underline");
        assert_eq!(CursorStyle::Hidden.name(), "hidden");
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
        assert_eq!(mode.discriminant(), 0); // default discriminant
    }

    #[test]
    fn test_mode_id_with_discriminant() {
        let module = test_module();
        let mode = ModeId::with_discriminant(module.clone(), "INSERT", 1);
        assert_eq!(mode.module(), &module);
        assert_eq!(mode.name(), "INSERT");
        assert_eq!(mode.discriminant(), 1);
    }

    #[test]
    fn test_mode_id_display() {
        let module = test_module();
        let mode = ModeId::with_discriminant(module, "NORMAL", 0);
        assert_eq!(format!("{mode}"), "test-module:NORMAL");
    }

    #[test]
    fn test_mode_id_equality_by_discriminant() {
        let module = test_module();
        // Same module + discriminant = equal (even if names differ)
        let mode1 = ModeId::with_discriminant(module.clone(), "normal", 0);
        let mode2 = ModeId::with_discriminant(module.clone(), "NORMAL", 0);
        let mode3 = ModeId::with_discriminant(module, "insert", 1);
        assert_eq!(mode1, mode2); // Same discriminant
        assert_ne!(mode1, mode3); // Different discriminant
    }

    #[test]
    fn test_mode_id_hash_by_discriminant() {
        use std::collections::HashSet;
        let module = test_module();
        // Same module + discriminant = same hash
        let mode1 = ModeId::with_discriminant(module.clone(), "normal", 0);
        let mode2 = ModeId::with_discriminant(module.clone(), "NORMAL", 0);
        let mode3 = ModeId::with_discriminant(module, "insert", 1);

        let mut set = HashSet::new();
        set.insert(mode1);
        assert!(set.contains(&mode2)); // Same discriminant
        assert!(!set.contains(&mode3)); // Different discriminant
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

    // Note: OperatorId tests moved to modules/vim (Epic #385)

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

    #[test]
    fn test_mode_stack_home() {
        let module = test_module();
        let normal = ModeId::new(module.clone(), "normal");
        let insert = ModeId::new(module, "insert");

        let mut stack = ModeStack::new(normal.clone());
        assert_eq!(stack.home(), &normal);

        // Home doesn't change when we push
        stack.push(insert);
        assert_eq!(stack.home(), &normal);

        // Home doesn't change when we pop
        stack.pop();
        assert_eq!(stack.home(), &normal);
    }

    #[test]
    fn test_mode_stack_contains() {
        let module = test_module();
        // Use different discriminants for different modes
        let normal = ModeId::with_discriminant(module.clone(), "normal", 0);
        let insert = ModeId::with_discriminant(module.clone(), "insert", 1);
        let visual = ModeId::with_discriminant(module, "visual", 2);

        let mut stack = ModeStack::new(normal.clone());
        assert!(stack.contains(&normal));
        assert!(!stack.contains(&insert));
        assert!(!stack.contains(&visual));

        stack.push(insert.clone());
        assert!(stack.contains(&normal));
        assert!(stack.contains(&insert));
        assert!(!stack.contains(&visual));

        stack.pop();
        assert!(stack.contains(&normal));
        assert!(!stack.contains(&insert));
    }

    // ========================================================================
    // Mode trait tests
    // ========================================================================

    // The Mode trait is intentionally NOT object-safe.
    // It requires Copy + Clone + PartialEq + Eq + Hash, making dyn Mode invalid.
    // This is by design: runtime storage uses ModeId, not dyn Mode.

    // Test implementation of Mode trait
    const TEST_MODULE: ModuleId = ModuleId::new("test");

    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
    #[repr(u16)]
    enum TestMode {
        Normal = 0,
        Insert = 1,
        Visual = 2,
    }

    impl Mode for TestMode {
        fn module() -> ModuleId {
            TEST_MODULE
        }

        fn discriminant(&self) -> u16 {
            *self as u16
        }

        fn display_name(&self) -> &'static str {
            match self {
                Self::Normal => "NORMAL",
                Self::Insert => "INSERT",
                Self::Visual => "VISUAL",
            }
        }

        fn cursor_style(&self) -> CursorStyle {
            match self {
                Self::Insert => CursorStyle::Bar,
                _ => CursorStyle::Block,
            }
        }

        fn accepts_char_input(&self) -> bool {
            matches!(self, Self::Insert)
        }

        fn has_selection(&self) -> bool {
            matches!(self, Self::Visual)
        }

        fn inherits_from(&self) -> Option<Self> {
            match self {
                Self::Visual => Some(Self::Normal),
                _ => None,
            }
        }
    }

    #[test]
    fn test_mode_implementation() {
        let mode = TestMode::Normal;
        assert_eq!(mode.id().name(), "NORMAL");
        assert_eq!(mode.id().module().as_str(), "test");
        assert_eq!(mode.discriminant(), 0);
    }

    #[test]
    fn test_mode_cursor_style() {
        assert_eq!(TestMode::Normal.cursor_style(), CursorStyle::Block);
        assert_eq!(TestMode::Insert.cursor_style(), CursorStyle::Bar);
        assert_eq!(TestMode::Visual.cursor_style(), CursorStyle::Block);
    }

    #[test]
    fn test_mode_accepts_char_input() {
        assert!(!TestMode::Normal.accepts_char_input());
        assert!(TestMode::Insert.accepts_char_input());
        assert!(!TestMode::Visual.accepts_char_input());
    }

    #[test]
    fn test_mode_has_selection() {
        assert!(!TestMode::Normal.has_selection());
        assert!(!TestMode::Insert.has_selection());
        assert!(TestMode::Visual.has_selection());
    }

    #[test]
    fn test_mode_inherits_from() {
        assert_eq!(TestMode::Normal.inherits_from(), None);
        assert_eq!(TestMode::Insert.inherits_from(), None);
        assert_eq!(TestMode::Visual.inherits_from(), Some(TestMode::Normal));
    }

    #[test]
    fn test_mode_into_mode_id() {
        let mode = TestMode::Insert;
        let id: ModeId = mode.into();
        assert_eq!(id.name(), "INSERT");
        assert_eq!(id.discriminant(), 1);
        assert_eq!(id.module(), &TEST_MODULE);
    }

    #[test]
    fn test_mode_stack_with_mode_trait() {
        // ModeStack accepts impl Into<ModeId>, so Mode types work directly
        let mut stack = ModeStack::new(TestMode::Normal);
        assert_eq!(stack.current().discriminant(), 0);

        stack.push(TestMode::Visual);
        assert_eq!(stack.current().discriminant(), 2);

        stack.set(TestMode::Insert);
        assert_eq!(stack.current().discriminant(), 1);
    }

    /// Test that `ModeId`s created with static vs dynamic `ModuleId`s are equal.
    ///
    /// This is critical for the keymap wiring scenario where:
    /// - Session mode is created with `ModeId::new(ModuleId::new("editor"), "normal")`
    /// - Wired keybindings use `ModeId::new(ModuleId::from_string("editor".to_string()), "normal")`
    ///
    /// Both should hash and compare equal for `HashMap` lookups to work.
    #[test]
    fn test_mode_id_static_vs_dynamic_module() {
        use std::collections::HashMap;

        // Static module ID (like in fallback_default_mode)
        let static_module = ModuleId::new("editor");
        let mode_a = ModeId::new(static_module, "normal");

        // Dynamic module ID (like in wire_module_keybindings)
        let dynamic_module = ModuleId::from_string("editor".to_string());
        let mode_b = ModeId::new(dynamic_module, "normal");

        // Should be equal
        assert_eq!(mode_a, mode_b, "ModeIds with same content should be equal");
        assert_eq!(mode_a.module(), mode_b.module(), "Module IDs should be equal");
        assert_eq!(mode_a.discriminant(), mode_b.discriminant(), "Discriminants should be equal");

        // Should have same hash (HashMap lookup must work)
        let mut map: HashMap<ModeId, &str> = HashMap::new();
        map.insert(mode_a.clone(), "from_a");

        assert_eq!(map.get(&mode_a), Some(&"from_a"), "Lookup with same ModeId should work");
        assert_eq!(map.get(&mode_b), Some(&"from_a"), "Lookup with equivalent ModeId should work");
    }
}
