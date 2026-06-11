//! Tests for `frame.rs` (L12.1 sibling, compiled under `selftest`).
//!
//! `compose_ansi_frame` is a pure function: given projection bytes, it
//! produces a deterministic ANSI byte sequence. These are golden-able tests —
//! they verify the DEV3 frame format contract.

use {
    crate::frame::{ComposeError, compose_ansi_frame},
    reovim_arch::arch_test,
};

/// Builds a 16-byte projection header + optional content bytes.
/// Layout: `[buf_id u32 LE][win_id u32 LE][cursor u32 LE][span_end u32 LE][content]`.
fn make_proj(
    buf_id: u32,
    win_id: u32,
    cursor: u32,
    span_end: u32,
    content: &[u8],
) -> reovim_arch::ds::Bytes {
    let mut v = reovim_arch::ds::Bytes::new();
    v.try_extend_from_slice(&buf_id.to_le_bytes()).unwrap();
    v.try_extend_from_slice(&win_id.to_le_bytes()).unwrap();
    v.try_extend_from_slice(&cursor.to_le_bytes()).unwrap();
    v.try_extend_from_slice(&span_end.to_le_bytes()).unwrap();
    v.try_extend_from_slice(content).unwrap();
    v
}

arch_test!(frame_compose_truncated_header_returns_err, {
    // A projection shorter than 16 bytes must return ComposeError::Truncated.
    let short = [0u8; 10];
    reovim_arch::testrt::check_eq(compose_ansi_frame(&short).err(), Some(ComposeError::Truncated));
});

arch_test!(frame_compose_empty_content_contains_escape_seqs, {
    // An empty buffer (span_end = 0, cursor = 0) produces erase + home + cursor-pos.
    let proj = make_proj(1, 1, 0, 0, b"");
    let frame = compose_ansi_frame(&proj).expect("compose succeeds on valid header");
    let bytes = frame.as_slice();
    // Must start with ESC[2J (erase display) + ESC[H (cursor home).
    reovim_arch::testrt::check(bytes.starts_with(b"\x1b[2J\x1b[H"), "frame starts with erase+home");
    // Must end with cursor reposition (ESC[1;1H for cursor_byte=0 → col=1).
    reovim_arch::testrt::check(
        bytes.ends_with(b"\x1b[1;1H"),
        "frame ends with cursor home ESC[1;1H",
    );
});

arch_test!(frame_compose_content_appears_in_frame, {
    // Content bytes must appear in the ANSI frame between the erase sequence and
    // the cursor-positioning suffix.
    let content = b"hello";
    let proj = make_proj(1, 1, 3, 5, content);
    let frame = compose_ansi_frame(&proj).expect("compose succeeds");
    let bytes = frame.as_slice();
    // "hello" must be present somewhere in the frame.
    reovim_arch::testrt::check(
        bytes.windows(5).any(|w| w == b"hello"),
        "content 'hello' appears in ANSI frame",
    );
    // Cursor col = 3 + 1 = 4 → ESC[1;4H (6 bytes).
    reovim_arch::testrt::check(
        bytes.windows(6).any(|w| w == b"\x1b[1;4H"),
        "cursor positioned at column 4",
    );
});

arch_test!(frame_compose_cursor_at_end_of_content, {
    let content = b"abc";
    let proj = make_proj(1, 1, 3, 3, content);
    let frame = compose_ansi_frame(&proj).expect("compose succeeds");
    let bytes = frame.as_slice();
    // cursor_byte = 3 → col = 4 → ESC[1;4H
    reovim_arch::testrt::check(
        bytes.windows(6).any(|w| w == b"\x1b[1;4H"),
        "cursor at end: col = span_end + 1",
    );
});

arch_test!(frame_compose_span_end_truncates_content, {
    // span_end=2 with content b"abcde": only first 2 bytes rendered.
    let content = b"abcde";
    let proj = make_proj(1, 1, 0, 2, content);
    let frame = compose_ansi_frame(&proj).expect("compose succeeds");
    let bytes = frame.as_slice();
    reovim_arch::testrt::check(
        bytes.windows(2).any(|w| w == b"ab"),
        "first 2 bytes of content present",
    );
    // 'c' must not appear (span_end=2 cuts it).
    reovim_arch::testrt::check(
        !bytes.windows(1).any(|w| w == b"c"),
        "byte beyond span_end not present",
    );
});

// ── push_decimal zero arm (frame.rs line 118) ─────────────────────────────────
//
// `push_decimal(n=0)` hits the early return: `if n == 0 { return dst.try_push(b'0') }`.
// This is reached when `cursor_byte = usize::MAX` so that `saturating_add(1)`
// stays at `usize::MAX`... but that is not the zero case.
//
// The zero case: `col = cursor_byte.saturating_add(1)`. `col == 0` only when
// `cursor_byte.saturating_add(1) == 0`, which is impossible because saturating_add
// from any usize value ≥ 0 gives ≥ 1. So `push_decimal` is never called with n=0
// from `compose_ansi_frame`. The zero branch is dead code under the current caller.
//
// However, `push_decimal` is a private fn so we test it indirectly via the
// public API — the nearest we can get is col=1 (cursor_byte=0) which is NOT
// the zero case, and col=0 is unreachable from compose_ansi_frame.
//
// This is classified as a structural-dead-code residue in the ledger.
// The test below documents the fact that col >= 1 always holds and the frame
// output never contains a bare "0" column (ESC[1;0H is never emitted).

arch_test!(frame_compose_cursor_byte_zero_gives_col_one, {
    // cursor_byte = 0 → col = saturating_add(1) = 1 → push_decimal(1) not zero.
    // The output must contain ESC[1;1H.
    let proj = make_proj(1, 1, 0, 0, b"");
    let frame = compose_ansi_frame(&proj).expect("compose succeeds");
    let bytes = frame.as_slice();
    reovim_arch::testrt::check(
        bytes.windows(6).any(|w| w == b"\x1b[1;1H"),
        "cursor_byte=0 must produce ESC[1;1H (col=1, not col=0)",
    );
});

// ── compose_ansi_frame alloc-fault sweep (frame.rs alloc error arms) ─────────
//
// `push_bytes` and `push_decimal` both call `try_push` and return `Err(())`
// on allocation failure, which propagates as `ComposeError::Alloc`. The alloc-
// fault sweep forces each `try_push` site to fail, exercising every Alloc arm.

arch_test!(frame_compose_alloc_fault_sweep, {
    let proj = make_proj(1, 1, 5, 10, b"0123456789");
    let mut k = 0isize;
    loop {
        reovim_arch::alloc::fault::fail_after(k);
        let result = compose_ansi_frame(proj.as_slice());
        reovim_arch::alloc::fault::reset();
        if result.is_ok() {
            break;
        }
        reovim_arch::testrt::check_eq(result.err(), Some(ComposeError::Alloc));
        k += 1;
        assert!(k < 200, "compose must succeed within bounded alloc count");
    }
});

arch_test!(push_decimal_renders_zero, {
    // No caller passes 0 today (columns are 1-based), but the rendering
    // contract documents zero; exercise the arm directly.
    let mut dst = reovim_arch::ds::Seq::new();
    crate::frame::push_decimal(&mut dst, 0).expect("push 0");
    assert_eq!(dst.as_slice(), b"0");
});
