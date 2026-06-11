//! `Bytes` and `Str` — owned byte/UTF-8 strings over the arch allocator.
//!
//! `Bytes` is a growable owned `[u8]` (the `Vec<u8>` analog); `Str` wraps it
//! with a maintained UTF-8 invariant (the `String` analog). Both are fallible
//! on growth, like every floor data structure.
//!
//! [`BytesWriter`] is the alloc-free, fallible [`core::fmt::Write`] seam the
//! panic handler (Phase 4) renders the LOG2 line through: formatting flows
//! into a `Bytes` and a failed push surfaces as `fmt::Error` rather than an
//! abort. Designing it here keeps the panic path's rendering target in the
//! floor from the start.

use core::{fmt, ops::Deref};

use crate::{alloc::AllocError, ds::seq::Seq};

/// A growable, heap-owning byte string.
///
/// ```rust
/// use reovim_arch::ds::Bytes;
///
/// let mut b = Bytes::new();
/// b.try_push(b'h').unwrap();
/// b.try_extend_from_slice(b"ello").unwrap();
/// assert_eq!(b.as_slice(), b"hello");
/// assert_eq!(b.len(), 5);
/// assert!(!b.is_empty());
/// ```
pub struct Bytes {
    inner: Seq<u8>,
}

impl Bytes {
    /// Creates an empty `Bytes` with no allocation.
    ///
    /// ```rust
    /// use reovim_arch::ds::Bytes;
    /// let b = Bytes::new();
    /// assert!(b.is_empty());
    /// ```
    #[must_use]
    pub const fn new() -> Self {
        Self { inner: Seq::new() }
    }

    /// Builds a `Bytes` containing a copy of `slice`.
    ///
    /// # Errors
    ///
    /// Returns [`AllocError`] when the backing allocation is refused.
    ///
    /// ```rust
    /// use reovim_arch::ds::Bytes;
    /// let b = Bytes::try_from_slice(b"hello").unwrap();
    /// assert_eq!(b.as_slice(), b"hello");
    /// ```
    pub fn try_from_slice(slice: &[u8]) -> Result<Self, AllocError> {
        let mut b = Self::new();
        b.try_extend_from_slice(slice)?;
        Ok(b)
    }

    /// Appends a single byte.
    ///
    /// # Errors
    ///
    /// Returns [`AllocError`] when a needed growth is refused.
    ///
    /// ```rust
    /// use reovim_arch::ds::Bytes;
    /// let mut b = Bytes::new();
    /// b.try_push(b'x').unwrap();
    /// assert_eq!(b.as_slice(), b"x");
    /// ```
    pub fn try_push(&mut self, byte: u8) -> Result<(), AllocError> {
        self.inner.try_push(byte)
    }

    /// Appends every byte of `slice`.
    ///
    /// # Errors
    ///
    /// Returns [`AllocError`] when a needed growth is refused. On error the
    /// bytes pushed before the failure remain (partial append), matching the
    /// fallible-builder contract.
    ///
    /// ```rust
    /// use reovim_arch::ds::Bytes;
    /// let mut b = Bytes::new();
    /// b.try_extend_from_slice(b"ab").unwrap();
    /// b.try_extend_from_slice(b"cd").unwrap();
    /// assert_eq!(b.as_slice(), b"abcd");
    /// ```
    pub fn try_extend_from_slice(&mut self, slice: &[u8]) -> Result<(), AllocError> {
        for &byte in slice {
            self.inner.try_push(byte)?;
        }
        Ok(())
    }

    /// The bytes as a slice.
    ///
    /// ```rust
    /// use reovim_arch::ds::Bytes;
    /// let b = Bytes::try_from_slice(b"hi").unwrap();
    /// assert_eq!(b.as_slice(), b"hi");
    /// ```
    #[must_use]
    pub const fn as_slice(&self) -> &[u8] {
        self.inner.as_slice()
    }

    /// The number of bytes.
    ///
    /// ```rust
    /// use reovim_arch::ds::Bytes;
    /// let b = Bytes::try_from_slice(b"abc").unwrap();
    /// assert_eq!(b.len(), 3);
    /// ```
    #[must_use]
    pub const fn len(&self) -> usize {
        self.inner.len()
    }

    /// Whether there are no bytes.
    ///
    /// ```rust
    /// use reovim_arch::ds::Bytes;
    /// assert!(Bytes::new().is_empty());
    /// assert!(!Bytes::try_from_slice(b"x").unwrap().is_empty());
    /// ```
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.inner.is_empty()
    }
}

impl Default for Bytes {
    fn default() -> Self {
        Self::new()
    }
}

impl Deref for Bytes {
    type Target = [u8];

    fn deref(&self) -> &[u8] {
        self.as_slice()
    }
}

