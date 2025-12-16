//! Selection handling for buffers

use crate::screen::Position;

/// Represents a text selection with anchor and cursor positions
#[derive(Clone, Copy, Debug, Default)]
pub struct Selection {
    /// The fixed anchor point where selection started
    pub anchor: Position,
    /// Whether selection is active
    pub active: bool,
}

/// Selection operations for Buffer
pub trait SelectionOps {
    /// Start visual selection at current cursor position
    fn start_selection(&mut self);
    /// Clear selection
    fn clear_selection(&mut self);
    /// Get normalized selection bounds (start always before end)
    fn selection_bounds(&self) -> (Position, Position);
    /// Get selected text
    fn get_selected_text(&self) -> String;
    /// Delete selected text and return it
    fn delete_selection(&mut self) -> String;
}
