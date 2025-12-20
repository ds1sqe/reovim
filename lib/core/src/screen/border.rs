//! Window border configuration and rendering

use crate::{frame::FrameBuffer, highlight::Style};

/// Border style preset
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum BorderStyle {
    /// No border
    #[default]
    None,
    /// Thin single-line border: ─ │ ┌ ┐ └ ┘
    Thin,
    /// Rounded corners: ─ │ ╭ ╮ ╰ ╯
    Rounded,
    /// Double-line border: ═ ║ ╔ ╗ ╚ ╝
    Double,
    /// Heavy/thick border: ━ ┃ ┏ ┓ ┗ ┛
    Heavy,
    /// ASCII fallback: - | + + + +
    Ascii,
}

/// When to draw borders
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum BorderMode {
    /// Always draw borders on all configured sides
    #[default]
    Always,
    /// Only draw borders on sides that have adjacent windows (collision)
    OnCollide,
    /// Only draw borders on floating windows (not on tiled/split windows)
    OnFloat,
    /// Never draw borders (overrides style)
    Never,
}

/// Individual border characters for all positions
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BorderChars {
    pub top: char,
    pub bottom: char,
    pub left: char,
    pub right: char,
    pub top_left: char,
    pub top_right: char,
    pub bottom_left: char,
    pub bottom_right: char,
    /// For T-intersections when borders meet
    pub tee_left: char,
    pub tee_right: char,
    pub tee_top: char,
    pub tee_bottom: char,
    pub cross: char,
}

impl BorderChars {
    pub const EMPTY: Self = Self {
        top: ' ',
        bottom: ' ',
        left: ' ',
        right: ' ',
        top_left: ' ',
        top_right: ' ',
        bottom_left: ' ',
        bottom_right: ' ',
        tee_left: ' ',
        tee_right: ' ',
        tee_top: ' ',
        tee_bottom: ' ',
        cross: ' ',
    };

    pub const THIN: Self = Self {
        top: '─',
        bottom: '─',
        left: '│',
        right: '│',
        top_left: '┌',
        top_right: '┐',
        bottom_left: '└',
        bottom_right: '┘',
        tee_left: '├',
        tee_right: '┤',
        tee_top: '┬',
        tee_bottom: '┴',
        cross: '┼',
    };

    pub const ROUNDED: Self = Self {
        top: '─',
        bottom: '─',
        left: '│',
        right: '│',
        top_left: '╭',
        top_right: '╮',
        bottom_left: '╰',
        bottom_right: '╯',
        tee_left: '├',
        tee_right: '┤',
        tee_top: '┬',
        tee_bottom: '┴',
        cross: '┼',
    };

    pub const DOUBLE: Self = Self {
        top: '═',
        bottom: '═',
        left: '║',
        right: '║',
        top_left: '╔',
        top_right: '╗',
        bottom_left: '╚',
        bottom_right: '╝',
        tee_left: '╠',
        tee_right: '╣',
        tee_top: '╦',
        tee_bottom: '╩',
        cross: '╬',
    };

    pub const HEAVY: Self = Self {
        top: '━',
        bottom: '━',
        left: '┃',
        right: '┃',
        top_left: '┏',
        top_right: '┓',
        bottom_left: '┗',
        bottom_right: '┛',
        tee_left: '┣',
        tee_right: '┫',
        tee_top: '┳',
        tee_bottom: '┻',
        cross: '╋',
    };

    pub const ASCII: Self = Self {
        top: '-',
        bottom: '-',
        left: '|',
        right: '|',
        top_left: '+',
        top_right: '+',
        bottom_left: '+',
        bottom_right: '+',
        tee_left: '+',
        tee_right: '+',
        tee_top: '+',
        tee_bottom: '+',
        cross: '+',
    };
}

impl BorderStyle {
    /// Get the character set for this border style
    #[must_use]
    pub const fn chars(&self) -> BorderChars {
        match self {
            Self::None => BorderChars::EMPTY,
            Self::Thin => BorderChars::THIN,
            Self::Rounded => BorderChars::ROUNDED,
            Self::Double => BorderChars::DOUBLE,
            Self::Heavy => BorderChars::HEAVY,
            Self::Ascii => BorderChars::ASCII,
        }
    }

