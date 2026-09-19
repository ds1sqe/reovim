//! The seven colors a Driver under `/bin` may print in, and the one
//! table that pairs each with the SGR parameter a terminal draws it in.
//!
//! A Driver knows the style and never the escape: it wraps text with
//! [`paint`] and that is all it says. A Client reading a Driver's
//! output takes the escapes back apart with [`style`], and whatever
//! draws the Client's screen writes them again with [`code`]. The two
//! directions are one table, so a color cannot mean one thing at the
//! Driver and another at the terminal.

use alloc::format;
use alloc::string::{String, ToString};

/// Bold blue, the first of the seven: what one of these means is the
/// color alone, and which stands for what is the caller's.
pub const BLUE: u64 = 3;
/// Green.
pub const GREEN: u64 = 4;
/// Red.
pub const RED: u64 = 5;
/// Bold red.
pub const RED_BOLD: u64 = 6;
/// Bold.
pub const BOLD: u64 = 7;
/// Yellow.
pub const YELLOW: u64 = 8;
/// Dim.
pub const DIM: u64 = 9;

/// Each style and the SGR parameter it is drawn in, in one order.
const PAIRS: [(u64, &str); 7] = [
    (BLUE, "1;34"),
    (GREEN, "32"),
    (RED, "31"),
    (RED_BOLD, "1;31"),
    (BOLD, "1"),
    (YELLOW, "33"),
    (DIM, "2"),
];

/// The SGR parameter `style` is drawn in; one that is none of the
/// seven is drawn plain, which is the reset.
#[must_use]
pub fn code(style: u64) -> &'static str {
    PAIRS.iter().find(|pair| pair.0 == style).map_or("0", |pair| pair.1)
}

/// The style an SGR parameter stands for, `None` for anything else,
/// the reset first.
#[must_use]
pub fn style(code: &str) -> Option<u64> {
    PAIRS.iter().find(|pair| pair.1 == code).map(|pair| pair.0)
}

/// `text` in one SGR sequence when the run asked for color, reset
/// after it, so a line never leaks a color into the next.
#[must_use]
pub fn paint(color: bool, style: u64, text: &str) -> String {
    let code = code(style);
    if color { format!("\x1b[{code}m{text}\x1b[0m") } else { text.to_string() }
}
