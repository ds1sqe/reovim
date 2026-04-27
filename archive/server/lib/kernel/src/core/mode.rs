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
//! | Kernel (this) | Identity + Behavior: `ModeId`, `CommandId`, `Mode` trait, `CursorStyle` |
//! | Display Driver | Display: `ModeDisplay` (uses kernel `CursorStyle`) |
//! | Input Driver | Input: `KeySequence`, `Keybinding` |
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

use std::{borrow::Cow, fmt, hash::Hash};

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
    name: Cow<'static, str>,
}

impl CommandId {
    /// Create a new command identifier from static strings.
    ///
    /// This is the preferred way to create command IDs for statically-known commands.
    /// It's a const fn and involves no allocation.
    #[must_use]
    pub const fn new(module: ModuleId, name: &'static str) -> Self {
        Self {
            module,
            name: Cow::Borrowed(name),
        }
    }

    /// Create a command identifier from owned strings.
    ///
    /// Use this for dynamically-generated command IDs (e.g., picker selections,
    /// FFI calls, manifest parsing).
    #[must_use]
    #[allow(clippy::missing_const_for_fn)] // String operations aren't const-stable
    pub fn from_owned(module: ModuleId, name: String) -> Self {
        Self {
            module,
            name: Cow::Owned(name),
        }
    }

    /// Create a command identifier from a qualified string like "module:command".
    ///
    /// This method is intended for dynamic use cases like FFI where command IDs
    /// are specified as strings at runtime.
    ///
    /// If the string doesn't contain ':', the entire string is treated as the
    /// command name with "unknown" as the module.
    ///
    /// # Example
    ///
    /// ```
    /// use reovim_kernel::api::v1::CommandId;
    ///
    /// let cmd = CommandId::from_qualified("editor:cursor-down".to_string());
    /// assert_eq!(cmd.module().as_str(), "editor");
    /// assert_eq!(cmd.name(), "cursor-down");
    /// ```
    #[must_use]
    pub fn from_qualified(qualified: String) -> Self {
        let (module_str, name_str) = if let Some(idx) = qualified.find(':') {
            (qualified[..idx].to_string(), qualified[idx + 1..].to_string())
        } else {
            ("unknown".to_string(), qualified)
        };

        Self {
            module: ModuleId::from_string(module_str),
            name: Cow::Owned(name_str),
        }
    }

    /// Get the owning module.
    #[must_use]
    pub const fn module(&self) -> &ModuleId {
        &self.module
    }

    /// Get the local name.
    #[must_use]
    pub fn name(&self) -> &str {
        // SAFETY NOTE: This returns &str borrowing from &self.
        // If you need a value that outlives the CommandId (e.g. calling
        // .name() on a temporary), use .name_owned() instead.
        &self.name
    }

    /// Get the local name as an owned `Cow`.
    ///
    /// Use this when you need a value that isn't tied to the `CommandId`'s
    /// lifetime (e.g., when calling `.id().name_owned()` on a temporary).
    #[must_use]
    pub fn name_owned(&self) -> Cow<'static, str> {
        self.name.clone()
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
    // LLVM coverage artifact: default trait method body is a single `false` literal;
    // LLVM marks the closing brace DA:0 even when the method is called through overrides.
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn has_selection(&self) -> bool {
        false
    }

    /// Get the parent mode for keybinding inheritance.
    ///
    /// For example, `VisualLine` might inherit from Visual, which inherits
    /// from Normal. Returns `None` for root modes.
    // LLVM coverage artifact: default trait method body is a single `None` literal;
    // LLVM marks the closing brace DA:0 even when the method is called through overrides.
    #[cfg_attr(coverage_nightly, coverage(off))]
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
    #[cfg_attr(coverage_nightly, coverage(off))]
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

// =========================================================================
// #713 repro: CommandId::from_qualified now uses Cow<'static, str>.
// No more Box::leak — memory is freed when CommandId is dropped.
// =========================================================================

#[cfg(test)]
mod b2_repro {
    use super::CommandId;

    #[test]
    fn b2_from_qualified_no_longer_leaks() {
        // from_qualified uses Cow::Owned — memory freed on drop.
        for i in 0..1000 {
            let cmd = CommandId::from_qualified(format!("module{i}:command{i}"));
            assert_eq!(cmd.module().as_str(), format!("module{i}"));
            assert_eq!(cmd.name(), format!("command{i}").as_str());
        }
        // All 1000 CommandIds have been dropped — no leaked memory.
    }

    #[test]
    fn b2_static_new_still_const() {
        use crate::api::module::ModuleId;
        // const fn new() still works with static strings
        const CMD: CommandId = CommandId::new(ModuleId::new("editor"), "cursor-down");
        assert_eq!(CMD.name(), "cursor-down");
        assert_eq!(CMD.module().as_str(), "editor");
    }
}
