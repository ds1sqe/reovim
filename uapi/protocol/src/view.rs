//! Zero-copy borrowing views for variable-length decoded fields (SP17).
//!
//! Under the no-allocation rule (SP15) `decode` builds no owned collections.
//! Fixed-width fields decode by value; every variable-length field is a
//! validated *view* over the input buffer.  This module holds the list-view
//! types whose elements re-walk a borrowed, already-validated slice.
//!
//! **Validation timing (SP17):** the view types here are only ever
//! constructed by a message `decode` *after* it has validated the whole
//! field (count, bounds, UTF-8, discriminants).  Therefore iterating a view
//! returned from `decode` cannot fail — the iterators yield plain values, not
//! `Result`s.  The constructors are crate-internal; a caller can only obtain a
//! view through a successful `decode`.

use crate::codec::Decoder;

/// A borrowing iterator over a `list<str>` field (SP17).
///
/// Constructed only by a message `decode` over an already-validated slice, so
/// iteration is infallible.  Each item borrows the input buffer.
///
/// # Examples
///
/// ```rust
/// use reovim_uapi_protocol::view::StrList;
///
/// // count = 1, "ab"
/// let body = [
///     0x01, 0x00, 0x00, 0x00, // count
///     0x02, 0x00, 0x00, 0x00, b'a', b'b',
/// ];
/// let list = StrList::from_validated(&body);
/// assert_eq!(list.len(), 1);
/// assert_eq!(list.iter().next(), Some("ab"));
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StrList<'a> {
    body: &'a [u8],
    count: u32,
}

impl<'a> StrList<'a> {
    /// Wraps an already-validated `list<str>` field body (count prefix +
    /// `count` length-prefixed UTF-8 strings).  Crate-internal: callers reach
    /// this only through a successful `decode`.
    #[must_use]
    pub(crate) const fn from_parts(body: &'a [u8], count: u32) -> Self {
        Self { body, count }
    }

    /// Constructs from a full `count`-prefixed buffer, re-validating the
    /// structure.  Used by doc-tests and tests; panics on malformed input
    /// because the public path always pre-validates.
    ///
    /// ```rust
    /// use reovim_uapi_protocol::view::StrList;
    ///
    /// let body = [0x00, 0x00, 0x00, 0x00]; // empty list
    /// assert_eq!(StrList::from_validated(&body).len(), 0);
    /// ```
    ///
    /// # Panics
    ///
    /// Panics if `buf` was not pre-validated by `decode` (the SP17
    /// contract: only `decode` constructs views, after full structural
    /// validation) — unreachable through the public API.
    #[must_use]
    pub fn from_validated(buf: &'a [u8]) -> Self {
        let mut dec = Decoder::new(buf);
        let count = dec.get_u32().expect("validated count");
        let body = &buf[crate::codec::LEN_PREFIX..];
        Self { body, count }
    }

    /// Number of strings in the list.
    ///
    /// ```rust
    /// use reovim_uapi_protocol::view::StrList;
    ///
    /// let body = [0x00, 0x00, 0x00, 0x00];
    /// assert_eq!(StrList::from_validated(&body).len(), 0);
    /// ```
    #[must_use]
    pub const fn len(&self) -> usize {
        self.count as usize
    }

    /// Returns `true` when the list is empty.
    ///
    /// ```rust
    /// use reovim_uapi_protocol::view::StrList;
    ///
    /// let body = [0x00, 0x00, 0x00, 0x00];
    /// assert!(StrList::from_validated(&body).is_empty());
    /// ```
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.count == 0
    }

    /// Returns a fresh borrowing iterator over the strings.
    ///
    /// ```rust
    /// use reovim_uapi_protocol::view::StrList;
    ///
    /// let body = [
    ///     0x01, 0x00, 0x00, 0x00,
    ///     0x01, 0x00, 0x00, 0x00, b'x',
    /// ];
    /// let mut it = StrList::from_validated(&body).iter();
    /// assert_eq!(it.next(), Some("x"));
    /// assert_eq!(it.next(), None);
    /// ```
    #[must_use]
    pub const fn iter(&self) -> StrListIter<'a> {
        StrListIter {
            dec: Decoder::new(self.body),
            left: self.count,
        }
    }
}

impl<'a> IntoIterator for StrList<'a> {
    type Item = &'a str;
    type IntoIter = StrListIter<'a>;
    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

impl<'a> IntoIterator for &StrList<'a> {
    type Item = &'a str;
    type IntoIter = StrListIter<'a>;
    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

/// Infallible borrowing iterator over a validated `list<str>` (SP17).
pub struct StrListIter<'a> {
    dec: Decoder<'a>,
    left: u32,
}

impl<'a> Iterator for StrListIter<'a> {
    type Item = &'a str;

