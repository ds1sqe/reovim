//! Tests for `handler.rs` — `TextHandler` coverage.
//!
//! Registered under the `selftest` feature; runs on the arch no_std selftest
//! runner via the `kernel-selftest` fixture bin.

use {
    reovim_arch::{arch_test, ds::Bytes},
    reovim_subsys_domain::contract::OnRawInputHandler as _,
};

use crate::handler::TextHandler;

arch_test!(handler_insert_printable_char, {
    let h = TextHandler;
    let buf = Bytes::new();
    let result = h.on_raw_input(buf, 0, b"a");
    assert!(result.claimed);
    let new_buf = result.buffer;
    let new_cursor = result.cursor;
    assert_eq!(new_buf.as_slice(), b"a");
    assert_eq!(new_cursor, 1);
});

arch_test!(handler_insert_multiple_chars, {
    let h = TextHandler;
    let buf = Bytes::new();
    let result = h.on_raw_input(buf, 0, b"hello");
    let new_buf = result.buffer;
    let new_cursor = result.cursor;
    assert_eq!(new_buf.as_slice(), b"hello");
    assert_eq!(new_cursor, 5);
});

arch_test!(handler_backspace_deletes_char_before_cursor, {
    let h = TextHandler;
    let buf = Bytes::try_from_slice(b"hi").expect("alloc");
    // Backspace at cursor=2 deletes 'i'.
    let result = h.on_raw_input(buf, 2, b"\x08");
    let new_buf = result.buffer;
    let new_cursor = result.cursor;
    assert_eq!(new_buf.as_slice(), b"h");
    assert_eq!(new_cursor, 1);
});

arch_test!(handler_backspace_at_start_noop, {
    let h = TextHandler;
    let buf = Bytes::try_from_slice(b"hi").expect("alloc");
    // Backspace at cursor=0 is a no-op.
    let result = h.on_raw_input(buf, 0, b"\x08");
    assert!(!result.claimed);
    let new_buf = result.buffer;
    let new_cursor = result.cursor;
    assert_eq!(new_buf.as_slice(), b"hi");
    assert_eq!(new_cursor, 0);
});

arch_test!(handler_del_same_as_backspace, {
    let h = TextHandler;
    let buf = Bytes::try_from_slice(b"ab").expect("alloc");
    let result = h.on_raw_input(buf, 2, b"\x7f");
    let new_buf = result.buffer;
    let new_cursor = result.cursor;
    assert_eq!(new_buf.as_slice(), b"a");
    assert_eq!(new_cursor, 1);
});

arch_test!(handler_cursor_left_escape_sequence, {
    let h = TextHandler;
    let buf = Bytes::try_from_slice(b"ab").expect("alloc");
    // `\x1b[D` moves cursor left.
    let result = h.on_raw_input(buf, 2, b"\x1b[D");
    let new_buf = result.buffer;
    let new_cursor = result.cursor;
    assert_eq!(new_buf.as_slice(), b"ab", "buffer unchanged by cursor move");
    assert_eq!(new_cursor, 1);
});

arch_test!(handler_cursor_right_escape_sequence, {
    let h = TextHandler;
    let buf = Bytes::try_from_slice(b"ab").expect("alloc");
    // `\x1b[C` moves cursor right.
    let result = h.on_raw_input(buf, 0, b"\x1b[C");
    let new_buf = result.buffer;
    let new_cursor = result.cursor;
    assert_eq!(new_buf.as_slice(), b"ab", "buffer unchanged by cursor move");
    assert_eq!(new_cursor, 1);
});

arch_test!(handler_ctrl_b_cursor_left, {
    let h = TextHandler;
    let buf = Bytes::try_from_slice(b"abc").expect("alloc");
    let result = h.on_raw_input(buf, 3, b"\x02");
    let new_buf = result.buffer;
    let new_cursor = result.cursor;
    assert_eq!(new_buf.as_slice(), b"abc");
    assert_eq!(new_cursor, 2);
});

arch_test!(handler_ctrl_f_cursor_right, {
    let h = TextHandler;
    let buf = Bytes::try_from_slice(b"abc").expect("alloc");
    let result = h.on_raw_input(buf, 0, b"\x06");
    let new_buf = result.buffer;
    let new_cursor = result.cursor;
    assert_eq!(new_buf.as_slice(), b"abc");
    assert_eq!(new_cursor, 1);
});

