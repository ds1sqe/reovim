//! Minimal `Session` + `SessionState` for the walking-skeleton (§2.3, #797).
//!
//! A `Session` wraps a per-session `Mutex<SessionState>` (the turn-gate
//! serialising dispatch — CC14) and a `WindowId`. A single session, single
//! buffer, single-entry focus chain is the entire extent built here (§4.2
//! walking-skeleton subset note). Multi-session, multi-buffer, multi-window
//! support arrives with master Phase 4.
//!
//! ## CC14 dispatch shape
//!
//! Dispatch follows the CC14 pattern: snapshot under the state lock, drop the
//! lock, invoke the handler, re-lock to apply mutations. The lock is NEVER held
//! across the Domain handler invocation.
//!
//! ## `()` placeholders untouched
//!
//! The existing `Kernel` `()` placeholders (`config`, `lockfile`, `inventory`,
//! `correlation_alloc`, `force_overrides`) are NOT refilled here. The skeleton
//! grows NEW fields (`session`, `domain_router`) onto `Kernel`.

use reovim_arch::{
    ds::{Bytes, Map, Seq, Shared},
    sync::Mutex,
};

use crate::{
    projection::Projection,
    router::{DomainId, DomainRouter, OnRawInputHandler, RenderProjector},
};

// IDs and focus entries live in the Domain contract tier because they cross
// kernel/ext boundaries. Re-exported here so callers that name
// `reovim_kernel::session::*` continue to compile.
pub use reovim_subsys_domain::{
    carrier::{PositionCarrier, PositionHeader},
    id::{
        BufferId, ClientId, DomainAttachmentId, PendingAttachmentId, ReplayQueueId, SessionId,
        WindowId,
    },
    tree::{
        DispatchVerdict, DomainAttachment, DomainScope, FocusEntry, FocusEntrySnapshot,
        FocusTransition, PendingAttachment,
    },
};

/// Bounded focus-chain depth for the Phase 4 in-kernel substrate.
pub const MAX_FOCUS_CHAIN_DEPTH: usize = 8;

const fn empty_scope() -> DomainScope {
    DomainScope::new(
        PositionCarrier::new(PositionHeader::from_raw_parts(0, 0, 0), Bytes::new()),
        PositionCarrier::new(PositionHeader::from_raw_parts(0, 0, 0), Bytes::new()),
        0,
    )
}

// ── SessionState ─────────────────────────────────────────────────────────────

/// Mutable per-session state, held behind a `Mutex` (CC14).
///
/// The walking-skeleton subset carries:
/// - one text buffer (`arch::ds::Bytes`),
/// - one `DomainId` (the registered text Domain),
/// - one `DomainAttachmentId` for the root attachment,
/// - a single-entry `FocusChain = [Resolved(root)]` (§4.2 subset note),
/// - the cursor byte offset within the buffer.
///
/// Multi-session, multi-buffer, and multi-window support arrives with Phase 4.
pub struct SessionState {
    /// Session id used in focus-transition records.
    pub session_id: SessionId,
    /// Client id used by the current single-client skeleton path.
    pub client_id: ClientId,
    /// Clients attached to this session in stable append order.
    pub clients: Seq<ClientId>,
    /// Buffers attached to this session in stable append order.
    pub buffers: Seq<BufferId>,
    /// Windows attached to this session in stable append order.
    pub windows: Seq<WindowId>,
    /// The buffer bytes (DT1: bytes + identity, no interpretation inside).
    pub buffer: Bytes,
    /// Cursor position in bytes within `buffer`.
    pub cursor: usize,
    /// The Domain this session's root attachment belongs to.
    pub root_domain_id: DomainId,
    /// The root `DomainAttachment` for this session's single buffer (§4.2).
    pub root_attachment: DomainAttachmentId,
    /// Root attachment record. Stored directly so `SessionState::new` remains
    /// infallible; dynamically added roots/children live in `attachments`.
    pub root_attachment_record: DomainAttachment,
    /// Extra root attachments and all child attachments by id.
    pub attachments: Map<DomainAttachmentId, DomainAttachment>,
    /// Extra root attachment ids in stable append order.
    pub root_attachments: Seq<DomainAttachmentId>,
    /// Pending attachment contexts by pending id.
    pub pending_attachments: Map<PendingAttachmentId, PendingAttachment>,
    /// Replay queues for pending leaves.
    pub replay_queues: Map<ReplayQueueId, Seq<Bytes>>,
    /// Published DT14..DT16 focus-transition records.
    pub focus_transitions: Seq<FocusTransition>,
    /// Focus chain storage. The initialized prefix is `focus_len`.
    pub focus_chain: [FocusEntry; MAX_FOCUS_CHAIN_DEPTH],
    /// Number of initialized entries in `focus_chain`.
    pub focus_len: usize,
    /// The `WindowId` for this session's single window.
    pub window_id: WindowId,
    /// The `BufferId` for this session's single buffer.
    pub buffer_id: BufferId,
    next_attachment_id: u32,
    next_pending_attachment_id: u32,
    next_replay_queue_id: u32,
    next_focus_transition_seq: u64,
}

impl SessionState {
    /// Creates a new `SessionState` with an empty buffer and a registered
    /// text Domain.
    ///
    /// The root attachment `struct_refs = focus_refs = 1` is implicit (§4.2
    /// walking-skeleton subset: the skeleton grows root-only, so the refcount
    /// starts at 1; multi-session allocation arrives with #778 Phase 4).
    ///
    /// ```rust,no_run
    /// // no_run: requires arch allocator runtime.
    /// use reovim_kernel::{
    ///     router::DomainId,
    ///     session::{BufferId, DomainAttachmentId, SessionState, WindowId},
    /// };
    /// use core::num::NonZeroU32;
    ///
    /// let domain_id = DomainId::new(NonZeroU32::new(1).unwrap());
    /// let state = SessionState::new(
    ///     domain_id,
    ///     DomainAttachmentId::new(1),
    ///     BufferId::new(1),
    ///     WindowId::new(1),
    /// );
    /// assert!(state.buffer.is_empty());
    /// assert_eq!(state.cursor, 0);
    /// ```
    #[must_use]
    pub const fn new(
        root_domain_id: DomainId,
        root_attachment: DomainAttachmentId,
        buffer_id: BufferId,
        window_id: WindowId,
    ) -> Self {
        Self::new_with_client(
            root_domain_id,
            root_attachment,
            buffer_id,
            window_id,
            ClientId::new(1),
        )
    }

