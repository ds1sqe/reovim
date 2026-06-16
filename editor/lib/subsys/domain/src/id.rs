//! Opaque Domain-tier identifiers shared across the contract surface
//! (§3.x, §4.x, walking-skeleton subset grown by #798).
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

// ── SessionId ────────────────────────────────────────────────────────────────

/// Opaque session identifier for in-process substrate contracts (§3.1).
///
/// Durable session names cross persistence/surface boundaries as strings; this
/// numeric handle is process-local.
///
/// ```rust
/// use reovim_subsys_domain::id::SessionId;
///
/// let id = SessionId::new(9);
/// assert_eq!(id.as_u32(), 9);
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SessionId(u32);

impl SessionId {
    /// Constructs a `SessionId` from a raw `u32`.
    ///
    /// ```rust
    /// use reovim_subsys_domain::id::SessionId;
    ///
    /// assert_eq!(SessionId::new(1).as_u32(), 1);
    /// ```
    #[must_use]
    pub const fn new(v: u32) -> Self {
        Self(v)
    }

    /// Returns the raw process-local value.
    ///
    /// ```rust
    /// use reovim_subsys_domain::id::SessionId;
    ///
    /// assert_eq!(SessionId::new(2).as_u32(), 2);
    /// ```
    #[must_use]
    pub const fn as_u32(self) -> u32 {
        self.0
    }
}

// ── ClientId ─────────────────────────────────────────────────────────────────

/// Opaque client identifier scoped to a running kernel (§3.1).
///
/// ```rust
/// use reovim_subsys_domain::id::ClientId;
///
/// let id = ClientId::new(4);
/// assert_eq!(id.as_u32(), 4);
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ClientId(u32);

impl ClientId {
    /// Constructs a `ClientId` from a raw `u32`.
    ///
    /// ```rust
    /// use reovim_subsys_domain::id::ClientId;
    ///
    /// assert_eq!(ClientId::new(4).as_u32(), 4);
    /// ```
    #[must_use]
    pub const fn new(v: u32) -> Self {
        Self(v)
    }

    /// Returns the raw process-local value.
    ///
    /// ```rust
    /// use reovim_subsys_domain::id::ClientId;
    ///
    /// assert_eq!(ClientId::new(5).as_u32(), 5);
    /// ```
    #[must_use]
    pub const fn as_u32(self) -> u32 {
        self.0
    }
}

// ── DomainAttachmentId ───────────────────────────────────────────────────────

/// Opaque Domain attachment identifier (§4.2).
///
/// ```rust
/// use reovim_subsys_domain::id::DomainAttachmentId;
///
/// let id = DomainAttachmentId::new(1);
/// assert_eq!(id.as_u32(), 1);
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct DomainAttachmentId(u32);

impl DomainAttachmentId {
    /// Constructs a `DomainAttachmentId` from a raw `u32`.
    ///
    /// ```rust
    /// use reovim_subsys_domain::id::DomainAttachmentId;
    ///
    /// assert_eq!(DomainAttachmentId::new(1).as_u32(), 1);
    /// ```
    #[must_use]
    pub const fn new(v: u32) -> Self {
        Self(v)
    }

    /// Returns the raw process-local value.
    ///
    /// ```rust
    /// use reovim_subsys_domain::id::DomainAttachmentId;
    ///
    /// assert_eq!(DomainAttachmentId::new(2).as_u32(), 2);
    /// ```
    #[must_use]
    pub const fn as_u32(self) -> u32 {
        self.0
    }
}

// ── PendingAttachmentId ──────────────────────────────────────────────────────

/// Opaque pending-attachment identifier (§4.2 DT12).
///
/// ```rust
/// use reovim_subsys_domain::id::PendingAttachmentId;
///
/// let id = PendingAttachmentId::new(7);
/// assert_eq!(id.as_u32(), 7);
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct PendingAttachmentId(u32);

impl PendingAttachmentId {
    /// Constructs a `PendingAttachmentId` from a raw `u32`.
    ///
    /// ```rust
    /// use reovim_subsys_domain::id::PendingAttachmentId;
    ///
    /// assert_eq!(PendingAttachmentId::new(7).as_u32(), 7);
    /// ```
    #[must_use]
    pub const fn new(v: u32) -> Self {
        Self(v)
    }

