//! TUI output trait for display-side I/O abstraction.
//!
//! This module defines the `TuiOutput` trait — a pure display-side concern
//! with only 6 methods. Unlike the old `TuiModel` (14 methods mixing I/O,
//! control, and input), `TuiOutput` handles ONLY output to the display device.
//!
//! # Implementations
//!
//! - `TerminalOutput`: Flushes `FrameBuffer` → Screen → Terminal (interactive)
//! - `HeadlessOutput`: All no-ops (headless — `FrameBuffer` is the final output)
//!
//! # Design
//!
//! ```text
//! ┌─────────────────────────────────────────────────────────────┐
//! │  TuiApp<O: TuiOutput>                                       │
//! │    frame_buffer: FrameBuffer   ← always owned by TuiApp    │
//! │    output: O                   ← display adapter only       │
//! ├──────────────────────────┬──────────────────────────────────┤
//! │  TerminalOutput          │  HeadlessOutput                   │
//! │    flush(fb) → Screen    │    flush(fb) → no-op              │
//! │    position_cursor()     │    position_cursor() → no-op      │
//! │    terminal cursor: yes  │    terminal cursor: no             │
//! └──────────────────────────┴──────────────────────────────────┘
//! ```

use std::io;

use reovim_driver_display::FrameBuffer;

/// Hint for cursor style (mode-dependent).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum CursorStyleHint {
    /// Block cursor (normal mode).
    #[default]
    Block,
    /// Vertical bar cursor (insert mode).
    Bar,
    /// Underline cursor (replace mode).
    Underline,
}

/// Display-side I/O trait.
///
/// Implementations handle only the output path — flushing rendered content
/// to a display device. Input is handled separately via the common input channel.
///
/// # Contract
///
/// - `flush()` is called after `render_frame()` writes to the `FrameBuffer`
/// - `position_cursor()` is called after flush for terminal cursor placement
/// - `uses_terminal_cursor()` determines whether the render engine draws
///   the cursor into the `FrameBuffer` (`false`) or lets the terminal handle it (`true`)
pub trait TuiOutput {
    /// Flush `FrameBuffer` content to the display device.
    ///
    /// - Interactive: copies FB cells → Screen → Terminal (with diff optimization)
    /// - Headless: no-op (`FrameBuffer` is the final output)
    ///
    /// # Errors
    ///
    /// Returns an error if terminal rendering fails.
    fn flush(&mut self, frame: &FrameBuffer) -> io::Result<()>;

    /// Position the hardware cursor at (x, y).
    ///
    /// - Interactive: moves the terminal cursor
    /// - Headless: no-op
    fn position_cursor(&mut self, x: u16, y: u16);

    /// Whether this output has a terminal cursor.
    ///
    /// - `true`: Interactive mode (uses terminal cursor, render engine skips self cursor)
    /// - `false`: Headless mode (render engine draws cursor into `FrameBuffer`)
    fn uses_terminal_cursor(&self) -> bool;

    /// Set the cursor style.
    ///
    /// - Interactive: changes terminal cursor appearance
    /// - Headless: no-op
    fn set_cursor_style(&mut self, style: CursorStyleHint);

    /// Set cursor visibility.
    ///
    /// - Interactive: shows/hides terminal cursor
    /// - Headless: no-op
    fn set_cursor_visible(&mut self, visible: bool);

    /// Force a full redraw on next flush.
    ///
    /// Used after resize or when screen state may be corrupted.
    /// - Interactive: invalidates Screen's diff cache
    /// - Headless: no-op
    fn invalidate(&mut self);
}
