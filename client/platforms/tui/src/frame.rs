//! Pure ANSI frame composer.
//!
//! Converts a raw `projection` byte slice (the flat wire encoding from
//! `reovim-subsys-domain::projection::Projection::encode`) into an ANSI escape
//! sequence byte buffer ready to `write` to a terminal file descriptor.
//!
//! ## Projection wire layout (from reovim-subsys-domain)
//!
//! ```text
//! [0..4]   buffer_id (u32 LE)
//! [4..8]   window_id (u32 LE)
//! [8..12]  cursor_byte (u32 LE)
//! [12..16] span_end (u32 LE)
//! [16..]   buffer bytes (raw content)
//! ```
//!
//! ## ANSI frame format (DEV3/DEV5 golden baseline)
//!
//! ```text
//! ESC[2J      — erase display
//! ESC[H       — cursor home (row 1, col 1)
//! <content>   — the raw buffer bytes (newlines → \r\n under OPOST=off)
//! ESC[<row>;<col>H  — position cursor at (1-based row, 1-based col)
//! ```
//!
//! Cursor positioning: the skeleton models one screen line (no wrapping). The
//! column is `cursor_byte + 1` (1-based). Row is always 1 for the skeleton's
//! single-line buffer model.
//!
//! This is a pure function: no arch runtime allocation pattern required beyond
//! the output `Seq<u8>`.

use reovim_lib_ds::Seq;

/// Why composing an ANSI frame was refused.
///
/// ```rust
/// use reovim_platform_tui::frame::ComposeError;
///
/// assert_ne!(ComposeError::Truncated, ComposeError::Alloc);
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ComposeError {
    /// `projection_bytes` is shorter than the 16-byte header.
    Truncated,
    /// A backing allocation failed.
    Alloc,
}

/// Composes an ANSI frame from `projection_bytes` into a `Seq<u8>` suitable
/// for writing directly to a raw-`termios` terminal fd.
///
/// The output format is:
/// - `ESC[2J` (erase display) + `ESC[H` (cursor home).
/// - The raw buffer content bytes (printable portion up to `span_end`).
/// - `ESC[1;<col>H` to reposition the cursor at column `cursor_byte + 1`.
///
/// # Errors
///
/// Returns [`ComposeError::Truncated`] when `projection_bytes` is shorter than
/// 16 bytes, and [`ComposeError::Alloc`] when an allocation fails.
///
/// ```rust,no_run
/// // no_run: requires arch allocator runtime.
/// use reovim_platform_tui::frame::compose_ansi_frame;
///
/// // 16-byte header with empty content.
/// let proj = [0u8; 16];
/// let frame = compose_ansi_frame(&proj).unwrap();
/// assert!(!frame.is_empty());
/// ```
pub fn compose_ansi_frame(projection_bytes: &[u8]) -> Result<Seq<u8>, ComposeError> {
    if projection_bytes.len() < 16 {
        return Err(ComposeError::Truncated);
    }

    let le_u32 = |at: usize| -> u32 {
        let chunk: [u8; 4] = projection_bytes[at..at + 4].try_into().unwrap_or([0; 4]);
        u32::from_le_bytes(chunk)
    };

    let cursor_byte = le_u32(8) as usize;
    let span_end = le_u32(12) as usize;
    let content = &projection_bytes[16..];
    // Only render up to `span_end` bytes of content.
    let visible_len = span_end.min(content.len());
    let visible = &content[..visible_len];

    let mut out: Seq<u8> = Seq::new();

    // Erase display + cursor home.
    push_bytes(&mut out, b"\x1b[2J\x1b[H").map_err(|()| ComposeError::Alloc)?;

    // Write the visible buffer content.
    push_bytes(&mut out, visible).map_err(|()| ComposeError::Alloc)?;

    // Reposition the cursor (1-based column = cursor_byte + 1, row = 1).
    let col = cursor_byte.saturating_add(1);
    push_bytes(&mut out, b"\x1b[1;").map_err(|()| ComposeError::Alloc)?;
    push_decimal(&mut out, col).map_err(|()| ComposeError::Alloc)?;
    push_bytes(&mut out, b"H").map_err(|()| ComposeError::Alloc)?;

    Ok(out)
}

/// Pushes all bytes of `src` into `dst`. Returns `Err(())` on allocation failure.
fn push_bytes(dst: &mut Seq<u8>, src: &[u8]) -> Result<(), ()> {
    for &b in src {
        dst.try_push(b).map_err(|_| ())?;
    }
    Ok(())
}

/// Writes the decimal representation of `n` into `dst`.
/// Maximum value rendered is `usize::MAX`; a zero produces `"0"`.
pub(crate) fn push_decimal(dst: &mut Seq<u8>, n: usize) -> Result<(), ()> {
    if n == 0 {
        return dst.try_push(b'0').map_err(|_| ());
    }
    // Collect digits in reverse order then push in forward order.
    let mut digits = [0u8; 20]; // log10(2^64) < 20
    let mut len = 0;
    let mut remaining = n;
    while remaining > 0 {
        #[allow(clippy::cast_possible_truncation)]
        let digit = (remaining % 10) as u8;
        digits[len] = b'0' + digit;
        len += 1;
        remaining /= 10;
    }
    // Digits are in reverse order; push in reverse.
    for i in (0..len).rev() {
        dst.try_push(digits[i]).map_err(|_| ())?;
    }
    Ok(())
}

// L12 layout: tests in sibling frame_tests.rs, declared in lib.rs.