    /// Creates a new `SessionState` with explicit client/window/buffer ids.
    ///
    /// ```rust,no_run
    /// // no_run: requires arch allocator runtime.
    /// ```
    #[must_use]
    pub const fn new_with_client(
        root_domain_id: DomainId,
        root_attachment: DomainAttachmentId,
        buffer_id: BufferId,
        window_id: WindowId,
        client_id: ClientId,
    ) -> Self {
        Self {
            session_id: SessionId::new(1),
            client_id,
            clients: Seq::new(),
            buffers: Seq::new(),
            windows: Seq::new(),
            buffer: Bytes::new(),
            cursor: 0,
            root_domain_id,
            root_attachment,
            root_attachment_record: DomainAttachment::new(
                root_attachment,
                buffer_id,
                root_domain_id,
                empty_scope(),
                None,
                0,
                1,
                1,
            ),
            attachments: Map::new(),
            root_attachments: Seq::new(),
            pending_attachments: Map::new(),
            replay_queues: Map::new(),
            focus_transitions: Seq::new(),
            focus_chain: [FocusEntry::Resolved(root_attachment); MAX_FOCUS_CHAIN_DEPTH],
            focus_len: 1,
            window_id,
            buffer_id,
            next_attachment_id: root_attachment.as_u32() + 1,
            next_pending_attachment_id: 1,
            next_replay_queue_id: 1,
            next_focus_transition_seq: 1,
        }
    }

    /// Sets the owning session id. Called by [`Session::new`].
    pub const fn set_session_id(&mut self, id: SessionId) {
        self.session_id = id;
    }