arch_test!(handler_cursor_right_at_end_noop, {
    let h = TextHandler;
    let buf = Bytes::try_from_slice(b"x").expect("alloc");
    // `\x1b[C` at end: cursor stays.
    let result = h.on_raw_input(buf, 1, b"\x1b[C");
    assert!(!result.claimed);
    let new_buf = result.buffer;
    let new_cursor = result.cursor;
    assert_eq!(new_buf.as_slice(), b"x");
    assert_eq!(new_cursor, 1);
});

arch_test!(handler_cursor_left_at_start_noop, {
    let h = TextHandler;
    let buf = Bytes::try_from_slice(b"x").expect("alloc");
    let result = h.on_raw_input(buf, 0, b"\x1b[D");
    assert!(!result.claimed);
    let new_buf = result.buffer;
    let new_cursor = result.cursor;
    assert_eq!(new_buf.as_slice(), b"x");
    assert_eq!(new_cursor, 0);
});

arch_test!(handler_insert_in_middle_of_buffer, {
    let h = TextHandler;
    // Buffer "ab", cursor at 1, insert 'x' → "axb".
    let buf = Bytes::try_from_slice(b"ab").expect("alloc");
    let result = h.on_raw_input(buf, 1, b"x");
    let new_buf = result.buffer;
    let new_cursor = result.cursor;
    assert_eq!(new_buf.as_slice(), b"axb");
    assert_eq!(new_cursor, 2);
});

arch_test!(handler_deterministic_byte_stable_output, {
    // Same input twice → same output (DEV5 reproducibility).
    let h = TextHandler;
    let run = |input: &[u8]| -> Bytes {
        let buf = Bytes::new();
        h.on_raw_input(buf, 0, input).buffer
    };
    let a = run(b"reovim");
    let b = run(b"reovim");
    assert_eq!(a.as_slice(), b.as_slice(), "handler output must be byte-stable");
});

// ── Unknown escape sequence arm (handler.rs line 65: `_ => {}`) ──────────────
//
// `\x1b[X` where X is not 'D' or 'C' is consumed as an unknown CSI sequence,
// leaves buffer/cursor unchanged, and reports `Ignored` for ancestor fallthrough.

arch_test!(handler_unknown_escape_sequence_is_ignored, {
    let h = TextHandler;
    let buf = Bytes::try_from_slice(b"ab").expect("alloc");
    // `\x1b[Z` — unknown escape → both `_ => {}` arms hit; buffer unchanged.
    let result = h.on_raw_input(buf, 1, b"\x1b[Z");
    assert!(!result.claimed);
    let new_buf = result.buffer;
    let new_cursor = result.cursor;
    assert_eq!(new_buf.as_slice(), b"ab", "unknown escape must not modify buffer");
    assert_eq!(new_cursor, 1, "unknown escape must not move cursor");
});

// ── Main match `_ => {}` arm (handler.rs line 95) ────────────────────────────
//
// Bytes not handled by any named arm fall through to `_ => {}`:
// - `0x00` (NUL) — not in printable range, not BS/DEL/Ctrl-B/Ctrl-F.
// - `0x01` (Ctrl-A) — not handled.
// These must be ignored (buffer and cursor unchanged).

arch_test!(handler_unhandled_control_bytes_are_ignored, {
    let h = TextHandler;
    // NUL byte → `_ => {}` (line 95).
    let buf = Bytes::try_from_slice(b"x").expect("alloc");
    let result = h.on_raw_input(buf, 1, b"\x00");
    assert!(!result.claimed);
    let new_buf = result.buffer;
    let new_cursor = result.cursor;
    assert_eq!(new_buf.as_slice(), b"x", "NUL must not modify buffer");
    assert_eq!(new_cursor, 1, "NUL must not move cursor");

    // Ctrl-A (0x01) → `_ => {}`.
    let buf2 = Bytes::try_from_slice(b"hi").expect("alloc");
    let result2 = h.on_raw_input(buf2, 0, b"\x01");
    assert!(!result2.claimed);
    let new_buf2 = result2.buffer;
    let new_cursor2 = result2.cursor;
    assert_eq!(new_buf2.as_slice(), b"hi", "Ctrl-A must not modify buffer");
    assert_eq!(new_cursor2, 0, "Ctrl-A must not move cursor");
});

// ── Ctrl-F at end-of-buffer (positional no-op, also `_ => {}` via guard) ─────
//
// The Ctrl-F arm is recognized, but reports `Ignored` when it cannot move.

