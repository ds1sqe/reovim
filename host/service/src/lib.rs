#![no_std]
#![forbid(unsafe_code)]
//! Bounded line framing for the experimental local command service.

extern crate alloc;
#[cfg(feature = "std")]
extern crate std;

pub mod request;

use alloc::{string::String, vec::Vec};

pub const MAX_LINE: usize = 65_536;

/// ```
/// assert_eq!(reovim_service::hex(&[0, 255]), "00ff");
/// ```
#[must_use]
pub fn hex(bytes: &[u8]) -> String {
    const DIGITS: &[u8] = b"0123456789abcdef";
    bytes
        .iter()
        .flat_map(|byte| {
            [
                char::from(DIGITS[usize::from(byte >> 4)]),
                char::from(DIGITS[usize::from(byte & 15)]),
            ]
        })
        .collect()
}

/// ```
/// assert_eq!(reovim_service::unhex("00ff"), Some(vec![0, 255]));
/// assert_eq!(reovim_service::unhex("xyz"), None);
/// ```
#[must_use]
pub fn unhex(value: &str) -> Option<Vec<u8>> {
    if !value.len().is_multiple_of(2) || value.len() > MAX_LINE {
        return None;
    }
    value
        .as_bytes()
        .as_chunks::<2>()
        .0
        .iter()
        .map(|pair| {
            let high = char::from(pair[0]).to_digit(16)?;
            let low = char::from(pair[1]).to_digit(16)?;
            u8::try_from(high * 16 + low).ok()
        })
        .collect()
}

#[cfg(feature = "std")]
pub mod transport {
    use super::MAX_LINE;
    use alloc::{string::String, vec::Vec};
    use std::io::{self, BufRead, Read, Write};

    /// Reads the final response while the peer reports a pending operation.
    /// # Errors
    /// Rejects invalid frames, disconnected peers and underlying I/O errors.
    pub fn response(reader: &mut impl BufRead) -> io::Result<String> {
        let mut frame = receive(reader)?;
        while frame == "pending" {
            frame = receive(reader)?;
        }
        Ok(frame)
    }

    /// ```
    /// let mut input = std::io::Cursor::new(b"v1\tread\n");
    /// assert_eq!(reovim_service::transport::receive(&mut input)?, "v1\tread");
    /// # Ok::<(), std::io::Error>(())
    /// ```
    /// # Errors
    /// Rejects incomplete, oversized or non-UTF-8 frames.
    pub fn receive(reader: &mut impl BufRead) -> io::Result<String> {
        let mut bytes = Vec::new();
        reader.take(MAX_LINE as u64 + 1).read_until(b'\n', &mut bytes)?;
        if bytes.len() > MAX_LINE || bytes.pop() != Some(b'\n') {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "incomplete or oversized frame",
            ));
        }
        String::from_utf8(bytes).map_err(|_| {
            io::Error::new(io::ErrorKind::InvalidData, "invalid UTF-8 frame")
        })
    }

    /// ```
    /// let mut output = Vec::new();
    /// reovim_service::transport::send(&mut output, "ok")?;
    /// assert_eq!(output, b"ok\n");
    /// # Ok::<(), std::io::Error>(())
    /// ```
    /// # Errors
    /// Rejects embedded newlines, oversized frames and transport failures.
    pub fn send(writer: &mut impl Write, value: &str) -> io::Result<()> {
        if value.len() >= MAX_LINE || value.contains(['\n', '\r']) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "invalid frame",
            ));
        }
        writer.write_all(value.as_bytes())?;
        writer.write_all(b"\n")?;
        writer.flush()
    }
}
