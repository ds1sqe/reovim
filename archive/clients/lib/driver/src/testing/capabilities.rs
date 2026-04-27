//! Mock platform capabilities for testing.

use crate::{ColorDepth, Insets, PlatformCapabilities, RenderingModel};

/// Configurable mock for `PlatformCapabilities`.
///
/// All fields are public with TUI-like defaults. Use builder methods for
/// chainable configuration.
#[derive(Debug, Clone)]
#[allow(clippy::struct_excessive_bools)]
pub struct MockPlatformCapabilities {
    pub rendering_model: RenderingModel,
    pub grid_size: Option<(u16, u16)>,
    pub color_depth: ColorDepth,
    pub pixel_size: Option<(u32, u32)>,
    pub reliable_unicode_width: bool,
    pub dark_mode: bool,
    pub smooth_scroll: bool,
    pub pointer_events: bool,
    pub touch_input: bool,
    pub haptic: bool,
    pub safe_area: Insets,
    pub has_focus: bool,
    pub clipboard_available: bool,
    pub screen_reader_active: bool,
}

impl Default for MockPlatformCapabilities {
    fn default() -> Self {
        Self {
            rendering_model: RenderingModel::CellGrid,
            grid_size: Some((80, 24)),
            color_depth: ColorDepth::TrueColor,
            pixel_size: None,
            reliable_unicode_width: true,
            dark_mode: true,
            smooth_scroll: false,
            pointer_events: false,
            touch_input: false,
            haptic: false,
            safe_area: Insets::ZERO,
            has_focus: true,
            clipboard_available: true,
            screen_reader_active: false,
        }
    }
}

impl MockPlatformCapabilities {
    /// Create with TUI-like defaults (80x24, `TrueColor`, dark mode).
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Set grid size (builder pattern).
    #[must_use]
    pub const fn with_grid_size(mut self, width: u16, height: u16) -> Self {
        self.grid_size = Some((width, height));
        self
    }

    /// Set color depth (builder pattern).
    #[must_use]
    pub const fn with_color_depth(mut self, depth: ColorDepth) -> Self {
        self.color_depth = depth;
        self
    }

    /// Set dark mode (builder pattern).
    #[must_use]
    pub const fn with_dark_mode(mut self, dark: bool) -> Self {
        self.dark_mode = dark;
        self
    }

    /// Set focus state (builder pattern).
    #[must_use]
    pub const fn with_focus(mut self, focused: bool) -> Self {
        self.has_focus = focused;
        self
    }
}

impl PlatformCapabilities for MockPlatformCapabilities {
    fn rendering_model(&self) -> RenderingModel {
        self.rendering_model
    }
    fn grid_size(&self) -> Option<(u16, u16)> {
        self.grid_size
    }
    fn color_depth(&self) -> ColorDepth {
        self.color_depth
    }
    fn pixel_size(&self) -> Option<(u32, u32)> {
        self.pixel_size
    }
    fn reliable_unicode_width(&self) -> bool {
        self.reliable_unicode_width
    }
    fn dark_mode(&self) -> bool {
        self.dark_mode
    }
    fn smooth_scroll(&self) -> bool {
        self.smooth_scroll
    }
    fn pointer_events(&self) -> bool {
        self.pointer_events
    }
    fn touch_input(&self) -> bool {
        self.touch_input
    }
    fn haptic(&self) -> bool {
        self.haptic
    }
    fn safe_area(&self) -> Insets {
        self.safe_area
    }
    fn has_focus(&self) -> bool {
        self.has_focus
    }
    fn clipboard_available(&self) -> bool {
        self.clipboard_available
    }
    fn screen_reader_active(&self) -> bool {
        self.screen_reader_active
    }
}
