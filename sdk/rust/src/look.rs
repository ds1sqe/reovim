//! A look: what a run of text is drawn in, bits and never a theme's
//! name for one. `crate::sgr` resolves a meaning to a look; this
//! module knows no meaning at all, only what a terminal draws.
//!
//! One `u64`. Bits 0-11 are the attrs, one bit apiece, the rest 0.
//! Bits 12-37 are the foreground: the low 24 its value, the top 2 its
//! kind — 0 default (no run holds one), 1 a palette index 0-255, 2 an
//! rgb triple packed `r<<16 | g<<8 | b`. Bits 38-63 are the
//! background, the same shape, 26 higher. 0 is no look at all, and no
//! run ever holds it.

use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;
use core::fmt::Write as _;

/// Bold.
pub const BOLD: u64 = 1;
/// Dim.
pub const DIM: u64 = 2;
/// Italic.
pub const ITALIC: u64 = 4;
/// Underline.
pub const UNDERLINE: u64 = 8;
/// Inverse (reverse video).
pub const INVERSE: u64 = 16;
/// Strike-through.
pub const STRIKE: u64 = 32;

/// Every attr and the SGR parameter it draws in, in the order
/// [`params`] writes them.
const ATTRS: [(u64, &str); 6] = [
    (BOLD, "1"),
    (DIM, "2"),
    (ITALIC, "3"),
    (UNDERLINE, "4"),
    (INVERSE, "7"),
    (STRIKE, "9"),
];

/// Where the foreground's 26 bits start; the background's are the
/// same shape this many bits higher.
const FG: u32 = 12;
const BG: u32 = 38;
/// The 26 bits one color takes: the low 24 its value, the top 2 its
/// kind.
const COLOR: u64 = (1 << 26) - 1;
const VALUE: u64 = (1 << 24) - 1;

const PALETTE: u64 = 1;
const RGB: u64 = 2;

/// A palette color 0-255, packed for [`fg`] or [`bg`].
#[must_use]
pub fn palette(n: u8) -> u32 {
    (1 << 24) | u32::from(n)
}

/// An rgb color, packed for [`fg`] or [`bg`].
#[must_use]
pub fn rgb(r: u8, g: u8, b: u8) -> u32 {
    (2 << 24) | (u32::from(r) << 16) | (u32::from(g) << 8) | u32::from(b)
}

/// `color` (from [`palette`] or [`rgb`]) as a look's foreground.
#[must_use]
pub fn fg(color: u32) -> u64 {
    (u64::from(color) & COLOR) << FG
}

/// `color` as a look's background.
#[must_use]
pub fn bg(color: u32) -> u64 {
    (u64::from(color) & COLOR) << BG
}

/// One color of `look`, at bit `shift`, as the SGR parameter a
/// terminal draws it in: `base` a palette 0-7 takes, `base + 60` a
/// palette 8-15, and `wide` the escape a palette past 15 or an rgb
/// one takes. `None` for the default, which draws in nothing.
fn part(look: u64, shift: u32, base: u64, wide: u64) -> Option<String> {
    let packed = (look >> shift) & COLOR;
    let value = packed & VALUE;
    match packed >> 24 {
        PALETTE if value < 8 => Some(format!("{}", base + value)),
        PALETTE if value < 16 => Some(format!("{}", base + 60 + (value - 8))),
        PALETTE => Some(format!("{wide};5;{value}")),
        RGB => {
            let r = value >> 16 & 0xFF;
            let g = value >> 8 & 0xFF;
            let b = value & 0xFF;
            Some(format!("{wide};2;{r};{g};{b}"))
        }
        _ => None,
    }
}

/// The SGR parameters `look` draws in, joined by `;`: the attrs it
/// holds first, then its foreground, then its background. A run never
/// holds a look of 0, so this is never asked to draw nothing.
#[must_use]
pub fn params(look: u64) -> String {
    let mut parts: Vec<String> = Vec::new();
    for &(bit, code) in &ATTRS {
        if look & bit != 0 {
            parts.push(String::from(code));
        }
    }
    parts.extend(part(look, FG, 30, 38));
    parts.extend(part(look, BG, 40, 48));
    parts.join(";")
}

