//! The deterministic byte codec (7.3 §6).
//!
//! One serialization world, identical to the deterministic byte encoding the
//! 6.3 `*Wire` catalog implies.  No schema compiler, no reflection, no
//! self-describing tags.  Every §7 field maps to exactly one primitive in the
//! §6 table.
//!
//! The codec is **sans-IO and no-alloc** (SP15): [`Encoder`] writes into a
//! caller-provided `&mut [u8]` and [`Decoder`] reads from a borrowed `&[u8]`.
//! Nothing is allocated; decoded variable-length fields are *views* into the
//! input buffer (SP17).
//!
//! Encoding is little-endian with no padding between fields.  The same value
//! always produces the same bytes (determinism, SP13).
//!
//! ## Error discipline
//!
//! - A short output buffer fails [`ErrorCode::BufferTooSmall`] with nothing
//!   written past the failure point; callers size by `encoded_size()` first
//!   (SP13), so a partial write is never observed by a conforming caller.
//! - A truncated input fails [`ErrorCode::ProtocolViolation`].
//! - A `bool` byte other than `0`/`1` fails [`ErrorCode::InvalidArgument`].
//! - A non-UTF-8 `str` fails [`ErrorCode::Utf8Invalid`].

use reovim_uapi_abi::ErrorCode;

/// Sequential little-endian writer over a caller-provided buffer (§6).
///
/// The encoder tracks a write cursor.  Each `put_*` method advances the
/// cursor by the primitive's wire width or fails [`ErrorCode::BufferTooSmall`]
/// when the remaining buffer is too short.
///
/// # Examples
///
/// ```rust
/// use reovim_uapi_protocol::codec::Encoder;
///
/// let mut buf = [0u8; 8];
/// let mut enc = Encoder::new(&mut buf);
/// enc.put_u16(0x0102).unwrap();
/// enc.put_u32(0x0A0B_0C0D).unwrap();
/// assert_eq!(enc.position(), 6);
/// assert_eq!(&buf[..6], &[0x02, 0x01, 0x0D, 0x0C, 0x0B, 0x0A]);
/// ```
pub struct Encoder<'a> {
    buf: &'a mut [u8],
    pos: usize,
}

impl<'a> Encoder<'a> {
    /// Creates an encoder over `buf`, writing from offset 0.
    ///
    /// ```rust
    /// use reovim_uapi_protocol::codec::Encoder;
    ///
    /// let mut buf = [0u8; 4];
    /// let enc = Encoder::new(&mut buf);
    /// assert_eq!(enc.position(), 0);
    /// ```
    pub const fn new(buf: &'a mut [u8]) -> Self {
        Self { buf, pos: 0 }
    }

    /// Returns the number of bytes written so far.
    ///
    /// ```rust
    /// use reovim_uapi_protocol::codec::Encoder;
    ///
    /// let mut buf = [0u8; 4];
    /// let mut enc = Encoder::new(&mut buf);
    /// enc.put_u8(7).unwrap();
    /// assert_eq!(enc.position(), 1);
    /// ```
    #[must_use]
    pub const fn position(&self) -> usize {
        self.pos
    }

    const fn reserve(&mut self, n: usize) -> Result<usize, ErrorCode> {
        let start = self.pos;
        // No overflow guard needed: `pos <= buf.len() <= isize::MAX` (a Rust
        // slice bound) and every caller's `n` is a fixed width or a slice
        // length, both `<= isize::MAX`; the sum cannot wrap a usize on the
        // only target (x86_64, 6.3 §1).
        let end = start + n;
        if end > self.buf.len() {
            return Err(ErrorCode::BufferTooSmall);
        }
        self.pos = end;
        Ok(start)
    }

    /// Writes a `u8`.
    ///
    /// ```rust
    /// use reovim_uapi_protocol::codec::Encoder;
    ///
    /// let mut buf = [0u8; 1];
    /// Encoder::new(&mut buf).put_u8(0xAB).unwrap();
    /// assert_eq!(buf, [0xAB]);
    /// ```
    ///
    /// # Errors
    ///
    /// Fails with [`ErrorCode::BufferTooSmall`] under the conditions described above.
    pub fn put_u8(&mut self, v: u8) -> Result<(), ErrorCode> {
        let at = self.reserve(1)?;
        self.buf[at] = v;
        Ok(())
    }

