//! TUI cell styling: colors + attribute flags.

use bitflags::bitflags;

/// Color vocabulary used by a [`CellStyle`].
///
/// `Default` means "use the terminal's default foreground or background",
/// which is typically rendered as no color escape sequence.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum CellColor {
    /// 24-bit RGB triple.
    Rgb(u8, u8, u8),
    /// ANSI 256-color palette index.
    Ansi256(u8),
    /// Named 16-color palette index (0..=15).
    Named(u8),
    /// Terminal default color.
    #[default]
    Default,
}

bitflags! {
    /// Cell attribute flags. Multiple may be set simultaneously
    /// (e.g. `BOLD | UNDERLINE`).
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Hash)]
    pub struct CellAttrs: u8 {
        /// Bold / bright text.
        const BOLD      = 0b0000_0001;
        /// Italic text.
        const ITALIC    = 0b0000_0010;
        /// Underline.
        const UNDERLINE = 0b0000_0100;
        /// Reverse-video (foreground and background swapped).
        const REVERSE   = 0b0000_1000;
        /// Dim / half-bright text.
        const DIM       = 0b0001_0000;
    }
}

/// Complete styling for a single [`Cell`](crate::Cell):
/// foreground color, background color, attribute flags.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct CellStyle {
    /// Foreground color, or `None` to mean "inherit from prior cell".
    pub fg: Option<CellColor>,
    /// Background color, or `None` to mean "inherit from prior cell".
    pub bg: Option<CellColor>,
    /// Set of attribute flags (bold, italic, underline, …).
    pub attrs: CellAttrs,
}

impl CellStyle {
    /// Create a plain style with no explicit colors and no attributes.
    #[must_use]
    pub const fn plain() -> Self {
        Self {
            fg: None,
            bg: None,
            attrs: CellAttrs::empty(),
        }
    }

    /// Set the foreground color.
    #[must_use]
    pub const fn with_fg(mut self, color: CellColor) -> Self {
        self.fg = Some(color);
        self
    }

    /// Set the background color.
    #[must_use]
    pub const fn with_bg(mut self, color: CellColor) -> Self {
        self.bg = Some(color);
        self
    }

    /// Set the attribute flags, replacing any prior attrs.
    #[must_use]
    pub const fn with_attrs(mut self, attrs: CellAttrs) -> Self {
        self.attrs = attrs;
        self
    }
}
