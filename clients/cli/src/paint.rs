//! Color, when the app says there may be one. The palette is these
//! methods and nothing else: each wraps its text in one SGR sequence
//! and resets after it, so a line never leaks a color into the next.
//!
//! Whether color is on is the app's to decide — a tty, `NO_COLOR`,
//! `--color` — and this crate never asks the terminal itself.

use alloc::format;
use alloc::string::{String, ToString};

/// Ends every colored piece.
const RESET: &str = "\x1b[0m";

/// The palette, on or off. Off, every method gives back the text.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct Paint {
    /// Whether the text may carry SGR sequences.
    pub on: bool,
}

impl Paint {
    /// No color at all: what every test reads and what a pipe gets.
    #[must_use]
    pub const fn off() -> Self {
        Self { on: false }
    }

    /// Color, as `--color=always` and a tty ask for.
    #[must_use]
    pub const fn on() -> Self {
        Self { on: true }
    }

    /// A Container in a listing, and the `-H` minute break.
    #[must_use]
    pub fn blue(&self, text: &str) -> String {
        self.sgr("1;34", text)
    }

    /// A cell that offers an api, and the `dmesg` stamp.
    #[must_use]
    pub fn green(&self, text: &str) -> String {
        self.sgr("32", text)
    }

    /// An `err` entry, and a watch event that took something away.
    #[must_use]
    pub fn red(&self, text: &str) -> String {
        self.sgr("31", text)
    }

    /// `crit` and above.
    #[must_use]
    pub fn red_bold(&self, text: &str) -> String {
        self.sgr("1;31", text)
    }

    /// A `warn` entry.
    #[must_use]
    pub fn bold(&self, text: &str) -> String {
        self.sgr("1", text)
    }

    /// `Attrib` and `Move`.
    #[must_use]
    pub fn yellow(&self, text: &str) -> String {
        self.sgr("33", text)
    }

    /// The `-x` decode in front of an entry.
    #[must_use]
    pub fn dim(&self, text: &str) -> String {
        self.sgr("2", text)
    }

    fn sgr(self, code: &str, text: &str) -> String {
        if self.on {
            format!("\x1b[{code}m{text}{RESET}")
        } else {
            text.to_string()
        }
    }
}
