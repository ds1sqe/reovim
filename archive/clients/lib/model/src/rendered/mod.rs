//! Rendered state types (client-side interpretation).
//!
//! These types represent the client's interpretation of wire format data.
//! They include computed values like screen positions, z-ordering, and
//! platform-specific rendering decisions.

pub mod overlay;
pub mod panel;
pub mod window;

pub use {
    overlay::{OverlayStack, RenderedOverlay},
    panel::PanelState,
    window::{Window, WindowTree},
};
