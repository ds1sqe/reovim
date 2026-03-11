//! Render backend abstraction and TUI extension trait.
//!
//! `RenderBackend` abstracts over different render targets (`Screen` for
//! interactive TUI, `FrameBuffer` for headless TUI).
//!
//! `TuiExtension` is the trait for client-side extensions that own their
//! state, handle notifications, and render through `RenderBackend`.

use {
    crate::{Cell, FrameBuffer, Style},
    reovim_arch::Color,
    std::borrow::Cow,
};

// ============================================================================
// RenderBackend trait
// ============================================================================

/// Trait for render backends that can display cell-based content.
///
/// This trait abstracts over `Screen` (terminal) and `FrameBuffer` (memory),
/// allowing unified rendering code to work with both interactive and headless TUIs.
pub trait RenderBackend {
    /// Write a character at (x, y) with the given style.
    ///
    /// Coordinates are 0-indexed. Out-of-bounds writes are silently ignored.
    fn set_cell(&mut self, x: u16, y: u16, ch: char, style: &Style);

    /// Apply style to an existing cell without changing its character.
    ///
    /// Used for overlays like cursor highlighting where we want to preserve
    /// the underlying character but change its appearance.
    fn apply_style(&mut self, x: u16, y: u16, style: &Style);

    /// Write a string starting at (x, y) with the given style.
    ///
    /// Returns the number of columns used (accounting for wide characters).
    /// Does not wrap to the next line.
    fn write_str(&mut self, x: u16, y: u16, text: &str, style: &Style) -> u16;

    /// Get the dimensions of the render target.
    fn size(&self) -> (u16, u16);

    /// Clear the entire render target.
    fn clear(&mut self);

    /// Overlay a background color on an existing cell.
    ///
    /// Preserves the character and foreground color, only changing background.
    /// Used for selection highlighting.
    fn overlay_bg(&mut self, x: u16, y: u16, bg: Color);

    /// Fill a horizontal line with a character.
    fn fill_horizontal(&mut self, x: u16, y: u16, width: u16, ch: char, style: &Style) {
        for col in x..x.saturating_add(width) {
            self.set_cell(col, y, ch, style);
        }
    }

    /// Fill a vertical line with a character.
    fn fill_vertical(&mut self, x: u16, y: u16, height: u16, ch: char, style: &Style) {
        for row in y..y.saturating_add(height) {
            self.set_cell(x, row, ch, style);
        }
    }

    /// Fill a rectangular region with a character.
    fn fill_region(&mut self, x: u16, y: u16, width: u16, height: u16, ch: char, style: &Style) {
        let (w, h) = self.size();
        for row in y..y.saturating_add(height).min(h) {
            for col in x..x.saturating_add(width).min(w) {
                self.set_cell(col, row, ch, style);
            }
        }
    }
}

// ============================================================================
// FrameBuffer impl
// ============================================================================

impl RenderBackend for FrameBuffer {
    fn set_cell(&mut self, x: u16, y: u16, ch: char, style: &Style) {
        self.put_char(x, y, ch, style);
    }

    #[allow(clippy::use_self)] // FrameBuffer::apply_style is inherent method
    fn apply_style(&mut self, x: u16, y: u16, style: &Style) {
        FrameBuffer::apply_style(self, x, y, style);
    }

    #[allow(clippy::use_self)] // FrameBuffer::write_str is inherent method
    fn write_str(&mut self, x: u16, y: u16, text: &str, style: &Style) -> u16 {
        FrameBuffer::write_str(self, x, y, text, style)
    }

    fn size(&self) -> (u16, u16) {
        (self.width(), self.height())
    }

    #[allow(clippy::use_self)] // FrameBuffer::clear is inherent method
    fn clear(&mut self) {
        FrameBuffer::clear(self);
    }

    fn overlay_bg(&mut self, x: u16, y: u16, bg: Color) {
        // FrameBuffer doesn't have overlay_bg, implement manually
        if let Some(cell) = self.get(x, y).cloned() {
            let mut new_style = cell.style.clone();
            new_style.bg = Some(bg);
            let new_cell = Cell::new(cell.char, new_style);
            self.set(x, y, new_cell);
        }
    }
}

// ============================================================================
// ViewportContext
// ============================================================================

