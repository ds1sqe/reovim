//! Border rendering for window decoration.
//!
//! This module provides border styles and rendering functions for window
//! decoration. It supports multiple border styles and handles T-junctions
//! for adjacent windows.
//!
//! # Border Styles
//!
//! ```text
//! Single:   ┌─────┐    Double:  ╔═════╗
//!           │     │             ║     ║
//!           └─────┘             ╚═════╝
//!
//! Rounded:  ╭─────╮    Bold:    ┏━━━━━┓
//!           │     │             ┃     ┃
//!           ╰─────╯             ┗━━━━━┛
//! ```
//!
//! # T-Junction Support
//!
//! When windows are adjacent, corners become T-junctions:
//!
//! ```text
//! ┌─────┬─────┐    ├ left tee
//! │     │     │    ┤ right tee
//! └─────┴─────┘    ┬ top tee
//!                  ┴ bottom tee
//!                  ┼ cross
//! ```

use crate::{
    compositor::Style,
    frame::{Cell, FrameBuffer},
    window::Rect,
};

/// Border style for window decoration.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum BorderStyle {
    /// No border.
    #[default]
    None,
    /// Single line border (─ │).
    Single,
    /// Double line border (═ ║).
    Double,
    /// Rounded corners with single lines (╭ ╮ ╰ ╯).
    Rounded,
    /// Bold/heavy line border (━ ┃).
    Bold,
}

impl BorderStyle {
    /// Get the border characters for this style.
    #[must_use]
    pub const fn chars(&self) -> Option<BorderChars> {
        match self {
            Self::None => None,
            Self::Single => Some(BorderChars::SINGLE),
            Self::Double => Some(BorderChars::DOUBLE),
            Self::Rounded => Some(BorderChars::ROUNDED),
            Self::Bold => Some(BorderChars::BOLD),
        }
    }
}

/// When to draw borders.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum BorderMode {
    /// Always draw borders around windows.
    #[default]
    Always,
    /// Only draw borders where windows meet (split windows).
    OnCollide,
    /// Only draw borders on floating windows.
    OnFloat,
    /// Never draw borders.
    Never,
}

/// Border characters for each style.
///
/// Includes corners, edges, and T-junctions for adjacent windows.
#[derive(Debug, Clone, Copy)]
pub struct BorderChars {
    // Corners
    /// Top-left corner (e.g., ┌).
    pub top_left: char,
    /// Top-right corner (e.g., ┐).
    pub top_right: char,
    /// Bottom-left corner (e.g., └).
    pub bottom_left: char,
    /// Bottom-right corner (e.g., ┘).
    pub bottom_right: char,

    // Edges
    /// Horizontal edge (e.g., ─).
    pub horizontal: char,
    /// Vertical edge (e.g., │).
    pub vertical: char,

    // T-junctions (for adjacent windows)
    /// Left tee - window touches from right (├).
    pub tee_left: char,
    /// Right tee - window touches from left (┤).
    pub tee_right: char,
    /// Top tee - window touches from bottom (┬).
    pub tee_top: char,
    /// Bottom tee - window touches from top (┴).
    pub tee_bottom: char,
    /// Cross - windows touch from all sides (┼).
    pub cross: char,
}

impl BorderChars {
    /// Single line characters.
    pub const SINGLE: Self = Self {
        top_left: '┌',
        top_right: '┐',
        bottom_left: '└',
        bottom_right: '┘',
        horizontal: '─',
        vertical: '│',
        tee_left: '├',
        tee_right: '┤',
        tee_top: '┬',
        tee_bottom: '┴',
        cross: '┼',
    };

    /// Double line characters.
    pub const DOUBLE: Self = Self {
        top_left: '╔',
        top_right: '╗',
        bottom_left: '╚',
        bottom_right: '╝',
        horizontal: '═',
        vertical: '║',
        tee_left: '╠',
        tee_right: '╣',
        tee_top: '╦',
        tee_bottom: '╩',
        cross: '╬',
    };

    /// Rounded corners (with single T-junctions).
    pub const ROUNDED: Self = Self {
        top_left: '╭',
        top_right: '╮',
        bottom_left: '╰',
        bottom_right: '╯',
        horizontal: '─',
        vertical: '│',
        tee_left: '├',
        tee_right: '┤',
        tee_top: '┬',
        tee_bottom: '┴',
        cross: '┼',
    };

