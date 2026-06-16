//! `TextHandler` — `OnRawInput` handler for the text Domain (#797).
//!
//! Applies the minimal edit set demanded by the walking skeleton:
//! - Printable ASCII character (0x20..=0x7e) → insert at cursor.
//! - `\x08` (BS) or `\x7f` (DEL) → delete the byte before the cursor.
//! - `\x1b[D` (cursor-left) or `\x02` (Ctrl-B) → cursor left by one byte.
//! - `\x1b[C` (cursor-right) or `\x06` (Ctrl-F) → cursor right by one byte.
//!
//! The input bytes are the raw `SendInput` payload (§7.5 of the protocol spec).
//! Multi-byte insertion is NOT yet handled — each byte is processed individually.
//! Full Unicode grapheme handling is #797 Phase 4+.
//!
//! Buffer mutation rebuilds the `Bytes` from slices around the edit point to
//! avoid needing `DerefMut` on `lib_ds::Bytes`.

use {
    reovim_lib_ds::Bytes,
    reovim_subsys_domain::contract::{OnRawInputHandler, RawInputResult},
};

// ── TextHandler ───────────────────────────────────────────────────────────────

/// The `OnRawInput` handler for the text Domain (walking-skeleton).
///
/// Stateless: all per-session state arrives as arguments via the CC14
/// snapshot-and-apply dispatch (§2.3).
///
/// ```no_run
/// // no_run: `on_raw_input` allocates through the `lib/ds` `Bytes`, which
/// // reaches the allocator via the boot-installed `kabi` platform handle. A
/// // doctest runs outside the arch boot path (no `rust_entry`/install), so the
/// // call cannot execute here; the runtime behaviour is covered by the booted
/// // kernel selftests. The example still type-checks the handler API.
/// use reovim_domain_text::handler::TextHandler;
/// use reovim_subsys_domain::contract::OnRawInputHandler as _;
/// use reovim_lib_ds::Bytes;
///
/// let h = TextHandler;
/// let buf = Bytes::new();
/// // Insert 'a' (printable ASCII).
/// let result = h.on_raw_input(buf, 0, b"a");
/// assert_eq!(result.buffer.as_slice(), b"a");
/// assert_eq!(result.cursor, 1);
/// assert!(result.claimed);
/// ```
pub struct TextHandler;

impl OnRawInputHandler for TextHandler {
    fn on_raw_input(&self, buffer: Bytes, cursor: usize, input: &[u8]) -> RawInputResult {
        let mut current = buffer;
        let mut cur = cursor;
        let mut claimed = false;
        let mut i = 0;

        while i < input.len() {
            let b = input[i];

            // ── Escape sequences (cursor left / cursor right) ─────────────────
            if b == 0x1b && i + 2 < input.len() && input[i + 1] == b'[' {
                match input[i + 2] {
                    b'D' => {
                        // Cursor left: `\x1b[D`
                        if cur > 0 {
                            cur -= 1;
                            claimed = true;
                        }
                        i += 3;
                        continue;
                    }
                    b'C' => {
                        // Cursor right: `\x1b[C`
                        if cur < current.len() {
                            cur += 1;
                            claimed = true;
                        }
                        i += 3;
                        continue;
                    }
                    _ => {
                        // Unknown CSI final byte: consume the whole 3-byte
                        // sequence — leaking its tail into the buffer as
                        // printable bytes would corrupt the text.
                        i += 3;
                        continue;
                    }
                }
            }

            match b {
                // Backspace (BS) or DEL: delete byte before cursor.
                0x08 | 0x7f if cur > 0 => {
                    cur -= 1;
                    current = delete_at(current, cur);
                    claimed = true;
                }
                // Ctrl-B: cursor left.
                0x02 if cur > 0 => {
                    cur -= 1;
                    claimed = true;
                }
                // Ctrl-F: cursor right; ignored at the end of the buffer so an
                // ancestor may handle boundary navigation.
                0x06 if cur < current.len() => {
                    cur += 1;
                    claimed = true;
                }
                // Printable ASCII: insert at cursor.
                0x20..=0x7e => {
                    // insert_at returns original on allocation failure (byte dropped).
                    let old_len = current.len();
                    current = insert_at(current, cur, b);
                    if current.len() == old_len + 1 {
                        cur += 1;
                    }
                    claimed = true;
                }
                // All other bytes (other control characters, high bytes) and
                // positional no-ops (backspace at start): ignore.
                _ => {}
            }
            i += 1;
        }

        if claimed {
            RawInputResult::claimed(current, cur)
        } else {
            RawInputResult::ignored(current, cur)
        }
    }
}

// ── Buffer mutation helpers ───────────────────────────────────────────────────

/// Returns a new `Bytes` with `byte` inserted at position `pos`.
///
/// Rebuilds as `buf[..pos] ++ [byte] ++ buf[pos..]`. On allocation failure
/// returns the original buffer unchanged (OOM is non-fatal in insert).
fn insert_at(buf: Bytes, pos: usize, byte: u8) -> Bytes {
    let slice = buf.as_slice();
    let mut out = Bytes::new();
    // Reserve space upfront (one extra byte).
    if out.try_extend_from_slice(&slice[..pos]).is_err()
        || out.try_push(byte).is_err()
        || out.try_extend_from_slice(&slice[pos..]).is_err()
    {
        // OOM: return original.
        return buf;
    }
    out
}

/// Returns a new `Bytes` with the byte at position `pos` removed.
///
/// Rebuilds as `buf[..pos] ++ buf[pos+1..]`. No-ops if `pos >= buf.len()`.
/// On allocation failure returns the original buffer unchanged.
pub(crate) fn delete_at(buf: Bytes, pos: usize) -> Bytes {
    let slice = buf.as_slice();
    if pos >= slice.len() {
        return buf;
    }
    let mut out = Bytes::new();
    if out.try_extend_from_slice(&slice[..pos]).is_err()
        || out.try_extend_from_slice(&slice[pos + 1..]).is_err()
    {
        return buf;
    }
    out
}

// L12 layout: tests in sibling handler_tests.rs, declared in lib.rs.