    /// Adds `client_id` to this session if it is not already present.
    ///
    /// # Errors
    ///
    /// Returns `Err("alloc")` if the client list cannot grow.
    pub fn try_attach_client(&mut self, client_id: ClientId) -> Result<bool, &'static str> {
        if self.clients.as_slice().contains(&client_id) {
            return Ok(false);
        }
        self.clients.try_push(client_id).map_err(|_| "alloc")?;
        Ok(true)
    }

    /// Adds `buffer_id` to this session if it is not already present.
    ///
    /// # Errors
    ///
    /// Returns `Err("alloc")` if the buffer list cannot grow.
    pub fn try_attach_buffer(&mut self, buffer_id: BufferId) -> Result<bool, &'static str> {
        if self.buffers.as_slice().contains(&buffer_id) {
            return Ok(false);
        }
        self.buffers.try_push(buffer_id).map_err(|_| "alloc")?;
        Ok(true)
    }

    /// Adds `window_id` to this session if it is not already present.
    ///
    /// # Errors
    ///
    /// Returns `Err("alloc")` if the window list cannot grow.
    pub fn try_attach_window(&mut self, window_id: WindowId) -> Result<bool, &'static str> {
        if self.windows.as_slice().contains(&window_id) {
            return Ok(false);
        }
        self.windows.try_push(window_id).map_err(|_| "alloc")?;
        Ok(true)
    }

    /// Returns whether this session has `client_id`.
    #[must_use]
    pub fn has_client(&self, client_id: ClientId) -> bool {
        self.clients.as_slice().contains(&client_id)
    }

    /// Returns whether this session has `buffer_id`.
    #[must_use]
    pub fn has_buffer(&self, buffer_id: BufferId) -> bool {
        self.buffers.as_slice().contains(&buffer_id)
    }

    /// Returns whether this session has `window_id`.
    #[must_use]
    pub fn has_window(&self, window_id: WindowId) -> bool {
        self.windows.as_slice().contains(&window_id)
    }

    /// Returns the initialized focus entries.
    ///
    /// ```rust,no_run
    /// // no_run: requires arch allocator runtime.
    /// ```
    #[must_use]
    pub fn focus_entries(&self) -> &[FocusEntry] {
        &self.focus_chain[..self.focus_len]
    }

    /// Pushes a focus entry as the new leaf.
    ///
    /// # Errors
    ///
    /// Returns `Err("focus: max depth")` if the bounded focus chain is full.
    ///
    /// ```rust,no_run
    /// // no_run: requires arch allocator runtime.
    /// ```
    pub fn try_focus_push(&mut self, entry: FocusEntry) -> Result<(), &'static str> {
        if self.focus_len >= MAX_FOCUS_CHAIN_DEPTH {
            return Err("focus: max depth");
        }
        if self
            .focus_entries()
            .last()
            .is_some_and(|entry| entry.is_pending())
        {
            return Err("focus: pending leaf must resolve first");
        }
        let before = self.focus_snapshot()?;
        self.validate_focus_entry(entry)?;
        let after = self.focus_snapshot_with_appended(entry)?;
        let transition = self.prepare_focus_transition(before, after)?;
        self.focus_transitions.try_reserve(1).map_err(|_| "alloc")?;
        if let FocusEntry::Resolved(id) = entry {
            self.increment_focus_ref(id)?;
        }
        self.focus_chain[self.focus_len] = entry;
        self.focus_len += 1;
        self.commit_focus_transition_after_apply(transition)?;
        Ok(())
    }

    /// Pops the leaf focus entry while preserving the root.
    ///
    /// # Errors
    ///
    /// Returns an error if transition publication fails or the focused
    /// attachment refcount would underflow.
    ///
    /// ```rust,no_run
    /// // no_run: requires arch allocator runtime.
    /// ```
    pub fn focus_pop(&mut self) -> Result<Option<FocusEntry>, &'static str> {
        if self.focus_len <= 1 {
            return Ok(None);
        }
        let before = self.focus_snapshot()?;
        let after = self.focus_snapshot_without_leaf()?;
        let transition = self.prepare_focus_transition(before, after)?;
        self.focus_transitions.try_reserve(1).map_err(|_| "alloc")?;
        let popped = self.focus_chain[self.focus_len - 1];
        if let FocusEntry::Resolved(id) = popped {
            self.decrement_focus_ref(id)?;
        }
        self.focus_len -= 1;
        self.commit_focus_transition_after_apply(transition)?;
        Ok(Some(popped))
    }

    /// Returns a live attachment by id.
    #[must_use]
    pub fn attachment(&self, id: DomainAttachmentId) -> Option<&DomainAttachment> {
        if id == self.root_attachment {
            Some(&self.root_attachment_record)
        } else {
            self.attachments.get(&id)
        }
    }

    /// Attaches a new root Domain in stable root append order.
    ///
    /// # Errors
    ///
    /// Returns an error if attachment allocation, root-list growth, or ordinal
    /// allocation fails.
    pub fn try_attach_root(
        &mut self,
        domain_id: DomainId,
        scope: DomainScope,
    ) -> Result<DomainAttachmentId, &'static str> {
        self.attach_root_with_focus_refs(domain_id, scope, 0)
    }

    /// Attaches a new child Domain below `parent` in stable child append order.
    ///
    /// # Errors
    ///
    /// Returns an error if `parent` is missing, child-list growth fails, or
    /// attachment allocation fails.
    pub fn try_attach_child(
        &mut self,
        parent: DomainAttachmentId,
        domain_id: DomainId,
        scope: DomainScope,
    ) -> Result<DomainAttachmentId, &'static str> {
        self.attach_child_with_focus_refs(parent, domain_id, scope, 0)
    }

    /// Creates a pending child attachment context without changing focus.
    ///
    /// # Errors
    ///
    /// Returns an error if `parent` is missing, replay-queue allocation fails,
    /// or pending-record allocation fails.
    pub fn try_create_pending_child(
        &mut self,
        parent: DomainAttachmentId,
        domain_id: DomainId,
        scope: DomainScope,
        bootstrap_payload: Bytes,
    ) -> Result<PendingAttachmentId, &'static str> {
        if self.attachment(parent).is_none() {
            return Err("tree: missing parent attachment");
        }
        let pending_id = self.alloc_pending_id()?;
        let replay_queue_id = self.alloc_replay_queue_id()?;
        self.replay_queues
            .try_insert(replay_queue_id, Seq::new())
            .map_err(|_| "alloc")?;
        let pending = PendingAttachment::new(
            pending_id,
            domain_id,
            Some(parent),
            scope,
            self.buffer_id,
            self.client_id,
            self.window_id,
            bootstrap_payload,
            replay_queue_id,
        );
        if self
            .pending_attachments
            .try_insert(pending_id, pending)
            .map_err(|_| "alloc")?
            .is_some()
        {
            self.replay_queues.remove(&replay_queue_id);
            return Err("tree: duplicate pending attachment");
        }
        Ok(pending_id)
    }

    /// Creates a pending child and pushes it as the focus leaf.
    ///
    /// # Errors
    ///
    /// Returns an error from pending creation or focus push; on focus-push
    /// failure the pending record and replay queue are removed.
    pub fn try_focus_pending_child(
        &mut self,
        parent: DomainAttachmentId,
        domain_id: DomainId,
        scope: DomainScope,
        bootstrap_payload: Bytes,
    ) -> Result<PendingAttachmentId, &'static str> {
        let pending_id =
            self.try_create_pending_child(parent, domain_id, scope, bootstrap_payload)?;
        if let Err(err) = self.try_focus_push(FocusEntry::Pending(pending_id)) {
            if let Some(pending) = self.pending_attachments.remove(&pending_id) {
                self.replay_queues.remove(&pending.replay_queue_id);
            }
            return Err(err);
        }
        Ok(pending_id)
    }

    /// Resolves a pending focus leaf into a live attachment.
    ///
    /// # Errors
    ///
    /// Returns an error if `pending_id` is not the leaf, the pending record is
    /// missing, attachment allocation fails, or transition publication fails.
    pub fn resolve_pending_leaf(
        &mut self,
        pending_id: PendingAttachmentId,
    ) -> Result<(DomainAttachmentId, ReplayQueueId), &'static str> {
        if self.focus_len == 0
            || self.focus_chain[self.focus_len - 1] != FocusEntry::Pending(pending_id)
        {
            return Err("tree: pending attachment is not focus leaf");
        }
        let (pending_domain_id, pending_parent, replay_queue_id, attachment_scope) = {
            let pending = self
                .pending_attachments
                .get(&pending_id)
                .ok_or("tree: missing pending attachment")?;
            (
                pending.domain_id,
                pending.parent,
                pending.replay_queue_id,
                clone_domain_scope(&pending.scope)?,
            )
        };
        let before = self.focus_snapshot()?;
        let attachment_id = self.alloc_attachment_id()?;
        let after =
            self.focus_snapshot_replacing_leaf(pending_id, attachment_id, pending_domain_id)?;
        let transition = self.prepare_focus_transition(before, after)?;
        self.focus_transitions.try_reserve(1).map_err(|_| "alloc")?;
        if let Some(parent) = pending_parent {
            self.attach_child_with_id(
                attachment_id,
                parent,
                pending_domain_id,
                attachment_scope,
                1,
            )?;
        } else {
            self.attach_root_with_id(attachment_id, pending_domain_id, attachment_scope, 1)?;
        }
        let pending = self
            .pending_attachments
            .remove(&pending_id)
            .ok_or("tree: missing pending attachment")?;
        debug_assert_eq!(pending.replay_queue_id, replay_queue_id);
        self.focus_chain[self.focus_len - 1] = FocusEntry::Resolved(attachment_id);
        self.commit_focus_transition_after_apply(transition)?;
        Ok((attachment_id, replay_queue_id))
    }

    /// Returns the pending replay queue length.
    #[must_use]
    pub fn replay_queue_len(&self, replay_queue_id: ReplayQueueId) -> Option<usize> {
        self.replay_queues.get(&replay_queue_id).map(Seq::len)
    }

    /// Returns the most recently published focus transition.
    #[must_use]
    pub fn last_focus_transition(&self) -> Option<&FocusTransition> {
        self.focus_transitions
            .get(self.focus_transitions.len().saturating_sub(1))
    }

    fn dispatch_domains(
        &mut self,
        input: &[u8],
    ) -> Result<([DomainId; MAX_FOCUS_CHAIN_DEPTH], usize, DispatchVerdict), &'static str> {
        if self.focus_len == 0 {
            return Err("dispatch: empty focus chain");
        }
        if let FocusEntry::Pending(pending_id) = self.focus_chain[self.focus_len - 1] {
            self.queue_pending_input(pending_id, input)?;
            return Ok((
                [self.root_domain_id; MAX_FOCUS_CHAIN_DEPTH],
                0,
                DispatchVerdict::QueuedPending(pending_id),
            ));
        }
        let mut domains = [self.root_domain_id; MAX_FOCUS_CHAIN_DEPTH];
        let mut count = 0usize;
        let mut idx = self.focus_len;
        while idx > 0 {
            idx -= 1;
            match self.focus_chain[idx] {
                FocusEntry::Resolved(id) => {
                    domains[count] = self
                        .attachment_domain(id)
                        .ok_or("dispatch: missing attachment")?;
                    count += 1;
                }
                FocusEntry::Pending(_) => return Err("dispatch: pending entry below leaf"),
            }
        }
        Ok((domains, count, DispatchVerdict::Ignored))
    }

    fn alloc_attachment_id(&mut self) -> Result<DomainAttachmentId, &'static str> {
        let raw = self.next_attachment_id;
        self.next_attachment_id = self
            .next_attachment_id
            .checked_add(1)
            .ok_or("tree: attachment id overflow")?;
        Ok(DomainAttachmentId::new(raw))
    }

    fn alloc_pending_id(&mut self) -> Result<PendingAttachmentId, &'static str> {
        let raw = self.next_pending_attachment_id;
        self.next_pending_attachment_id = self
            .next_pending_attachment_id
            .checked_add(1)
            .ok_or("tree: pending id overflow")?;
        Ok(PendingAttachmentId::new(raw))
    }

    fn alloc_replay_queue_id(&mut self) -> Result<ReplayQueueId, &'static str> {
        let raw = self.next_replay_queue_id;
        self.next_replay_queue_id = self
            .next_replay_queue_id
            .checked_add(1)
            .ok_or("tree: replay queue id overflow")?;
        Ok(ReplayQueueId::new(raw))
    }

    fn attachment_mut(&mut self, id: DomainAttachmentId) -> Option<&mut DomainAttachment> {
        if id == self.root_attachment {
            Some(&mut self.root_attachment_record)
        } else {
            self.attachments.get_mut(&id)
        }
    }

    fn attachment_domain(&self, id: DomainAttachmentId) -> Option<DomainId> {
        self.attachment(id).map(|attachment| attachment.domain_id)
    }

    fn attach_root_with_focus_refs(
        &mut self,
        domain_id: DomainId,
        scope: DomainScope,
        focus_refs: u32,
    ) -> Result<DomainAttachmentId, &'static str> {
        let id = self.alloc_attachment_id()?;
        self.attach_root_with_id(id, domain_id, scope, focus_refs)?;
        Ok(id)
    }

    fn attach_root_with_id(
        &mut self,
        id: DomainAttachmentId,
        domain_id: DomainId,
        scope: DomainScope,
        focus_refs: u32,
    ) -> Result<(), &'static str> {
        let ordinal = u32::try_from(self.root_attachments.len() + 1)
            .map_err(|_| "tree: root ordinal overflow")?;
        self.root_attachments.try_reserve(1).map_err(|_| "alloc")?;
        let attachment = DomainAttachment::new(
            id,
            self.buffer_id,
            domain_id,
            scope,
            None,
            ordinal,
            1,
            focus_refs,
        );
        self.attachments
            .try_insert(id, attachment)
            .map_err(|_| "alloc")?;
        self.root_attachments.try_push(id).map_err(|_| "alloc")?;
        Ok(())
    }

    fn attach_child_with_focus_refs(
        &mut self,
        parent: DomainAttachmentId,
        domain_id: DomainId,
        scope: DomainScope,
        focus_refs: u32,
    ) -> Result<DomainAttachmentId, &'static str> {
        let id = self.alloc_attachment_id()?;
        self.attach_child_with_id(id, parent, domain_id, scope, focus_refs)?;
        Ok(id)
    }

    fn attach_child_with_id(
        &mut self,
        id: DomainAttachmentId,
        parent: DomainAttachmentId,
        domain_id: DomainId,
        scope: DomainScope,
        focus_refs: u32,
    ) -> Result<(), &'static str> {
        let child_ordinal = {
            let parent_attachment = self
                .attachment(parent)
                .ok_or("tree: missing parent attachment")?;
            u32::try_from(parent_attachment.children.len())
                .map_err(|_| "tree: child ordinal overflow")?
        };
        {
            let parent_attachment = self
                .attachment_mut(parent)
                .ok_or("tree: missing parent attachment")?;
            parent_attachment
                .children
                .try_reserve(1)
                .map_err(|_| "alloc")?;
        }
        let attachment = DomainAttachment::new(
            id,
            self.buffer_id,
            domain_id,
            scope,
            Some(parent),
            child_ordinal,
            1,
            focus_refs,
        );
        self.attachments
            .try_insert(id, attachment)
            .map_err(|_| "alloc")?;
        let parent_attachment = self
            .attachment_mut(parent)
            .ok_or("tree: missing parent attachment")?;
        parent_attachment.try_push_child(id).map_err(|_| "alloc")?;
        Ok(())
    }

    fn validate_focus_entry(&self, entry: FocusEntry) -> Result<(), &'static str> {
        match entry {
            FocusEntry::Resolved(id) => self
                .attachment(id)
                .map(|_| ())
                .ok_or("focus: unknown attachment"),
            FocusEntry::Pending(id) => self
                .pending_attachments
                .get(&id)
                .map(|_| ())
                .ok_or("focus: unknown pending attachment"),
        }
    }

    fn increment_focus_ref(&mut self, id: DomainAttachmentId) -> Result<(), &'static str> {
        let attachment = self.attachment_mut(id).ok_or("focus: unknown attachment")?;
        attachment.focus_refs = attachment
            .focus_refs
            .checked_add(1)
            .ok_or("focus: ref overflow")?;
        Ok(())
    }

    fn decrement_focus_ref(&mut self, id: DomainAttachmentId) -> Result<(), &'static str> {
        let attachment = self.attachment_mut(id).ok_or("focus: unknown attachment")?;
        attachment.focus_refs = attachment
            .focus_refs
            .checked_sub(1)
            .ok_or("focus: ref underflow")?;
        Ok(())
    }

    fn queue_pending_input(
        &mut self,
        pending_id: PendingAttachmentId,
        input: &[u8],
    ) -> Result<(), &'static str> {
        let replay_queue_id = self
            .pending_attachments
            .get(&pending_id)
            .ok_or("dispatch: missing pending attachment")?
            .replay_queue_id;
        let mut queued = Bytes::new();
        queued.try_extend_from_slice(input).map_err(|_| "alloc")?;
        self.replay_queues
            .get_mut(&replay_queue_id)
            .ok_or("dispatch: missing replay queue")?
            .try_push(queued)
            .map_err(|_| "alloc")
    }

    fn focus_snapshot(&self) -> Result<Seq<FocusEntrySnapshot>, &'static str> {
        let mut snapshot = Seq::new();
        for entry in self.focus_entries() {
            snapshot
                .try_push(self.snapshot_entry(*entry)?)
                .map_err(|_| "alloc")?;
        }
        Ok(snapshot)
    }

    fn focus_snapshot_with_appended(
        &self,
        entry: FocusEntry,
    ) -> Result<Seq<FocusEntrySnapshot>, &'static str> {
        let mut snapshot = self.focus_snapshot()?;
        snapshot
            .try_push(self.snapshot_entry(entry)?)
            .map_err(|_| "alloc")?;
        Ok(snapshot)
    }

    fn focus_snapshot_without_leaf(&self) -> Result<Seq<FocusEntrySnapshot>, &'static str> {
        let mut snapshot = Seq::new();
        for entry in &self.focus_chain[..self.focus_len - 1] {
            snapshot
                .try_push(self.snapshot_entry(*entry)?)
                .map_err(|_| "alloc")?;
        }
        Ok(snapshot)
    }

    fn focus_snapshot_replacing_leaf(
        &self,
        pending_id: PendingAttachmentId,
        attachment_id: DomainAttachmentId,
        domain_id: DomainId,
    ) -> Result<Seq<FocusEntrySnapshot>, &'static str> {
        let mut snapshot = Seq::new();
        let last = self.focus_len - 1;
        for (idx, entry) in self.focus_entries().iter().enumerate() {
            if idx == last {
                if *entry != FocusEntry::Pending(pending_id) {
                    return Err("tree: pending attachment is not focus leaf");
                }
                snapshot
                    .try_push(FocusEntrySnapshot::Resolved(attachment_id, domain_id))
                    .map_err(|_| "alloc")?;
            } else {
                snapshot
                    .try_push(self.snapshot_entry(*entry)?)
                    .map_err(|_| "alloc")?;
            }
        }
        Ok(snapshot)
    }

    fn snapshot_entry(&self, entry: FocusEntry) -> Result<FocusEntrySnapshot, &'static str> {
        match entry {
            FocusEntry::Resolved(id) => Ok(FocusEntrySnapshot::Resolved(
                id,
                self.attachment_domain(id)
                    .ok_or("focus: unknown attachment")?,
            )),
            FocusEntry::Pending(id) => Ok(FocusEntrySnapshot::Pending(
                id,
                self.pending_attachments
                    .get(&id)
                    .ok_or("focus: unknown pending attachment")?
                    .domain_id,
            )),
        }
    }

    fn prepare_focus_transition(
        &self,
        before: Seq<FocusEntrySnapshot>,
        after: Seq<FocusEntrySnapshot>,
    ) -> Result<FocusTransition, &'static str> {
        self.next_focus_transition_seq
            .checked_add(1)
            .ok_or("focus: transition seq overflow")?;
        let mut transition = FocusTransition::new(
            self.session_id,
            self.client_id,
            self.buffer_id,
            self.window_id,
            self.next_focus_transition_seq,
        );
        transition.before = before;
        transition.after = after;
        Ok(transition)
    }

    fn commit_focus_transition_after_apply(
        &mut self,
        transition: FocusTransition,
    ) -> Result<(), &'static str> {
        self.next_focus_transition_seq = self
            .next_focus_transition_seq
            .checked_add(1)
            .ok_or("focus: transition seq overflow")?;
        self.focus_transitions
            .try_push(transition)
            .map_err(|_| "alloc")
    }

    /// Adds this state's primary client, buffer, and window to its membership
    /// lists.
    ///
    /// # Errors
    ///
    /// Returns `Err("alloc")` if any membership list cannot grow.
    pub fn ensure_default_membership(&mut self) -> Result<(), &'static str> {
        self.try_attach_client(self.client_id)?;
        self.try_attach_buffer(self.buffer_id)?;
        self.try_attach_window(self.window_id)?;
        Ok(())
    }
}