    /// Writes a `u16` little-endian.
    ///
    /// ```rust
    /// use reovim_uapi_protocol::codec::Encoder;
    ///
    /// let mut buf = [0u8; 2];
    /// Encoder::new(&mut buf).put_u16(0x0102).unwrap();
    /// assert_eq!(buf, [0x02, 0x01]);
    /// ```
    ///
    /// # Errors
    ///
    /// Fails with [`ErrorCode::BufferTooSmall`] under the conditions described above.
    pub fn put_u16(&mut self, v: u16) -> Result<(), ErrorCode> {
        let at = self.reserve(2)?;
        self.buf[at..at + 2].copy_from_slice(&v.to_le_bytes());
        Ok(())
    }

    /// Writes a `u32` little-endian.
    ///
    /// ```rust
    /// use reovim_uapi_protocol::codec::Encoder;
    ///
    /// let mut buf = [0u8; 4];
    /// Encoder::new(&mut buf).put_u32(0x0102_0304).unwrap();
    /// assert_eq!(buf, [0x04, 0x03, 0x02, 0x01]);
    /// ```
    ///
    /// # Errors
    ///
    /// Fails with [`ErrorCode::BufferTooSmall`] under the conditions described above.
    pub fn put_u32(&mut self, v: u32) -> Result<(), ErrorCode> {
        let at = self.reserve(4)?;
        self.buf[at..at + 4].copy_from_slice(&v.to_le_bytes());
        Ok(())
    }

    /// Writes a `u64` little-endian.
    ///
    /// ```rust
    /// use reovim_uapi_protocol::codec::Encoder;
    ///
    /// let mut buf = [0u8; 8];
    /// Encoder::new(&mut buf).put_u64(1).unwrap();
    /// assert_eq!(buf[0], 1);
    /// ```
    ///
    /// # Errors
    ///
    /// Fails with [`ErrorCode::BufferTooSmall`] under the conditions described above.
    pub fn put_u64(&mut self, v: u64) -> Result<(), ErrorCode> {
        let at = self.reserve(8)?;
        self.buf[at..at + 8].copy_from_slice(&v.to_le_bytes());
        Ok(())
    }

    /// Writes an `i32` little-endian (two's-complement).
    ///
    /// ```rust
    /// use reovim_uapi_protocol::codec::Encoder;
    ///
    /// let mut buf = [0u8; 4];
    /// Encoder::new(&mut buf).put_i32(-1).unwrap();
    /// assert_eq!(buf, [0xFF, 0xFF, 0xFF, 0xFF]);
    /// ```
    ///
    /// # Errors
    ///
    /// Fails with [`ErrorCode::BufferTooSmall`] under the conditions described above.
    pub fn put_i32(&mut self, v: i32) -> Result<(), ErrorCode> {
        self.put_u32(v.cast_unsigned())
    }

    /// Writes a `bool` as one byte (`0` or `1`).
    ///
    /// ```rust
    /// use reovim_uapi_protocol::codec::Encoder;
    ///
    /// let mut buf = [0u8; 1];
    /// Encoder::new(&mut buf).put_bool(true).unwrap();
    /// assert_eq!(buf, [1]);
    /// ```
    ///
    /// # Errors
    ///
    /// Fails with [`ErrorCode::BufferTooSmall`] under the conditions described above.
    pub fn put_bool(&mut self, v: bool) -> Result<(), ErrorCode> {
        self.put_u8(u8::from(v))
    }

    /// Writes a length-prefixed byte sequence (`len: u32`, then bytes).
    ///
    /// ```rust
    /// use reovim_uapi_protocol::codec::Encoder;
    ///
    /// let mut buf = [0u8; 6];
    /// Encoder::new(&mut buf).put_bytes(&[0xAA, 0xBB]).unwrap();
    /// assert_eq!(buf, [0x02, 0, 0, 0, 0xAA, 0xBB]);
    /// ```
    ///
    /// # Errors
    ///
    /// Fails with [`ErrorCode::ResourceExhausted`] under the conditions described above.
    pub fn put_bytes(&mut self, v: &[u8]) -> Result<(), ErrorCode> {
        let len = u32::try_from(v.len()).map_err(|_| ErrorCode::ResourceExhausted)?;
        self.put_u32(len)?;
        let at = self.reserve(v.len())?;
        self.buf[at..at + v.len()].copy_from_slice(v);
        Ok(())
    }

