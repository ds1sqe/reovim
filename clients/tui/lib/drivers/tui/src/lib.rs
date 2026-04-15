#![cfg_attr(coverage_nightly, allow(unused_features))]
#![cfg_attr(coverage_nightly, feature(coverage_attribute))]
//! Reovim TUI driver - terminal abstraction layer.
//!
//! This crate provides the **mechanism** for terminal-based user interfaces.
//! It handles the low-level details of terminal I/O while leaving **policy**
//! decisions (what to render, styling, layout) to the TUI client.
//!
//! # Architecture
//!
//! ```text
//! ┌─────────────────────────────────────────────────────────────┐
//! │  TUI Client (lib/clients/tui/)                   POLICY     │
//! │    - Layout decisions                                       │
//! │    - Styling and theming                                    │
//! │    - What content to display                                │
//! ├─────────────────────────────────────────────────────────────┤
//! │  TUI Driver (this crate)                         MECHANISM  │
//! │    - Terminal session (raw mode, alternate screen)          │
//! │    - Input event stream                                     │
//! │    - Screen rendering primitives                            │
//! │    - Cursor management                                      │
//! │    - FrameBuffer, Cell, FrameRenderer, Style                │
//! └─────────────────────────────────────────────────────────────┘
//! ```
//!
//! # Components
//!
//! - [`Terminal`] - Session management (enter/exit raw mode, alternate screen)
//! - [`InputReader`] - Async input event stream
//! - [`Screen`] - Frame buffer + renderer combo
//! - [`Cursor`] - Cursor position and style management
//! - [`FrameBuffer`] - 2D cell grid storage
//! - [`FrameRenderer`] - Double-buffer renderer with diff optimization
//! - [`Style`] - Text styling with colors and attributes
//!
//! # Example
//!
//! ```no_run
//! use reovim_driver_tui::{Terminal, Screen, InputReader, Cursor, CursorStyle, Style};
//!
//! #[tokio::main]
//! async fn main() -> std::io::Result<()> {
//!     // Enter terminal session
//!     let mut terminal = Terminal::enter()?;
//!     let mut screen = Screen::new(80, 24);
//!     let mut input = InputReader::new();
//!     let mut cursor = Cursor::new();
//!
//!     // Render content
//!     screen.write_str(0, 0, "Hello, TUI!", &Style::default());
//!     screen.render(&mut terminal)?;
//!
//!     // Handle input
//!     while let Some(event) = input.next_event().await {
//!         // Process event...
//!     }
//!
//!     // Terminal restored automatically on drop
//!     Ok(())
//! }
//! ```

mod cursor;
pub mod frame;
mod input;
mod screen;
pub mod style;
mod terminal;

pub use {
    cursor::{Cursor, CursorStyle},
    frame::{Cell, FrameBuffer, FrameRenderer, char_width},
    input::{PlatformEvent, InputReader, KeyEvent, MouseEvent, ResizeEvent},
    screen::Screen,
    style::{Attributes, ColorMode, Style},
    terminal::Terminal,
};
