//! Plugin buffer provider trait

use crate::{event::KeyEvent, plugin::PluginStateRegistry, screen::Position};

/// Trait for plugins that provide buffer content
///
/// Plugins can implement this trait to create virtual buffers
/// with programmatically-generated content (e.g., file explorer, LSP diagnostics).
pub trait PluginBufferProvider: Send + Sync {
    /// Get the current lines for this buffer
    ///
    /// Called on every render to generate content.
    fn get_lines(&self, ctx: &BufferContext) -> Vec<String>;

    /// Handle cursor movement within the buffer
    ///
    /// Called when the cursor moves in this buffer's window.
    fn on_cursor_move(&mut self, position: Position, ctx: &mut BufferContext);

    /// Handle user input (optional - for interactive buffers)
    ///
    /// Called when a key is pressed while this buffer's window is active.
    /// Return `InputResult::Handled` to consume the event.
    fn on_input(&mut self, _key: KeyEvent, _ctx: &mut BufferContext) -> InputResult {
        InputResult::Unhandled
    }

    /// Whether this buffer allows editing
    ///
    /// If false, insert mode is disabled.
    fn is_editable(&self) -> bool {
        false
    }
}

/// Context passed to buffer provider methods
pub struct BufferContext<'a> {
    /// Buffer ID
    pub buffer_id: usize,
    /// Window width
    pub width: u16,
    /// Window height
    pub height: u16,
    /// Shared plugin state registry
    pub state: &'a PluginStateRegistry,
}

/// Result of input handling
pub enum InputResult {
    /// Input was handled by the provider
    Handled,
    /// Input was not handled (pass to default handler)
    Unhandled,
    /// Request to close this buffer/window
    RequestClose,
}