    /// Returns the raw process-local value.
    ///
    /// ```rust
    /// use reovim_subsys_domain::id::PendingAttachmentId;
    ///
    /// assert_eq!(PendingAttachmentId::new(8).as_u32(), 8);
    /// ```
    #[must_use]
    pub const fn as_u32(self) -> u32 {
        self.0
    }
}

// ── ReplayQueueId ────────────────────────────────────────────────────────────

/// Opaque replay queue identifier for pending attachment input (§4.2 DT12).
///
/// ```rust
/// use reovim_subsys_domain::id::ReplayQueueId;
///
/// assert_eq!(ReplayQueueId::new(3).as_u32(), 3);
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ReplayQueueId(u32);

impl ReplayQueueId {
    /// Constructs a `ReplayQueueId` from a raw `u32`.
    ///
    /// ```rust
    /// use reovim_subsys_domain::id::ReplayQueueId;
    ///
    /// assert_eq!(ReplayQueueId::new(3).as_u32(), 3);
    /// ```
    #[must_use]
    pub const fn new(v: u32) -> Self {
        Self(v)
    }

    /// Returns the raw process-local value.
    ///
    /// ```rust
    /// use reovim_subsys_domain::id::ReplayQueueId;
    ///
    /// assert_eq!(ReplayQueueId::new(4).as_u32(), 4);
    /// ```
    #[must_use]
    pub const fn as_u32(self) -> u32 {
        self.0
    }
}

// ── CdylibId ─────────────────────────────────────────────────────────────────

/// Opaque owner identifier for statically or dynamically registered rows.
///
/// `CdylibId` is process-local. Phase 4 uses it for static owner rows; Phase 5
/// connects it to the real inventory and dynamic loader.
///
/// ```rust
/// use core::num::NonZeroU32;
/// use reovim_subsys_domain::id::CdylibId;
///
/// let id = CdylibId::new(NonZeroU32::new(1).unwrap());
/// assert_eq!(id.as_u32(), 1);
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct CdylibId(NonZeroU32);

impl CdylibId {
    /// Wraps a `NonZeroU32` as a `CdylibId`.
    ///
    /// ```rust
    /// use core::num::NonZeroU32;
    /// use reovim_subsys_domain::id::CdylibId;
    ///
    /// assert_eq!(CdylibId::new(NonZeroU32::new(2).unwrap()).as_u32(), 2);
    /// ```
    #[must_use]
    pub const fn new(v: NonZeroU32) -> Self {
        Self(v)
    }

    /// Returns the raw non-zero value.
    ///
    /// ```rust
    /// use core::num::NonZeroU32;
    /// use reovim_subsys_domain::id::CdylibId;
    ///
    /// assert_eq!(CdylibId::new(NonZeroU32::new(3).unwrap()).as_u32(), 3);
    /// ```
    #[must_use]
    pub const fn as_u32(self) -> u32 {
        self.0.get()
    }
}

// ── RegisterId ───────────────────────────────────────────────────────────────

/// Opaque register-name intern identifier (§3.4).
///
/// ```rust
/// use reovim_subsys_domain::id::RegisterId;
///
/// assert_eq!(RegisterId::new(12).as_u32(), 12);
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct RegisterId(u32);

impl RegisterId {
    /// Constructs a `RegisterId` from a raw `u32`.
    ///
    /// ```rust
    /// use reovim_subsys_domain::id::RegisterId;
    ///
    /// assert_eq!(RegisterId::new(12).as_u32(), 12);
    /// ```
    #[must_use]
    pub const fn new(v: u32) -> Self {
        Self(v)
    }

    /// Returns the raw process-local value.
    ///
    /// ```rust
    /// use reovim_subsys_domain::id::RegisterId;
    ///
    /// assert_eq!(RegisterId::new(13).as_u32(), 13);
    /// ```
    #[must_use]
    pub const fn as_u32(self) -> u32 {
        self.0
    }
}