/// Viewport context for extensions that render at buffer positions.
///
/// Provides the mapping between buffer coordinates (line, col) and screen
/// coordinates (x, y). Passed to [`TuiExtension::render_with_viewport`]
/// by the render engine.
///
/// Extensions that render at absolute screen positions (popups, sidebars)
/// can ignore this — the default `render_with_viewport` delegates to `render()`.
#[derive(Debug, Clone)]
pub struct ViewportContext {
    /// First visible buffer line (0-indexed).
    ///
    /// A buffer match at `line` maps to screen row `line - scroll_top`.
    pub scroll_top: usize,
    /// X coordinate where buffer content starts (gutter + sidebar inset).
    ///
    /// A buffer match at `col` maps to screen x `content_x + col`.
    pub content_x: u16,
    /// Number of visible buffer rows (terminal height minus statusline).
    pub content_height: u16,
    /// Buffer ID of the focused window (`None` if unknown).
    ///
    /// Used by inline extensions (e.g., diagnostics) that filter entries
    /// by the current buffer.
    pub buffer_id: Option<u64>,
}

// ============================================================================
// RenderBehavior — shared type for token classification
// ============================================================================

/// How the client renders a token based on its semantic category.
///
/// The render engine executes these as mechanism. Extensions decide
/// which categories map to which behavior (policy) via
/// [`TuiExtension::classify_token`].
#[derive(Debug, Clone)]
pub enum RenderBehavior {
    /// Apply a style overlay (the common case — syntax highlighting).
    Highlight,
    /// Conceal the byte range and replace with a glyph.
    Conceal { replacement: Cow<'static, str> },
    /// Apply a background color only.
    Background,
    /// Hide the byte range entirely (zero-width conceal).
    Hide,
    /// Fill the entire viewport width with a repeated character.
    FullWidthLine { ch: char },
}

// ============================================================================
// Virtual line types
// ============================================================================

/// A virtual line injected between buffer lines.
///
/// Counterpart to [`TuiExtension::fold_hidden_lines`] which removes lines.
/// The render engine inserts these at the appropriate screen position.
#[derive(Debug, Clone)]
pub struct VirtualLine {
    /// Buffer line this virtual line is associated with.
    pub buffer_line: usize,
    /// Whether to insert before or after the buffer line.
    pub position: VirtualLinePosition,
    /// Text content of the virtual line.
    pub content: String,
    /// Style for the virtual line.
    pub style: Style,
}

/// Where to insert a virtual line relative to a buffer line.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VirtualLinePosition {
    /// Insert before the associated buffer line.
    Before,
    /// Insert after the associated buffer line.
    After,
}

/// A transformed line replacing the raw buffer content.
///
/// Returned by [`TuiExtension::transform_line`] when an extension wants
/// to replace a buffer line with different visual content (e.g., expanded
/// table rows with box-drawing borders).
#[derive(Debug, Clone)]
pub struct TransformedLine {
    /// Replacement text to render instead of the buffer line.
    pub text: String,
    /// Per-character style: `styles[i]` applies to `text.chars().nth(i)`.
    /// If shorter than text, remaining chars use default style.
    pub styles: Vec<Option<Style>>,
}

// ============================================================================
// TuiExtension trait
// ============================================================================

/// Trait for client-side TUI extensions.
///
/// Extensions own their state, handle notifications, and render through
/// `RenderBackend`. The engine dispatches generically via
/// `Vec<Box<dyn TuiExtension>>` — zero extension knowledge.
///
/// # Game-Mod Separation
///
/// The engine NEVER imports individual extension crates. It only knows
/// this trait. Concrete extensions are provided by a `defaults` meta-crate
/// via `create_extensions() -> Vec<Box<dyn TuiExtension>>`.
///
/// # Lifecycle
///
/// 1. Engine calls `create_extensions()` at startup
/// 2. On `ExtensionUpdated` notification: engine calls `apply_notification()`
///    for extensions matching the notification's `kind`
/// 3. On render: engine iterates extensions, calls `render_with_viewport()`
///    on active ones (with viewport context for buffer-position mapping)
pub trait TuiExtension: Send + Sync {
    /// Extension kind identifier.
    ///
    /// Must match the `kind` field in gRPC `ExtensionUpdatedPayload`.
    fn kind(&self) -> &'static str;

    /// Whether this extension is currently active (visible).
    ///
    /// The engine only calls `render()` when this returns `true`.
    fn is_active(&self) -> bool;

    /// Apply notification data from the server.
    ///
    /// `data` is the JSON string from `ExtensionUpdatedPayload.data`,
    /// produced by the server-side `ExtensionStateBridge::snapshot()`.
    fn apply_notification(&mut self, data: &str);

    /// Render to the given backend.
    ///
    /// Called by the engine for each active extension after core content
    /// (buffer, cursors, statusline) has been rendered.
    fn render(&self, backend: &mut dyn RenderBackend);

    /// Periodic tick for time-based state transitions.
    ///
    /// Called by the engine on each redraw timer tick (~16ms / 60 FPS).
    /// Extensions that need delayed visibility (e.g., show-after-timeout)
    /// override this to check elapsed time and update state.
    ///
    /// Returns `true` if the extension's visible state changed (forces redraw).
    /// Default: no-op, returns `false`.
    fn tick(&mut self) -> bool {
        false
    }

