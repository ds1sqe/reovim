//! Visual mode selection state for the `gv` reselect command.
//!
//! Tracks the last visual selection so it can be restored.

use reovim_kernel::api::v1::{BufferId, ModeId, Position, SelectionMode};

// ============================================================================
// Visual Mode Selection Infrastructure
// ============================================================================

/// Record of the last visual selection for `gv` (reselect) command.
///
/// When visual mode is exited, the selection boundaries are stored here
/// so that `gv` can restore the exact same selection.
#[derive(Debug, Clone)]
pub struct LastVisualSelection {
    /// The buffer where the selection was made.
    pub buffer_id: BufferId,
    /// The anchor position (where visual mode was entered).
    pub anchor: Position,
    /// The cursor position (where visual mode was exited).
    pub cursor: Position,
    /// The selection mode (Character, Line, or Block).
    pub mode: SelectionMode,
    /// The visual mode ID to restore (editor:visual, editor:visual-line, etc.).
    ///
    /// Storing the actual `ModeId` removes the need for hardcoded mode mapping
    /// from `SelectionMode` to `ModeId` in the runner.
    pub mode_id: ModeId,
}

impl LastVisualSelection {
    /// Create a new last visual selection record.
    #[must_use]
    #[allow(clippy::missing_const_for_fn)] // ModeId cannot be const-constructed
    pub fn new(
        buffer_id: BufferId,
        anchor: Position,
        cursor: Position,
        mode: SelectionMode,
        mode_id: ModeId,
    ) -> Self {
        Self {
            buffer_id,
            anchor,
            cursor,
            mode,
            mode_id,
        }
    }
}

#[cfg(test)]
mod tests {
    use {super::*, reovim_kernel::api::v1::ModuleId};

    fn test_visual_mode_id() -> ModeId {
        ModeId::new(ModuleId::new("editor"), "visual")
    }

    fn test_visual_line_mode_id() -> ModeId {
        ModeId::new(ModuleId::new("editor"), "visual-line")
    }

    fn test_visual_block_mode_id() -> ModeId {
        ModeId::new(ModuleId::new("editor"), "visual-block")
    }

    #[test]
    fn test_last_visual_selection_new() {
        let buffer_id = BufferId::from_raw(1);
        let anchor = Position::new(0, 5);
        let cursor = Position::new(2, 10);
        let mode = SelectionMode::Character;
        let mode_id = test_visual_mode_id();

        let selection = LastVisualSelection::new(buffer_id, anchor, cursor, mode, mode_id.clone());

        assert_eq!(selection.buffer_id, buffer_id);
        assert_eq!(selection.anchor, anchor);
        assert_eq!(selection.cursor, cursor);
        assert_eq!(selection.mode, mode);
        assert_eq!(selection.mode_id, mode_id);
    }

    #[test]
    fn test_last_visual_selection_line_mode() {
        let buffer_id = BufferId::from_raw(2);
        let selection = LastVisualSelection::new(
            buffer_id,
            Position::new(1, 0),
            Position::new(3, 0),
            SelectionMode::Line,
            test_visual_line_mode_id(),
        );

        assert_eq!(selection.mode, SelectionMode::Line);
    }

    #[test]
    fn test_last_visual_selection_block_mode() {
        let buffer_id = BufferId::from_raw(3);
        let selection = LastVisualSelection::new(
            buffer_id,
            Position::new(0, 0),
            Position::new(5, 10),
            SelectionMode::Block,
            test_visual_block_mode_id(),
        );

        assert_eq!(selection.mode, SelectionMode::Block);
    }
}