    fn next(&mut self) -> Option<&'a str> {
        if self.left == 0 {
            return None;
        }
        self.left -= 1;
        // SP17: the slice was validated at decode time, so this cannot fail.
        Some(self.dec.get_str().expect("validated str element"))
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        (self.left as usize, Some(self.left as usize))
    }
}

impl ExactSizeIterator for StrListIter<'_> {}

/// A decoded `RawInput` element (6.3 §7, 7.3 §6.1): a kind discriminant plus a
/// borrowed payload slice interpreted per kind.
///
/// The payload is the raw per-kind bytes (a `KeyEvent`/`MouseEvent`/… struct,
/// UTF-8 text, or an `ImeEvent` header + text) — opaque to the codec.
///
/// # Examples
///
/// ```rust
/// use reovim_uapi_protocol::view::RawInputRef;
/// use reovim_uapi_abi::input::RawInputKind;
///
/// let item = RawInputRef { kind: RawInputKind::Text, payload: b"hi" };
/// assert_eq!(item.kind, RawInputKind::Text);
/// assert_eq!(item.payload, b"hi");
/// ```
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RawInputRef<'a> {
    /// The input kind discriminant.
    pub kind: reovim_uapi_abi::input::RawInputKind,
    /// The per-kind payload, borrowed from the input buffer.
    pub payload: &'a [u8],
}

/// A borrowing view over a `list<RawInput>` field (SP17).
///
/// Each element encodes as `kind: u8` (the [`RawInputKind`] discriminant)
/// followed by `payload: bytes`.  Constructed only by a validated `decode`.
///
/// [`RawInputKind`]: reovim_uapi_abi::input::RawInputKind
///
/// # Examples
///
/// ```rust
/// use reovim_uapi_protocol::view::RawInputList;
///
/// let body = [0x00, 0x00, 0x00, 0x00]; // empty
/// assert!(RawInputList::from_validated(&body).is_empty());
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RawInputList<'a> {
    body: &'a [u8],
    count: u32,
}

impl<'a> RawInputList<'a> {
    #[must_use]
    pub(crate) const fn from_parts(body: &'a [u8], count: u32) -> Self {
        Self { body, count }
    }

    /// Constructs from a full `count`-prefixed buffer, re-validating structure.
    ///
    /// ```rust
    /// use reovim_uapi_protocol::view::RawInputList;
    ///
    /// let body = [
    ///     0x01, 0x00, 0x00, 0x00, // count = 1
    ///     0x03,                   // kind = Text
    ///     0x02, 0x00, 0x00, 0x00, b'h', b'i', // payload "hi"
    /// ];
    /// let list = RawInputList::from_validated(&body);
    /// let item = list.iter().next().unwrap();
    /// assert_eq!(item.payload, b"hi");
    /// ```
    ///
    /// # Panics
    ///
    /// Panics if `buf` was not pre-validated by `decode` (the SP17
    /// contract: only `decode` constructs views, after full structural
    /// validation) — unreachable through the public API.
    #[must_use]
    pub fn from_validated(buf: &'a [u8]) -> Self {
        let mut dec = Decoder::new(buf);
        let count = dec.get_u32().expect("validated count");
        Self {
            body: &buf[crate::codec::LEN_PREFIX..],
            count,
        }
    }

    /// Number of inputs in the list.
    #[must_use]
    pub const fn len(&self) -> usize {
        self.count as usize
    }

    /// Returns `true` when the list is empty.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.count == 0
    }

    /// Returns a fresh borrowing iterator.
    ///
    /// ```rust
    /// use reovim_uapi_protocol::view::RawInputList;
    ///
    /// let body = [0x00, 0x00, 0x00, 0x00];
    /// assert_eq!(RawInputList::from_validated(&body).iter().count(), 0);
    /// ```
    #[must_use]
    pub const fn iter(&self) -> RawInputIter<'a> {
        RawInputIter {
            dec: Decoder::new(self.body),
            left: self.count,
        }
    }
}

impl<'a> IntoIterator for RawInputList<'a> {
    type Item = RawInputRef<'a>;
    type IntoIter = RawInputIter<'a>;
    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

impl<'a> IntoIterator for &RawInputList<'a> {
    type Item = RawInputRef<'a>;
    type IntoIter = RawInputIter<'a>;
    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

/// Infallible borrowing iterator over a validated `list<RawInput>` (SP17).
pub struct RawInputIter<'a> {
    dec: Decoder<'a>,
    left: u32,
}

impl<'a> Iterator for RawInputIter<'a> {
    type Item = RawInputRef<'a>;

