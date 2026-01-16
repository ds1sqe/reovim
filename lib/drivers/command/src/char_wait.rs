//! Character-wait state machine infrastructure.
//!
//! Commands that need a character argument (f/F/t/T/r) return
//! `CommandResult::WaitingForChar` with context about the operation.
//!
//! # Types
//!
//! - [`FindType`] - The four find-char motions (f, F, t, T)
//! - [`CharWaitOp`] - Generalized character-waiting operation type
//! - [`CharWaitContext`] - Context for a command waiting for character input

use reovim_kernel::api::v1::Position;

/// Type of find-char operation.
///
/// Represents the four find-char motions in Vim.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FindType {
    /// Find character forward, cursor on char (f)
    FindForward,
    /// Find character backward, cursor on char (F)
    FindBackward,
    /// Till character forward, cursor before char (t)
    TillForward,
    /// Till character backward, cursor after char (T)
    TillBackward,
}

/// Type of character-waiting operation.
///
/// Generalizes `FindType` to include operations beyond find-char motions.
/// Commands return this to indicate what operation is waiting for character input.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CharWaitOp {
    /// Find character forward, cursor on char (f)
    FindForward,
    /// Find character backward, cursor on char (F)
    FindBackward,
    /// Till character forward, cursor before char (t)
    TillForward,
    /// Till character backward, cursor after char (T)
    TillBackward,
    /// Replace character at cursor position (r{char})
    ReplaceChar,
}

impl CharWaitOp {
    /// Check if this is a find-char operation.
    #[must_use]
    pub const fn is_find_char(&self) -> bool {
        matches!(
            self,
            Self::FindForward | Self::FindBackward | Self::TillForward | Self::TillBackward
        )
    }

    /// Check if this is a replace-char operation.
    #[must_use]
    pub const fn is_replace_char(&self) -> bool {
        matches!(self, Self::ReplaceChar)
    }

    /// Convert to `FindType` if this is a find-char operation.
    #[must_use]
    pub const fn to_find_type(&self) -> Option<FindType> {
        match self {
            Self::FindForward => Some(FindType::FindForward),
            Self::FindBackward => Some(FindType::FindBackward),
            Self::TillForward => Some(FindType::TillForward),
            Self::TillBackward => Some(FindType::TillBackward),
            Self::ReplaceChar => None,
        }
    }
}

impl From<FindType> for CharWaitOp {
    fn from(find_type: FindType) -> Self {
        match find_type {
            FindType::FindForward => Self::FindForward,
            FindType::FindBackward => Self::FindBackward,
            FindType::TillForward => Self::TillForward,
            FindType::TillBackward => Self::TillBackward,
        }
    }
}

/// Context for a command waiting for character input.
///
/// Commands that need a character argument return this to indicate what
/// operation is pending. The runner uses this to set up pending-char state.
///
/// # Example
///
/// ```ignore
/// // Find-char-forward command (f)
/// fn execute(&self, ctx: &mut KernelContext, args: &CommandContext) -> CommandResult {
///     let pos = ctx.buffers.get(args.buffer_id().unwrap())
///         .unwrap()
///         .read()
///         .position();
///
///     CommandResult::WaitingForChar(CharWaitContext {
///         op_type: CharWaitOp::FindForward,
///         count: None,
///         start_position: Some(pos),
///     })
/// }
///
/// // Replace-char command (r)
/// fn execute(&self, ctx: &mut KernelContext, args: &CommandContext) -> CommandResult {
///     let count = args.count().unwrap_or(1);
///     CommandResult::WaitingForChar(CharWaitContext {
///         op_type: CharWaitOp::ReplaceChar,
///         count: Some(count),
///         start_position: None,
///     })
/// }
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CharWaitContext {
    /// The type of character-waiting operation.
    pub op_type: CharWaitOp,
    /// Count for the operation (e.g., 3rx replaces 3 chars).
    pub count: Option<usize>,
    /// Starting cursor position (for find-char operator range calculation).
    ///
    /// Required for find-char operations, not needed for replace-char.
    pub start_position: Option<Position>,
}

impl CharWaitContext {
    /// Create a new char-wait context for find-char operations.
    #[must_use]
    pub const fn find_char(find_type: FindType, start_position: Position) -> Self {
        Self {
            op_type: match find_type {
                FindType::FindForward => CharWaitOp::FindForward,
                FindType::FindBackward => CharWaitOp::FindBackward,
                FindType::TillForward => CharWaitOp::TillForward,
                FindType::TillBackward => CharWaitOp::TillBackward,
            },
            count: None,
            start_position: Some(start_position),
        }
    }

