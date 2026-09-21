//! The meanings a run may carry, and the one theme that resolves each
//! to a [`crate::look`].
//!
//! `SELECTION` and `MATCH` are a Plugin's own, and no Driver ever
//! writes them; the next seven are the colors a Driver under `/bin`
//! may print in, one table pairing each with the look it always drew
//! in, so a color cannot mean one thing at the Driver and another at
//! the terminal. The nine after those are what a decoration means, the
//! `style` of a look a Plugin serves over a Buffer.
//!
//! [`paint`] wraps text in a meaning's escape and [`style`] takes one
//! back apart; [`look()`] is the theme itself, one meaning to one look,
//! for whoever resolves meanings to looks in the first place.
//!
//! [`csi`] and [`plain`] know no meaning at all: they are what a
//! reader with no colour takes an escape sequence back out with.

use alloc::string::{String, ToString};
use alloc::vec::Vec;

use crate::look::{self, palette};

/// A selected span: the Plugin's own, and no Driver ever writes it.
pub const SELECTION: u64 = 1;
/// A span that matches the last search: the Plugin's own.
pub const MATCH: u64 = 2;
/// Bold blue, the first of the seven a Driver under `/bin` may print
/// in: what one of these means is the color alone, and which stands
/// for what is the caller's.
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

/// A heading, the first of the nine a decoration means: a Plugin over
/// a Buffer serves the construct and the theme says the look.
pub const HEADING: u64 = 10;
/// Emphasised text.
pub const EMPHASIS: u64 = 11;
/// Strongly emphasised text.
pub const STRONG: u64 = 12;
/// A code span.
pub const CODE: u64 = 13;
/// A quoted block.
pub const QUOTE: u64 = 14;
/// A horizontal rule.
pub const RULE: u64 = 15;
/// A link.
pub const LINK: u64 = 16;
/// A fenced block, tinted for its whole width.
pub const FENCE: u64 = 17;
/// The marker of a construct, which the drawer conceals: the one row
/// of the theme whose look carries [`crate::look::HIDDEN`].
pub const MARKER: u64 = 18;

/// The seven a Driver may write, in [`style`]'s search order.
const DRIVEN: [u64; 7] = [BLUE, GREEN, RED, RED_BOLD, BOLD, YELLOW, DIM];

/// Every meaning and the look this theme resolves it to. `params` of
/// each of the seven a Driver writes is the string this crate always
/// drew it in, so the bytes of `ls`, `dmesg` and `watch` do not move
/// with it.
fn theme() -> [(u64, u64); 18] {
    [
        (SELECTION, look::INVERSE),
        (MATCH, look::UNDERLINE),
        (BLUE, look::BOLD | look::fg(palette(4))),
        (GREEN, look::fg(palette(2))),
        (RED, look::fg(palette(1))),
        (RED_BOLD, look::BOLD | look::fg(palette(1))),
        (BOLD, look::BOLD),
        (YELLOW, look::fg(palette(3))),
        (DIM, look::DIM),
        (HEADING, look::BOLD | look::fg(palette(4))),
        (EMPHASIS, look::ITALIC),
        (STRONG, look::BOLD),
        (CODE, look::fg(palette(6))),
        (QUOTE, look::ITALIC | look::fg(palette(8))),
        (RULE, look::DIM),
        (LINK, look::UNDERLINE | look::fg(palette(4))),
        (FENCE, look::bg(palette(236))),
        (MARKER, look::HIDDEN),
    ]
}

/// The look this theme resolves `meaning` to; 0 for one it holds not,
/// which is no look and no run holds it.
#[must_use]
pub fn look(meaning: u64) -> u64 {
    theme().iter().find(|&&(m, _)| m == meaning).map_or(0, |&(_, l)| l)
}

/// The meaning an SGR parameter stands for, `None` for one none of
/// the seven a Driver may write draws in — the Plugin's own two
/// apart, which no Driver writes.
#[must_use]
pub fn style(code: &str) -> Option<u64> {
    DRIVEN.into_iter().find(|&meaning| look::params(look(meaning)) == code)
}

/// `text` in one SGR sequence when the run asked for color, reset
/// after it, so a line never leaks a color into the next.
#[must_use]
pub fn paint(color: bool, style: u64, text: &str) -> String {
    if !color {
        return text.to_string();
    }
    let resolved = look(style);
    if resolved == 0 {
        return text.to_string();
    }
    let len = u64::try_from(text.len()).unwrap_or(u64::MAX);
    look::painted(text, &[(0, len, resolved)])
}

/// What a control sequence begins with, and what a CSI's begins with.
const ESC: u8 = 0x1b;
const CSI: &str = "\x1b[";

/// The control sequence `seq` begins with: the parameters of an SGR,
/// `None` for a sequence that is not one, and what follows it either
/// way.
///
/// A CSI ends at its first byte of 0x40-0x7E, and an escape that
/// begins no CSI is one byte of nothing. `None` for a sequence that is
/// not whole yet, which is the caller's own carry.
#[must_use]
pub fn csi(seq: &str) -> Option<(Option<&str>, &str)> {
    let Some(body) = seq.strip_prefix(CSI) else {
        return (seq.len() > 1).then(|| (None, &seq[1..]));
    };
    let end = body.bytes().position(|b| (0x40..=0x7e).contains(&b))?;
    let code = (body.as_bytes()[end] == b'm').then(|| &body[..end]);
    Some((code, &body[end + 1..]))
}