    fn next(&mut self) -> Option<RawInputRef<'a>> {
        if self.left == 0 {
            return None;
        }
        self.left -= 1;
        let kind = crate::messages::raw_input_kind_from_u8(
            self.dec.get_u8().expect("validated kind byte"),
        )
        .expect("validated kind discriminant");
        let payload = self.dec.get_bytes().expect("validated payload");
        Some(RawInputRef { kind, payload })
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        (self.left as usize, Some(self.left as usize))
    }
}

impl ExactSizeIterator for RawInputIter<'_> {}

/// A decoded `DomainEntry` element (7.3 §8): a borrowed name plus flags.
///
/// # Examples
///
/// ```rust
/// use reovim_uapi_protocol::view::DomainEntryRef;
///
/// let e = DomainEntryRef { domain_name: "text", inner_id: 1, has_display: true, has_semantic: false };
/// assert_eq!(e.domain_name, "text");
/// assert_eq!(e.inner_id, 1);
/// ```
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DomainEntryRef<'a> {
    /// Stable UTF-8 domain name (CR9).
    pub domain_name: &'a str,
    /// Inner numeric id within the domain.
    pub inner_id: u32,
    /// Whether the domain has a display projection.
    pub has_display: bool,
    /// Whether the domain has a semantic projection.
    pub has_semantic: bool,
}

/// A borrowing view over a `list<DomainEntry>` field (SP17, 7.3 §8).
///
/// Each element encodes as `domain_name: str`, `inner_id: u32`,
/// `has_display: bool`, `has_semantic: bool`.  Constructed only by a validated
/// `decode`.
///
/// # Examples
///
/// ```rust
/// use reovim_uapi_protocol::view::DomainEntryList;
///
/// let body = [0x00, 0x00, 0x00, 0x00]; // empty
/// assert!(DomainEntryList::from_validated(&body).is_empty());
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DomainEntryList<'a> {
    body: &'a [u8],
    count: u32,
}

impl<'a> DomainEntryList<'a> {
    #[must_use]
    pub(crate) const fn from_parts(body: &'a [u8], count: u32) -> Self {
        Self { body, count }
    }

    /// Constructs from a full `count`-prefixed buffer, re-validating structure.
    ///
    /// ```rust
    /// use reovim_uapi_protocol::view::DomainEntryList;
    ///
    /// let body = [
    ///     0x01, 0x00, 0x00, 0x00, // count = 1
    ///     0x01, 0x00, 0x00, 0x00, b't', // domain_name "t"
    ///     0x05, 0x00, 0x00, 0x00, // inner_id = 5
    ///     0x01, // has_display
    ///     0x00, // has_semantic
    /// ];
    /// let e = DomainEntryList::from_validated(&body).iter().next().unwrap();
    /// assert_eq!(e.domain_name, "t");
    /// assert_eq!(e.inner_id, 5);
    /// assert!(e.has_display);
    /// ```
    ///
    /// # Panics
    ///
    /// Panics if `buf` was not pre-validated by `decode` (the SP17
    /// contract: only `decode` constructs views, after full structural
    /// validation) — unreachable through the public API.
    #[must_use]
    pub fn from_validated(buf: &'a [u8]) -> Self {
        let mut dec = Decoder::new(buf);
        let count = dec.get_u32().expect("validated count");
        Self {
            body: &buf[crate::codec::LEN_PREFIX..],
            count,
        }
    }

    /// Number of entries.
    #[must_use]
    pub const fn len(&self) -> usize {
        self.count as usize
    }

    /// Returns `true` when the list is empty.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.count == 0
    }

    /// Returns a fresh borrowing iterator.
    #[must_use]
    pub const fn iter(&self) -> DomainEntryIter<'a> {
        DomainEntryIter {
            dec: Decoder::new(self.body),
            left: self.count,
        }
    }
}

impl<'a> IntoIterator for DomainEntryList<'a> {
    type Item = DomainEntryRef<'a>;
    type IntoIter = DomainEntryIter<'a>;
    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

impl<'a> IntoIterator for &DomainEntryList<'a> {
    type Item = DomainEntryRef<'a>;
    type IntoIter = DomainEntryIter<'a>;
    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

/// Infallible borrowing iterator over a validated `list<DomainEntry>` (SP17).
pub struct DomainEntryIter<'a> {
    dec: Decoder<'a>,
    left: u32,
}

impl<'a> Iterator for DomainEntryIter<'a> {
    type Item = DomainEntryRef<'a>;

