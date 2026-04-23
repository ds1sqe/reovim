//! Platform capabilities base trait.
//!
//! Platforms declare the set of capabilities they satisfy via `PlatformCapabilities`.
//! The capability loader uses this to filter modules at load time: modules whose
//! `Requirements` are not satisfied by the active platform enter dormant state.

use crate::draw::{ColorDepth, Insets, RenderingModel};

/// Platform capabilities and constraints.
///
/// Queried by modules to adapt their behavior to the current platform
/// (terminal, web, mobile, etc.).
pub trait PlatformCapabilities: Send + Sync {
    /// The rendering model this platform uses.
    fn rendering_model(&self) -> RenderingModel;

    /// Grid size in cells (columns, rows). `None` if not a grid-based platform.
    fn grid_size(&self) -> Option<(u16, u16)>;

    /// Color depth supported by the display.
    fn color_depth(&self) -> ColorDepth;

    /// Pixel dimensions of the display. `None` if not available.
    fn pixel_size(&self) -> Option<(u32, u32)>;

    /// Whether Unicode width calculations are reliable on this platform.
    fn reliable_unicode_width(&self) -> bool;

    /// Whether the display is in dark mode.
    fn dark_mode(&self) -> bool;

    /// Whether smooth scrolling is supported.
    fn smooth_scroll(&self) -> bool;

    /// Whether pointer (mouse) events are available.
    fn pointer_events(&self) -> bool;

    /// Whether touch input is available.
    fn touch_input(&self) -> bool;

    /// Whether haptic feedback is available.
    fn haptic(&self) -> bool;

    /// Safe area insets (for notch/rounded corners).
    fn safe_area(&self) -> Insets;

    /// Whether the window currently has focus.
    fn has_focus(&self) -> bool;

    /// Whether clipboard access is available.
    fn clipboard_available(&self) -> bool;

    /// Whether a screen reader is active.
    fn screen_reader_active(&self) -> bool;
}

#[cfg(test)]
#[path = "platform_caps_tests.rs"]
mod tests;
