//! Opaque Domain-tier identifiers shared across the contract surface
//! (§4.1 §1, §4.2, walking-skeleton subset, #797).
//!
//! These newtypes appear in the Domain contract signatures
//! ([`crate::contract::RenderProjector::render`]) and in
//! [`crate::projection::Projection`], so they live in the contract tier rather
//! than the kernel. The kernel re-exports them at their historical paths.
//!
//! Numeric values are allocation-order-dependent across runs (CR9); the durable
//! identity for a Domain is its interned name, not the number.

use core::num::NonZeroU32;

// ── DomainId ──────────────────────────────────────────────────────────────────

/// Stable opaque Domain identifier (§4.1 §1).
///
/// `NonZeroU32` so the niche optimization applies inside `Option<DomainId>`.
/// Numeric `DomainId` is allocation-order-dependent across runs (CR9) — the
/// durable identity is the interned name, not the number.
///
/// ```rust
/// use reovim_subsys_domain::id::DomainId;
/// use core::num::NonZeroU32;
///
/// let id = DomainId::new(NonZeroU32::new(1).unwrap());
/// assert_eq!(id.as_u32(), 1);
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct DomainId(NonZeroU32);

impl DomainId {
    /// Wraps a `NonZeroU32` as a `DomainId`.
    ///
    /// ```rust
    /// use reovim_subsys_domain::id::DomainId;
    /// use core::num::NonZeroU32;
    ///
    /// let id = DomainId::new(NonZeroU32::new(2).unwrap());
    /// assert_eq!(id.as_u32(), 2);
    /// ```
    #[must_use]
    pub const fn new(v: NonZeroU32) -> Self {
        Self(v)
    }

    /// Returns the raw `u32` value (always non-zero).
    ///
    /// ```rust
    /// use reovim_subsys_domain::id::DomainId;
    /// use core::num::NonZeroU32;
    ///
    /// let id = DomainId::new(NonZeroU32::new(3).unwrap());
    /// assert_eq!(id.as_u32(), 3);
    /// ```
    #[must_use]
    pub const fn as_u32(self) -> u32 {
        self.0.get()
    }
}

// ── BufferId ─────────────────────────────────────────────────────────────────

/// Opaque buffer identifier (walking-skeleton: one buffer, value = 1).
///
/// Appears in [`crate::contract::RenderProjector::render`] and
/// [`crate::projection::Projection::buffer_id`], so it is part of the contract
/// surface.
///
/// ```rust
/// use reovim_subsys_domain::id::BufferId;
///
/// let id = BufferId::new(1);
/// assert_eq!(id.as_u32(), 1);
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct BufferId(u32);

impl BufferId {
    /// Constructs a `BufferId` from a raw `u32`.
    ///
    /// ```rust
    /// use reovim_subsys_domain::id::BufferId;
    ///
    /// let id = BufferId::new(1);
    /// assert_eq!(id.as_u32(), 1);
    /// ```
    #[must_use]
    pub const fn new(v: u32) -> Self {
        Self(v)
    }

    /// Returns the raw `u32` value.
    ///
    /// ```rust
    /// use reovim_subsys_domain::id::BufferId;
    ///
    /// assert_eq!(BufferId::new(3).as_u32(), 3);
    /// ```
    #[must_use]
    pub const fn as_u32(self) -> u32 {
        self.0
    }
}

// ── WindowId ─────────────────────────────────────────────────────────────────

/// Opaque window identifier (walking-skeleton: one window, value = 1).
///
/// Appears in [`crate::contract::RenderProjector::render`] and
/// [`crate::projection::Projection::window_id`], so it is part of the contract
/// surface.
///
/// ```rust
/// use reovim_subsys_domain::id::WindowId;
///
/// let id = WindowId::new(1);
/// assert_eq!(id.as_u32(), 1);
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct WindowId(u32);

impl WindowId {
    /// Constructs a `WindowId` from a raw `u32`.
    ///
    /// ```rust
    /// use reovim_subsys_domain::id::WindowId;
    ///
    /// let id = WindowId::new(1);
    /// assert_eq!(id.as_u32(), 1);
    /// ```
    #[must_use]
    pub const fn new(v: u32) -> Self {
        Self(v)
    }

    /// Returns the raw `u32` value.
    ///
    /// ```rust
    /// use reovim_subsys_domain::id::WindowId;
    ///
    /// assert_eq!(WindowId::new(5).as_u32(), 5);
    /// ```
    #[must_use]
    pub const fn as_u32(self) -> u32 {
        self.0
    }
}

// L12 layout: tests in sibling id_tests.rs, declared in lib.rs.