    /// Returns true if this style draws any border
    #[must_use]
    pub const fn is_visible(&self) -> bool {
        !matches!(self, Self::None)
    }
}

/// Which sides of the border to draw
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BorderSides {
    bits: u8,
}

impl BorderSides {
    pub const NONE: Self = Self { bits: 0 };
    pub const TOP: Self = Self { bits: 0b0001 };
    pub const BOTTOM: Self = Self { bits: 0b0010 };
    pub const LEFT: Self = Self { bits: 0b0100 };
    pub const RIGHT: Self = Self { bits: 0b1000 };
    pub const ALL: Self = Self { bits: 0b1111 };

    #[must_use]
    pub const fn contains(&self, other: Self) -> bool {
        (self.bits & other.bits) == other.bits
    }

    #[must_use]
    pub const fn union(self, other: Self) -> Self {
        Self {
            bits: self.bits | other.bits,
        }
    }

    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.bits == 0
    }
}

impl Default for BorderSides {
    fn default() -> Self {
        Self::ALL
    }
}

/// Title alignment in border
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum TitleAlignment {
    #[default]
    Left,
    Center,
    Right,
}

/// Border configuration for a window
#[derive(Debug, Clone, Default)]
pub struct BorderConfig {
    /// Border style preset
    pub style: BorderStyle,
    /// When to draw borders
    pub mode: BorderMode,
    /// Which sides to draw borders on (when mode is Always)
    pub sides: BorderSides,
    /// Title to display in the border (optional)
    pub title: Option<String>,
    /// Title alignment
    pub title_align: TitleAlignment,
}

impl BorderConfig {
    /// Create a new border config with the given style (always draw all sides)
    #[must_use]
    pub const fn new(style: BorderStyle) -> Self {
        Self {
            style,
            mode: BorderMode::Always,
            sides: BorderSides::ALL,
            title: None,
            title_align: TitleAlignment::Left,
        }
    }

    /// Create a border config that only draws on collision with adjacent windows
    #[must_use]
    pub const fn on_collide(style: BorderStyle) -> Self {
        Self {
            style,
            mode: BorderMode::OnCollide,
            sides: BorderSides::ALL,
            title: None,
            title_align: TitleAlignment::Left,
        }
    }

    /// Create a border config that only draws on floating windows
    #[must_use]
    pub const fn on_float(style: BorderStyle) -> Self {
        Self {
            style,
            mode: BorderMode::OnFloat,
            sides: BorderSides::ALL,
            title: None,
            title_align: TitleAlignment::Left,
        }
    }

    /// Create a border config with no border
    #[must_use]
    pub const fn none() -> Self {
        Self {
            style: BorderStyle::None,
            mode: BorderMode::Never,
            sides: BorderSides::NONE,
            title: None,
            title_align: TitleAlignment::Left,
        }
    }

    /// Set the border mode
    #[must_use]
    pub const fn with_mode(mut self, mode: BorderMode) -> Self {
        self.mode = mode;
        self
    }

    /// Set a title for the border
    #[must_use]
    pub fn with_title(mut self, title: impl Into<String>) -> Self {
        self.title = Some(title.into());
        self
    }

    /// Set title alignment
    #[must_use]
    pub const fn with_title_align(mut self, align: TitleAlignment) -> Self {
        self.title_align = align;
        self
    }

    /// Set which sides to draw (for Always mode)
    #[must_use]
    pub const fn with_sides(mut self, sides: BorderSides) -> Self {
        self.sides = sides;
        self
    }

    /// Compute the effective sides to draw based on mode, adjacency, and float status
    #[must_use]
    pub const fn effective_sides(
        &self,
        adjacency: &WindowAdjacency,
        is_floating: bool,
    ) -> BorderSides {
        match self.mode {
            BorderMode::Never => BorderSides::NONE,
            BorderMode::Always => self.sides,
            BorderMode::OnFloat => {
                // Only draw borders on floating windows
                if is_floating {
                    self.sides
                } else {
                    BorderSides::NONE
                }
            }
            BorderMode::OnCollide => {
                // Only draw borders on sides with adjacent windows
                let mut bits = 0u8;
                if adjacency.top && self.sides.contains(BorderSides::TOP) {
                    bits |= BorderSides::TOP.bits;
                }
                if adjacency.bottom && self.sides.contains(BorderSides::BOTTOM) {
                    bits |= BorderSides::BOTTOM.bits;
                }
                if adjacency.left && self.sides.contains(BorderSides::LEFT) {
                    bits |= BorderSides::LEFT.bits;
                }
                if adjacency.right && self.sides.contains(BorderSides::RIGHT) {
                    bits |= BorderSides::RIGHT.bits;
                }
                BorderSides { bits }
            }
        }
    }