    /// Writes a length-prefixed UTF-8 string (same wire form as `bytes`).
    ///
    /// ```rust
    /// use reovim_uapi_protocol::codec::Encoder;
    ///
    /// let mut buf = [0u8; 6];
    /// Encoder::new(&mut buf).put_str("hi").unwrap();
    /// assert_eq!(&buf, &[0x02, 0, 0, 0, b'h', b'i']);
    /// ```
    ///
    /// # Errors
    ///
    /// Fails with [`ErrorCode::BufferTooSmall`] under the conditions described above.
    pub fn put_str(&mut self, v: &str) -> Result<(), ErrorCode> {
        self.put_bytes(v.as_bytes())
    }

    /// Writes a raw 8-byte array (no length prefix), used by the `carrier`
    /// primitive's header (§6).
    ///
    /// ```rust
    /// use reovim_uapi_protocol::codec::Encoder;
    ///
    /// let mut buf = [0u8; 8];
    /// Encoder::new(&mut buf).put_u8_array8(&[1, 2, 3, 4, 5, 6, 7, 8]).unwrap();
    /// assert_eq!(buf, [1, 2, 3, 4, 5, 6, 7, 8]);
    /// ```
    ///
    /// # Errors
    ///
    /// Fails with [`ErrorCode::BufferTooSmall`] under the conditions described above.
    pub fn put_u8_array8(&mut self, v: &[u8; 8]) -> Result<(), ErrorCode> {
        let at = self.reserve(8)?;
        self.buf[at..at + 8].copy_from_slice(v);
        Ok(())
    }
}

/// Wire width of the `bytes`/`str`/`list` count prefix, in bytes.
pub const LEN_PREFIX: usize = 4;

/// Returns the encoded size of a length-prefixed byte sequence (`bytes`/`str`).
///
/// ```rust
/// use reovim_uapi_protocol::codec::bytes_size;
///
/// assert_eq!(bytes_size(3), 7); // 4-byte prefix + 3 bytes
/// ```
#[must_use]
pub const fn bytes_size(payload_len: usize) -> usize {
    LEN_PREFIX + payload_len
}

/// Sequential little-endian reader over a borrowed input buffer (§6, SP17).
///
/// The decoder tracks a read cursor and validates bounds on every read.  All
/// `get_*` methods that return borrowed slices yield views into the original
/// `&'a [u8]`, never owned copies (no allocation).  A read past the end of the
/// buffer fails [`ErrorCode::ProtocolViolation`].
///
/// # Examples
///
/// ```rust
/// use reovim_uapi_protocol::codec::Decoder;
///
/// let bytes = [0x02, 0x01, 0xAA];
/// let mut dec = Decoder::new(&bytes);
/// assert_eq!(dec.get_u16().unwrap(), 0x0102);
/// assert_eq!(dec.get_u8().unwrap(), 0xAA);
/// assert!(dec.is_empty());
/// ```
pub struct Decoder<'a> {
    buf: &'a [u8],
    pos: usize,
}

impl<'a> Decoder<'a> {
    /// Creates a decoder over `buf`, reading from offset 0.
    ///
    /// ```rust
    /// use reovim_uapi_protocol::codec::Decoder;
    ///
    /// let bytes = [1u8, 2, 3];
    /// let dec = Decoder::new(&bytes);
    /// assert_eq!(dec.remaining(), 3);
    /// ```
    #[must_use]
    pub const fn new(buf: &'a [u8]) -> Self {
        Self { buf, pos: 0 }
    }

    /// Returns the number of unread bytes.
    ///
    /// ```rust
    /// use reovim_uapi_protocol::codec::Decoder;
    ///
    /// let bytes = [1u8, 2];
    /// let mut dec = Decoder::new(&bytes);
    /// dec.get_u8().unwrap();
    /// assert_eq!(dec.remaining(), 1);
    /// ```
    #[must_use]
    pub const fn remaining(&self) -> usize {
        self.buf.len() - self.pos
    }