    fn next(&mut self) -> Option<DomainEntryRef<'a>> {
        if self.left == 0 {
            return None;
        }
        self.left -= 1;
        let domain_name = self.dec.get_str().expect("validated domain_name");
        let inner_id = self.dec.get_u32().expect("validated inner_id");
        let has_display = self.dec.get_bool().expect("validated has_display");
        let has_semantic = self.dec.get_bool().expect("validated has_semantic");
        Some(DomainEntryRef {
            domain_name,
            inner_id,
            has_display,
            has_semantic,
        })
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        (self.left as usize, Some(self.left as usize))
    }
}

impl ExactSizeIterator for DomainEntryIter<'_> {}

/// A decoded `carrier` element (7.3 §6.2): an 8-byte header plus borrowed
/// content bytes.
///
/// # Examples
///
/// ```rust
/// use reovim_uapi_protocol::view::CarrierRef;
///
/// let c = CarrierRef { header: [0u8; 8], content: b"xy" };
/// assert_eq!(c.content, b"xy");
/// ```
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CarrierRef<'a> {
    /// The opaque 8-byte carrier header (`CursorHeader`/`PositionHeader`).
    pub header: [u8; 8],
    /// The carrier content bytes, borrowed from the input buffer.
    pub content: &'a [u8],
}

/// A borrowing view over a `list<carrier>` field (SP17, 7.3 §6.2).
///
/// Each element encodes as `header: [u8; 8]` raw, then `content: bytes`.
/// Constructed only by a validated `decode`.
///
/// # Examples
///
/// ```rust
/// use reovim_uapi_protocol::view::CarrierList;
///
/// let body = [0x00, 0x00, 0x00, 0x00]; // empty cursor set (CR3 absence-is-structural)
/// assert!(CarrierList::from_validated(&body).is_empty());
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CarrierList<'a> {
    body: &'a [u8],
    count: u32,
}

impl<'a> CarrierList<'a> {
    #[must_use]
    pub(crate) const fn from_parts(body: &'a [u8], count: u32) -> Self {
        Self { body, count }
    }

    /// Constructs from a full `count`-prefixed buffer, re-validating structure.
    ///
    /// ```rust
    /// use reovim_uapi_protocol::view::CarrierList;
    ///
    /// let body = [
    ///     0x01, 0x00, 0x00, 0x00, // count = 1
    ///     1, 2, 3, 4, 5, 6, 7, 8, // header
    ///     0x01, 0x00, 0x00, 0x00, 0xAA, // content
    /// ];
    /// let c = CarrierList::from_validated(&body).iter().next().unwrap();
    /// assert_eq!(c.header, [1, 2, 3, 4, 5, 6, 7, 8]);
    /// assert_eq!(c.content, &[0xAA]);
    /// ```
    ///
    /// # Panics
    ///
    /// Panics if `buf` was not pre-validated by `decode` (the SP17
    /// contract: only `decode` constructs views, after full structural
    /// validation) — unreachable through the public API.
    #[must_use]
    pub fn from_validated(buf: &'a [u8]) -> Self {
        let mut dec = Decoder::new(buf);
        let count = dec.get_u32().expect("validated count");
        Self {
            body: &buf[crate::codec::LEN_PREFIX..],
            count,
        }
    }

    /// Number of carriers.
    #[must_use]
    pub const fn len(&self) -> usize {
        self.count as usize
    }

    /// Returns `true` when the list is empty (an empty cursor set, CR3).
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.count == 0
    }

    /// Returns a fresh borrowing iterator.
    #[must_use]
    pub const fn iter(&self) -> CarrierIter<'a> {
        CarrierIter {
            dec: Decoder::new(self.body),
            left: self.count,
        }
    }
}

impl<'a> IntoIterator for CarrierList<'a> {
    type Item = CarrierRef<'a>;
    type IntoIter = CarrierIter<'a>;
    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

impl<'a> IntoIterator for &CarrierList<'a> {
    type Item = CarrierRef<'a>;
    type IntoIter = CarrierIter<'a>;
    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

/// Infallible borrowing iterator over a validated `list<carrier>` (SP17).
pub struct CarrierIter<'a> {
    dec: Decoder<'a>,
    left: u32,
}

impl<'a> Iterator for CarrierIter<'a> {
    type Item = CarrierRef<'a>;

    fn next(&mut self) -> Option<CarrierRef<'a>> {
        if self.left == 0 {
            return None;
        }
        self.left -= 1;
        let hdr = self.dec.get_u8_array8().expect("validated carrier header");
        let content = self.dec.get_bytes().expect("validated carrier content");
        Some(CarrierRef {
            header: hdr,
            content,
        })
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        (self.left as usize, Some(self.left as usize))
    }
}

impl ExactSizeIterator for CarrierIter<'_> {}
