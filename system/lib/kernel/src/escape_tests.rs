//! Tests for the sans-IO escape parser, compiled into the lib under
//! `selftest` and run by the `arch-selftest` no_std runner.
//!
//! The parser is pure (no framebuffer, no UART), so every case drives it as a
//! byte stream and asserts the [`Action`] stream it yields — exactly how the
//! console consumes it, with nothing to mock.

use {
    super::{Action, Parser},
    reovim_testrt::{self as testrt, arch_test},
};

/// True when `action` is a `Print` of `byte`.
fn is_print(action: Option<Action>, byte: u8) -> bool {
    matches!(action, Some(Action::Print(c)) if c == byte)
}

/// True when `action` is an `Sgr` carrying exactly `expected`.
fn is_sgr(action: Option<Action>, expected: &[u16]) -> bool {
    matches!(action, Some(Action::Sgr(params)) if params.as_slice() == expected)
}

/// Feeds a whole sequence and returns the last action it produced — handy for
/// single-sequence cases where only the final byte yields one.
fn feed_sgr(seq: &[u8]) -> Option<Action> {
    let mut parser = Parser::new();
    let mut last = None;
    for &byte in seq {
        if let Some(action) = parser.advance(byte) {
            last = Some(action);
        }
    }
    last
}

arch_test!(escape_ground_prints_and_controls, {
    let mut parser = Parser::new();
    testrt::check(is_print(parser.advance(b'A'), b'A'), "a plain byte prints");
    testrt::check(matches!(parser.advance(b'\n'), Some(Action::LineFeed)), "\\n is a line feed");
    testrt::check(
        matches!(parser.advance(b'\r'), Some(Action::CarriageReturn)),
        "\\r is a carriage return",
    );
});

arch_test!(escape_non_csi_escape_is_dropped, {
    // ESC followed by a non-`[` byte is a sequence we do not handle; the byte
    // is dropped (not printed) and the parser returns to ground.
    let mut parser = Parser::new();
    testrt::check(parser.advance(0x1B).is_none(), "ESC alone yields nothing");
    testrt::check(parser.advance(b'c').is_none(), "ESC c is dropped, not printed");
    testrt::check(is_print(parser.advance(b'A'), b'A'), "ground resumes after a dropped escape");
});

arch_test!(escape_csi_sgr_single_param, {
    // Only the final `m` yields the action; every preceding byte pends.
    let mut parser = Parser::new();
    testrt::check(parser.advance(0x1B).is_none(), "ESC pends");
    testrt::check(parser.advance(b'[').is_none(), "[ pends");
    testrt::check(parser.advance(b'3').is_none(), "first digit pends");
    testrt::check(parser.advance(b'1').is_none(), "second digit pends");
    testrt::check(is_sgr(parser.advance(b'm'), &[31]), "m completes Sgr[31]");
});

arch_test!(escape_csi_sgr_multi_param, {
    testrt::check(
        is_sgr(feed_sgr(b"\x1b[38;5;46m"), &[38, 5, 46]),
        "ESC[38;5;46m parses to [38, 5, 46]",
    );
});

arch_test!(escape_csi_sgr_empty_and_leading_separator, {
    // ESC[m carries no parameters (SGR reads that as a reset).
    testrt::check(
        matches!(feed_sgr(b"\x1b[m"), Some(Action::Sgr(p)) if p.as_slice().is_empty()),
        "ESC[m yields an empty parameter set",
    );
    // A leading separator still counts the implicit zero parameter.
    testrt::check(is_sgr(feed_sgr(b"\x1b[;5m"), &[0, 5]), "ESC[;5m parses to [0, 5]");
});

arch_test!(escape_csi_non_sgr_final_is_noop, {
    // A recognized CSI with a non-`m` final (here erase, `J`) is consumed with
    // no action, and the parser is back in ground afterward.
    let mut parser = Parser::new();
    for &byte in b"\x1b[2J" {
        testrt::check(parser.advance(byte).is_none(), "no action through a non-SGR CSI");
    }
    testrt::check(is_print(parser.advance(b'X'), b'X'), "ground resumes after a non-SGR CSI");
});

arch_test!(escape_csi_private_marker_is_swallowed, {
    // ESC[?25h (private cursor-visibility): the `?` diverts to the ignore
    // path; the whole sequence — including the "25h" — is swallowed, so none
    // of it leaks into the printable stream.
    let mut parser = Parser::new();
    for &byte in b"\x1b[?25h" {
        testrt::check(
            parser.advance(byte).is_none(),
            "a private CSI produces no action and no stray print",
        );
    }
    testrt::check(is_print(parser.advance(b'X'), b'X'), "ground resumes after a private CSI");
});

arch_test!(escape_param_saturates, {
    // A parameter far past u16 range saturates rather than wrapping or
    // panicking on the bare-metal target.
    testrt::check(
        is_sgr(feed_sgr(b"\x1b[99999999m"), &[u16::MAX]),
        "an overlong parameter saturates to u16::MAX",
    );
});

arch_test!(escape_param_count_overflow_drops_excess, {
    // More parameters than the buffer holds: the retained slice clamps to the
    // capacity with no out-of-bounds write or panic.
    let mut parser = Parser::new();
    parser.advance(0x1B);
    parser.advance(b'[');
    for _ in 0..20 {
        parser.advance(b'1');
        parser.advance(b';');
    }
    parser.advance(b'1');
    let last = parser.advance(b'm');
    testrt::check(
        matches!(last, Some(Action::Sgr(sp)) if sp.as_slice().len() == 16),
        "excess parameters drop; the retained count clamps to the 16-slot buffer",
    );
});

arch_test!(escape_integration_action_stream, {
    // Integration smoke: a full real byte run composes into the expected
    // ordered action stream.
    //   ESC[38;5;46m -> Sgr[38, 5, 46]
    //   X            -> Print('X')
    //   ESC[0m       -> Sgr[0]
    let mut parser = Parser::new();
    let mut actions: [Option<Action>; 3] = [None; 3];
    let mut n = 0;
    for &byte in b"\x1b[38;5;46mX\x1b[0m" {
        if let Some(action) = parser.advance(byte) {
            if n < actions.len() {
                actions[n] = Some(action);
            }
            n += 1;
        }
    }
    testrt::check(n == 3, "exactly three actions surface from the stream");
    testrt::check(is_sgr(actions[0], &[38, 5, 46]), "the leading sequence sets the indexed color");
    testrt::check(is_print(actions[1], b'X'), "the glyph prints between the two sequences");
    testrt::check(is_sgr(actions[2], &[0]), "the trailing reset surfaces");
});