fn clone_bytes(bytes: &Bytes) -> Result<Bytes, &'static str> {
    Bytes::try_from_slice(bytes.as_slice()).map_err(|_| "alloc")
}

fn clone_position_carrier(carrier: &PositionCarrier) -> Result<PositionCarrier, &'static str> {
    Ok(PositionCarrier::new(carrier.header, clone_bytes(&carrier.content)?))
}

fn clone_domain_scope(scope: &DomainScope) -> Result<DomainScope, &'static str> {
    Ok(DomainScope::new(
        clone_position_carrier(&scope.start)?,
        clone_position_carrier(&scope.end)?,
        scope.flags,
    ))
}

// ── SessionTable ─────────────────────────────────────────────────────────────

/// Kernel-owned table of live sessions for Phase 3 fixtures.
///
/// The legacy [`Kernel::session`](crate::kernel::Kernel::session) field remains
/// for the walking-skeleton server path. New Phase 3 code should route through
/// this table so multiple sessions/clients/buffers/windows are represented by
/// real records instead of a singleton placeholder.
pub struct SessionTable {
    sessions: Map<SessionId, Shared<Session>>,
    next_session_id: u32,
    next_buffer_id: u32,
    next_window_id: u32,
    next_client_id: u32,
}

impl SessionTable {
    /// Creates an empty session table.
    ///
    /// ```rust,no_run
    /// // no_run: requires arch allocator runtime.
    /// ```
    #[must_use]
    pub const fn new() -> Self {
        Self {
            sessions: Map::new(),
            next_session_id: 1,
            next_buffer_id: 1,
            next_window_id: 1,
            next_client_id: 1,
        }
    }