    /// Bold/heavy line characters.
    pub const BOLD: Self = Self {
        top_left: '┏',
        top_right: '┓',
        bottom_left: '┗',
        bottom_right: '┛',
        horizontal: '━',
        vertical: '┃',
        tee_left: '┣',
        tee_right: '┫',
        tee_top: '┳',
        tee_bottom: '┻',
        cross: '╋',
    };
}

/// Tracks which sides have neighboring windows.
///
/// Used for adjacency detection to determine when to use T-junctions
/// instead of corners.
#[allow(clippy::struct_excessive_bools)]
#[derive(Debug, Clone, Copy, Default)]
pub struct WindowAdjacency {
    /// Has window to the left.
    pub left: bool,
    /// Has window to the right.
    pub right: bool,
    /// Has window above.
    pub top: bool,
    /// Has window below.
    pub bottom: bool,
}

impl core::ops::BitOr for WindowAdjacency {
    type Output = Self;

    fn bitor(self, rhs: Self) -> Self {
        Self {
            left: self.left || rhs.left,
            right: self.right || rhs.right,
            top: self.top || rhs.top,
            bottom: self.bottom || rhs.bottom,
        }
    }
}

impl WindowAdjacency {
    /// No adjacent windows in any direction.
    pub const NONE: Self = Self {
        left: false,
        right: false,
        top: false,
        bottom: false,
    };

    /// Adjacent window to the left only.
    pub const LEFT: Self = Self {
        left: true,
        ..Self::NONE
    };

    /// Adjacent window to the right only.
    pub const RIGHT: Self = Self {
        right: true,
        ..Self::NONE
    };

    /// Adjacent window above only.
    pub const TOP: Self = Self {
        top: true,
        ..Self::NONE
    };

    /// Adjacent window below only.
    pub const BOTTOM: Self = Self {
        bottom: true,
        ..Self::NONE
    };

    /// Adjacent windows in all four directions.
    pub const ALL: Self = Self {
        left: true,
        right: true,
        top: true,
        bottom: true,
    };

    /// Create new adjacency with all sides set to false.
    #[must_use]
    pub const fn none() -> Self {
        Self::NONE
    }

    /// Compute adjacency for a target rect from a list of other rects.
    ///
    /// A window is considered adjacent if it shares an edge with the target.
    #[must_use]
    pub fn compute(target: &Rect, all_windows: &[Rect]) -> Self {
        let mut adj = Self::none();

        for other in all_windows {
            if other == target {
                continue;
            }

            // Check left adjacency: other's right edge touches target's left edge
            if other.x + other.width == target.x
                && ranges_overlap(
                    other.y,
                    other.y + other.height,
                    target.y,
                    target.y + target.height,
                )
            {
                adj.left = true;
            }

            // Check right adjacency: other's left edge touches target's right edge
            if other.x == target.x + target.width
                && ranges_overlap(
                    other.y,
                    other.y + other.height,
                    target.y,
                    target.y + target.height,
                )
            {
                adj.right = true;
            }

            // Check top adjacency: other's bottom edge touches target's top edge
            if other.y + other.height == target.y
                && ranges_overlap(other.x, other.x + other.width, target.x, target.x + target.width)
            {
                adj.top = true;
            }

            // Check bottom adjacency: other's top edge touches target's bottom edge
            if other.y == target.y + target.height
                && ranges_overlap(other.x, other.x + other.width, target.x, target.x + target.width)
            {
                adj.bottom = true;
            }
        }

        adj
    }
}

/// Check if two ranges overlap.
const fn ranges_overlap(a_start: u16, a_end: u16, b_start: u16, b_end: u16) -> bool {
    a_start < b_end && b_start < a_end
}

