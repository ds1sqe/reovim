//! Coordination carriers shared by Domain substrate code (§4.5, #798).
//!
//! A carrier is `header(8) + content(N)`. The header is a little-endian
//! `(domain_id: u32, inner_id: u16, flags: u16)` tuple. Position and cursor
//! carriers use distinct header types to prevent accidental cross-use.

use reovim_arch::ds::Bytes;

use crate::id::DomainId;

/// Bit reserved for kernel annotation in carrier flags (§4.5 CR2).
///
/// ```rust
/// use reovim_subsys_domain::carrier::KERNEL_FLAG;
///
/// assert_eq!(KERNEL_FLAG, 0x8000);
/// ```
pub const KERNEL_FLAG: u16 = 0x8000;

/// Header for a position carrier (§4.5 CR1/CR2).
///
/// ```rust
/// use core::num::NonZeroU32;
/// use reovim_subsys_domain::{carrier::PositionHeader, id::DomainId};
///
/// let h = PositionHeader::new(DomainId::new(NonZeroU32::new(1).unwrap()), 7, 0);
/// assert_eq!(h.domain_raw(), 1);
/// assert_eq!(h.inner_id(), 7);
/// ```
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PositionHeader {
    /// Raw little-endian header bytes.
    pub bytes: [u8; 8],
}

/// Header for a cursor carrier (§4.5 CR1/CR2).
///
/// ```rust
/// use core::num::NonZeroU32;
/// use reovim_subsys_domain::{carrier::CursorHeader, id::DomainId};
///
/// let h = CursorHeader::new(DomainId::new(NonZeroU32::new(2).unwrap()), 3, 1);
/// assert_eq!(h.domain_raw(), 2);
/// assert_eq!(h.flags(), 1);
/// ```
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CursorHeader {
    /// Raw little-endian header bytes.
    pub bytes: [u8; 8],
}

impl PositionHeader {
    /// Builds a position header from a typed Domain id, codec id, and flags.
    ///
    /// ```rust
    /// use core::num::NonZeroU32;
    /// use reovim_subsys_domain::{carrier::PositionHeader, id::DomainId};
    ///
    /// let h = PositionHeader::new(DomainId::new(NonZeroU32::new(4).unwrap()), 9, 2);
    /// assert_eq!(h.inner_id(), 9);
    /// ```
    #[must_use]
    pub const fn new(domain_id: DomainId, inner_id: u16, flags: u16) -> Self {
        Self::from_raw_parts(domain_id.as_u32(), inner_id, flags)
    }

    /// Builds a position header from raw pieces, including invalid raw ids for tests.
    ///
    /// ```rust
    /// use reovim_subsys_domain::carrier::PositionHeader;
    ///
    /// let h = PositionHeader::from_raw_parts(0, 1, 0);
    /// assert!(!h.valid_non_empty_domain());
    /// ```
    #[must_use]
    pub const fn from_raw_parts(domain_id: u32, inner_id: u16, flags: u16) -> Self {
        let d = domain_id.to_le_bytes();
        let i = inner_id.to_le_bytes();
        let f = flags.to_le_bytes();
        Self {
            bytes: [d[0], d[1], d[2], d[3], i[0], i[1], f[0], f[1]],
        }
    }

    /// Returns the raw domain id.
    ///
    /// ```rust
    /// use reovim_subsys_domain::carrier::PositionHeader;
    ///
    /// assert_eq!(PositionHeader::from_raw_parts(5, 1, 0).domain_raw(), 5);
    /// ```
    #[must_use]
    pub const fn domain_raw(self) -> u32 {
        u32::from_le_bytes([self.bytes[0], self.bytes[1], self.bytes[2], self.bytes[3]])
    }

    /// Returns the codec-local inner id.
    ///
    /// ```rust
    /// use reovim_subsys_domain::carrier::PositionHeader;
    ///
    /// assert_eq!(PositionHeader::from_raw_parts(5, 6, 0).inner_id(), 6);
    /// ```
    #[must_use]
    pub const fn inner_id(self) -> u16 {
        u16::from_le_bytes([self.bytes[4], self.bytes[5]])
    }