    /// Returns the number of live sessions.
    #[must_use]
    pub const fn len(&self) -> usize {
        self.sessions.len()
    }

    /// Returns whether the table is empty.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.sessions.is_empty()
    }

    /// Inserts an existing shared session record.
    ///
    /// # Errors
    ///
    /// Returns an error on duplicate id, allocation failure, or id counter
    /// overflow while preserving monotonic future allocation.
    pub fn insert_shared(&mut self, session: Shared<Session>) -> Result<(), &'static str> {
        if self.sessions.get(&session.id).is_some() {
            return Err("session-table: duplicate session");
        }
        let next = session
            .id
            .as_u32()
            .checked_add(1)
            .ok_or("session-table: id overflow")?;
        if next > self.next_session_id {
            self.next_session_id = next;
        }
        self.sessions
            .try_insert(session.id, session)
            .map_err(|_| "alloc")?;
        Ok(())
    }

    /// Creates and inserts a new session with fresh client/buffer/window ids.
    ///
    /// # Errors
    ///
    /// Returns an error if id allocation or table growth fails.
    pub fn create_session(
        &mut self,
        root_domain_id: DomainId,
    ) -> Result<Shared<Session>, &'static str> {
        let session_id = self.alloc_session_id()?;
        let buffer_id = self.alloc_buffer_id()?;
        let window_id = self.alloc_window_id()?;
        let client_id = self.alloc_client_id()?;
        let mut state = SessionState::new_with_client(
            root_domain_id,
            DomainAttachmentId::new(1),
            buffer_id,
            window_id,
            client_id,
        );
        state.ensure_default_membership()?;
        let session = Shared::try_new(Session::new(session_id, state)).map_err(|_| "alloc")?;
        self.sessions
            .try_insert(session_id, Shared::clone(&session))
            .map_err(|_| "alloc")?;
        Ok(session)
    }

    /// Returns a cloned shared handle for `id`, if present.
    #[must_use]
    pub fn get(&self, id: SessionId) -> Option<Shared<Session>> {
        self.sessions.get(&id).cloned()
    }

    /// Removes `id` from the table.
    pub fn remove(&mut self, id: SessionId) -> Option<Shared<Session>> {
        self.sessions.remove(&id)
    }

    fn alloc_session_id(&mut self) -> Result<SessionId, &'static str> {
        let raw = self.next_session_id;
        self.next_session_id = self
            .next_session_id
            .checked_add(1)
            .ok_or("session-table: id overflow")?;
        Ok(SessionId::new(raw))
    }

    fn alloc_buffer_id(&mut self) -> Result<BufferId, &'static str> {
        let raw = self.next_buffer_id;
        self.next_buffer_id = self
            .next_buffer_id
            .checked_add(1)
            .ok_or("session-table: buffer id overflow")?;
        Ok(BufferId::new(raw))
    }

    fn alloc_window_id(&mut self) -> Result<WindowId, &'static str> {
        let raw = self.next_window_id;
        self.next_window_id = self
            .next_window_id
            .checked_add(1)
            .ok_or("session-table: window id overflow")?;
        Ok(WindowId::new(raw))
    }

    fn alloc_client_id(&mut self) -> Result<ClientId, &'static str> {
        let raw = self.next_client_id;
        self.next_client_id = self
            .next_client_id
            .checked_add(1)
            .ok_or("session-table: client id overflow")?;
        Ok(ClientId::new(raw))
    }
}