    /// Create a new char-wait context for replace-char operation.
    #[must_use]
    pub const fn replace_char(count: usize) -> Self {
        Self {
            op_type: CharWaitOp::ReplaceChar,
            count: Some(count),
            start_position: None,
        }
    }

    /// DEPRECATED: Create a new char-wait context (backward compatibility).
    ///
    /// Use `find_char()` or `replace_char()` instead.
    #[must_use]
    pub const fn new(find_type: FindType, start_position: Position) -> Self {
        Self::find_char(find_type, start_position)
    }

    /// Get the `FindType` if this is a find-char operation.
    #[must_use]
    pub const fn find_type(&self) -> Option<FindType> {
        self.op_type.to_find_type()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_find_type_variants() {
        // Verify all variants can be used
        assert_eq!(FindType::FindForward, FindType::FindForward);
        assert_eq!(FindType::FindBackward, FindType::FindBackward);
        assert_eq!(FindType::TillForward, FindType::TillForward);
        assert_eq!(FindType::TillBackward, FindType::TillBackward);
    }

    #[test]
    fn test_char_wait_context_new() {
        let ctx = CharWaitContext::new(FindType::FindForward, Position::new(1, 5));
        assert_eq!(ctx.find_type(), Some(FindType::FindForward));
        assert_eq!(ctx.start_position, Some(Position::new(1, 5)));
    }

    #[test]
    fn test_char_wait_context_find_char() {
        let ctx = CharWaitContext::find_char(FindType::TillBackward, Position::new(2, 3));
        assert_eq!(ctx.op_type, CharWaitOp::TillBackward);
        assert_eq!(ctx.find_type(), Some(FindType::TillBackward));
        assert_eq!(ctx.start_position, Some(Position::new(2, 3)));
        assert!(ctx.count.is_none());
    }

    #[test]
    fn test_char_wait_context_replace_char() {
        let ctx = CharWaitContext::replace_char(3);
        assert_eq!(ctx.op_type, CharWaitOp::ReplaceChar);
        assert_eq!(ctx.find_type(), None);
        assert!(ctx.start_position.is_none());
        assert_eq!(ctx.count, Some(3));
    }

    #[test]
    fn test_char_wait_op_is_find_char() {
        assert!(CharWaitOp::FindForward.is_find_char());
        assert!(CharWaitOp::FindBackward.is_find_char());
        assert!(CharWaitOp::TillForward.is_find_char());
        assert!(CharWaitOp::TillBackward.is_find_char());
        assert!(!CharWaitOp::ReplaceChar.is_find_char());
    }

    #[test]
    fn test_char_wait_op_is_replace_char() {
        assert!(!CharWaitOp::FindForward.is_replace_char());
        assert!(CharWaitOp::ReplaceChar.is_replace_char());
    }

    #[test]
    fn test_char_wait_op_to_find_type() {
        assert_eq!(CharWaitOp::FindForward.to_find_type(), Some(FindType::FindForward));
        assert_eq!(CharWaitOp::FindBackward.to_find_type(), Some(FindType::FindBackward));
        assert_eq!(CharWaitOp::TillForward.to_find_type(), Some(FindType::TillForward));
        assert_eq!(CharWaitOp::TillBackward.to_find_type(), Some(FindType::TillBackward));
        assert_eq!(CharWaitOp::ReplaceChar.to_find_type(), None);
    }

    #[test]
    fn test_char_wait_op_from_find_type() {
        let op: CharWaitOp = FindType::FindForward.into();
        assert_eq!(op, CharWaitOp::FindForward);

        let op: CharWaitOp = FindType::TillBackward.into();
        assert_eq!(op, CharWaitOp::TillBackward);
    }

    #[test]
    fn test_char_wait_context_equality() {
        let ctx1 = CharWaitContext::new(FindType::FindForward, Position::new(0, 5));
        let ctx2 = CharWaitContext::new(FindType::FindForward, Position::new(0, 5));
        let ctx3 = CharWaitContext::new(FindType::FindBackward, Position::new(0, 5));

        assert_eq!(ctx1, ctx2);
        assert_ne!(ctx1, ctx3);
    }

    #[test]
    fn test_find_type_equality() {
        assert_eq!(FindType::FindForward, FindType::FindForward);
        assert_ne!(FindType::FindForward, FindType::FindBackward);
        assert_ne!(FindType::TillForward, FindType::FindForward);
    }
}