    /// Calculate the insets (border thickness) for this config
    ///
    /// For `OnCollide` mode, use `insets_with_adjacency` instead.
    #[must_use]
    pub const fn insets(&self) -> BorderInsets {
        if !self.style.is_visible() || matches!(self.mode, BorderMode::Never) {
            return BorderInsets::ZERO;
        }
        // For Always mode, return based on configured sides
        // For OnCollide mode, this returns maximum possible insets
        BorderInsets {
            top: if self.sides.contains(BorderSides::TOP) {
                1
            } else {
                0
            },
            bottom: if self.sides.contains(BorderSides::BOTTOM) {
                1
            } else {
                0
            },
            left: if self.sides.contains(BorderSides::LEFT) {
                1
            } else {
                0
            },
            right: if self.sides.contains(BorderSides::RIGHT) {
                1
            } else {
                0
            },
        }
    }

    /// Calculate the actual insets based on adjacency and float status
    #[must_use]
    pub const fn insets_with_context(
        &self,
        adjacency: &WindowAdjacency,
        is_floating: bool,
    ) -> BorderInsets {
        if !self.style.is_visible() || matches!(self.mode, BorderMode::Never) {
            return BorderInsets::ZERO;
        }
        let effective = self.effective_sides(adjacency, is_floating);
        BorderInsets {
            top: if effective.contains(BorderSides::TOP) {
                1
            } else {
                0
            },
            bottom: if effective.contains(BorderSides::BOTTOM) {
                1
            } else {
                0
            },
            left: if effective.contains(BorderSides::LEFT) {
                1
            } else {
                0
            },
            right: if effective.contains(BorderSides::RIGHT) {
                1
            } else {
                0
            },
        }
    }
}

/// Represents border insets (how much space the border takes)
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct BorderInsets {
    pub top: u16,
    pub bottom: u16,
    pub left: u16,
    pub right: u16,
}

impl BorderInsets {
    pub const ZERO: Self = Self {
        top: 0,
        bottom: 0,
        left: 0,
        right: 0,
    };

    /// Total horizontal inset (left + right)
    #[must_use]
    pub const fn horizontal(&self) -> u16 {
        self.left + self.right
    }

    /// Total vertical inset (top + bottom)
    #[must_use]
    pub const fn vertical(&self) -> u16 {
        self.top + self.bottom
    }
}

/// Adjacency information for border merging
#[derive(Debug, Clone, Copy, Default)]
#[allow(clippy::struct_excessive_bools)]
pub struct WindowAdjacency {
    /// Window to the left shares our left border
    pub left: bool,
    /// Window to the right shares our right border
    pub right: bool,
    /// Window above shares our top border
    pub top: bool,
    /// Window below shares our bottom border
    pub bottom: bool,
}

