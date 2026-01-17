//! Character operation types for find-char and replace-char commands.
//!
//! Handles f/F/t/T motions and r{char} replacement commands that
//! require a character argument to complete.

use reovim_kernel::api::v1::{Direction, Motion, Position};

// ============================================================================
// Pending Character Operation Infrastructure
// ============================================================================

/// Type of find-char operation.
///
/// Represents the four find-char motions in Vim:
/// - `f` - find forward, cursor on char
/// - `F` - find backward, cursor on char
/// - `t` - till forward, cursor before char
/// - `T` - till backward, cursor after char
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

impl FindType {
    /// Convert to kernel Direction.
    #[must_use]
    pub const fn direction(self) -> Direction {
        match self {
            Self::FindForward | Self::TillForward => Direction::Forward,
            Self::FindBackward | Self::TillBackward => Direction::Backward,
        }
    }

    /// Whether this is a "till" motion (stops before/after the character).
    #[must_use]
    pub const fn is_till(self) -> bool {
        matches!(self, Self::TillForward | Self::TillBackward)
    }
}

/// Pending character operation.
///
/// Commands that need a single character argument return
/// `CommandResult::WaitingForChar` and the event loop stores
/// the operation here until the character is received.
///
/// This unified enum covers both:
/// - Find-char motions (f, F, t, T) - need a character to search for
/// - Replace-char operation (r{char}) - need a character to replace with
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PendingCharOp {
    /// Find character forward (f)
    FindForward {
        /// Starting position for operator range calculation.
        start: Position,
    },
    /// Find character backward (F)
    FindBackward {
        /// Starting position for operator range calculation.
        start: Position,
    },
    /// Till character forward (t)
    TillForward {
        /// Starting position for operator range calculation.
        start: Position,
    },
    /// Till character backward (T)
    TillBackward {
        /// Starting position for operator range calculation.
        start: Position,
    },
    /// Replace character (r{char})
    ReplaceChar {
        /// Count for replacement (e.g., 3rx replaces 3 chars with 'x').
        count: usize,
    },
}

impl PendingCharOp {
    /// Create a find-forward pending operation.
    #[must_use]
    pub const fn find_forward(start: Position) -> Self {
        Self::FindForward { start }
    }

    /// Create a find-backward pending operation.
    #[must_use]
    pub const fn find_backward(start: Position) -> Self {
        Self::FindBackward { start }
    }

    /// Create a till-forward pending operation.
    #[must_use]
    pub const fn till_forward(start: Position) -> Self {
        Self::TillForward { start }
    }

    /// Create a till-backward pending operation.
    #[must_use]
    pub const fn till_backward(start: Position) -> Self {
        Self::TillBackward { start }
    }

    /// Create a replace-char pending operation.
    #[must_use]
    pub const fn replace_char(count: usize) -> Self {
        Self::ReplaceChar { count }
    }

    /// Create from `FindType` and position (for find-char commands).
    #[must_use]
    pub const fn from_find_type(find_type: FindType, start: Position) -> Self {
        match find_type {
            FindType::FindForward => Self::FindForward { start },
            FindType::FindBackward => Self::FindBackward { start },
            FindType::TillForward => Self::TillForward { start },
            FindType::TillBackward => Self::TillBackward { start },
        }
    }

    /// Check if this is a find-char operation.
    #[must_use]
    pub const fn is_find_char(&self) -> bool {
        matches!(
            self,
            Self::FindForward { .. }
                | Self::FindBackward { .. }
                | Self::TillForward { .. }
                | Self::TillBackward { .. }
        )
    }

    /// Check if this is a replace-char operation.
    #[must_use]
    pub const fn is_replace_char(&self) -> bool {
        matches!(self, Self::ReplaceChar { .. })
    }

    /// Get the `FindType` if this is a find-char operation.
    #[must_use]
    pub const fn find_type(&self) -> Option<FindType> {
        match self {
            Self::FindForward { .. } => Some(FindType::FindForward),
            Self::FindBackward { .. } => Some(FindType::FindBackward),
            Self::TillForward { .. } => Some(FindType::TillForward),
            Self::TillBackward { .. } => Some(FindType::TillBackward),
            Self::ReplaceChar { .. } => None,
        }
    }

    /// Get the start position if this is a find-char operation.
    #[must_use]
    pub const fn start_position(&self) -> Option<Position> {
        match self {
            Self::FindForward { start }
            | Self::FindBackward { start }
            | Self::TillForward { start }
            | Self::TillBackward { start } => Some(*start),
            Self::ReplaceChar { .. } => None,
        }
    }
}