impl Default for SessionTable {
    fn default() -> Self {
        Self::new()
    }
}

// ── Dispatch snapshots ──────────────────────────────────────────────────────

/// Session-side dispatch snapshot prepared while holding `Session.state`.
pub struct DispatchSnapshot {
    /// Domains to try in leaf-to-root order.
    pub domains: [DomainId; MAX_FOCUS_CHAIN_DEPTH],
    /// Initialized prefix of [`Self::domains`].
    pub domain_count: usize,
    /// Buffer snapshot for handlers.
    pub buffer: Bytes,
    /// Cursor snapshot for handlers.
    pub cursor: usize,
}

/// One resolved handler row copied out of the router before invocation.
#[derive(Clone, Copy)]
pub struct ResolvedHandler<'a> {
    /// Domain this handler belongs to.
    pub domain_id: DomainId,
    /// Callable slot.
    pub handler: &'a dyn OnRawInputHandler,
}

/// One resolved projector row copied out of the router before invocation.
#[derive(Clone, Copy)]
pub struct ResolvedProjector<'a> {
    /// Domain this projector belongs to.
    pub domain_id: DomainId,
    /// Callable slot.
    pub projector: &'a dyn RenderProjector,
}

/// Handler/projector slots resolved before dispatch invokes cdylib code.
pub struct DispatchRoutes<'a> {
    handlers: [Option<ResolvedHandler<'a>>; MAX_FOCUS_CHAIN_DEPTH],
    handler_count: usize,
    projectors: [Option<ResolvedProjector<'a>>; MAX_FOCUS_CHAIN_DEPTH],
    projector_count: usize,
}

impl DispatchRoutes<'static> {
    /// Resolves routes for `snapshot` from a borrowed router.
    ///
    /// Callers that borrowed a router lock must drop it after this function and
    /// before invoking [`Session::dispatch_prepared`].
    #[must_use]
    pub fn from_router(snapshot: &DispatchSnapshot, router: &DomainRouter) -> Self {
        let mut resolved = Self::new();
        for domain_id in &snapshot.domains[..snapshot.domain_count] {
            if let Some(handler) = router.handler(*domain_id)
                && resolved.try_push_handler(*domain_id, handler).is_err()
            {
                break;
            }
            if let Some(projector) = router.projector(*domain_id)
                && resolved.try_push_projector(*domain_id, projector).is_err()
            {
                break;
            }
        }
        resolved
    }
}