    /// Returns the raw flags field.
    ///
    /// ```rust
    /// use reovim_subsys_domain::carrier::PositionHeader;
    ///
    /// assert_eq!(PositionHeader::from_raw_parts(5, 6, 7).flags(), 7);
    /// ```
    #[must_use]
    pub const fn flags(self) -> u16 {
        u16::from_le_bytes([self.bytes[6], self.bytes[7]])
    }

    /// Returns true when a non-empty carrier can use this header.
    ///
    /// ```rust
    /// use reovim_subsys_domain::carrier::PositionHeader;
    ///
    /// assert!(PositionHeader::from_raw_parts(1, 0, 0).valid_non_empty_domain());
    /// assert!(!PositionHeader::from_raw_parts(0, 0, 0).valid_non_empty_domain());
    /// ```
    #[must_use]
    pub const fn valid_non_empty_domain(self) -> bool {
        self.domain_raw() != 0
    }

    /// Returns true when the kernel-reserved flag bit is set.
    ///
    /// ```rust
    /// use reovim_subsys_domain::carrier::{PositionHeader, KERNEL_FLAG};
    ///
    /// assert!(PositionHeader::from_raw_parts(1, 0, KERNEL_FLAG).has_kernel_flag());
    /// ```
    #[must_use]
    pub const fn has_kernel_flag(self) -> bool {
        self.flags() & KERNEL_FLAG != 0
    }
}

impl CursorHeader {
    /// Builds a cursor header from a typed Domain id, codec id, and flags.
    ///
    /// ```rust
    /// use core::num::NonZeroU32;
    /// use reovim_subsys_domain::{carrier::CursorHeader, id::DomainId};
    ///
    /// let h = CursorHeader::new(DomainId::new(NonZeroU32::new(4).unwrap()), 9, 2);
    /// assert_eq!(h.inner_id(), 9);
    /// ```
    #[must_use]
    pub const fn new(domain_id: DomainId, inner_id: u16, flags: u16) -> Self {
        Self::from_raw_parts(domain_id.as_u32(), inner_id, flags)
    }

    /// Builds a cursor header from raw pieces, including invalid raw ids for tests.
    ///
    /// ```rust
    /// use reovim_subsys_domain::carrier::CursorHeader;
    ///
    /// let h = CursorHeader::from_raw_parts(0, 1, 0);
    /// assert!(!h.valid_non_empty_domain());
    /// ```
    #[must_use]
    pub const fn from_raw_parts(domain_id: u32, inner_id: u16, flags: u16) -> Self {
        let d = domain_id.to_le_bytes();
        let i = inner_id.to_le_bytes();
        let f = flags.to_le_bytes();
        Self {
            bytes: [d[0], d[1], d[2], d[3], i[0], i[1], f[0], f[1]],
        }
    }

    /// Returns the raw domain id.
    ///
    /// ```rust
    /// use reovim_subsys_domain::carrier::CursorHeader;
    ///
    /// assert_eq!(CursorHeader::from_raw_parts(5, 1, 0).domain_raw(), 5);
    /// ```
    #[must_use]
    pub const fn domain_raw(self) -> u32 {
        u32::from_le_bytes([self.bytes[0], self.bytes[1], self.bytes[2], self.bytes[3]])
    }

    /// Returns the codec-local inner id.
    ///
    /// ```rust
    /// use reovim_subsys_domain::carrier::CursorHeader;
    ///
    /// assert_eq!(CursorHeader::from_raw_parts(5, 6, 0).inner_id(), 6);
    /// ```
    #[must_use]
    pub const fn inner_id(self) -> u16 {
        u16::from_le_bytes([self.bytes[4], self.bytes[5]])
    }

