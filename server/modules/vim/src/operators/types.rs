//! Operator types - POLICY.
//!
//! These types define the operator system for vim-style editing.
//! They were moved from kernel to this module following mechanism-vs-policy.
//!
//! - **Mechanism (Kernel)**: Buffer management, position types, register storage
//! - **Policy (This Module)**: What operators exist, how they behave

use {
    reovim_kernel::api::v1::{BufferId, KernelContext},
    reovim_types_text::{HistoryRing, Position, Register, RegisterBank},
};

// ============================================================================
// Range Type
// ============================================================================

/// Text range for operator execution.
///
/// Represents a contiguous range of text in a buffer.
/// Used by operators to know which text to act on.
///
/// The `is_linewise` flag indicates whether the range should be treated
/// as spanning complete lines (e.g., `yj`, `dd`) or character positions
/// (e.g., `yw`, `d$`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Range {
    /// Start position (inclusive).
    pub start: Position,
    /// End position (exclusive).
    pub end: Position,
    /// Whether this range represents linewise selection.
    ///
    /// Linewise ranges affect paste behavior:
    /// - `true`: Paste inserts on new lines (above/below)
    /// - `false`: Paste inserts inline at cursor
    pub is_linewise: bool,
}

impl Range {
    /// Create a new characterwise range (default).
    #[must_use]
    pub const fn new(start: Position, end: Position) -> Self {
        Self {
            start,
            end,
            is_linewise: false,
        }
    }

    /// Create a new linewise range.
    #[must_use]
    pub const fn linewise(start: Position, end: Position) -> Self {
        Self {
            start,
            end,
            is_linewise: true,
        }
    }

    /// Create a range from a single position (zero-width, characterwise).
    #[must_use]
    pub const fn from_position(pos: Position) -> Self {
        Self {
            start: pos,
            end: pos,
            is_linewise: false,
        }
    }

    /// Convert this range to linewise.
    #[must_use]
    pub const fn to_linewise(self) -> Self {
        Self {
            start: self.start,
            end: self.end,
            is_linewise: true,
        }
    }

    /// Check if the range is empty.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.start.line == self.end.line && self.start.column == self.end.column
    }

    /// Check if this is a single-line range.
    #[must_use]
    pub const fn is_single_line(&self) -> bool {
        self.start.line == self.end.line
    }

    /// Get the number of lines spanned.
    #[must_use]
    pub const fn line_count(&self) -> usize {
        if self.end.line >= self.start.line {
            self.end.line - self.start.line + 1
        } else {
            0
        }
    }

    /// Normalize the range so start <= end.
    #[must_use]
    #[allow(clippy::missing_const_for_fn)]
    pub fn normalized(self) -> Self {
        if self.start.line > self.end.line
            || (self.start.line == self.end.line && self.start.column > self.end.column)
        {
            Self {
                start: self.end,
                end: self.start,
                is_linewise: self.is_linewise,
            }
        } else {
            self
        }
    }
}

// ============================================================================
// Operator Trait
// ============================================================================

/// Operators execute actions on text ranges.
///
/// - **Mechanism (Kernel)**: Range calculation, text manipulation APIs
/// - **Policy (Module)**: What each operator does with the range
///
/// # Examples
///
/// - `delete` - Remove text in range, save to register
/// - `yank` - Copy text in range to register
/// - `change` - Delete text and enter insert mode
/// - `indent` - Increase indentation of lines in range
pub trait Operator: Send + Sync {
    /// Unique identifier for this operator.
    fn id(&self) -> &'static str;

    /// Execute the operator on a range.
    ///
    /// # Arguments
    ///
    /// * `ctx` - Execution context with kernel access
    /// * `range` - Text range to operate on
    ///
    /// # Errors
    ///
    /// Returns `OperatorError` if the operation fails.
    fn execute(&self, ctx: &mut OperatorContext<'_>, range: Range) -> Result<(), OperatorError>;

    /// Whether this operator works on whole lines (dd, yy).
    ///
    /// Linewise operators extend the range to include full lines.
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn is_linewise(&self) -> bool {
        false
    }

    /// Whether this operator modifies text.
    ///
    /// Used for undo grouping - modifying operators create checkpoints.
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn is_text_modifying(&self) -> bool {
        true
    }
}

/// Context passed to operator execution.
pub struct OperatorContext<'a> {
    /// Kernel context for accessing services.
    pub kernel: &'a KernelContext,
    /// Per-client register storage (#515).
    pub registers: &'a mut RegisterBank,
    /// Per-client clipboard history (#515).
    pub clipboard_history: &'a mut HistoryRing,
    /// Buffer to operate on.
    pub buffer_id: BufferId,
    /// Target register for operator output.
    pub register: Register,
    /// Count prefix (e.g., `3dd` has count 3).
    pub count: usize,
    /// Cursor position before operator execution (for undo tracking).
    ///
    /// This is passed from the caller who has access to window state.
    /// Used to record `cursor_before` for undo.
    pub cursor_position: Position,
    /// Cursor position the operator requests after execution.
    ///
    /// Set by operators during `execute()` to communicate desired post-operation
    /// cursor position. `None` means the caller applies a default clamped fallback.
    /// This makes `OperatorContext` a bidirectional channel — consistent with
    /// `registers` and `clipboard_history` which are also mutated during execute.
    pub cursor_after: Option<Position>,
}

/// Operator execution errors.
#[derive(Debug, Clone)]
pub enum OperatorError {
    /// Buffer not found.
    BufferNotFound(BufferId),
    /// Invalid range.
    InvalidRange(Range),
    /// Operation failed.
    OperationFailed(String),
}

impl std::fmt::Display for OperatorError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::BufferNotFound(id) => write!(f, "buffer not found: {id:?}"),
            Self::InvalidRange(r) => write!(f, "invalid range: {r:?}"),
            Self::OperationFailed(msg) => write!(f, "operation failed: {msg}"),
        }
    }
}

impl std::error::Error for OperatorError {}

// ============================================================================
// Tests
// ============================================================================