/// Select the appropriate corner character based on adjacency.
///
/// When windows are adjacent, corners become T-junctions or crosses:
/// - No adjacency: corner (┌ ┐ └ ┘)
/// - One adjacency: T-junction (├ ┤ ┬ ┴)
/// - Both adjacencies: cross (┼)
#[must_use]
pub const fn select_corner_char(
    chars: &BorderChars,
    corner: Corner,
    adjacency: WindowAdjacency,
) -> char {
    match corner {
        Corner::TopLeft => match (adjacency.left, adjacency.top) {
            (false, false) => chars.top_left,
            (true, false) => chars.tee_left,
            (false, true) => chars.tee_top,
            (true, true) => chars.cross,
        },
        Corner::TopRight => match (adjacency.right, adjacency.top) {
            (false, false) => chars.top_right,
            (true, false) => chars.tee_right,
            (false, true) => chars.tee_top,
            (true, true) => chars.cross,
        },
        Corner::BottomLeft => match (adjacency.left, adjacency.bottom) {
            (false, false) => chars.bottom_left,
            (true, false) => chars.tee_left,
            (false, true) => chars.tee_bottom,
            (true, true) => chars.cross,
        },
        Corner::BottomRight => match (adjacency.right, adjacency.bottom) {
            (false, false) => chars.bottom_right,
            (true, false) => chars.tee_right,
            (false, true) => chars.tee_bottom,
            (true, true) => chars.cross,
        },
    }
}

/// Corner position for adjacency selection.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Corner {
    /// Top-left corner.
    TopLeft,
    /// Top-right corner.
    TopRight,
    /// Bottom-left corner.
    BottomLeft,
    /// Bottom-right corner.
    BottomRight,
}

/// Render a border around a rectangle with adjacency detection.
///
/// Uses T-junctions where windows are adjacent.
pub fn render_border(
    buffer: &mut FrameBuffer,
    bounds: Rect,
    style: BorderStyle,
    adjacency: &WindowAdjacency,
    border_style: &Style,
) {
    let Some(chars) = style.chars() else {
        return;
    };

    if bounds.width < 2 || bounds.height < 2 {
        return;
    }

    let right = bounds.x + bounds.width - 1;
    let bottom = bounds.y + bounds.height - 1;

    // Corners with adjacency
    buffer.set(
        bounds.x,
        bounds.y,
        Cell::new(select_corner_char(&chars, Corner::TopLeft, *adjacency), border_style.clone()),
    );
    buffer.set(
        right,
        bounds.y,
        Cell::new(select_corner_char(&chars, Corner::TopRight, *adjacency), border_style.clone()),
    );
    buffer.set(
        bounds.x,
        bottom,
        Cell::new(select_corner_char(&chars, Corner::BottomLeft, *adjacency), border_style.clone()),
    );
    buffer.set(
        right,
        bottom,
        Cell::new(
            select_corner_char(&chars, Corner::BottomRight, *adjacency),
            border_style.clone(),
        ),
    );

    // Top edge
    for x in (bounds.x + 1)..right {
        buffer.set(x, bounds.y, Cell::new(chars.horizontal, border_style.clone()));
    }

    // Bottom edge
    for x in (bounds.x + 1)..right {
        buffer.set(x, bottom, Cell::new(chars.horizontal, border_style.clone()));
    }

    // Left edge
    for y in (bounds.y + 1)..bottom {
        buffer.set(bounds.x, y, Cell::new(chars.vertical, border_style.clone()));
    }

    // Right edge
    for y in (bounds.y + 1)..bottom {
        buffer.set(right, y, Cell::new(chars.vertical, border_style.clone()));
    }
}

/// Render a simple border without adjacency detection.
///
/// Always uses corner characters, never T-junctions.
pub fn render_border_simple(
    buffer: &mut FrameBuffer,
    bounds: Rect,
    style: BorderStyle,
    border_style: &Style,
) {
    render_border(buffer, bounds, style, &WindowAdjacency::none(), border_style);
}

/// Get the inner content area after accounting for borders.
///
/// Returns the rect representing the content area inside the border.
#[must_use]
pub fn inner_bounds(bounds: Rect, style: BorderStyle) -> Rect {
    if style == BorderStyle::None {
        bounds
    } else {
        Rect::new(
            bounds.x + 1,
            bounds.y + 1,
            bounds.width.saturating_sub(2),
            bounds.height.saturating_sub(2),
        )
    }
}

#[cfg(test)]
#[path = "border_tests.rs"]
mod tests;