/// An alloc-free, fallible [`core::fmt::Write`] adapter over a [`Bytes`].
///
/// Borrows a `Bytes` and routes `write_str`/`write_fmt` into it. A growth
/// failure surfaces as [`fmt::Error`] (the only error `fmt::Write` can
/// report), so the panic handler can render without aborting on OOM. The
/// adapter borrows rather than owns so the caller keeps the rendered `Bytes`
/// after formatting.
///
/// ```rust
/// use core::fmt::Write as _;
/// use reovim_arch::ds::{Bytes, BytesWriter};
///
/// let mut buf = Bytes::new();
/// let mut w = BytesWriter::new(&mut buf);
/// write!(w, "val={}", 42).unwrap();
/// assert_eq!(buf.as_slice(), b"val=42");
/// ```
pub struct BytesWriter<'a> {
    target: &'a mut Bytes,
}

impl<'a> BytesWriter<'a> {
    /// Wraps `target` for formatting into.
    ///
    /// ```rust
    /// use reovim_arch::ds::{Bytes, BytesWriter};
    /// let mut buf = Bytes::new();
    /// let _w = BytesWriter::new(&mut buf);
    /// ```
    pub const fn new(target: &'a mut Bytes) -> Self {
        Self { target }
    }
}

impl fmt::Write for BytesWriter<'_> {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        // A refused growth becomes `fmt::Error`: the panic path renders
        // best-effort rather than recursing into another panic.
        self.target
            .try_extend_from_slice(s.as_bytes())
            .map_err(|_| fmt::Error)
    }
}

/// An owned UTF-8 string maintained over a [`Bytes`].
///
/// Every mutator preserves the UTF-8 invariant: bytes only enter through
/// `&str` inputs, so [`as_str`](Str::as_str) can soundly skip re-validation.
///
/// ```rust
/// use reovim_arch::ds::Str;
///
/// let mut s = Str::try_from_str("hello").unwrap();
/// s.try_push_str(" world").unwrap();
/// assert_eq!(s.as_str(), "hello world");
/// assert_eq!(s.len(), 11);
/// assert!(!s.is_empty());
/// ```
pub struct Str {
    bytes: Bytes,
}

impl Str {
    /// Creates an empty `Str`.
    ///
    /// ```rust
    /// use reovim_arch::ds::Str;
    /// let s = Str::new();
    /// assert!(s.is_empty());
    /// ```
    #[must_use]
    pub const fn new() -> Self {
        Self {
            bytes: Bytes::new(),
        }
    }

    /// Builds a `Str` from `s`.
    ///
    /// # Errors
    ///
    /// Returns [`AllocError`] when the backing allocation is refused.
    ///
    /// ```rust
    /// use reovim_arch::ds::Str;
    /// let s = Str::try_from_str("ok").unwrap();
    /// assert_eq!(s.as_str(), "ok");
    /// ```
    pub fn try_from_str(s: &str) -> Result<Self, AllocError> {
        Ok(Self {
            bytes: Bytes::try_from_slice(s.as_bytes())?,
        })
    }

    /// Appends `s`, preserving the UTF-8 invariant.
    ///
    /// # Errors
    ///
    /// Returns [`AllocError`] when a needed growth is refused.
    ///
    /// ```rust
    /// use reovim_arch::ds::Str;
    /// let mut s = Str::new();
    /// s.try_push_str("hi").unwrap();
    /// s.try_push_str("!").unwrap();
    /// assert_eq!(s.as_str(), "hi!");
    /// ```
    pub fn try_push_str(&mut self, s: &str) -> Result<(), AllocError> {
        self.bytes.try_extend_from_slice(s.as_bytes())
    }

    /// The contents as a `&str`.
    ///
    /// ```rust
    /// use reovim_arch::ds::Str;
    /// let s = Str::try_from_str("abc").unwrap();
    /// assert_eq!(s.as_str(), "abc");
    /// ```
    #[must_use]
    pub const fn as_str(&self) -> &str {
        // SAFETY: every byte entered through a `&str` input
        // (`try_from_str`/`try_push_str`), so the buffer is valid UTF-8. The
        // invariant is maintained by construction, so `from_utf8_unchecked`
        // is sound and avoids re-validating on every read.
        unsafe { core::str::from_utf8_unchecked(self.bytes.as_slice()) }
    }

    /// The number of UTF-8 bytes.
    ///
    /// ```rust
    /// use reovim_arch::ds::Str;
    /// let s = Str::try_from_str("hi").unwrap();
    /// assert_eq!(s.len(), 2);
    /// ```
    #[must_use]
    pub const fn len(&self) -> usize {
        self.bytes.len()
    }

    /// Whether the string is empty.
    ///
    /// ```rust
    /// use reovim_arch::ds::Str;
    /// assert!(Str::new().is_empty());
    /// assert!(!Str::try_from_str("x").unwrap().is_empty());
    /// ```
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.bytes.is_empty()
    }
}

impl Default for Str {
    fn default() -> Self {
        Self::new()
    }
}

impl Deref for Str {
    type Target = str;

    fn deref(&self) -> &str {
        self.as_str()
    }
}

// L12 layout (#785 Phase 5): tests live in the sibling file `bytes_tests.rs`.
#[cfg(feature = "selftest")]
#[path = "bytes_tests.rs"]
mod tests;
