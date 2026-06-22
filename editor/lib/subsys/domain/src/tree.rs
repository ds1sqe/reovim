//! Domain-tree contract shapes (§4.2, #798).
//!
//! These types describe attachment records, focus chains, pending attachment
//! resolution context, and DT14..DT16 focus-transition snapshots. The kernel
//! owns storage, locking, and mutation sequencing; this module keeps the shared
//! data vocabulary in the contract tier.

use reovim_lib_ds::{Bytes, Seq};

use crate::{
    carrier::PositionCarrier,
    id::{
        BufferId, ClientId, DomainAttachmentId, DomainId, PendingAttachmentId, ReplayQueueId,
        SessionId, WindowId,
    },
};

/// Allocation failure while mutating domain contract storage.
///
/// The domain contract tier exposes its own upper-facing error vocabulary; the
/// down-face allocator refusal type stays behind `lib/ds`/`kabi`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct DomainAllocError;

/// Attachment scope expressed in the parent Domain's position vocabulary.
///
/// ```rust,no_run
/// // no_run: requires arch allocator runtime for carrier content.
/// ```
pub struct DomainScope {
    /// Inclusive start endpoint carrier.
    pub start: PositionCarrier,
    /// Exclusive end endpoint carrier.
    pub end: PositionCarrier,
    /// Domain-defined scope flags.
    pub flags: u8,
}

impl DomainScope {
    /// Builds a scope from endpoints and flags.
    ///
    /// ```rust,no_run
    /// // no_run: requires arch allocator runtime for carrier content.
    /// ```
    #[must_use]
    pub const fn new(start: PositionCarrier, end: PositionCarrier, flags: u8) -> Self {
        Self { start, end, flags }
    }
}

/// Live Domain attachment record (§4.2 §1).
///
/// ```rust,no_run
/// // no_run: requires arch allocator runtime for scope carriers.
/// ```
pub struct DomainAttachment {
    /// Attachment id.
    pub id: DomainAttachmentId,
    /// Buffer this attachment belongs to.
    pub buffer_id: BufferId,
    /// Attached Domain.
    pub domain_id: DomainId,
    /// Scope in the parent Domain's position vocabulary.
    pub scope: DomainScope,
    /// Parent attachment, or `None` for a root attachment.
    pub parent: Option<DomainAttachmentId>,
    /// Child attachments in stable append order.
    pub children: Seq<DomainAttachmentId>,
    /// Stable append ordinal inside the parent/root list.
    pub child_ordinal: u32,
    /// Structural lifetime references.
    pub struct_refs: u32,
    /// Focus-chain lifetime references.
    pub focus_refs: u32,
}

impl DomainAttachment {
    /// Builds a live attachment with explicit DT11 refcounts.
    ///
    /// ```rust,no_run
    /// // no_run: requires arch allocator runtime for scope carriers.
    /// ```
    #[expect(
        clippy::too_many_arguments,
        reason = "contract record mirrors DT11 fields"
    )]
    #[must_use]
    pub const fn new(
        id: DomainAttachmentId,
        buffer_id: BufferId,
        domain_id: DomainId,
        scope: DomainScope,
        parent: Option<DomainAttachmentId>,
        child_ordinal: u32,
        struct_refs: u32,
        focus_refs: u32,
    ) -> Self {
        Self {
            id,
            buffer_id,
            domain_id,
            scope,
            parent,
            children: Seq::new(),
            child_ordinal,
            struct_refs,
            focus_refs,
        }
    }

    /// Builds the DT13 resolved-attachment default (`struct_refs = 1`,
    /// `focus_refs = 1`).
    ///
    /// ```rust,no_run
    /// // no_run: requires arch allocator runtime for scope carriers.
    /// ```
    #[must_use]
    pub const fn resolved(
        id: DomainAttachmentId,
        buffer_id: BufferId,
        domain_id: DomainId,
        scope: DomainScope,
        parent: Option<DomainAttachmentId>,
        child_ordinal: u32,
    ) -> Self {
        Self::new(id, buffer_id, domain_id, scope, parent, child_ordinal, 1, 1)
    }

    /// Appends a child id, preserving tree insertion order.
    ///
    /// # Errors
    ///
    /// Returns [`DomainAllocError`] when the backing child sequence cannot grow.
    ///
    /// ```rust,no_run
    /// // no_run: requires arch allocator runtime for scope carriers.
    /// ```
    pub fn try_push_child(&mut self, child: DomainAttachmentId) -> Result<(), DomainAllocError> {
        self.children.try_push(child).map_err(|_| DomainAllocError)
    }

    /// Returns whether structural detach is allowed by DT11/DT8.
    ///
    /// ```rust,no_run
    /// // no_run: requires arch allocator runtime for scope carriers.
    /// ```
    #[must_use]
    pub const fn can_detach(&self) -> bool {
        self.struct_refs == 1 && self.focus_refs == 0
    }
}