    /// Hardware cursor position override.
    ///
    /// When this extension is active and wants to control the terminal's
    /// hardware cursor (e.g., a command-line input popup), return
    /// `Some((x, y))`. The engine uses the first non-`None` result.
    ///
    /// Default: `None` (no cursor override).
    fn cursor_position(&self, _terminal_width: u16, _terminal_height: u16) -> Option<(u16, u16)> {
        None
    }

    /// Content area left inset for sidebar extensions.
    ///
    /// When active, the render engine shifts buffer content, selections,
    /// cursors, and line numbers right by this amount to make room for
    /// the sidebar. Multiple sidebars can stack (values are summed).
    ///
    /// Default: `0` (no inset).
    fn content_offset_left(&self) -> u16 {
        0
    }

    /// Ranges of buffer lines hidden by this extension.
    ///
    /// Returns `(start_line, hidden_count)` pairs indicating which buffer
    /// lines should be skipped during rendering. The render engine collects
    /// these from all extensions to build a fold-aware line mapping.
    ///
    /// Default: empty (no lines hidden).
    fn fold_hidden_lines(&self) -> &[(u32, u32)] {
        &[]
    }

    /// Render with viewport context for buffer-position mapping.
    ///
    /// Called by the render engine instead of [`render`] when viewport
    /// context is available. Extensions that render at buffer positions
    /// (e.g., jump labels, fold markers) override this to use the viewport
    /// for coordinate mapping.
    ///
    /// Default: ignores viewport, delegates to [`render`].
    fn render_with_viewport(&self, backend: &mut dyn RenderBackend, _viewport: &ViewportContext) {
        self.render(backend);
    }

    /// Classify a token category into a render behavior.
    ///
    /// Return `Some(behavior)` to claim this category. The render engine
    /// queries all active extensions; first `Some` wins. If no extension
    /// claims a category, the engine falls back to [`RenderBehavior::Highlight`].
    ///
    /// Default: `None` (does not claim any categories).
    fn classify_token(&self, _category: &str) -> Option<RenderBehavior> {
        None
    }

    /// Called when buffer content changes.
    ///
    /// Extensions that need to analyze buffer content (e.g., detect tables)
    /// cache derived state here. Called after the buffer cache is updated.
    ///
    /// Default: no-op.
    fn on_buffer_update(&mut self, _buffer_id: u64, _lines: &[String]) {}

    /// Virtual lines to inject between buffer lines.
    ///
    /// Counterpart to [`fold_hidden_lines`](Self::fold_hidden_lines) (which removes lines).
    /// The render engine inserts these at appropriate screen positions.
    ///
    /// Default: empty (no virtual lines).
    fn virtual_lines(&self) -> &[VirtualLine] {
        &[]
    }

    /// Transform a buffer line's visual content.
    ///
    /// Return `Some(TransformedLine)` to replace the raw buffer line with
    /// custom content (e.g., expanded table rows). First extension returning
    /// `Some` wins.
    ///
    /// Default: `None` (no transformation).
    fn transform_line(
        &self,
        _buffer_id: u64,
        _line_idx: usize,
        _line: &str,
    ) -> Option<TransformedLine> {
        None
    }

    /// Map buffer column to visual column for a transformed line.
    ///
    /// Used for cursor positioning inside transformed lines.
    /// Return `None` to use default 1:1 mapping.
    ///
    /// Default: `None`.
    fn map_cursor_column(
        &self,
        _buffer_id: u64,
        _line_idx: usize,
        _buffer_col: usize,
    ) -> Option<u16> {
        None
    }

    /// Called when cursor position changes.
    ///
    /// Default: no-op.
    fn on_cursor_update(&mut self, _buffer_id: u64, _line: usize, _col: usize) {}

    /// Called when mode changes.
    ///
    /// Default: no-op.
    fn on_mode_change(&mut self, _mode_name: &str, _is_insert: bool) {}

    /// Extension kinds that this extension depends on.
    ///
    /// Extensions listed here will be initialized before this one.
    /// Used by `create_extensions()` to topologically sort the extension list.
    ///
    /// Default: empty (no dependencies).
    fn dependencies(&self) -> &[&'static str] {
        &[]
    }

    /// Called once after all extensions have been created and sorted.
    ///
    /// Use for initialization that depends on the extension system being ready.
    /// Called in dependency order (dependencies first).
    ///
    /// Default: no-op.
    fn init(&mut self) {}

    /// Called during shutdown in reverse dependency order.
    ///
    /// Use for cleanup that must happen before the extension system tears down.
    ///
    /// Default: no-op.
    fn exit(&mut self) {}
}

#[cfg(test)]
#[path = "render_backend_tests.rs"]
mod tests;