/// A CSI's own bytes, `0x40-0x7E` apart, are always ASCII, so the
/// byte after `[` to the first of those is the same body [`csi`]
/// takes apart, worked over bytes that need not be UTF-8.
fn csi_body(after_bracket: &[u8]) -> Option<(Option<&[u8]>, &[u8])> {
    let end = after_bracket.iter().position(|&b| (0x40..=0x7e).contains(&b))?;
    let code = (after_bracket[end] == b'm').then(|| &after_bracket[..end]);
    Some((code, &after_bracket[end + 1..]))
}

/// `bytes` with every whole CSI sequence taken out: a reader with no
/// colour sees a Driver's own escapes stripped this way.
///
/// `cat` of anything under `/bin` is a bin too, so unlike [`csi`] —
/// which a chunked reader may carry a lone escape past, presuming a
/// Driver never writes one alone — this keeps an escape byte that
/// begins no CSI, and a CSI this one-shot answer cuts off in the
/// middle, as the content they may simply be.
#[must_use]
pub fn plain(bytes: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(bytes.len());
    let mut rest = bytes;
    while let Some(at) = rest.iter().position(|&b| b == ESC) {
        out.extend_from_slice(&rest[..at]);
        rest = &rest[at..];
        let Some(body) = rest.strip_prefix(CSI.as_bytes()) else {
            out.push(ESC);
            rest = &rest[1..];
            continue;
        };
        let Some((_, after)) = csi_body(body) else {
            // Cut in the middle: nothing more of this one-shot answer
            // will complete it, so the escape and what follows stand.
            out.extend_from_slice(rest);
            rest = &[];
            break;
        };
        rest = after;
    }
    out.extend_from_slice(rest);
    out
}

#[cfg(test)]
mod tests {
    use super::{
        BLUE, BOLD, CODE, DIM, EMPHASIS, FENCE, GREEN, HEADING, LINK, MARKER,
        MATCH, QUOTE, RED, RED_BOLD, RULE, SELECTION, STRONG, YELLOW, look,
        paint, plain, style,
    };
    use crate::look::{HIDDEN, params};

    /// The nine rows a run of a terminal's own colors rests on, and
    /// the string this crate always drew each of them in: the seven a
    /// Driver writes, and the Plugin's own two, which the app
    /// hardcoded as `"7"` and `"4"`.
    #[test]
    fn every_row_of_the_theme_prints_the_string_it_always_drew() {
        let rows = [
            (SELECTION, "7"),
            (MATCH, "4"),
            (BLUE, "1;34"),
            (GREEN, "32"),
            (RED, "31"),
            (RED_BOLD, "1;31"),
            (BOLD, "1"),
            (YELLOW, "33"),
            (DIM, "2"),
        ];
        for (meaning, code) in rows {
            assert_eq!(params(look(meaning)), code, "meaning {meaning}");
        }
    }

    /// The nine a decoration means: each has a look of its own, and
    /// `MARKER`'s is the one the drawer does not draw.
    #[test]
    fn every_decoration_means_a_look_and_marker_is_the_hidden_one() {
        let meanings =
            [HEADING, EMPHASIS, STRONG, CODE, QUOTE, RULE, LINK, FENCE, MARKER];
        for meaning in meanings {
            assert_ne!(look(meaning), 0, "meaning {meaning}");
        }
        assert_eq!(look(MARKER), HIDDEN);
    }

    #[test]
    fn a_meaning_no_row_holds_looks_like_nothing() {
        assert_eq!(look(0), 0);
    }

    #[test]
    fn style_finds_the_meaning_a_driver_s_own_code_stands_for() {
        assert_eq!(style("1;34"), Some(BLUE));
        assert_eq!(style("32"), Some(GREEN));
        assert_eq!(style("7"), None, "no Driver writes the Plugin's own");
    }

    #[test]
    fn paint_wraps_in_the_code_the_style_has_always_drawn_in() {
        assert_eq!(paint(true, BLUE, "bin"), "\x1b[1;34mbin\x1b[0m");
        assert_eq!(paint(false, BLUE, "bin"), "bin", "no color, no escape");
    }

    #[test]
    fn plain_takes_every_escape_out() {
        let said = b"\x1b[1;34mbin\x1b[0m  \x1b[32mls\x1b[0m  plain\n";
        assert_eq!(plain(said), b"bin  ls  plain\n");
    }

    #[test]
    fn plain_of_nothing_colored_is_the_same_bytes() {
        assert_eq!(plain(b"a  b  sub\n"), b"a  b  sub\n");
    }

    #[test]
    fn plain_keeps_an_escape_that_begins_no_csi() {
        // A small number's own bytes may hold 0x1b by coincidence; it
        // is not a Driver's escape, and no byte of it goes missing.
        let bytes = [1, 0x1b, 0, 0, 0, 0, 0, 0];
        assert_eq!(plain(&bytes), bytes);
    }

    #[test]
    fn plain_keeps_a_csi_this_one_shot_answer_cuts_off() {
        let bytes = b"one\x1b[3";
        assert_eq!(plain(bytes), bytes);
    }
}
