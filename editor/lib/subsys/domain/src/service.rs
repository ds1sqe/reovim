//! Service-registry contract shapes (§3.3, #798).
//!
//! FFI-frozen vtables and raw handles belong in the ABI layer when they cross a
//! cdylib boundary. This module carries the safe in-process metadata and lease
//! state that kernel and Domains share.

use reovim_lib_ds::Bytes;

use crate::{
    id::{CdylibId, ServiceKey, ServiceLeaseId},
    routing::DomainApiVersion,
};

/// Service thread-safety and re-entrancy flags (§3.3 §3).
///
/// ```rust
/// use reovim_subsys_domain::service::ServiceFlags;
///
/// let flags = ServiceFlags::new(ServiceFlags::SEND_SAFE | ServiceFlags::SYNC_SAFE);
/// assert!(flags.send_safe());
/// assert!(flags.sync_safe());
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ServiceFlags(u32);

impl ServiceFlags {
    /// Service handle may move across threads.
    pub const SEND_SAFE: u32 = 0x0001;
    /// Concurrent calls are allowed.
    pub const SYNC_SAFE: u32 = 0x0002;
    /// Service vtable may call `HostApi` during execution.
    pub const HOSTAPI_REENTRANT: u32 = 0x0004;
    /// Service drop callback may call `HostApi`.
    pub const DROP_MAY_CALL_HOSTAPI: u32 = 0x0008;

    /// Wraps raw flag bits.
    ///
    /// ```rust
    /// use reovim_subsys_domain::service::ServiceFlags;
    ///
    /// assert_eq!(ServiceFlags::new(3).bits(), 3);
    /// ```
    #[must_use]
    pub const fn new(bits: u32) -> Self {
        Self(bits)
    }

    /// Returns raw flag bits.
    ///
    /// ```rust
    /// use reovim_subsys_domain::service::ServiceFlags;
    ///
    /// assert_eq!(ServiceFlags::new(4).bits(), 4);
    /// ```
    #[must_use]
    pub const fn bits(self) -> u32 {
        self.0
    }

    /// Returns whether all `mask` bits are set.
    ///
    /// ```rust
    /// use reovim_subsys_domain::service::ServiceFlags;
    ///
    /// assert!(ServiceFlags::new(ServiceFlags::HOSTAPI_REENTRANT).contains(ServiceFlags::HOSTAPI_REENTRANT));
    /// ```
    #[must_use]
    pub const fn contains(self, mask: u32) -> bool {
        self.0 & mask == mask
    }

    /// Returns whether `SEND_SAFE` is set.
    ///
    /// ```rust
    /// use reovim_subsys_domain::service::ServiceFlags;
    ///
    /// assert!(!ServiceFlags::new(0).send_safe());
    /// ```
    #[must_use]
    pub const fn send_safe(self) -> bool {
        self.contains(Self::SEND_SAFE)
    }

    /// Returns whether `SYNC_SAFE` is set.
    ///
    /// ```rust
    /// use reovim_subsys_domain::service::ServiceFlags;
    ///
    /// assert!(!ServiceFlags::new(0).sync_safe());
    /// ```
    #[must_use]
    pub const fn sync_safe(self) -> bool {
        self.contains(Self::SYNC_SAFE)
    }
}

/// Service row visibility/lifecycle state (§3.3 §4/§5).
///
/// ```rust
/// use reovim_subsys_domain::service::ServiceRowState;
///
/// assert!(ServiceRowState::Visible.is_visible());
/// assert!(!ServiceRowState::Revoked.is_visible());
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ServiceRowState {
    /// Row is visible to lookup.
    Visible,
    /// Row is hidden while outstanding leases drain.
    DrainingHidden,
    /// Row has been revoked by owner unload/unregister.
    Revoked,
}

impl ServiceRowState {
    /// Returns whether lookup may see this row.
    ///
    /// ```rust
    /// use reovim_subsys_domain::service::ServiceRowState;
    ///
    /// assert!(!ServiceRowState::DrainingHidden.is_visible());
    /// ```
    #[must_use]
    pub const fn is_visible(self) -> bool {
        matches!(self, Self::Visible)
    }
}

/// Service lookup/call failure surfaced by row-state checks.
///
/// ```rust
/// use reovim_subsys_domain::service::ServiceAccessError;
///
/// assert_ne!(ServiceAccessError::Busy, ServiceAccessError::Stale);
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ServiceAccessError {
    /// `!SEND_SAFE` service was invoked from the wrong thread.
    InvalidState,
    /// `!SYNC_SAFE` service already has an active call or a lease timed out.
    Busy,
    /// Owner generation changed or row was revoked.
    Stale,
    /// No visible row exists for the requested key.
    NotFound,
}

/// Safe descriptor metadata for a service row (§3.3 §1).
///
/// ```rust,no_run
/// // no_run: requires arch allocator runtime for the diagnostic type name.
/// ```
pub struct ServiceDescriptorMeta {
    /// Stable interned service key.
    pub key: ServiceKey,
    /// Owning cdylib.
    pub owner_cdylib_id: CdylibId,
    /// ABI version for the service vtable.
    pub abi_version: DomainApiVersion,
    /// API version for semantic compatibility.
    pub api_version: DomainApiVersion,
    /// Diagnostic type name; not used for equality.
    pub type_name: Bytes,
    /// Size of the vtable shape the owner provided.
    pub vtable_size: usize,
    /// Declared service flags.
    pub flags: ServiceFlags,
}