/// Render a window's border to the frame buffer
#[allow(clippy::too_many_arguments)]
pub fn render_border_to_buffer(
    buffer: &mut FrameBuffer,
    x: u16,
    y: u16,
    width: u16,
    height: u16,
    config: &BorderConfig,
    style: &Style,
    adjacency: &WindowAdjacency,
    is_floating: bool,
) {
    if !config.style.is_visible() || width < 2 || height < 2 {
        return;
    }

    // Compute effective sides based on mode, adjacency, and float status
    let effective_sides = config.effective_sides(adjacency, is_floating);
    if effective_sides.is_empty() {
        return;
    }

    let chars = config.style.chars();

    // Top border
    if effective_sides.contains(BorderSides::TOP) {
        // Top-left corner (only if we also have left border or adjacency)
        let draw_top_left = effective_sides.contains(BorderSides::LEFT) || adjacency.left;
        if draw_top_left {
            let corner_top_left =
                select_corner_char(&chars, adjacency.left, adjacency.top, false, false);
            buffer.put_char(x, y, corner_top_left, style);
        }

        // Top edge (with optional title)
        let start_col = u16::from(draw_top_left);
        let draw_top_right = effective_sides.contains(BorderSides::RIGHT) || adjacency.right;
        let end_col = if draw_top_right {
            width.saturating_sub(1)
        } else {
            width
        };

        if let Some(title) = &config.title {
            render_title_in_border(
                buffer,
                x + start_col,
                y,
                end_col.saturating_sub(start_col),
                title,
                config.title_align,
                chars.top,
                style,
            );
        } else {
            for col in start_col..end_col {
                buffer.put_char(x + col, y, chars.top, style);
            }
        }

        // Top-right corner
        if draw_top_right {
            let corner_top_right =
                select_corner_char(&chars, adjacency.right, adjacency.top, true, false);
            buffer.put_char(x + width.saturating_sub(1), y, corner_top_right, style);
        }
    }

    // Left and right borders (vertical edges)
    let start_row = u16::from(effective_sides.contains(BorderSides::TOP));
    let end_row = if effective_sides.contains(BorderSides::BOTTOM) {
        height.saturating_sub(1)
    } else {
        height
    };

    for row in start_row..end_row {
        if effective_sides.contains(BorderSides::LEFT) {
            buffer.put_char(x, y + row, chars.left, style);
        }
        if effective_sides.contains(BorderSides::RIGHT) {
            buffer.put_char(x + width.saturating_sub(1), y + row, chars.right, style);
        }
    }

    // Bottom border
    if effective_sides.contains(BorderSides::BOTTOM) {
        let draw_bottom_left = effective_sides.contains(BorderSides::LEFT) || adjacency.left;
        if draw_bottom_left {
            let corner_bottom_left =
                select_corner_char(&chars, adjacency.left, adjacency.bottom, false, true);
            buffer.put_char(x, y + height.saturating_sub(1), corner_bottom_left, style);
        }

        let start_col = u16::from(draw_bottom_left);
        let draw_bottom_right = effective_sides.contains(BorderSides::RIGHT) || adjacency.right;
        let end_col = if draw_bottom_right {
            width.saturating_sub(1)
        } else {
            width
        };

        for col in start_col..end_col {
            buffer.put_char(x + col, y + height.saturating_sub(1), chars.bottom, style);
        }

        if draw_bottom_right {
            let corner_bottom_right =
                select_corner_char(&chars, adjacency.right, adjacency.bottom, true, true);
            buffer.put_char(
                x + width.saturating_sub(1),
                y + height.saturating_sub(1),
                corner_bottom_right,
                style,
            );
        }
    }
}

/// Select the appropriate corner character based on adjacency
#[allow(clippy::fn_params_excessive_bools)]
const fn select_corner_char(
    chars: &BorderChars,
    adjacent_horizontal: bool,
    adjacent_vertical: bool,
    is_right: bool,
    is_bottom: bool,
) -> char {
    match (adjacent_horizontal, adjacent_vertical) {
        (true, true) => chars.cross,
        (true, false) => {
            if is_bottom {
                chars.tee_bottom
            } else {
                chars.tee_top
            }
        }
        (false, true) => {
            if is_right {
                chars.tee_right
            } else {
                chars.tee_left
            }
        }
        (false, false) => match (is_right, is_bottom) {
            (false, false) => chars.top_left,
            (true, false) => chars.top_right,
            (false, true) => chars.bottom_left,
            (true, true) => chars.bottom_right,
        },
    }
}

