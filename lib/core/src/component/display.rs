//! Render context for UI components
//!
//! This module provides the `RenderContext` which is passed to UI components
//! during rendering. It contains screen dimensions, theme, and color mode info.

use crate::highlight::{ColorMode, Theme};

/// Context passed to UI components during rendering
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