impl<'a> DispatchRoutes<'a> {
    /// Creates an empty route snapshot.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            handlers: [None; MAX_FOCUS_CHAIN_DEPTH],
            handler_count: 0,
            projectors: [None; MAX_FOCUS_CHAIN_DEPTH],
            projector_count: 0,
        }
    }

    /// Appends one resolved handler.
    ///
    /// # Errors
    ///
    /// Returns `Err("dispatch: too many handlers")` when the bounded focus
    /// route array is full.
    pub fn try_push_handler(
        &mut self,
        domain_id: DomainId,
        handler: &'a dyn OnRawInputHandler,
    ) -> Result<(), &'static str> {
        if self.handler_count >= MAX_FOCUS_CHAIN_DEPTH {
            return Err("dispatch: too many handlers");
        }
        self.handlers[self.handler_count] = Some(ResolvedHandler { domain_id, handler });
        self.handler_count += 1;
        Ok(())
    }

    /// Appends one resolved projector.
    ///
    /// # Errors
    ///
    /// Returns `Err("dispatch: too many projectors")` when the bounded focus
    /// route array is full.
    pub fn try_push_projector(
        &mut self,
        domain_id: DomainId,
        projector: &'a dyn RenderProjector,
    ) -> Result<(), &'static str> {
        if self.projector_count >= MAX_FOCUS_CHAIN_DEPTH {
            return Err("dispatch: too many projectors");
        }
        self.projectors[self.projector_count] = Some(ResolvedProjector {
            domain_id,
            projector,
        });
        self.projector_count += 1;
        Ok(())
    }

    fn handler_for(&self, domain_id: DomainId) -> Option<&'a dyn OnRawInputHandler> {
        self.handlers[..self.handler_count]
            .iter()
            .flatten()
            .find(|row| row.domain_id == domain_id)
            .map(|row| row.handler)
    }

    fn projector_for(&self, domain_id: DomainId) -> Option<&'a dyn RenderProjector> {
        self.projectors[..self.projector_count]
            .iter()
            .flatten()
            .find(|row| row.domain_id == domain_id)
            .map(|row| row.projector)
    }
}

impl Default for DispatchRoutes<'_> {
    fn default() -> Self {
        Self::new()
    }
}

// ── Session ───────────────────────────────────────────────────────────────────

/// A single editing session (walking-skeleton: one session per kernel).
///
/// The `Mutex<SessionState>` is the turn gate (CC14): dispatch snapshots state
/// under the lock, drops it, invokes the handler, then re-locks to apply
/// mutations. The lock is NEVER held across a Domain handler call.
pub struct Session {
    /// Per-session mutable state, serialised by `Mutex` (CC14).
    pub state: Mutex<SessionState>,
    /// This session's identifier.
    pub id: SessionId,
}

impl Session {
    /// Constructs a new `Session` with the given initial state.
    ///
    /// ```rust,no_run
    /// // no_run: requires arch allocator + futex runtime.
    /// use reovim_kernel::{
    ///     router::DomainId,
    ///     session::{BufferId, DomainAttachmentId, Session, SessionId, SessionState, WindowId},
    /// };
    /// use core::num::NonZeroU32;
    ///
    /// let domain_id = DomainId::new(NonZeroU32::new(1).unwrap());
    /// let state = SessionState::new(
    ///     domain_id,
    ///     DomainAttachmentId::new(1),
    ///     BufferId::new(1),
    ///     WindowId::new(1),
    /// );
    /// let session = Session::new(SessionId::new(1), state);
    /// assert_eq!(session.id.as_u32(), 1);
    /// ```
    #[must_use]
    pub const fn new(id: SessionId, mut state: SessionState) -> Self {
        state.set_session_id(id);
        Self {
            state: Mutex::new(state),
            id,
        }
    }