/// One entry in a focus chain (§4.2 §3).
///
/// ```rust
/// use reovim_subsys_domain::{id::PendingAttachmentId, tree::FocusEntry};
///
/// assert!(FocusEntry::Pending(PendingAttachmentId::new(1)).is_pending());
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FocusEntry {
    /// Pending leaf waiting for its owner to become active.
    Pending(PendingAttachmentId),
    /// Resolved live attachment.
    Resolved(DomainAttachmentId),
}

impl FocusEntry {
    /// Returns whether this entry is pending.
    ///
    /// ```rust
    /// use reovim_subsys_domain::{id::DomainAttachmentId, tree::FocusEntry};
    ///
    /// assert!(!FocusEntry::Resolved(DomainAttachmentId::new(2)).is_pending());
    /// ```
    #[must_use]
    pub const fn is_pending(self) -> bool {
        matches!(self, Self::Pending(_))
    }

    /// Returns the resolved attachment id, if any.
    ///
    /// ```rust
    /// use reovim_subsys_domain::{id::DomainAttachmentId, tree::FocusEntry};
    ///
    /// assert_eq!(
    ///     FocusEntry::Resolved(DomainAttachmentId::new(3)).resolved_id(),
    ///     Some(DomainAttachmentId::new(3)),
    /// );
    /// ```
    #[must_use]
    pub const fn resolved_id(self) -> Option<DomainAttachmentId> {
        match self {
            Self::Resolved(id) => Some(id),
            Self::Pending(_) => None,
        }
    }
}

/// Focus-chain storage type used by sessions (§4.2 §3).
///
/// ```rust,no_run
/// // no_run: requires arch allocator runtime when entries are pushed.
/// ```
pub type FocusChain = Seq<FocusEntry>;

/// Resolution context owned by a pending focus leaf (§4.2 §4).
///
/// ```rust,no_run
/// // no_run: requires arch allocator runtime for scope and payload bytes.
/// ```
pub struct PendingAttachment {
    /// Pending id referenced by the focus chain.
    pub id: PendingAttachmentId,
    /// Domain that will resolve the attachment.
    pub domain_id: DomainId,
    /// Parent attachment, or `None` for a root attachment.
    pub parent: Option<DomainAttachmentId>,
    /// Requested scope.
    pub scope: DomainScope,
    /// Target buffer.
    pub buffer_id: BufferId,
    /// Client whose focus chain owns this pending entry.
    pub client_id: ClientId,
    /// Window whose focus chain owns this pending entry.
    pub window_id: WindowId,
    /// Owned bootstrap payload bytes.
    pub bootstrap_payload: Bytes,
    /// Replay queue for raw input blocked by this pending leaf.
    pub replay_queue_id: ReplayQueueId,
}

impl PendingAttachment {
    /// Builds a pending attachment context.
    ///
    /// ```rust,no_run
    /// // no_run: requires arch allocator runtime for scope and payload bytes.
    /// ```
    #[expect(
        clippy::too_many_arguments,
        reason = "contract record mirrors DT12 fields"
    )]
    #[must_use]
    pub const fn new(
        id: PendingAttachmentId,
        domain_id: DomainId,
        parent: Option<DomainAttachmentId>,
        scope: DomainScope,
        buffer_id: BufferId,
        client_id: ClientId,
        window_id: WindowId,
        bootstrap_payload: Bytes,
        replay_queue_id: ReplayQueueId,
    ) -> Self {
        Self {
            id,
            domain_id,
            parent,
            scope,
            buffer_id,
            client_id,
            window_id,
            bootstrap_payload,
            replay_queue_id,
        }
    }
}

