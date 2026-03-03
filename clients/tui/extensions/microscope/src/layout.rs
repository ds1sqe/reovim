//! Layout calculations for the microscope picker.

/// Minimum height for the microscope overlay.
pub const MIN_HEIGHT: u16 = 6;

/// Fraction of terminal height used for the picker (0.4 = 40%).
const HEIGHT_RATIO: f32 = 0.4;

/// Fraction of picker width used for the results panel (0.4 = 40%).
const RESULTS_WIDTH_RATIO: f32 = 0.4;

/// Minimum terminal width to show the preview panel.
const MIN_PREVIEW_WIDTH: u16 = 60;

/// Computed layout bounds for the microscope overlay.
#[derive(Debug, Clone)]
pub struct LayoutBounds {
    /// Left edge x coordinate.
    pub x: u16,
    /// Top edge y coordinate.
    pub y: u16,
    /// Full width of the picker overlay.
    pub width: u16,
    /// Total height of the picker overlay.
    pub total_height: u16,
    /// Y coordinate of the query input row.
    pub query_row: u16,
    /// Y coordinate where the results/preview panel starts.
    pub panel_start_y: u16,
    /// Height of the results/preview panel area.
    pub panel_height: u16,
    /// Width of the results panel (left side).
    pub results_width: u16,
    /// Whether to show the preview panel.
    pub show_preview: bool,
    /// Width of the preview panel (right side), 0 if hidden.
    pub preview_width: u16,
    /// X coordinate where the preview panel starts.
    pub preview_x: u16,
}

impl LayoutBounds {
    /// Calculate layout bounds from terminal dimensions.
    #[must_use]
    #[allow(
        clippy::cast_sign_loss,
        clippy::cast_precision_loss,
        clippy::cast_possible_truncation
    )]
    pub fn calculate(terminal_width: u16, terminal_height: u16) -> Self {
        let total_height = (f32::from(terminal_height) * HEIGHT_RATIO)
            .round()
            .max(f32::from(MIN_HEIGHT)) as u16;
        let total_height = total_height.min(terminal_height);

        let x = 0;
        let y = terminal_height.saturating_sub(total_height);
        let width = terminal_width;

        // Query row is the first row.
        let query_row = y;
        // Separator row after query.
        let panel_start_y = y + 2;
        let panel_height = total_height.saturating_sub(2);

        let show_preview = terminal_width >= MIN_PREVIEW_WIDTH;
        let results_width = if show_preview {
            (f32::from(width) * RESULTS_WIDTH_RATIO).round() as u16
        } else {
            width
        };
        let preview_width = if show_preview {
            width.saturating_sub(results_width).saturating_sub(1) // -1 for separator
        } else {
            0
        };
        let preview_x = if show_preview {
            results_width + 1 // After separator
        } else {
            0
        };

        Self {
            x,
            y,
            width,
            total_height,
            query_row,
            panel_start_y,
            panel_height,
            results_width,
            show_preview,
            preview_width,
            preview_x,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn standard_layout() {
        let bounds = LayoutBounds::calculate(100, 40);
        assert!(bounds.total_height >= MIN_HEIGHT);
        assert!(bounds.show_preview);
        assert!(bounds.results_width > 0);
        assert!(bounds.preview_width > 0);
        assert_eq!(bounds.x, 0);
        assert_eq!(bounds.width, 100);
    }

    #[test]
    fn narrow_screen_hides_preview() {
        let bounds = LayoutBounds::calculate(50, 24);
        assert!(!bounds.show_preview);
        assert_eq!(bounds.preview_width, 0);
        assert_eq!(bounds.results_width, 50);
    }

    #[test]
    fn tiny_screen() {
        let bounds = LayoutBounds::calculate(20, 5);
        assert!(bounds.total_height <= 5);
    }

    #[test]
    fn query_row_position() {
        let bounds = LayoutBounds::calculate(80, 24);
        assert_eq!(bounds.query_row, bounds.y);
        assert_eq!(bounds.panel_start_y, bounds.y + 2);
    }

    #[test]
    fn panel_height() {
        let bounds = LayoutBounds::calculate(80, 30);
        assert_eq!(bounds.panel_height, bounds.total_height.saturating_sub(2));
    }

    #[test]
    fn preview_x_after_separator() {
        let bounds = LayoutBounds::calculate(100, 40);
        if bounds.show_preview {
            assert_eq!(bounds.preview_x, bounds.results_width + 1);
        }
    }

    #[test]
    fn layout_debug() {
        let bounds = LayoutBounds::calculate(80, 24);
        let debug = format!("{bounds:?}");
        assert!(debug.contains("LayoutBounds"));
    }

    #[test]
    fn layout_clone() {
        let bounds = LayoutBounds::calculate(80, 24);
        #[allow(clippy::redundant_clone)]
        let cloned = bounds.clone();
        assert_eq!(cloned.width, bounds.width);
    }

    #[test]
    fn exact_min_preview_width() {
        let bounds = LayoutBounds::calculate(MIN_PREVIEW_WIDTH, 24);
        assert!(bounds.show_preview);

        let bounds_below = LayoutBounds::calculate(MIN_PREVIEW_WIDTH - 1, 24);
        assert!(!bounds_below.show_preview);
    }
}