    /// Returns the raw flags field.
    ///
    /// ```rust
    /// use reovim_subsys_domain::carrier::CursorHeader;
    ///
    /// assert_eq!(CursorHeader::from_raw_parts(5, 6, 7).flags(), 7);
    /// ```
    #[must_use]
    pub const fn flags(self) -> u16 {
        u16::from_le_bytes([self.bytes[6], self.bytes[7]])
    }

    /// Returns true when a non-empty carrier can use this header.
    ///
    /// ```rust
    /// use reovim_subsys_domain::carrier::CursorHeader;
    ///
    /// assert!(CursorHeader::from_raw_parts(1, 0, 0).valid_non_empty_domain());
    /// assert!(!CursorHeader::from_raw_parts(0, 0, 0).valid_non_empty_domain());
    /// ```
    #[must_use]
    pub const fn valid_non_empty_domain(self) -> bool {
        self.domain_raw() != 0
    }

    /// Returns true when the kernel-reserved flag bit is set.
    ///
    /// ```rust
    /// use reovim_subsys_domain::carrier::{CursorHeader, KERNEL_FLAG};
    ///
    /// assert!(CursorHeader::from_raw_parts(1, 0, KERNEL_FLAG).has_kernel_flag());
    /// ```
    #[must_use]
    pub const fn has_kernel_flag(self) -> bool {
        self.flags() & KERNEL_FLAG != 0
    }
}

/// Position carrier with owned content bytes (§4.5 CR1).
///
/// ```rust,no_run
/// // no_run: requires arch allocator runtime.
/// ```
pub struct PositionCarrier {
    /// Header identifying Domain, codec, and flags.
    pub header: PositionHeader,
    /// Codec-defined content bytes.
    pub content: Bytes,
}

/// Cursor carrier with owned content bytes (§4.5 CR1).
///
/// ```rust,no_run
/// // no_run: requires arch allocator runtime.
/// ```
pub struct CursorCarrier {
    /// Header identifying Domain, codec, and flags.
    pub header: CursorHeader,
    /// Codec-defined content bytes.
    pub content: Bytes,
}

impl PositionCarrier {
    /// Constructs a position carrier from header and content.
    ///
    /// ```rust,no_run
    /// // no_run: requires arch allocator runtime.
    /// ```
    #[must_use]
    pub const fn new(header: PositionHeader, content: Bytes) -> Self {
        Self { header, content }
    }

    /// Returns this carrier's structural status without consulting a codec.
    ///
    /// ```rust,no_run
    /// // no_run: requires arch allocator runtime.
    /// ```
    #[must_use]
    pub const fn structural_status(&self) -> CarrierStatus {
        if self.content.is_empty() || self.header.valid_non_empty_domain() {
            CarrierStatus::ValidOpaque
        } else {
            CarrierStatus::Invalid
        }
    }
}

impl CursorCarrier {
    /// Constructs a cursor carrier from header and content.
    ///
    /// ```rust,no_run
    /// // no_run: requires arch allocator runtime.
    /// ```
    #[must_use]
    pub const fn new(header: CursorHeader, content: Bytes) -> Self {
        Self { header, content }
    }

    /// Returns this carrier's structural status without consulting a codec.
    ///
    /// ```rust,no_run
    /// // no_run: requires arch allocator runtime.
    /// ```
    #[must_use]
    pub const fn structural_status(&self) -> CarrierStatus {
        if self.content.is_empty() || self.header.valid_non_empty_domain() {
            CarrierStatus::ValidOpaque
        } else {
            CarrierStatus::Invalid
        }
    }
}

/// Carrier validation status (§4.5 CR8).
///
/// ```rust
/// use reovim_subsys_domain::carrier::CarrierStatus;
///
/// assert_ne!(CarrierStatus::ValidKnown, CarrierStatus::Invalid);
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CarrierStatus {
    /// Codec registered and validation passed.
    ValidKnown,
    /// Structurally well-framed, but codec unavailable.
    ValidOpaque,
    /// Malformed or rejected by ingress validation.
    Invalid,
}

// L12 layout: tests in sibling carrier_tests.rs, declared in lib.rs.