// ── SlotKindId ───────────────────────────────────────────────────────────────

/// Opaque interned view-slot kind identifier (§3.2).
///
/// ```rust
/// use reovim_subsys_domain::id::SlotKindId;
///
/// assert_eq!(SlotKindId::new(6).as_u32(), 6);
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SlotKindId(u32);

impl SlotKindId {
    /// Constructs a `SlotKindId` from a raw `u32`.
    ///
    /// ```rust
    /// use reovim_subsys_domain::id::SlotKindId;
    ///
    /// assert_eq!(SlotKindId::new(6).as_u32(), 6);
    /// ```
    #[must_use]
    pub const fn new(v: u32) -> Self {
        Self(v)
    }

    /// Returns the raw process-local value.
    ///
    /// ```rust
    /// use reovim_subsys_domain::id::SlotKindId;
    ///
    /// assert_eq!(SlotKindId::new(7).as_u32(), 7);
    /// ```
    #[must_use]
    pub const fn as_u32(self) -> u32 {
        self.0
    }
}

// ── ServiceKey / ServiceLeaseId ──────────────────────────────────────────────

/// Opaque interned service key (§3.3).
///
/// ```rust
/// use reovim_subsys_domain::id::ServiceKey;
///
/// assert_eq!(ServiceKey::new(5).as_u32(), 5);
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ServiceKey(u32);

impl ServiceKey {
    /// Constructs a `ServiceKey` from a raw `u32`.
    ///
    /// ```rust
    /// use reovim_subsys_domain::id::ServiceKey;
    ///
    /// assert_eq!(ServiceKey::new(5).as_u32(), 5);
    /// ```
    #[must_use]
    pub const fn new(v: u32) -> Self {
        Self(v)
    }

    /// Returns the raw process-local value.
    ///
    /// ```rust
    /// use reovim_subsys_domain::id::ServiceKey;
    ///
    /// assert_eq!(ServiceKey::new(6).as_u32(), 6);
    /// ```
    #[must_use]
    pub const fn as_u32(self) -> u32 {
        self.0
    }
}

/// Opaque service lease identifier (§3.3).
///
/// ```rust
/// use reovim_subsys_domain::id::ServiceLeaseId;
///
/// assert_eq!(ServiceLeaseId::new(99).as_u64(), 99);
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ServiceLeaseId(u64);

impl ServiceLeaseId {
    /// Constructs a `ServiceLeaseId` from a raw `u64`.
    ///
    /// ```rust
    /// use reovim_subsys_domain::id::ServiceLeaseId;
    ///
    /// assert_eq!(ServiceLeaseId::new(99).as_u64(), 99);
    /// ```
    #[must_use]
    pub const fn new(v: u64) -> Self {
        Self(v)
    }

    /// Returns the raw process-local value.
    ///
    /// ```rust
    /// use reovim_subsys_domain::id::ServiceLeaseId;
    ///
    /// assert_eq!(ServiceLeaseId::new(100).as_u64(), 100);
    /// ```
    #[must_use]
    pub const fn as_u64(self) -> u64 {
        self.0
    }
}

// ── StreamId ─────────────────────────────────────────────────────────────────

/// Opaque stream handle identifier (§4.4).
///
/// ```rust
/// use reovim_subsys_domain::id::StreamId;
///
/// assert_eq!(StreamId::new(11).as_u64(), 11);
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct StreamId(u64);

impl StreamId {
    /// Constructs a `StreamId` from a raw `u64`.
    ///
    /// ```rust
    /// use reovim_subsys_domain::id::StreamId;
    ///
    /// assert_eq!(StreamId::new(11).as_u64(), 11);
    /// ```
    #[must_use]
    pub const fn new(v: u64) -> Self {
        Self(v)
    }

    /// Returns the raw process-local value.
    ///
    /// ```rust
    /// use reovim_subsys_domain::id::StreamId;
    ///
    /// assert_eq!(StreamId::new(12).as_u64(), 12);
    /// ```
    #[must_use]
    pub const fn as_u64(self) -> u64 {
        self.0
    }
}

// L12 layout: tests in sibling id_tests.rs, declared in lib.rs.