/// `text` with every run of `runs` (`from`, `to`, look) drawn in its
/// own [`params`], reset after it.
///
/// `to` past `text`'s own length draws the one byte past it as a
/// space, which is how a run that takes a line's newline shows.
/// `runs` should be in order of `from` and not overlap, for what they
/// draw to mean anything, but this never panics on one that does not:
/// a run before the last one drawn, one whose `to` comes before its
/// own `from`, or one that does not cut `text` on a char's own
/// boundary, is skipped whole and paints nothing — the frame these
/// runs come in may be another Client's, corrupted or merely stale.
#[must_use]
pub fn painted(text: &str, runs: &[(u64, u64, u64)]) -> String {
    let len = text.len();
    let mut out = String::new();
    let mut at = 0;
    for &(from, to, look) in runs {
        let from = usize::try_from(from).unwrap_or(len).min(len);
        let raw_to = usize::try_from(to).unwrap_or(len);
        let end = raw_to.min(len);
        let boundary =
            text.is_char_boundary(from) && text.is_char_boundary(end);
        if from < at || end < from || !boundary {
            continue;
        }
        let pad = if raw_to > len { " " } else { "" };
        let _ = write!(
            out,
            "{}\x1b[{}m{}{pad}\x1b[0m",
            &text[at..from],
            params(look),
            &text[from..end],
        );
        at = end;
    }
    out.push_str(&text[at..]);
    out
}

#[cfg(test)]
mod tests {
    use super::{
        BOLD, DIM, INVERSE, ITALIC, STRIKE, UNDERLINE, bg, fg, painted,
        palette, params, rgb,
    };

    #[test]
    fn each_attr_prints_its_own_sgr_number() {
        let attrs = [
            (BOLD, "1"),
            (DIM, "2"),
            (ITALIC, "3"),
            (UNDERLINE, "4"),
            (INVERSE, "7"),
            (STRIKE, "9"),
        ];
        for (attr, code) in attrs {
            assert_eq!(params(attr), code);
        }
    }

    #[test]
    fn bold_and_inverse_print_both_in_that_order() {
        assert_eq!(params(BOLD | INVERSE), "1;7");
    }

    #[test]
    fn a_palette_color_under_eight_is_thirty_or_forty_plus_it() {
        assert_eq!(params(fg(palette(4))), "34");
        assert_eq!(params(bg(palette(4))), "44");
    }

    #[test]
    fn a_palette_color_eight_to_fifteen_is_the_bright_one() {
        assert_eq!(params(fg(palette(12))), "94");
        assert_eq!(params(bg(palette(12))), "104");
    }

    #[test]
    fn a_palette_color_past_fifteen_is_the_wide_escape() {
        assert_eq!(params(fg(palette(208))), "38;5;208");
        assert_eq!(params(bg(palette(208))), "48;5;208");
    }

    #[test]
    fn an_rgb_color_is_the_wide_escape_of_two() {
        assert_eq!(params(fg(rgb(10, 20, 30))), "38;2;10;20;30");
        assert_eq!(params(bg(rgb(1, 2, 3))), "48;2;1;2;3");
    }

    #[test]
    fn painted_wraps_each_run_and_resets_after_it() {
        let runs = [(0, 3, fg(palette(4)))];
        assert_eq!(painted("bin", &runs), "\x1b[34mbin\x1b[0m");
    }

    #[test]
    fn a_run_past_the_text_paints_one_space_past_it() {
        let runs = [(0, 4, INVERSE)];
        assert_eq!(painted("abc", &runs), "\x1b[7mabc \x1b[0m");
    }

    /// A run whose `to` comes before its own `from` is a Client's
    /// mistake, or worse, and not a range to slice on.
    #[test]
    fn a_run_that_goes_backwards_is_skipped_whole() {
        let runs = [(5, 2, INVERSE)];
        assert_eq!(painted("plain text", &runs), "plain text");
    }

    /// A run whose `to` lands inside a multi-byte char (`한` is bytes
    /// 1..4 of "a한b") is skipped whole, not sliced at the byte it
    /// named.
    #[test]
    fn a_run_ending_inside_a_char_is_skipped_whole() {
        let runs = [(0, 2, INVERSE)];
        assert_eq!(painted("a한b", &runs), "a한b");
    }

    /// A run whose `from` lands inside a multi-byte char, likewise.
    #[test]
    fn a_run_starting_inside_a_char_is_skipped_whole() {
        let runs = [(2, 4, INVERSE)];
        assert_eq!(painted("a한b", &runs), "a한b");
    }
}
