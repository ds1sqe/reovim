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
/// 3. On render: engine iterates extensions, calls `render()` on active ones
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
}

#[cfg(test)]
mod tests {
    use super::*;

    // =========================================================================
    // RenderBackend for FrameBuffer tests
    // =========================================================================

    #[test]
    fn test_framebuffer_set_cell() {
        let mut fb = FrameBuffer::new(10, 5);
        let style = Style::default();
        fb.set_cell(0, 0, 'H', &style);
        fb.set_cell(1, 0, 'i', &style);
        assert_eq!(fb.get(0, 0).unwrap().char, 'H');
        assert_eq!(fb.get(1, 0).unwrap().char, 'i');
    }

    #[test]
    fn test_framebuffer_write_str() {
        let mut fb = FrameBuffer::new(20, 5);
        let style = Style::default();
        let cols = RenderBackend::write_str(&mut fb, 0, 0, "Hello", &style);
        assert_eq!(cols, 5);
        assert_eq!(fb.get(0, 0).unwrap().char, 'H');
        assert_eq!(fb.get(4, 0).unwrap().char, 'o');
    }

    #[test]
    fn test_framebuffer_size() {
        let fb = FrameBuffer::new(80, 24);
        assert_eq!(RenderBackend::size(&fb), (80, 24));
    }

    #[test]
    fn test_framebuffer_clear() {
        let mut fb = FrameBuffer::new(10, 5);
        let style = Style::default();
        fb.set_cell(0, 0, 'X', &style);
        RenderBackend::clear(&mut fb);
        assert_eq!(fb.get(0, 0).unwrap().char, ' ');
    }

    #[test]
    fn test_framebuffer_apply_style() {
        let mut fb = FrameBuffer::new(10, 5);
        let style = Style::default();
        fb.set_cell(0, 0, 'A', &style);
        let highlight = Style::default().bg(Color::Yellow);
        RenderBackend::apply_style(&mut fb, 0, 0, &highlight);
        let cell = fb.get(0, 0).unwrap();
        assert_eq!(cell.char, 'A'); // Character preserved
        assert_eq!(cell.style.bg, Some(Color::Yellow));
    }

    #[test]
    fn test_framebuffer_overlay_bg() {
        let mut fb = FrameBuffer::new(10, 5);
        let style = Style::default().fg(Color::White);
        fb.set_cell(0, 0, 'B', &style);
        RenderBackend::overlay_bg(&mut fb, 0, 0, Color::Blue);
        let cell = fb.get(0, 0).unwrap();
        assert_eq!(cell.char, 'B');
        assert_eq!(cell.style.bg, Some(Color::Blue));
    }

    #[test]
    fn test_framebuffer_fill_horizontal() {
        let mut fb = FrameBuffer::new(20, 5);
        let style = Style::default();
        RenderBackend::fill_horizontal(&mut fb, 2, 1, 5, '-', &style);
        assert_eq!(fb.get(2, 1).unwrap().char, '-');
        assert_eq!(fb.get(6, 1).unwrap().char, '-');
        assert_eq!(fb.get(7, 1).unwrap().char, ' ');
    }

    #[test]
    fn test_framebuffer_fill_vertical() {
        let mut fb = FrameBuffer::new(10, 10);
        let style = Style::default();
        RenderBackend::fill_vertical(&mut fb, 3, 1, 4, '|', &style);
        assert_eq!(fb.get(3, 1).unwrap().char, '|');
        assert_eq!(fb.get(3, 4).unwrap().char, '|');
        assert_eq!(fb.get(3, 5).unwrap().char, ' ');
    }

    #[test]
    fn test_framebuffer_fill_region() {
        let mut fb = FrameBuffer::new(20, 10);
        let style = Style::default();
        RenderBackend::fill_region(&mut fb, 1, 1, 3, 2, '#', &style);
        assert_eq!(fb.get(1, 1).unwrap().char, '#');
        assert_eq!(fb.get(3, 2).unwrap().char, '#');
        assert_eq!(fb.get(4, 1).unwrap().char, ' ');
    }

    // =========================================================================
    // TuiExtension trait tests
    // =========================================================================

    use std::sync::atomic::{AtomicBool, Ordering};

    struct MockExtension {
        active: bool,
        rendered: AtomicBool,
    }

    impl TuiExtension for MockExtension {
        fn kind(&self) -> &'static str {
            "mock"
        }

        fn is_active(&self) -> bool {
            self.active
        }

        fn apply_notification(&mut self, _data: &str) {
            self.active = true;
        }

        fn render(&self, backend: &mut dyn RenderBackend) {
            self.rendered.store(true, Ordering::Relaxed);
            backend.set_cell(0, 0, 'M', &Style::default());
        }
    }

    #[test]
    fn test_extension_trait_dispatch() {
        let mut ext = MockExtension {
            active: false,
            rendered: AtomicBool::new(false),
        };
        assert_eq!(ext.kind(), "mock");
        assert!(!ext.is_active());

        ext.apply_notification(r#"{"active":true}"#);
        assert!(ext.is_active());

        let mut fb = FrameBuffer::new(10, 5);
        ext.render(&mut fb);
        assert!(ext.rendered.load(Ordering::Relaxed));
        assert_eq!(fb.get(0, 0).unwrap().char, 'M');
    }

    #[test]
    fn test_extension_default_cursor_position() {
        let ext = MockExtension {
            active: true,
            rendered: AtomicBool::new(false),
        };
        // Default cursor_position returns None
        assert!(ext.cursor_position(80, 24).is_none());
    }

    #[test]
    fn test_extension_default_tick() {
        let mut ext = MockExtension {
            active: false,
            rendered: AtomicBool::new(false),
        };
        // Default tick returns false (no-op)
        assert!(!ext.tick());
    }

    #[test]
    fn test_extension_trait_object() {
        let ext: Box<dyn TuiExtension> = Box::new(MockExtension {
            active: true,
            rendered: AtomicBool::new(false),
        });
        assert_eq!(ext.kind(), "mock");
        assert!(ext.is_active());

        let mut fb = FrameBuffer::new(10, 5);
        ext.render(&mut fb);
        assert_eq!(fb.get(0, 0).unwrap().char, 'M');
    }
}
