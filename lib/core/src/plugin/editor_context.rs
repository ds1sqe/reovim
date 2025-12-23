//! Editor context - single source of truth for plugin window calculations
//!
//! Provides plugins with unified access to editor state, similar to
//! Neovim's `vim.o` + `vim.api` pattern.
//!
//! # Example
//!
//! ```ignore
//! impl WindowProvider for MyWindowProvider {
//!     fn get_windows(&self, state: &Arc<PluginStateRegistry>, ctx: &EditorContext) -> Vec<Window> {
//!         // Use context to calculate window dimensions
//!         let (x, y, w, h) = ctx.side_panel(PanelPosition::Left, 30);
//!         let (x, y, w, h) = ctx.floating(0.8, 0.6);
//!         // ...
//!     }
//! }
//! ```

use crate::{
    highlight::ColorMode,
    modd::{EditMode, SubMode},
    ui_component::ComponentId,
};

/// Position for side panels and docked windows
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PanelPosition {
    /// Left side of the screen (like explorer)
    Left,
    /// Right side of the screen (like outline)
    Right,
    /// Top of the editor area (below tab line)
    Top,
    /// Bottom of the editor area (above status line)
    Bottom,
}

/// Single source of truth for editor state accessible to plugins
///
/// Built at render time with current editor state. Analogous to
/// Neovim's `vim.o` (options) + `vim.api` (window/buffer API) combined.
#[derive(Debug, Clone)]
pub struct EditorContext {
    // === Screen Dimensions ===
    /// Total screen width in columns
    pub screen_width: u16,
    /// Total screen height in rows
    pub screen_height: u16,
    /// Tab line height (typically 1)
    pub tab_line_height: u16,
    /// Status line height (typically 1)
    pub status_line_height: u16,
    /// Left panel offset (e.g., explorer width when visible)
    pub left_offset: u16,
    /// Right panel offset (e.g., outline width when visible)
    pub right_offset: u16,

    // === Mode State ===
    /// Current edit mode (Normal, Insert, Visual)
    pub edit_mode: EditMode,
    /// Current sub-mode (Command, `OperatorPending`, Interactor)
    pub sub_mode: SubMode,
    /// Currently focused component
    pub focused_component: ComponentId,

    // === Buffer State ===
    /// Currently active buffer ID
    pub active_buffer_id: usize,
    /// Total number of open buffers
    pub buffer_count: usize,

    // === Theme/Display ===
    /// Current color mode (`TrueColor`, `Color256`, `Color16`)
    pub color_mode: ColorMode,
}

impl EditorContext {
    /// Create a new editor context with all fields
    #[must_use]
    #[allow(clippy::too_many_arguments)]
    pub const fn new(
        screen_width: u16,
        screen_height: u16,
        edit_mode: EditMode,
        sub_mode: SubMode,
        focused_component: ComponentId,
        active_buffer_id: usize,
        buffer_count: usize,
        color_mode: ColorMode,
    ) -> Self {
        Self {
            screen_width,
            screen_height,
            tab_line_height: 0,
            status_line_height: 1,
            left_offset: 0,
            right_offset: 0,
            edit_mode,
            sub_mode,
            focused_component,
            active_buffer_id,
            buffer_count,
            color_mode,
        }
    }

    /// Set the left panel offset (builder pattern)
    #[must_use]
    pub const fn with_left_offset(mut self, offset: u16) -> Self {
        self.left_offset = offset;
        self
    }

    /// Set the right panel offset (builder pattern)
    #[must_use]
    pub const fn with_right_offset(mut self, offset: u16) -> Self {
        self.right_offset = offset;
        self
    }

    /// Height available for sidebars (full height minus tab/status lines)
    ///
    /// Use this for explorer, file tree, and other sidebar windows.
    #[must_use]
    pub const fn sidebar_height(&self) -> u16 {
        self.screen_height
            .saturating_sub(self.tab_line_height)
            .saturating_sub(self.status_line_height)
    }

    /// Y position where the editor area starts (after tab line)
    #[must_use]
    pub const fn editor_top(&self) -> u16 {
        self.tab_line_height
    }

    /// Editor area dimensions (width, height) excluding chrome and side panels
    #[must_use]
    pub const fn editor_area(&self) -> (u16, u16) {
        let width = self
            .screen_width
            .saturating_sub(self.left_offset)
            .saturating_sub(self.right_offset);
        (width, self.sidebar_height())
    }

    /// X position where the editor area starts (after left panel)
    #[must_use]
    pub const fn editor_left(&self) -> u16 {
        self.left_offset
    }

    /// Editor bounds as (x, y, width, height) accounting for all offsets
    #[must_use]
    pub const fn editor_bounds(&self) -> (u16, u16, u16, u16) {
        let (width, height) = self.editor_area();
        (self.left_offset, self.tab_line_height, width, height)
    }