/// Render a title within the border line
#[allow(clippy::too_many_arguments, clippy::cast_possible_truncation)]
fn render_title_in_border(
    buffer: &mut FrameBuffer,
    x: u16,
    y: u16,
    width: u16,
    title: &str,
    align: TitleAlignment,
    fill_char: char,
    style: &Style,
) {
    if width == 0 {
        return;
    }

    let title_len = title.chars().count().min(width as usize - 2); // Leave space for padding
    let truncated_title: String = title.chars().take(title_len).collect();
    let padded_title = format!(" {truncated_title} ");
    let padded_len = padded_title.chars().count();

    let start_offset = match align {
        TitleAlignment::Left => 1,
        TitleAlignment::Center => (width as usize).saturating_sub(padded_len) / 2,
        TitleAlignment::Right => (width as usize).saturating_sub(padded_len + 1),
    };

    // Fill with border char first
    for col in 0..width {
        buffer.put_char(x + col, y, fill_char, style);
    }

    // Overlay title
    for (i, ch) in padded_title.chars().enumerate() {
        let col = x + start_offset as u16 + i as u16;
        if col < x + width {
            buffer.put_char(col, y, ch, style);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_border_style_chars() {
        assert_eq!(BorderStyle::Thin.chars().top_left, '┌');
        assert_eq!(BorderStyle::Rounded.chars().top_left, '╭');
        assert_eq!(BorderStyle::Double.chars().top_left, '╔');
        assert_eq!(BorderStyle::Heavy.chars().top_left, '┏');
        assert_eq!(BorderStyle::Ascii.chars().top_left, '+');
    }

    #[test]
    fn test_border_sides() {
        let sides = BorderSides::TOP.union(BorderSides::BOTTOM);
        assert!(sides.contains(BorderSides::TOP));
        assert!(sides.contains(BorderSides::BOTTOM));
        assert!(!sides.contains(BorderSides::LEFT));
        assert!(!sides.contains(BorderSides::RIGHT));
    }

    #[test]
    fn test_border_insets() {
        let config = BorderConfig::new(BorderStyle::Thin);
        let insets = config.insets();
        assert_eq!(insets.top, 1);
        assert_eq!(insets.bottom, 1);
        assert_eq!(insets.left, 1);
        assert_eq!(insets.right, 1);

        let config_none = BorderConfig::none();
        let insets_none = config_none.insets();
        assert_eq!(insets_none, BorderInsets::ZERO);
    }

    #[test]
    fn test_border_config_builder() {
        let config = BorderConfig::new(BorderStyle::Rounded)
            .with_title("Test")
            .with_title_align(TitleAlignment::Center)
            .with_sides(BorderSides::TOP.union(BorderSides::BOTTOM));

        assert_eq!(config.style, BorderStyle::Rounded);
        assert_eq!(config.title, Some("Test".to_string()));
        assert_eq!(config.title_align, TitleAlignment::Center);
        assert!(config.sides.contains(BorderSides::TOP));
        assert!(config.sides.contains(BorderSides::BOTTOM));
        assert!(!config.sides.contains(BorderSides::LEFT));
    }

    #[test]
    fn test_border_mode_on_collide() {
        let config = BorderConfig::on_collide(BorderStyle::Thin);
        let no_adjacency = WindowAdjacency::default();
        let with_right = WindowAdjacency {
            right: true,
            ..Default::default()
        };

        // No adjacency = no borders
        let sides = config.effective_sides(&no_adjacency, false);
        assert!(sides.is_empty());

        // Right adjacency = only right border
        let sides = config.effective_sides(&with_right, false);
        assert!(!sides.contains(BorderSides::LEFT));
        assert!(sides.contains(BorderSides::RIGHT));
        assert!(!sides.contains(BorderSides::TOP));
        assert!(!sides.contains(BorderSides::BOTTOM));
    }

    #[test]
    fn test_border_mode_on_float() {
        let config = BorderConfig::on_float(BorderStyle::Rounded);
        let adjacency = WindowAdjacency::default();

        // Not floating = no borders
        let sides = config.effective_sides(&adjacency, false);
        assert!(sides.is_empty());

        // Floating = all borders
        let sides = config.effective_sides(&adjacency, true);
        assert!(sides.contains(BorderSides::ALL));
    }

    #[test]
    fn test_border_insets_with_context() {
        let config = BorderConfig::on_collide(BorderStyle::Thin);
        let no_adjacency = WindowAdjacency::default();
        let with_top_bottom = WindowAdjacency {
            top: true,
            bottom: true,
            ..Default::default()
        };

        // No adjacency = no insets
        let insets = config.insets_with_context(&no_adjacency, false);
        assert_eq!(insets, BorderInsets::ZERO);

        // Top/bottom adjacency = top/bottom insets only
        let insets = config.insets_with_context(&with_top_bottom, false);
        assert_eq!(insets.top, 1);
        assert_eq!(insets.bottom, 1);
        assert_eq!(insets.left, 0);
        assert_eq!(insets.right, 0);
    }
}