arch_test!(handler_ctrl_f_at_end_of_buffer_is_noop, {
    let h = TextHandler;
    let buf = Bytes::try_from_slice(b"z").expect("alloc");
    // cursor=1 == len=1 → Ctrl-F guard fails → `_ => {}`.
    let result = h.on_raw_input(buf, 1, b"\x06");
    assert!(!result.claimed);
    let new_buf = result.buffer;
    let new_cursor = result.cursor;
    assert_eq!(new_buf.as_slice(), b"z", "Ctrl-F at end must not modify buffer");
    assert_eq!(new_cursor, 1, "Ctrl-F at end must not move cursor");
});

// ── delete_at pos >= len (handler.rs line 131: `return buf;`) ────────────────
//
// `delete_at(buf, pos)` returns `buf` unchanged when `pos >= slice.len()`.
// This is reached from the backspace arm `0x08 | 0x7f if cur > 0` — after
// `cur -= 1` the delete is called at `cur`. But it is NOT reached that way
// because the guard `cur > 0` ensures `cur` is valid. However, since
// `delete_at` is a private helper, we call it indirectly by constructing
// a scenario where cursor > 0 but the buffer is shorter than cursor.
//
// Actually, in `on_raw_input` the handler always maintains `cur <= current.len()`.
// So `delete_at(buf, cur)` with `cur < buf.len()` is always the live path; the
// `pos >= slice.len()` branch in `delete_at` is only reachable if called with
// `pos` out of range. Since `delete_at` is private and only called from the
// handler with `pos = cur - 1` after `cur -= 1`, and `cur > 0` guard ensures
// `cur - 1 < current.len()` (because `cur` never exceeds `current.len()`),
// this branch is structurally dead code in the current handler.
//
// Classification: structural dead-code residue; see ledger.

// ── insert_at alloc-failure arm (handler.rs line 119: `return buf;`) ─────────
//
// `insert_at` returns the original `buf` when `try_extend_from_slice` fails.
// Provoked by an alloc-fault sweep.

arch_test!(handler_insert_alloc_fault_returns_original, {
    let h = TextHandler;
    let mut k = 0isize;
    loop {
        let buf = Bytes::try_from_slice(b"hello").expect("alloc buf");
        reovim_arch::alloc::fault::fail_after(k);
        let result = h.on_raw_input(buf, 2, b"X");
        assert!(result.claimed);
        let result_buf = result.buffer;
        reovim_arch::alloc::fault::reset();
        // Either the insert succeeded (larger buf) or returned original (b"hello").
        let s = result_buf.as_slice();
        if s == b"heXllo" {
            // Insert succeeded on this k; sweep is done.
            break;
        }
        // OOM: insert_at returned original.
        assert_eq!(s, b"hello", "insert_at OOM must return original buffer");
        k += 1;
        assert!(k < 64, "insert must succeed within bounded alloc count");
    }
});

// ── delete_at alloc-failure arm (handler.rs line 137: `return buf;`) ─────────
//
// `delete_at` returns the original `buf` when `try_extend_from_slice` fails.

arch_test!(handler_delete_alloc_fault_returns_original, {
    let h = TextHandler;
    let mut k = 0isize;
    loop {
        let buf = Bytes::try_from_slice(b"ab").expect("alloc buf");
        reovim_arch::alloc::fault::fail_after(k);
        // backspace at cursor=2: cur-=1 → cur=1, delete_at(buf, 1).
        let result = h.on_raw_input(buf, 2, b"\x08");
        assert!(result.claimed);
        let result_buf = result.buffer;
        reovim_arch::alloc::fault::reset();
        let s = result_buf.as_slice();
        if s == b"a" {
            // Delete succeeded.
            break;
        }
        // OOM: delete_at returned original.
        assert_eq!(s, b"ab", "delete_at OOM must return original buffer");
        k += 1;
        assert!(k < 64, "delete must succeed within bounded alloc count");
    }
});

arch_test!(delete_at_out_of_range_returns_buffer_unchanged, {
    // No caller passes pos >= len today (the cursor is clamped), but the
    // guard documents the contract; exercise the arm directly.
    let buf = Bytes::try_from_slice(b"ab").expect("alloc");
    let out = crate::handler::delete_at(buf, 5);
    assert_eq!(out.as_slice(), b"ab");
});
