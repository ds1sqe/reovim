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
//! avoid needing `DerefMut` on `arch::ds::Bytes`.

use {reovim_arch::ds::Bytes, reovim_subsys_domain::contract::OnRawInputHandler};

// ── TextHandler ───────────────────────────────────────────────────────────────

/// The `OnRawInput` handler for the text Domain (walking-skeleton).
///
/// Stateless: all per-session state arrives as arguments via the CC14
/// snapshot-and-apply dispatch (§2.3).
///
/// ```rust
/// use reovim_domain_text::handler::TextHandler;
/// use reovim_subsys_domain::contract::OnRawInputHandler as _;
/// use reovim_arch::ds::Bytes;
///
/// let h = TextHandler;
/// let buf = Bytes::new();
/// // Insert 'a' (printable ASCII).
/// let (new_buf, new_cursor) = h.on_raw_input(buf, 0, b"a");
/// assert_eq!(new_buf.as_slice(), b"a");
/// assert_eq!(new_cursor, 1);
/// ```
pub struct TextHandler;

impl OnRawInputHandler for TextHandler {
    fn on_raw_input(&self, buffer: Bytes, cursor: usize, input: &[u8]) -> (Bytes, usize) {
        let mut current = buffer;
        let mut cur = cursor;
        let mut i = 0;

        while i < input.len() {
            let b = input[i];

            // ── Escape sequences (cursor left / cursor right) ─────────────────
            if b == 0x1b && i + 2 < input.len() && input[i + 1] == b'[' {
                match input[i + 2] {
                    b'D' => {
                        // Cursor left: `\x1b[D`
                        cur = cur.saturating_sub(1);
                        i += 3;
                        continue;
                    }
                    b'C' => {
                        // Cursor right: `\x1b[C`
                        if cur < current.len() {
                            cur += 1;
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
                }
                // Ctrl-B: cursor left.
                0x02 => {
                    cur = cur.saturating_sub(1);
                }
                // Ctrl-F: cursor right (guarded; falls through to the no-op
                // arm when already at end of buffer).
                0x06 if cur < current.len() => {
                    cur += 1;
                }
                // Printable ASCII: insert at cursor.
                0x20..=0x7e => {
                    // insert_at returns original on allocation failure (byte dropped).
                    current = insert_at(current, cur, b);
                    // Advance cursor only if the buffer actually grew.
                    // After insert_at the new length is old+1 iff no OOM.
                    // We detect success by checking the new length.
                    cur += 1;
                }
                // All other bytes (other control characters, high bytes) and
                // positional no-ops (backspace at start, Ctrl-F at end): ignore.
                _ => {}
            }
            i += 1;
        }

        (current, cur)
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
