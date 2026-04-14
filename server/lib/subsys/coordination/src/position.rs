/// Fixed 8-byte position header.
///
/// Layout:
///   `[0..4]`  `domain_id: u32`  (assigned by server at driver enlistment)
///   `[4..6]`  `inner_id: u16`   (type variant within domain)
///   `[6..8]`  `flags: u16`      (domain-defined flags)
///
/// `domain_id=0` is reserved as sentinel (invalid/unassigned).
/// Domain IDs are assigned dynamically by the server when a driver enlists
/// via [`CoordinationRegistry::enlist_domain`](super::CoordinationRegistry::enlist_domain).
/// The driver provides a unique name; the server assigns the numeric ID.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct PositionHeader([u8; 8]);

impl PositionHeader {
    /// The sentinel domain ID (invalid/unassigned).
    pub const SENTINEL_DOMAIN_ID: u32 = 0;

    /// Creates a new position header from its components.
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

impl core::fmt::Debug for PositionHeader {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("PositionHeader")
            .field("domain_id", &self.domain_id())
            .field("inner_id", &self.inner_id())
            .field("flags", &self.flags())
            .finish()
    }
}

/// Domain-neutral position trait.
///
/// A position represents a location within a domain-specific buffer. The server
/// stores `Box<dyn Position>` without knowing what the coordinates mean — text
/// uses `(line, col)`, a 3D domain uses `(x, y, z)`, an image domain uses pixel
/// coordinates.
///
/// # Codec pattern
///
/// Every position carries a [`PositionHeader`] (8-byte discriminant) and opaque
/// content bytes. The server can:
/// - Compare positions via [`PartialEq`] (header + content byte equality)
/// - Encode for wire transport via [`encode`](Position::encode)
/// - Display for debugging via [`display`](Position::display)
/// - Clone via [`clone_box`](Position::clone_box)
///
/// Domain-specific operations (ordering, same-line checks) live on concrete types,
/// not on this trait.
pub trait Position: Send + Sync {
    /// Returns the 8-byte header identifying domain, type variant, and flags.
    fn header(&self) -> &PositionHeader;

    /// Returns the opaque content bytes (domain-specific encoding).
    ///
    /// The returned slice must be deterministic: calling `content()` twice on
    /// the same value must return identical bytes. Implementations should store
    /// the encoded bytes at construction time.
    fn content(&self) -> &[u8];

    /// Encodes the full position as header bytes followed by content bytes.
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

    /// Clone this position into a new boxed trait object.
    fn clone_box(&self) -> Box<dyn Position>;
}

impl PartialEq for dyn Position {
    fn eq(&self, other: &Self) -> bool {
        self.header() == other.header() && self.content() == other.content()
    }
}

impl Eq for dyn Position {}

impl core::fmt::Debug for dyn Position {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("dyn Position")
            .field("header", self.header())
            .field("content_len", &self.content().len())
            .field("display", &self.display())
            .finish()
    }
}

#[cfg(test)]
#[path = "position_tests.rs"]
mod tests;