/// Snapshot entry embedded in a DT14 focus-transition record.
///
/// ```rust
/// use core::num::NonZeroU32;
/// use reovim_subsys_domain::{
///     id::{DomainAttachmentId, DomainId},
///     tree::FocusEntrySnapshot,
/// };
///
/// let domain = DomainId::new(NonZeroU32::new(1).unwrap());
/// assert_eq!(
///     FocusEntrySnapshot::Resolved(DomainAttachmentId::new(2), domain).domain_id(),
///     domain,
/// );
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FocusEntrySnapshot {
    /// Pending entry plus snapshotted Domain id.
    Pending(PendingAttachmentId, DomainId),
    /// Resolved attachment plus snapshotted Domain id.
    Resolved(DomainAttachmentId, DomainId),
}

impl FocusEntrySnapshot {
    /// Returns the snapshotted Domain id.
    ///
    /// ```rust
    /// use core::num::NonZeroU32;
    /// use reovim_subsys_domain::{
    ///     id::{DomainId, PendingAttachmentId},
    ///     tree::FocusEntrySnapshot,
    /// };
    ///
    /// let domain = DomainId::new(NonZeroU32::new(4).unwrap());
    /// assert_eq!(
    ///     FocusEntrySnapshot::Pending(PendingAttachmentId::new(5), domain).domain_id(),
    ///     domain,
    /// );
    /// ```
    #[must_use]
    pub const fn domain_id(self) -> DomainId {
        match self {
            Self::Pending(_, domain_id) | Self::Resolved(_, domain_id) => domain_id,
        }
    }
}

/// One paired focus-transition record (§4.2 §8, DT14..DT16).
///
/// ```rust,no_run
/// // no_run: requires arch allocator runtime when snapshots are pushed.
/// ```
pub struct FocusTransition {
    /// Session that produced the transition.
    pub session_id: SessionId,
    /// Client whose focus chain changed.
    pub client_id: ClientId,
    /// Buffer whose focus chain changed.
    pub buffer_id: BufferId,
    /// Window whose focus chain changed.
    pub window_id: WindowId,
    /// Monotonic per-session transition sequence.
    pub seq: u64,
    /// Snapshot before the mutation.
    pub before: Seq<FocusEntrySnapshot>,
    /// Snapshot after the mutation.
    pub after: Seq<FocusEntrySnapshot>,
}

impl FocusTransition {
    /// Builds an empty transition record. Callers fill `before`/`after` before
    /// publication in the CC17 apply phase.
    ///
    /// ```rust,no_run
    /// // no_run: requires arch allocator runtime when snapshots are pushed.
    /// ```
    #[must_use]
    pub const fn new(
        session_id: SessionId,
        client_id: ClientId,
        buffer_id: BufferId,
        window_id: WindowId,
        seq: u64,
    ) -> Self {
        Self {
            session_id,
            client_id,
            buffer_id,
            window_id,
            seq,
            before: Seq::new(),
            after: Seq::new(),
        }
    }
}

/// Result of walking a focus chain for raw input (§4.2 §7).
///
/// ```rust
/// use reovim_subsys_domain::tree::DispatchVerdict;
///
/// assert!(DispatchVerdict::Claimed.stops_walk());
/// assert!(!DispatchVerdict::Ignored.stops_walk());
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DispatchVerdict {
    /// A handler claimed the input; dispatch stops.
    Claimed,
    /// A handler ignored the input; dispatch may continue to an ancestor.
    Ignored,
    /// A pending leaf queued the input for replay; dispatch stops.
    QueuedPending(PendingAttachmentId),
}

impl DispatchVerdict {
    /// Returns whether this verdict stops the leaf-to-root walk.
    ///
    /// ```rust
    /// use reovim_subsys_domain::{id::PendingAttachmentId, tree::DispatchVerdict};
    ///
    /// assert!(DispatchVerdict::QueuedPending(PendingAttachmentId::new(1)).stops_walk());
    /// ```
    #[must_use]
    pub const fn stops_walk(self) -> bool {
        matches!(self, Self::Claimed | Self::QueuedPending(_))
    }
}

// L12 layout: tests in sibling tree_tests.rs, declared in lib.rs.
