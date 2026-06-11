//! Raw-byte stdin read and `RawInputList` encoding for `SendInput`.
//!
//! Classifies each raw byte from stdin into the minimal edit set:
//! - Printable ASCII (`0x20..=0x7e`) → `Key` input.
//! - `\x08` (BS) or `\x7f` (DEL) → `Key` input (backspace).
//! - `\r` (`0x0d`) → `Key` input (enter, maps to `\n` on the wire).
//! - Other control bytes → dropped (not yet handled).
//!
//! Each byte is individually wrapped as a `RawInputList` entry. Multi-byte
//! sequences (ANSI escapes for arrow keys) are forwarded verbatim when the
//! first byte is `0x1b` and the sequence fits in the read buffer.

use {reovim_arch::ds::Seq, reovim_uapi_abi::input::RawInputKind};

/// Encodes `payload` as a single-entry `RawInputList` byte buffer.
///
/// Layout: `[count u32 LE][kind u8][payload_len u32 LE][payload bytes]`.
///
/// ```rust,no_run
/// // no_run: requires arch allocator runtime.
/// use reovim_platform_tui::input::encode_raw_input_list;
///
/// let buf = encode_raw_input_list(b"x").unwrap();
/// assert!(buf.len() > 0);
/// ```
#[must_use]
pub fn encode_raw_input_list(payload: &[u8]) -> Option<Seq<u8>> {
    let mut buf: Seq<u8> = Seq::new();
    // count = 1
    for b in 1u32.to_le_bytes() {
        buf.try_push(b).ok()?;
    }
    // kind = Key
    buf.try_push(RawInputKind::Key as u8).ok()?;
    // payload length as u32 LE
    let len = u32::try_from(payload.len()).ok()?;
    for b in len.to_le_bytes() {
        buf.try_push(b).ok()?;
    }
    // payload bytes
    for &b in payload {
        buf.try_push(b).ok()?;
    }
    Some(buf)
}

/// Returns `true` when `b` is a byte the walking-skeleton client should forward
/// to the server as a `SendInput`.
///
/// Accepted: printable ASCII, backspace (`\x08`, `\x7f`), enter (`\r`),
/// ANSI escape prefix (`\x1b`).
///
/// ```rust
/// use reovim_platform_tui::input::is_forwardable;
///
/// assert!(is_forwardable(b'a'));
/// assert!(is_forwardable(b'\x08'));
/// assert!(is_forwardable(b'\x7f'));
/// assert!(is_forwardable(b'\x1b'));
/// assert!(!is_forwardable(b'\x01')); // Ctrl-A — not yet handled
/// assert!(!is_forwardable(0x00));    // NUL — not forwarded
/// ```
#[must_use]
pub const fn is_forwardable(b: u8) -> bool {
    matches!(b, 0x08 | 0x0d | 0x1b | 0x20..=0x7e | 0x7f)
}

// L12: no sibling test file — the two public fns are tested via doc-tests
// and via the integration smoke in the client selftest fixture.
