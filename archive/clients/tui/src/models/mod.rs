//! TUI model implementations.
//!
//! This module provides concrete implementations of the `TuiModel` trait:
//! - `InteractiveModel`: Terminal-based TUI with keyboard input
//! - `HeadlessModel`: In-memory frame buffer for testing/scripting

pub mod headless;
pub mod interactive;

pub use {
    headless::{HeadlessCaptureError, HeadlessHandle, HeadlessModel},
    interactive::InteractiveModel,
};
