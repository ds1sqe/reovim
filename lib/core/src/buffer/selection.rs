//! Selection handling for buffers

use crate::screen::Position;

/// Selection mode (character-wise vs block-wise)
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum SelectionMode {
    /// Normal visual mode - character-wise selection (v)
    #[default]
    Character,
    /// Block visual mode - rectangular selection (Ctrl-V)
    Block,
    /// Line visual mode - line-wise selection (V) - future
    Line,
}

/// Represents a text selection with anchor and cursor positions
#[derive(Clone, Copy, Debug, Default)]
pub struct Selection {
    /// The fixed anchor point where selection started
    pub anchor: Position,
    /// Whether selection is active
    pub active: bool,
    /// Selection mode (character, block, or line)
    pub mode: SelectionMode,
}

/// Selection operations for Buffer
pub trait SelectionOps {
    /// Start visual selection at current cursor position (character mode)
    fn start_selection(&mut self);
    /// Start block selection at current cursor position
    fn start_block_selection(&mut self);
    /// Clear selection
    fn clear_selection(&mut self);
    /// Get normalized selection bounds (start always before end)
    fn selection_bounds(&self) -> (Position, Position);
    /// Get block selection bounds (top-left, bottom-right corners)
    fn block_bounds(&self) -> (Position, Position);
    /// Get selection mode
    fn selection_mode(&self) -> SelectionMode;
    /// Get selected text
    fn get_selected_text(&self) -> String;
    /// Delete selected text and return it
    fn delete_selection(&mut self) -> String;
}
