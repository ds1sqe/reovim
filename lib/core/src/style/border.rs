//! Border character presets for popup windows and panels
//!
//! Provides several border styles:
//! - Single line (─, │, ┌, ┐, └, ┘)
//! - Double line (═, ║, ╔, ╗, ╚, ╝)
//! - Rounded corners (╭, ╮, ╰, ╯)
//! - ASCII fallback (+, -, |)

/// Border character set for drawing window borders
#[derive(Debug, Clone, Copy)]
pub struct BorderChars {
    /// Top-left corner
    pub top_left: char,
    /// Top-right corner
    pub top_right: char,
    /// Bottom-left corner
    pub bottom_left: char,
    /// Bottom-right corner
    pub bottom_right: char,
    /// Horizontal line
    pub horizontal: char,
    /// Vertical line
    pub vertical: char,
}

impl BorderChars {
    /// Create a new border character set
    #[must_use]
    pub const fn new(
        top_left: char,
        top_right: char,
        bottom_left: char,
        bottom_right: char,
        horizontal: char,
        vertical: char,
    ) -> Self {
        Self {
            top_left,
            top_right,
            bottom_left,
            bottom_right,
            horizontal,
            vertical,
        }
    }
}

/// Single line border (─, │, ┌, ┐, └, ┘)
pub const BORDER_SINGLE: BorderChars = BorderChars {
    top_left: '┌',
    top_right: '┐',
    bottom_left: '└',
    bottom_right: '┘',
    horizontal: '─',
    vertical: '│',
};

/// Double line border (═, ║, ╔, ╗, ╚, ╝)
pub const BORDER_DOUBLE: BorderChars = BorderChars {
    top_left: '╔',
    top_right: '╗',
    bottom_left: '╚',
    bottom_right: '╝',
    horizontal: '═',
    vertical: '║',
};

/// Rounded corners border (╭, ╮, ╰, ╯)
pub const BORDER_ROUNDED: BorderChars = BorderChars {
    top_left: '╭',
    top_right: '╮',
    bottom_left: '╰',
    bottom_right: '╯',
    horizontal: '─',
    vertical: '│',
};

/// Heavy border (━, ┃, ┏, ┓, ┗, ┛)
pub const BORDER_HEAVY: BorderChars = BorderChars {
    top_left: '┏',
    top_right: '┓',
    bottom_left: '┗',
    bottom_right: '┛',
    horizontal: '━',
    vertical: '┃',
};

/// ASCII fallback border (+, -, |)
pub const BORDER_ASCII: BorderChars = BorderChars {
    top_left: '+',
    top_right: '+',
    bottom_left: '+',
    bottom_right: '+',
    horizontal: '-',
    vertical: '|',
};

/// No border (spaces)
pub const BORDER_NONE: BorderChars = BorderChars {
    top_left: ' ',
    top_right: ' ',
    bottom_left: ' ',
    bottom_right: ' ',
    horizontal: ' ',
    vertical: ' ',
};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_border_single() {
        assert_eq!(BORDER_SINGLE.top_left, '┌');
        assert_eq!(BORDER_SINGLE.horizontal, '─');
        assert_eq!(BORDER_SINGLE.vertical, '│');
    }

    #[test]
    fn test_border_rounded() {
        assert_eq!(BORDER_ROUNDED.top_left, '╭');
        assert_eq!(BORDER_ROUNDED.top_right, '╮');
    }

    #[test]
    fn test_border_ascii() {
        assert_eq!(BORDER_ASCII.top_left, '+');
        assert_eq!(BORDER_ASCII.horizontal, '-');
    }
}
