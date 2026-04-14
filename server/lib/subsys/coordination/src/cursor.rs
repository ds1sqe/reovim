/// Fixed 8-byte cursor header.
///
/// Layout:
///   `[0..4]`  `domain_id: u32`  (assigned by server at driver enlistment)
///   `[4..6]`  `inner_id: u16`   (type variant within domain)
///   `[6..8]`  `flags: u16`      (domain-defined flags)
///
/// Same binary layout as [`PositionHeader`](super::PositionHeader) but a
/// separate Rust type for compile-time safety — you cannot accidentally pass
/// a cursor header where a position header is expected.
///
/// `domain_id=0` is reserved as sentinel (invalid/unassigned).
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct CursorHeader([u8; 8]);

impl CursorHeader {
    /// The sentinel domain ID (invalid/unassigned).
    pub const SENTINEL_DOMAIN_ID: u32 = 0;

    /// Creates a new cursor header from its components.
    ///
    /// All fields are stored in little-endian byte order.
    #[must_use]
    pub const fn new(domain_id: u32, inner_id: u16, flags: u16) -> Self {
        let d = domain_id.to_le_bytes();
        let i = inner_id.to_le_bytes();
        let f = flags.to_le_bytes();
        Self([d[0], d[1], d[2], d[3], i[0], i[1], f[0], f[1]])
    }

    /// Returns the domain ID (bytes 0..4, little-endian).
    #[must_use]
    pub const fn domain_id(&self) -> u32 {
        u32::from_le_bytes([self.0[0], self.0[1], self.0[2], self.0[3]])
    }

    /// Returns the inner type ID (bytes 4..6, little-endian).
    #[must_use]
    pub const fn inner_id(&self) -> u16 {
        u16::from_le_bytes([self.0[4], self.0[5]])
    }

    /// Returns the flags (bytes 6..8, little-endian).
    #[must_use]
    pub const fn flags(&self) -> u16 {
        u16::from_le_bytes([self.0[6], self.0[7]])
    }

    /// Returns the raw 8-byte representation.
    #[must_use]
    pub const fn as_bytes(&self) -> &[u8; 8] {
        &self.0
    }

    /// Returns true if both headers belong to the same domain.
    #[must_use]
    pub const fn same_domain(&self, other: &Self) -> bool {
        self.0[0] == other.0[0]
            && self.0[1] == other.0[1]
            && self.0[2] == other.0[2]
            && self.0[3] == other.0[3]
    }
}

impl core::fmt::Debug for CursorHeader {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("CursorHeader")
            .field("domain_id", &self.domain_id())
            .field("inner_id", &self.inner_id())
            .field("flags", &self.flags())
            .finish()
    }
}

/// Domain-neutral cursor trait.
///
/// A cursor represents the user's active position and selection state within a
/// domain-specific buffer. The server stores `Box<dyn Cursor>` without knowing
/// what the coordinates mean — text uses anchor/head with selection shape, a 3D
/// domain uses selected vertices/faces/edges.
///
/// Cursors are identity-free: they carry no `client_id`. The server wraps
/// cursors with client metadata at broadcast time for presence.
///
/// Multi-cursor is `Vec<Box<dyn Cursor>>` in Window — the subsys provides the
/// mechanism, module policy decides the cursor count (vim: 1, multi-cursor: N).
///
/// # Codec pattern
///
/// Same pattern as [`Position`](super::Position): every cursor carries a
/// [`CursorHeader`] (8-byte discriminant) and opaque content bytes.
pub trait Cursor: Send + Sync {
    /// Returns the 8-byte header identifying domain, type variant, and flags.
    fn header(&self) -> &CursorHeader;

    /// Returns the opaque content bytes (domain-specific encoding).
    ///
    /// The returned slice must be deterministic: calling `content()` twice on
    /// the same value must return identical bytes. Implementations should store
    /// the encoded bytes at construction time.
    fn content(&self) -> &[u8];

    /// Encodes the full cursor as header bytes followed by content bytes.
    ///
    /// Default implementation concatenates `header().as_bytes()` and `content()`.
    fn encode(&self) -> Vec<u8> {
        let content = self.content();
        let mut buf = Vec::with_capacity(8 + content.len());
        buf.extend_from_slice(self.header().as_bytes());
        buf.extend_from_slice(content);
        buf
    }

    /// Returns a human-readable representation for debugging/tracing.
    ///
    /// This is for display purposes only (logging, statusline), not a
    /// machine-parseable API contract.
    fn display(&self) -> String;

    /// Clone this cursor into a new boxed trait object.
    fn clone_box(&self) -> Box<dyn Cursor>;
}

impl PartialEq for dyn Cursor {
    fn eq(&self, other: &Self) -> bool {
        self.header() == other.header() && self.content() == other.content()
    }
}

impl Eq for dyn Cursor {}

impl core::fmt::Debug for dyn Cursor {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("dyn Cursor")
            .field("header", self.header())
            .field("content_len", &self.content().len())
            .field("display", &self.display())
            .finish()
    }
}

#[cfg(test)]
#[path = "cursor_tests.rs"]
mod tests;
