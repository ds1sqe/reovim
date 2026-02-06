//! TUI output implementations.
//!
//! Concrete implementations of the `TuiOutput` trait:
//! - `TerminalOutput`: Interactive terminal with Screen + Cursor
//! - `HeadlessOutput`: No-op output for headless/testing mode

pub mod headless;
pub mod terminal;

pub use {headless::HeadlessOutput, terminal::TerminalOutput};