    /// Returns `true` when all bytes have been read.
    ///
    /// ```rust
    /// use reovim_uapi_protocol::codec::Decoder;
    ///
    /// let bytes = [1u8];
    /// let mut dec = Decoder::new(&bytes);
    /// dec.get_u8().unwrap();
    /// assert!(dec.is_empty());
    /// ```
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.pos == self.buf.len()
    }

    fn take(&mut self, n: usize) -> Result<&'a [u8], ErrorCode> {
        // No overflow guard needed: `pos <= buf.len() <= isize::MAX` and `n`
        // is a fixed width, a decoded `u32` length, or a span `<= buf.len()`;
        // the sum cannot wrap a usize on the only target (x86_64, 6.3 §1).
        let end = self.pos + n;
        if end > self.buf.len() {
            return Err(ErrorCode::ProtocolViolation);
        }
        let slice = &self.buf[self.pos..end];
        self.pos = end;
        Ok(slice)
    }

    /// Returns the still-unread tail of the input as a borrowed slice, without
    /// advancing the cursor.
    ///
    /// Used by message decoders to validate a variable-length field with a
    /// throwaway probe decoder before consuming it (SP17: validate first).
    ///
    /// ```rust
    /// use reovim_uapi_protocol::codec::Decoder;
    ///
    /// let bytes = [1u8, 2, 3];
    /// let mut dec = Decoder::new(&bytes);
    /// dec.get_u8().unwrap();
    /// assert_eq!(dec.remaining_slice(), &[2, 3]);
    /// ```
    #[must_use]
    pub const fn remaining_slice(&self) -> &'a [u8] {
        // `&'a [u8]` is `Copy`; copying the field reference out first keeps the
        // returned slice bound to `'a`, not to the borrow of `self`.
        let buf: &'a [u8] = self.buf;
        let (_, tail) = buf.split_at(self.pos);
        tail
    }

    /// Advances the cursor by exactly `n` bytes, returning the borrowed run; a
    /// short input fails [`ErrorCode::ProtocolViolation`].
    ///
    /// ```rust
    /// use reovim_uapi_protocol::codec::Decoder;
    ///
    /// let bytes = [1u8, 2, 3, 4];
    /// let mut dec = Decoder::new(&bytes);
    /// assert_eq!(dec.take_exact(2).unwrap(), &[1, 2]);
    /// assert_eq!(dec.remaining(), 2);
    /// ```
    ///
    /// # Errors
    ///
    /// Fails with [`ErrorCode::ProtocolViolation`] under the conditions described above.
    pub fn take_exact(&mut self, n: usize) -> Result<&'a [u8], ErrorCode> {
        self.take(n)
    }

    /// Reads a `u8`.
    ///
    /// ```rust
    /// use reovim_uapi_protocol::codec::Decoder;
    ///
    /// let bytes = [0xAB];
    /// assert_eq!(Decoder::new(&bytes).get_u8().unwrap(), 0xAB);
    /// ```
    ///
    /// # Errors
    ///
    /// Fails with [`ErrorCode::ProtocolViolation`] under the conditions described above.
    pub fn get_u8(&mut self) -> Result<u8, ErrorCode> {
        Ok(self.take(1)?[0])
    }

    /// Reads a `u16` little-endian.
    ///
    /// ```rust
    /// use reovim_uapi_protocol::codec::Decoder;
    ///
    /// let bytes = [0x02, 0x01];
    /// assert_eq!(Decoder::new(&bytes).get_u16().unwrap(), 0x0102);
    /// ```
    ///
    /// # Errors
    ///
    /// Fails with [`ErrorCode::ProtocolViolation`] under the conditions described above.
    pub fn get_u16(&mut self) -> Result<u16, ErrorCode> {
        let s = self.take(2)?;
        Ok(u16::from_le_bytes([s[0], s[1]]))
    }

    /// Reads a `u32` little-endian.
    ///
    /// ```rust
    /// use reovim_uapi_protocol::codec::Decoder;
    ///
    /// let bytes = [0x04, 0x03, 0x02, 0x01];
    /// assert_eq!(Decoder::new(&bytes).get_u32().unwrap(), 0x0102_0304);
    /// ```
    ///
    /// # Errors
    ///
    /// Fails with [`ErrorCode::ProtocolViolation`] under the conditions described above.
    pub fn get_u32(&mut self) -> Result<u32, ErrorCode> {
        let s = self.take(4)?;
        Ok(u32::from_le_bytes([s[0], s[1], s[2], s[3]]))
    }

    /// Reads a `u64` little-endian.
    ///
    /// ```rust
    /// use reovim_uapi_protocol::codec::Decoder;
    ///
    /// let bytes = [1u8, 0, 0, 0, 0, 0, 0, 0];
    /// assert_eq!(Decoder::new(&bytes).get_u64().unwrap(), 1);
    /// ```
    ///
    /// # Errors
    ///
    /// Fails with [`ErrorCode::ProtocolViolation`] under the conditions described above.
    pub fn get_u64(&mut self) -> Result<u64, ErrorCode> {
        let s = self.take(8)?;
        Ok(u64::from_le_bytes([s[0], s[1], s[2], s[3], s[4], s[5], s[6], s[7]]))
    }

    /// Reads an `i32` little-endian (two's-complement).
    ///
    /// ```rust
    /// use reovim_uapi_protocol::codec::Decoder;
    ///
    /// let bytes = [0xFF, 0xFF, 0xFF, 0xFF];
    /// assert_eq!(Decoder::new(&bytes).get_i32().unwrap(), -1);
    /// ```
    ///
    /// # Errors
    ///
    /// Fails with [`ErrorCode::ProtocolViolation`] under the conditions described above.
    pub fn get_i32(&mut self) -> Result<i32, ErrorCode> {
        Ok(self.get_u32()?.cast_signed())
    }

    /// Reads a `bool`; a byte other than `0`/`1` fails
    /// [`ErrorCode::InvalidArgument`].
    ///
    /// ```rust
    /// use reovim_uapi_protocol::codec::Decoder;
    /// use reovim_uapi_abi::ErrorCode;
    ///
    /// assert_eq!(Decoder::new(&[1u8]).get_bool().unwrap(), true);
    /// assert_eq!(Decoder::new(&[2u8]).get_bool(), Err(ErrorCode::InvalidArgument));
    /// ```
    ///
    /// # Errors
    ///
    /// Fails with [`ErrorCode::InvalidArgument`] under the conditions described above.
    pub fn get_bool(&mut self) -> Result<bool, ErrorCode> {
        match self.get_u8()? {
            0 => Ok(false),
            1 => Ok(true),
            _ => Err(ErrorCode::InvalidArgument),
        }
    }

    /// Reads a length-prefixed byte sequence and returns a borrowed view.
    ///
    /// ```rust
    /// use reovim_uapi_protocol::codec::Decoder;
    ///
    /// let bytes = [0x02, 0, 0, 0, 0xAA, 0xBB];
    /// assert_eq!(Decoder::new(&bytes).get_bytes().unwrap(), &[0xAA, 0xBB]);
    /// ```
    ///
    /// # Errors
    ///
    /// Fails with [`ErrorCode::ProtocolViolation`] under the conditions described above.
    pub fn get_bytes(&mut self) -> Result<&'a [u8], ErrorCode> {
        let len = self.get_u32()? as usize;
        self.take(len)
    }

    /// Reads a length-prefixed UTF-8 string and returns a borrowed view;
    /// non-UTF-8 content fails [`ErrorCode::Utf8Invalid`].
    ///
    /// ```rust
    /// use reovim_uapi_protocol::codec::Decoder;
    /// use reovim_uapi_abi::ErrorCode;
    ///
    /// let ok = [0x02, 0, 0, 0, b'h', b'i'];
    /// assert_eq!(Decoder::new(&ok).get_str().unwrap(), "hi");
    /// let bad = [0x01, 0, 0, 0, 0xFF];
    /// assert_eq!(Decoder::new(&bad).get_str(), Err(ErrorCode::Utf8Invalid));
    /// ```
    ///
    /// # Errors
    ///
    /// Fails with [`ErrorCode::Utf8Invalid`] under the conditions described above.
    pub fn get_str(&mut self) -> Result<&'a str, ErrorCode> {
        let raw = self.get_bytes()?;
        core::str::from_utf8(raw).map_err(|_| ErrorCode::Utf8Invalid)
    }

    /// Reads a raw 8-byte array (no length prefix), used by the `carrier`
    /// header (§6).
    ///
    /// ```rust
    /// use reovim_uapi_protocol::codec::Decoder;
    ///
    /// let bytes = [1u8, 2, 3, 4, 5, 6, 7, 8];
    /// assert_eq!(Decoder::new(&bytes).get_u8_array8().unwrap(), [1, 2, 3, 4, 5, 6, 7, 8]);
    /// ```
    ///
    /// # Errors
    ///
    /// Fails with [`ErrorCode::ProtocolViolation`] under the conditions described above.
    pub fn get_u8_array8(&mut self) -> Result<[u8; 8], ErrorCode> {
        let s = self.take(8)?;
        Ok([s[0], s[1], s[2], s[3], s[4], s[5], s[6], s[7]])
    }
}
