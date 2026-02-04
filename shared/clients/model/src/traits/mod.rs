//! Core trait definitions for client implementations.
//!
//! These traits define the contracts that platform-specific clients
//! must implement to provide editor functionality.

pub mod focus;
pub mod interpreter;
pub mod layout;
pub mod overlay;
pub mod panel;

pub use {
    focus::{Focus, FocusManager},
    interpreter::{DefaultLayoutInterpreter, LayoutInterpreter},
    layout::Layout,
    overlay::{OverlayManager, OverlayRenderer},
    panel::Panel,
};