    /// Calculate centered popup position and size
    ///
    /// # Arguments
    /// * `width_pct` - Width as percentage of screen (0.0-1.0)
    /// * `height_pct` - Height as percentage of editor area (0.0-1.0)
    ///
    /// # Returns
    /// Tuple of (x, y, width, height) for the popup window
    #[must_use]
    #[allow(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        clippy::cast_precision_loss
    )]
    pub fn centered_popup(&self, width_pct: f32, height_pct: f32) -> (u16, u16, u16, u16) {
        let area_h = self.sidebar_height();
        let w = (f32::from(self.screen_width) * width_pct) as u16;
        let h = (f32::from(area_h) * height_pct) as u16;
        let x = (self.screen_width.saturating_sub(w)) / 2;
        let y = self.tab_line_height + (area_h.saturating_sub(h)) / 2;
        (x, y, w, h)
    }

    /// Check if currently in insert mode
    #[must_use]
    pub const fn is_insert_mode(&self) -> bool {
        matches!(self.edit_mode, EditMode::Insert(_))
    }

    /// Check if currently in visual mode
    #[must_use]
    pub const fn is_visual_mode(&self) -> bool {
        matches!(self.edit_mode, EditMode::Visual(_))
    }

    /// Check if currently in normal mode
    #[must_use]
    pub const fn is_normal_mode(&self) -> bool {
        matches!(self.edit_mode, EditMode::Normal)
    }

    /// Check if a specific component has focus
    #[must_use]
    pub fn is_focused(&self, component: ComponentId) -> bool {
        self.focused_component == component
    }

    /// Check if the editor component has focus
    #[must_use]
    pub fn is_editor_focused(&self) -> bool {
        self.focused_component == ComponentId::EDITOR
    }

    // === Layout Helpers ===

    /// Calculate position and size for a side panel
    ///
    /// Side panels are docked to an edge of the editor area with a fixed
    /// width (for Left/Right) or height (for Top/Bottom).
    ///
    /// # Arguments
    /// * `position` - Which edge to dock to
    /// * `size` - Width for Left/Right panels, height for Top/Bottom panels
    ///
    /// # Returns
    /// Tuple of (x, y, width, height) for the panel window
    #[must_use]
    pub const fn side_panel(&self, position: PanelPosition, size: u16) -> (u16, u16, u16, u16) {
        let editor_top = self.tab_line_height;
        let sidebar_h = self.sidebar_height();

        match position {
            PanelPosition::Left => (0, editor_top, size, sidebar_h),
            PanelPosition::Right => {
                (self.screen_width.saturating_sub(size), editor_top, size, sidebar_h)
            }
            PanelPosition::Top => (0, editor_top, self.screen_width, size),
            PanelPosition::Bottom => {
                let y = editor_top + sidebar_h.saturating_sub(size);
                (0, y, self.screen_width, size)
            }
        }
    }

    /// Calculate position and size for a centered floating window
    ///
    /// Floating windows are centered in the editor area with dimensions
    /// specified as percentages of the available space.
    ///
    /// # Arguments
    /// * `width_pct` - Width as percentage of screen width (0.0-1.0)
    /// * `height_pct` - Height as percentage of editor area height (0.0-1.0)
    ///
    /// # Returns
    /// Tuple of (x, y, width, height) for the floating window
    #[must_use]
    #[allow(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        clippy::cast_precision_loss
    )]
    pub fn floating(&self, width_pct: f32, height_pct: f32) -> (u16, u16, u16, u16) {
        self.centered_popup(width_pct, height_pct)
    }

    /// Calculate position and size for a dropdown menu
    ///
    /// Dropdowns appear below a specified anchor position with a fixed
    /// width and height determined by content.
    ///
    /// # Arguments
    /// * `anchor_x` - X position to anchor to
    /// * `anchor_y` - Y position to anchor to (dropdown appears below)
    /// * `width` - Width of the dropdown
    /// * `max_height` - Maximum height (will be clamped to available space)
    ///
    /// # Returns
    /// Tuple of (x, y, width, height) for the dropdown window
    #[must_use]
    pub const fn dropdown(
        &self,
        anchor_x: u16,
        anchor_y: u16,
        width: u16,
        max_height: u16,
    ) -> (u16, u16, u16, u16) {
        // Y position is below the anchor
        let y = anchor_y + 1;

        // Calculate available height below the anchor
        let editor_bottom = self.tab_line_height + self.sidebar_height();
        let available_height = editor_bottom.saturating_sub(y);

        // Clamp height to available space
        let height = if max_height < available_height {
            max_height
        } else {
            available_height
        };

        // Clamp x to prevent overflow off right edge
        let x = if anchor_x + width > self.screen_width {
            self.screen_width.saturating_sub(width)
        } else {
            anchor_x
        };

        (x, y, width, height)
    }
}