impl ServiceDescriptorMeta {
    /// Builds service descriptor metadata.
    ///
    /// ```rust,no_run
    /// // no_run: requires arch allocator runtime for the diagnostic type name.
    /// ```
    #[must_use]
    pub const fn new(
        key: ServiceKey,
        owner_cdylib_id: CdylibId,
        contract_abi: DomainApiVersion,
        semantic_api: DomainApiVersion,
        type_name: Bytes,
        vtable_size: usize,
        flags: ServiceFlags,
    ) -> Self {
        Self {
            key,
            owner_cdylib_id,
            abi_version: contract_abi,
            api_version: semantic_api,
            type_name,
            vtable_size,
            flags,
        }
    }
}

/// Borrow-only service lookup token (§3.3 §2.1).
///
/// ```rust
/// use core::num::NonZeroU32;
/// use reovim_subsys_domain::{id::{CdylibId, ServiceKey}, service::ServiceBorrow};
///
/// let owner = CdylibId::new(NonZeroU32::new(1).unwrap());
/// let borrow = ServiceBorrow::new(ServiceKey::new(2), owner, 7);
/// assert_eq!(borrow.owner_generation, 7);
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ServiceBorrow {
    /// Borrowed service key.
    pub key: ServiceKey,
    /// Owner cdylib protected for the `HostApi` call duration.
    pub owner_cdylib_id: CdylibId,
    /// Owner generation captured by the lookup.
    pub owner_generation: u64,
}

impl ServiceBorrow {
    /// Builds a borrow token.
    ///
    /// ```rust
    /// use core::num::NonZeroU32;
    /// use reovim_subsys_domain::{id::{CdylibId, ServiceKey}, service::ServiceBorrow};
    ///
    /// let owner = CdylibId::new(NonZeroU32::new(1).unwrap());
    /// assert_eq!(ServiceBorrow::new(ServiceKey::new(3), owner, 4).key, ServiceKey::new(3));
    /// ```
    #[must_use]
    pub const fn new(key: ServiceKey, owner_cdylib_id: CdylibId, owner_generation: u64) -> Self {
        Self {
            key,
            owner_cdylib_id,
            owner_generation,
        }
    }
}

/// Retained service lease (§3.3 §2.2, SVC6).
///
/// ```rust
/// use core::num::NonZeroU32;
/// use reovim_subsys_domain::{id::{CdylibId, ServiceKey, ServiceLeaseId}, service::ServiceLease};
///
/// let owner = CdylibId::new(NonZeroU32::new(1).unwrap());
/// let lease = ServiceLease::new(ServiceLeaseId::new(1), ServiceKey::new(2), owner, 3, 100, 50);
/// assert!(lease.is_expired(151));
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ServiceLease {
    /// Lease id.
    pub id: ServiceLeaseId,
    /// Leased service key.
    pub key: ServiceKey,
    /// Owner cdylib protected by the lease.
    pub owner_cdylib_id: CdylibId,
    /// Owner generation captured when leased.
    pub owner_generation: u64,
    /// Monotonic acquisition time in milliseconds.
    pub acquired_at_ms: u64,
    /// EditorCore-wide lease timeout in milliseconds.
    pub timeout_ms: u64,
}

impl ServiceLease {
    /// Builds a service lease.
    ///
    /// ```rust
    /// use core::num::NonZeroU32;
    /// use reovim_subsys_domain::{id::{CdylibId, ServiceKey, ServiceLeaseId}, service::ServiceLease};
    ///
    /// let owner = CdylibId::new(NonZeroU32::new(1).unwrap());
    /// assert_eq!(
    ///     ServiceLease::new(ServiceLeaseId::new(1), ServiceKey::new(2), owner, 3, 4, 5).timeout_ms,
    ///     5,
    /// );
    /// ```
    #[must_use]
    pub const fn new(
        id: ServiceLeaseId,
        key: ServiceKey,
        owner_cdylib_id: CdylibId,
        owner_generation: u64,
        acquired_at_ms: u64,
        timeout_ms: u64,
    ) -> Self {
        Self {
            id,
            key,
            owner_cdylib_id,
            owner_generation,
            acquired_at_ms,
            timeout_ms,
        }
    }

    /// Returns whether `now_ms` exceeds the lease timeout.
    ///
    /// ```rust
    /// use core::num::NonZeroU32;
    /// use reovim_subsys_domain::{id::{CdylibId, ServiceKey, ServiceLeaseId}, service::ServiceLease};
    ///
    /// let owner = CdylibId::new(NonZeroU32::new(1).unwrap());
    /// let lease = ServiceLease::new(ServiceLeaseId::new(1), ServiceKey::new(2), owner, 3, 10, 20);
    /// assert!(!lease.is_expired(29));
    /// ```
    #[must_use]
    pub const fn is_expired(self, now_ms: u64) -> bool {
        now_ms.saturating_sub(self.acquired_at_ms) > self.timeout_ms
    }

    /// Returns whether the current owner generation still matches.
    ///
    /// ```rust
    /// use core::num::NonZeroU32;
    /// use reovim_subsys_domain::{id::{CdylibId, ServiceKey, ServiceLeaseId}, service::ServiceLease};
    ///
    /// let owner = CdylibId::new(NonZeroU32::new(1).unwrap());
    /// let lease = ServiceLease::new(ServiceLeaseId::new(1), ServiceKey::new(2), owner, 3, 10, 20);
    /// assert!(lease.matches_generation(3));
    /// ```
    #[must_use]
    pub const fn matches_generation(self, current_generation: u64) -> bool {
        self.owner_generation == current_generation
    }
}

// L12 layout: tests in sibling service_tests.rs, declared in lib.rs.
