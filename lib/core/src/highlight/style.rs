//! Text styling for syntax highlighting

use {
    crate::{
        constants::RESET_STYLE,
        highlight::color::{ColorMode, downgrade_color},
    },
    reovim_sys::style::Color,
};

/// Bitflags for text attributes
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Attributes(u8);

impl Attributes {
    pub const BOLD: u8 = 1 << 0;
    pub const ITALIC: u8 = 1 << 1;
    pub const UNDERLINE: u8 = 1 << 2;
    pub const STRIKETHROUGH: u8 = 1 << 3;
    pub const REVERSE: u8 = 1 << 4;

    #[must_use]
    pub const fn new() -> Self {
        Self(0)
    }

    #[must_use]
    pub const fn contains(self, attr: u8) -> bool {
        (self.0 & attr) != 0
    }

    #[allow(clippy::missing_const_for_fn)]
    pub fn set(&mut self, attr: u8) {
        self.0 |= attr;
    }

    #[must_use]
    pub const fn union(self, other: Self) -> Self {
        Self(self.0 | other.0)
    }
}

/// Represents text styling properties
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Style {
    pub fg: Option<Color>,
    pub bg: Option<Color>,
    pub attributes: Attributes,
}

impl Style {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    #[must_use]
    #[allow(clippy::missing_const_for_fn)]
    pub fn fg(mut self, color: Color) -> Self {
        self.fg = Some(color);
        self
    }

    #[must_use]
    #[allow(clippy::missing_const_for_fn)]
    pub fn bg(mut self, color: Color) -> Self {
        self.bg = Some(color);
        self
    }

    #[must_use]
    pub fn bold(mut self) -> Self {
        self.attributes.set(Attributes::BOLD);
        self
    }

    #[must_use]
    pub fn italic(mut self) -> Self {
        self.attributes.set(Attributes::ITALIC);
        self
    }

    #[must_use]
    pub fn underline(mut self) -> Self {
        self.attributes.set(Attributes::UNDERLINE);
        self
    }

    /// Merge another style on top of this one (other takes precedence for colors)
    #[must_use]
    pub fn merge(&self, other: &Self) -> Self {
        Self {
            fg: other.fg.or(self.fg),
            bg: other.bg.or(self.bg),
            attributes: self.attributes.union(other.attributes),
        }
    }

    /// Convert to ANSI escape sequence for starting this style
    /// Color mode determines how colors are converted for terminal compatibility
    #[must_use]
    pub fn to_ansi_start(&self, color_mode: ColorMode) -> String {
        let mut codes: Vec<&str> = Vec::new();
        let mut owned_codes: Vec<String> = Vec::new();

        // Foreground color (with downgrade based on color mode)
        if let Some(fg) = &self.fg {
            let converted = downgrade_color(*fg, color_mode);
            owned_codes.push(color_to_fg_ansi(&converted));
        }

        // Background color (with downgrade based on color mode)
        // When bg is None, emit reset code (49) to clear any previous background
        if let Some(bg) = &self.bg {
            let converted = downgrade_color(*bg, color_mode);
            owned_codes.push(color_to_bg_ansi(&converted));
        } else {
            codes.push("49"); // Reset background to terminal default
        }

        // Attributes
        if self.attributes.contains(Attributes::BOLD) {
            codes.push("1");
        }
        if self.attributes.contains(Attributes::ITALIC) {
            codes.push("3");
        }
        if self.attributes.contains(Attributes::UNDERLINE) {
            codes.push("4");
        }
        if self.attributes.contains(Attributes::STRIKETHROUGH) {
            codes.push("9");
        }
        if self.attributes.contains(Attributes::REVERSE) {
            codes.push("7");
        }

        if codes.is_empty() && owned_codes.is_empty() {
            String::new()
        } else {
            let all_codes: Vec<&str> = owned_codes
                .iter()
                .map(String::as_str)
                .chain(codes)
                .collect();
            format!("\x1b[{}m", all_codes.join(";"))
        }
    }

    /// ANSI reset sequence
    #[must_use]
    pub const fn ansi_reset() -> &'static str {
        RESET_STYLE
    }
}

#[allow(clippy::trivially_copy_pass_by_ref)]
fn color_to_fg_ansi(color: &Color) -> String {
    match color {
        Color::Reset => String::from("39"),
        Color::Black => String::from("30"),
        Color::DarkGrey => String::from("90"),
        Color::Red => String::from("31"),
        Color::DarkRed => String::from("91"),
        Color::Green => String::from("32"),
        Color::DarkGreen => String::from("92"),
        Color::Yellow => String::from("33"),
        Color::DarkYellow => String::from("93"),
        Color::Blue => String::from("34"),
        Color::DarkBlue => String::from("94"),
        Color::Magenta => String::from("35"),
        Color::DarkMagenta => String::from("95"),
        Color::Cyan => String::from("36"),
        Color::DarkCyan => String::from("96"),
        Color::White => String::from("37"),
        Color::Grey => String::from("97"),
        Color::AnsiValue(n) => format!("38;5;{n}"),
        Color::Rgb { r, g, b } => format!("38;2;{r};{g};{b}"),
    }
}

#[allow(clippy::trivially_copy_pass_by_ref)]
fn color_to_bg_ansi(color: &Color) -> String {
    match color {
        Color::Reset => String::from("49"),
        Color::Black => String::from("40"),
        Color::DarkGrey => String::from("100"),
        Color::Red => String::from("41"),
        Color::DarkRed => String::from("101"),
        Color::Green => String::from("42"),
        Color::DarkGreen => String::from("102"),
        Color::Yellow => String::from("43"),
        Color::DarkYellow => String::from("103"),
        Color::Blue => String::from("44"),
        Color::DarkBlue => String::from("104"),
        Color::Magenta => String::from("45"),
        Color::DarkMagenta => String::from("105"),
        Color::Cyan => String::from("46"),
        Color::DarkCyan => String::from("106"),
        Color::White => String::from("47"),
        Color::Grey => String::from("107"),
        Color::AnsiValue(n) => format!("48;5;{n}"),
        Color::Rgb { r, g, b } => format!("48;2;{r};{g};{b}"),
    }
}