/// Record of the last find-char operation for `;` and `,` repeat.
///
/// Stores the character and find type so that `;` can repeat the same
/// find and `,` can repeat in the opposite direction.
#[derive(Debug, Clone, Copy)]
pub struct LastFind {
    /// The character that was searched for.
    pub char: char,
    /// The type of find operation.
    pub find_type: FindType,
}

impl LastFind {
    /// Create a new last-find record.
    #[must_use]
    pub const fn new(char: char, find_type: FindType) -> Self {
        Self { char, find_type }
    }

    /// Create the Motion for repeating in the same direction (`;`).
    #[must_use]
    pub const fn repeat_motion(self) -> Motion {
        Motion::FindChar {
            char: self.char,
            direction: self.find_type.direction(),
            till: self.find_type.is_till(),
        }
    }

    /// Create the Motion for repeating in opposite direction (`,`).
    #[must_use]
    pub const fn reverse_motion(self) -> Motion {
        Motion::FindChar {
            char: self.char,
            direction: self.find_type.direction().opposite(),
            till: self.find_type.is_till(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_find_type_direction() {
        assert_eq!(FindType::FindForward.direction(), Direction::Forward);
        assert_eq!(FindType::FindBackward.direction(), Direction::Backward);
        assert_eq!(FindType::TillForward.direction(), Direction::Forward);
        assert_eq!(FindType::TillBackward.direction(), Direction::Backward);
    }

    #[test]
    fn test_find_type_is_till() {
        assert!(!FindType::FindForward.is_till());
        assert!(!FindType::FindBackward.is_till());
        assert!(FindType::TillForward.is_till());
        assert!(FindType::TillBackward.is_till());
    }

    #[test]
    fn test_last_find_new() {
        let last = LastFind::new('x', FindType::FindForward);
        assert_eq!(last.char, 'x');
        assert_eq!(last.find_type, FindType::FindForward);
    }

    #[test]
    fn test_last_find_repeat_motion() {
        let last = LastFind::new('x', FindType::FindForward);
        let motion = last.repeat_motion();

        assert_eq!(
            motion,
            Motion::FindChar {
                char: 'x',
                direction: Direction::Forward,
                till: false,
            }
        );
    }

    #[test]
    fn test_last_find_reverse_motion() {
        let last = LastFind::new('x', FindType::FindForward);
        let motion = last.reverse_motion();

        assert_eq!(
            motion,
            Motion::FindChar {
                char: 'x',
                direction: Direction::Backward,
                till: false,
            }
        );
    }

    #[test]
    fn test_last_find_till_repeat_motion() {
        let last = LastFind::new('.', FindType::TillForward);
        let motion = last.repeat_motion();

        assert_eq!(
            motion,
            Motion::FindChar {
                char: '.',
                direction: Direction::Forward,
                till: true,
            }
        );
    }

    #[test]
    fn test_last_find_till_reverse_motion() {
        let last = LastFind::new('.', FindType::TillBackward);
        let motion = last.reverse_motion();

        assert_eq!(
            motion,
            Motion::FindChar {
                char: '.',
                direction: Direction::Forward,
                till: true,
            }
        );
    }

    #[test]
    fn test_pending_char_op_find_forward() {
        let op = PendingCharOp::find_forward(Position::new(0, 5));
        assert!(op.is_find_char());
        assert!(!op.is_replace_char());
        assert_eq!(op.find_type(), Some(FindType::FindForward));
        assert_eq!(op.start_position(), Some(Position::new(0, 5)));
    }

    #[test]
    fn test_pending_char_op_find_backward() {
        let op = PendingCharOp::find_backward(Position::new(1, 3));
        assert!(op.is_find_char());
        assert_eq!(op.find_type(), Some(FindType::FindBackward));
        assert_eq!(op.start_position(), Some(Position::new(1, 3)));
    }

    #[test]
    fn test_pending_char_op_till_forward() {
        let op = PendingCharOp::till_forward(Position::new(0, 0));
        assert!(op.is_find_char());
        assert_eq!(op.find_type(), Some(FindType::TillForward));
    }

    #[test]
    fn test_pending_char_op_till_backward() {
        let op = PendingCharOp::till_backward(Position::new(2, 10));
        assert!(op.is_find_char());
        assert_eq!(op.find_type(), Some(FindType::TillBackward));
    }

    #[test]
    fn test_pending_char_op_replace_char() {
        let op = PendingCharOp::replace_char(3);
        assert!(!op.is_find_char());
        assert!(op.is_replace_char());
        assert_eq!(op.find_type(), None);
        assert_eq!(op.start_position(), None);
    }

    #[test]
    fn test_pending_char_op_from_find_type() {
        let op = PendingCharOp::from_find_type(FindType::TillBackward, Position::new(5, 5));
        assert_eq!(op.find_type(), Some(FindType::TillBackward));
        assert_eq!(op.start_position(), Some(Position::new(5, 5)));
    }
}
