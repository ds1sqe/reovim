//! Display component trait for UI components that only render (no input handling)
//!
//! Display components are pure rendering abstractions. Unlike Interactors which
//! handle user input, display components only read state and render to the frame buffer.
//!
//! Examples: `StatusLine`, `TabLine`

use crate::{
    frame::FrameBuffer,
    highlight::{ColorMode, Theme},
    screen::LayerBounds,
};

/// Context passed to display components during rendering
#[derive(Debug, Clone, Copy)]
pub struct RenderContext<'a> {
    /// Screen width in columns
    pub screen_width: u16,
    /// Screen height in rows
    pub screen_height: u16,
    /// Current theme
    pub theme: &'a Theme,
    /// Color mode (ANSI, 256, `TrueColor`)
    pub color_mode: ColorMode,
    /// Y offset for tab line (0 if no tabs, 1 if tabs visible)
    pub tab_line_offset: u16,
}

impl<'a> RenderContext<'a> {
    /// Create a new render context
    #[must_use]
    pub const fn new(
        screen_width: u16,
        screen_height: u16,
        theme: &'a Theme,
        color_mode: ColorMode,
    ) -> Self {
        Self {
            screen_width,
            screen_height,
            theme,
            color_mode,
            tab_line_offset: 0,
        }
    }

    /// Set the tab line offset
    #[must_use]
    pub const fn with_tab_offset(mut self, offset: u16) -> Self {
        self.tab_line_offset = offset;
        self
    }

    /// Get the status line row (bottom of screen)
    #[must_use]
    pub const fn status_line_row(&self) -> u16 {
        self.screen_height.saturating_sub(1)
    }
}

/// Trait for display-only UI components
///
/// Display components render content but do not handle input.
/// For input-handling components, use the [`Interactor`](crate::interactor::Interactor) trait.
///
/// # Examples
///
/// - `StatusLineComponent` - Displays mode, file info, cursor position
/// - `TabLineComponent` - Displays open tabs/buffers
pub trait DisplayComponent: std::fmt::Debug + Send + Sync {
    /// Unique identifier for this component
    fn component_id(&self) -> &'static str;

    /// Render this component to the frame buffer
    fn render_to_frame(&self, buffer: &mut FrameBuffer, context: &RenderContext<'_>);

    /// Bounds of this component (for z-ordering and layout)
    fn bounds(&self, context: &RenderContext<'_>) -> LayerBounds;

    /// Z-order for rendering (higher = on top)
    ///
    /// Default is `z_order::BASE` (0). Override for components that
    /// should render on top of others.
    fn z_order(&self) -> u8 {
        crate::screen::z_order::BASE
    }

    /// Whether this component is currently visible
    ///
    /// Default is `true`. Override for conditionally visible components.
    fn is_visible(&self, _context: &RenderContext<'_>) -> bool {
        true
    }
}
