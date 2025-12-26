//! Helix-style layout calculations for microscope
//!
//! Layout is bottom-anchored with horizontal split:
//! - Results panel (40% width) on the left
//! - Preview panel (60% width) on the right
//! - Picker takes 40% of screen height, anchored to bottom
//!
//! ```text
//! +---------------------------------------------------------------+
//! |                      Editor Content                            |
//! +------------------ y = screen_height - picker_height -----------+
//! | Results (40%)                    | Preview (60%)               |
//! | +-----------------------------+  | +---------------------------+|
//! | | > query_                [I] |  | | Preview: filename         ||
//! | | --------------------------- |  | | -------------------------  ||
//! | | > selected_item.rs          |  | | 1  fn main() {            ||
//! | |   another_file.rs           |  | | 2      ...                 ||
//! | |   third_file.rs             |  | | 3  }                       ||
//! | +-----------------------------+  | +---------------------------+||
//! +----------------------------------+------------------------------+
//! | 4/128 files                          <CR> Open | <Esc> Close   |
//! +---------------------------------------------------------------+
//! ```

/// Layout configuration for microscope
#[derive(Debug, Clone, Copy)]
pub struct LayoutConfig {
    /// Percentage of screen height for picker (0.0 - 1.0)
    pub height_ratio: f32,
    /// Percentage of width for results panel (0.0 - 1.0)
    pub results_width_ratio: f32,
    /// Minimum height in rows
    pub min_height: u16,
    /// Minimum width in columns
    pub min_width: u16,
    /// Whether to show preview panel
    pub show_preview: bool,
    /// Padding from screen edges
    pub padding: u16,
}

impl Default for LayoutConfig {
    fn default() -> Self {
        Self {
            height_ratio: 0.4,        // 40% of screen height
            results_width_ratio: 0.4, // 40% for results, 60% for preview
            min_height: 10,
            min_width: 40,
            show_preview: true,
            padding: 0,
        }
    }
}

/// Calculated layout bounds for microscope panels
#[derive(Debug, Clone, Copy, Default)]
pub struct LayoutBounds {
    /// Results panel bounds
    pub results: PanelBounds,
    /// Preview panel bounds (if enabled)
    pub preview: Option<PanelBounds>,
    /// Status line bounds (bottom row)
    pub status: PanelBounds,
    /// Total picker bounds
    pub total: PanelBounds,
}

/// Bounds for a single panel
#[derive(Debug, Clone, Copy, Default)]
pub struct PanelBounds {
    pub x: u16,
    pub y: u16,
    pub width: u16,
    pub height: u16,
}

impl PanelBounds {
    /// Create new panel bounds
    #[must_use]
    pub const fn new(x: u16, y: u16, width: u16, height: u16) -> Self {
        Self {
            x,
            y,
            width,
            height,
        }
    }

    /// Get the inner area (excluding borders)
    #[must_use]
    pub const fn inner(&self) -> Self {
        Self {
            x: self.x + 1,
            y: self.y + 1,
            width: self.width.saturating_sub(2),
            height: self.height.saturating_sub(2),
        }
    }

    /// Check if a point is within bounds
    #[must_use]
    pub const fn contains(&self, x: u16, y: u16) -> bool {
        x >= self.x && x < self.x + self.width && y >= self.y && y < self.y + self.height
    }
}

/// Calculate layout bounds for microscope
#[must_use]
#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
pub fn calculate_layout(
    screen_width: u16,
    screen_height: u16,
    config: &LayoutConfig,
) -> LayoutBounds {
    // Calculate total picker dimensions
    let total_height = ((f32::from(screen_height) * config.height_ratio) as u16)
        .max(config.min_height)
        .min(screen_height.saturating_sub(config.padding * 2));

    let total_width = screen_width
        .saturating_sub(config.padding * 2)
        .max(config.min_width);

    // Bottom-anchored position
    let x = config.padding;
    let y = screen_height.saturating_sub(total_height + config.padding);

    let total = PanelBounds::new(x, y, total_width, total_height);

    // Status line takes bottom row
    let status_height = 1;
    let content_height = total_height.saturating_sub(status_height);

    let status = PanelBounds::new(x, y + content_height, total_width, status_height);

    // Calculate results and preview panels
    let (results, preview) = if config.show_preview && total_width > 60 {
        let results_width = ((f32::from(total_width) * config.results_width_ratio) as u16).max(20);
        let preview_width = total_width.saturating_sub(results_width);

        let results = PanelBounds::new(x, y, results_width, content_height);
        let preview = PanelBounds::new(x + results_width, y, preview_width, content_height);

        (results, Some(preview))
    } else {
        // No preview - results take full width
        let results = PanelBounds::new(x, y, total_width, content_height);
        (results, None)
    };

    LayoutBounds {
        results,
        preview,
        status,
        total,
    }
}

/// Number of visible items based on panel height
#[must_use]
pub const fn visible_item_count(panel: &PanelBounds) -> usize {
    // Height minus: border top (1) + prompt line (1) + separator (1) + border bottom (1)
    panel.height.saturating_sub(4) as usize
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = LayoutConfig::default();
        assert!((config.height_ratio - 0.4).abs() < f32::EPSILON);
        assert!((config.results_width_ratio - 0.4).abs() < f32::EPSILON);
        assert!(config.show_preview);
    }

    #[test]
    fn test_calculate_layout() {
        let config = LayoutConfig::default();
        let bounds = calculate_layout(100, 50, &config);

        // Should be bottom-anchored
        assert_eq!(bounds.total.y + bounds.total.height, 50);

        // Should have preview when width > 60
        assert!(bounds.preview.is_some());

        // Results should be 40% width
        assert_eq!(bounds.results.width, 40);
    }

    #[test]
    fn test_no_preview_narrow_screen() {
        let config = LayoutConfig::default();
        let bounds = calculate_layout(50, 30, &config);

        // Narrow screen should not show preview
        assert!(bounds.preview.is_none());
        assert_eq!(bounds.results.width, 50);
    }

    #[test]
    fn test_panel_inner() {
        let panel = PanelBounds::new(10, 20, 30, 15);
        let inner = panel.inner();

        assert_eq!(inner.x, 11);
        assert_eq!(inner.y, 21);
        assert_eq!(inner.width, 28);
        assert_eq!(inner.height, 13);
    }

    #[test]
    fn test_visible_item_count() {
        let panel = PanelBounds::new(0, 0, 40, 20);
        assert_eq!(visible_item_count(&panel), 16); // 20 - 4 = 16
    }
}