    /// Dispatches a raw-input byte sequence through the router's `OnRawInput`
    /// handler, then calls the `Render` projector to produce a `Projection`.
    ///
    /// Follows the CC14 dispatch shape:
    /// 1. Lock state, snapshot what the handler needs (buffer + cursor), unlock.
    /// 2. Invoke the handler (lock not held — CC14).
    /// 3. Re-lock and apply the handler's mutation.
    /// 4. Lock state, snapshot what the projector needs, unlock.
    /// 5. Invoke the projector (lock not held — CC14).
    ///
    /// # Errors
    ///
    /// Returns `Err(&'static str)` if the router has no handler or projector
    /// registered for the session's root Domain.
    ///
    /// ```rust,no_run
    /// // no_run: requires arch allocator + futex runtime and a registered domain.
    /// ```
    pub fn dispatch_input(
        &self,
        input: &[u8],
        router: &DomainRouter,
    ) -> Result<Projection, &'static str> {
        let snapshot = self.prepare_dispatch(input)?;
        let resolved = DispatchRoutes::from_router(&snapshot, router);
        self.dispatch_prepared(&snapshot, input, &resolved)
    }

    /// Prepares a dispatch snapshot while holding `Session.state`.
    ///
    /// The returned snapshot owns all bytes needed by the cdylib slot
    /// invocation, so no session lock must be held after this returns.
    ///
    /// # Errors
    ///
    /// Returns dispatch-shape errors for empty focus chains or queued pending
    /// leaves, and allocation errors while cloning the buffer snapshot.
    pub fn prepare_dispatch(&self, input: &[u8]) -> Result<DispatchSnapshot, &'static str> {
        // ── Step 1: snapshot under lock ──────────────────────────────────────
        let (domains, domain_count, buffer, cursor) = {
            let mut guard = self.state.lock();
            let (domains, count, verdict) = guard.dispatch_domains(input)?;
            if matches!(verdict, DispatchVerdict::QueuedPending(_)) {
                return Err("dispatch: pending attachment replay queued");
            }
            let mut snap = Bytes::new();
            snap.try_extend_from_slice(guard.buffer.as_slice())
                .map_err(|_| "alloc: buffer snapshot failed")?;
            (domains, count, snap, guard.cursor)
        };
        // Lock released here — CC14 invariant: not held across handler call.

        if domain_count == 0 {
            return Err("dispatch: empty focus chain");
        }
        Ok(DispatchSnapshot {
            domains,
            domain_count,
            buffer,
            cursor,
        })
    }

    /// Dispatches using routes that were resolved before cdylib invocation.
    ///
    /// This is the CC14-safe path used by `Kernel`: router/session locks are
    /// dropped before handler and projector slots are invoked.
    ///
    /// # Errors
    ///
    /// Returns allocation errors while cloning handler/projector snapshots or
    /// an error when no resolved projector exists for the projection Domain.
    pub fn dispatch_prepared(
        &self,
        snapshot: &DispatchSnapshot,
        input: &[u8],
        routes: &DispatchRoutes<'_>,
    ) -> Result<Projection, &'static str> {
        // ── Step 2: invoke OnRawInput handlers leaf-to-root (no lock held) ───
        let mut claimed: Option<(DomainId, Bytes, usize)> = None;
        for domain_id in &snapshot.domains[..snapshot.domain_count] {
            let Some(handler) = routes.handler_for(*domain_id) else {
                continue;
            };
            let mut handler_buffer = Bytes::new();
            handler_buffer
                .try_extend_from_slice(snapshot.buffer.as_slice())
                .map_err(|_| "alloc: handler snapshot failed")?;
            let result = handler.on_raw_input(handler_buffer, snapshot.cursor, input);
            if result.claimed {
                claimed = Some((*domain_id, result.buffer, result.cursor));
                break;
            }
        }

        // ── Step 3: re-lock and apply mutations ──────────────────────────────
        if let Some((_, new_buffer, new_cursor)) = &claimed {
            let mut guard = self.state.lock();
            // Replace buffer bytes with the handler's output.
            let buf = &mut guard.buffer;
            // Truncate to zero and extend with new content.
            // `Bytes` does not have a `clear`; we truncate via pop loop.
            // Use the arch `Bytes::truncate` pattern by building a fresh `Bytes`.
            let mut new_buf = Bytes::new();
            new_buf
                .try_extend_from_slice(new_buffer.as_slice())
                .map_err(|_| "alloc: apply buffer failed")?;
            *buf = new_buf;
            guard.cursor = *new_cursor;
        }

        // ── Step 4: snapshot for projector (under lock) ──────────────────────
        let (projection_domain_id, proj_buffer, proj_cursor, buffer_id, window_id) = {
            let guard = self.state.lock();
            let mut snap = Bytes::new();
            snap.try_extend_from_slice(guard.buffer.as_slice())
                .map_err(|_| "alloc: projector snapshot failed")?;
            let domain_id = claimed
                .as_ref()
                .map_or(snapshot.domains[snapshot.domain_count - 1], |(id, _, _)| *id);
            (domain_id, snap, guard.cursor, guard.buffer_id, guard.window_id)
        };

        // ── Step 5: invoke Render projector (no lock held) ───────────────────
        let projector = routes
            .projector_for(projection_domain_id)
            .ok_or("dispatch: no Render projector for domain")?;
        let projection = projector.render(proj_buffer, proj_cursor, buffer_id, window_id)?;

        Ok(projection)
    }
}

// `Session` contains `Mutex<SessionState>` which is `Send + Sync` because:
// - `SessionState` holds `Bytes` (owns a raw allocation, `Send` but not `Sync`;
//   `Mutex<T>` requires only `T: Send` — `Bytes` is `Send`).
// - `Mutex` uses an `AtomicU32` lock word (futex), thread-safe by construction.
// - `SessionId` is `Copy` plain-integer data.
// The compiler auto-derives `Send + Sync` from the field types.

// L12 layout: tests in sibling session_tests.rs, declared in lib.rs.
