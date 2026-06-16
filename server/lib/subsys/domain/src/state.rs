//! Session-state substrate contracts (§3.2, §3.4, §4.3, #798).
//!
//! The kernel owns maps, locks, and `HostApi` entry points. This module keeps the
//! shared value vocabulary for view slots, registers, edit origins, and
//! byte-range undo groups in the Domain contract tier.

use reovim_arch::ds::{Bytes, Seq};

use crate::id::{BufferId, CdylibId, ClientId, RegisterId, SlotKindId, WindowId};

/// v0.16 view-slot scope vocabulary (§3.2 §6).
///
/// ```rust
/// use reovim_subsys_domain::state::ViewSlotScope;
///
/// assert_eq!(ViewSlotScope::Window.name(), "window");
/// assert_eq!(ViewSlotScope::Buffer.name(), "buffer");
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ViewSlotScope {
    /// Per `(client, buffer, window, kind)` state.
    Window,
    /// Per `(buffer, kind)` state shared by clients/windows.
    Buffer,
}

impl ViewSlotScope {
    /// Stable manifest spelling for this scope.
    ///
    /// ```rust
    /// use reovim_subsys_domain::state::ViewSlotScope;
    ///
    /// assert_eq!(ViewSlotScope::Window.name(), "window");
    /// ```
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Window => "window",
            Self::Buffer => "buffer",
        }
    }
}

/// View-slot lookup key (§3.2 §5/§6).
///
/// ```rust
/// use reovim_subsys_domain::{
///     id::{BufferId, ClientId, SlotKindId, WindowId},
///     state::{ViewSlotKey, ViewSlotScope},
/// };
///
/// let key = ViewSlotKey::window(ClientId::new(1), BufferId::new(2), WindowId::new(3), SlotKindId::new(4));
/// assert_eq!(key.scope(), ViewSlotScope::Window);
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ViewSlotKey {
    /// Window-scoped slot key.
    Window {
        /// Client component.
        client_id: ClientId,
        /// Buffer component.
        buffer_id: BufferId,
        /// Window component.
        window_id: WindowId,
        /// Slot kind component.
        kind_id: SlotKindId,
    },
    /// Buffer-scoped slot key.
    Buffer {
        /// Buffer component.
        buffer_id: BufferId,
        /// Slot kind component.
        kind_id: SlotKindId,
    },
}

impl ViewSlotKey {
    /// Builds a window-scoped slot key.
    ///
    /// ```rust
    /// use reovim_subsys_domain::{
    ///     id::{BufferId, ClientId, SlotKindId, WindowId},
    ///     state::ViewSlotKey,
    /// };
    ///
    /// let key = ViewSlotKey::window(ClientId::new(1), BufferId::new(2), WindowId::new(3), SlotKindId::new(4));
    /// assert_eq!(key.kind_id(), SlotKindId::new(4));
    /// ```
    #[must_use]
    pub const fn window(
        client_id: ClientId,
        buffer_id: BufferId,
        window_id: WindowId,
        kind_id: SlotKindId,
    ) -> Self {
        Self::Window {
            client_id,
            buffer_id,
            window_id,
            kind_id,
        }
    }

    /// Builds a buffer-scoped slot key.
    ///
    /// ```rust
    /// use reovim_subsys_domain::{id::{BufferId, SlotKindId}, state::ViewSlotKey};
    ///
    /// let key = ViewSlotKey::buffer(BufferId::new(2), SlotKindId::new(4));
    /// assert_eq!(key.buffer_id(), BufferId::new(2));
    /// ```
    #[must_use]
    pub const fn buffer(buffer_id: BufferId, kind_id: SlotKindId) -> Self {
        Self::Buffer { buffer_id, kind_id }
    }

    /// Returns the slot scope.
    ///
    /// ```rust
    /// use reovim_subsys_domain::{id::{BufferId, SlotKindId}, state::{ViewSlotKey, ViewSlotScope}};
    ///
    /// assert_eq!(
    ///     ViewSlotKey::buffer(BufferId::new(1), SlotKindId::new(2)).scope(),
    ///     ViewSlotScope::Buffer,
    /// );
    /// ```
    #[must_use]
    pub const fn scope(self) -> ViewSlotScope {
        match self {
            Self::Window { .. } => ViewSlotScope::Window,
            Self::Buffer { .. } => ViewSlotScope::Buffer,
        }
    }

    /// Returns the buffer component.
    ///
    /// ```rust
    /// use reovim_subsys_domain::{id::{BufferId, SlotKindId}, state::ViewSlotKey};
    ///
    /// assert_eq!(
    ///     ViewSlotKey::buffer(BufferId::new(9), SlotKindId::new(2)).buffer_id(),
    ///     BufferId::new(9),
    /// );
    /// ```
    #[must_use]
    pub const fn buffer_id(self) -> BufferId {
        match self {
            Self::Window { buffer_id, .. } | Self::Buffer { buffer_id, .. } => buffer_id,
        }
    }

    /// Returns the slot-kind component.
    ///
    /// ```rust
    /// use reovim_subsys_domain::{id::{BufferId, SlotKindId}, state::ViewSlotKey};
    ///
    /// assert_eq!(
    ///     ViewSlotKey::buffer(BufferId::new(1), SlotKindId::new(6)).kind_id(),
    ///     SlotKindId::new(6),
    /// );
    /// ```
    #[must_use]
    pub const fn kind_id(self) -> SlotKindId {
        match self {
            Self::Window { kind_id, .. } | Self::Buffer { kind_id, .. } => kind_id,
        }
    }
}

/// Opaque view-slot flags carried in diagnostics and row metadata.
///
/// ```rust
/// use reovim_subsys_domain::state::ViewSlotFlags;
///
/// let flags = ViewSlotFlags::new(ViewSlotFlags::HOSTAPI_REENTRANT);
/// assert!(flags.hostapi_reentrant());
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ViewSlotFlags(u32);

impl ViewSlotFlags {
    /// Slot may call `HostApi` re-entrantly.
    pub const HOSTAPI_REENTRANT: u32 = 0x0001;
    /// Slot bytes may be moved across threads by the owner.
    pub const SEND_SAFE: u32 = 0x0002;
    /// Drop callback may call `HostApi`.
    pub const DROP_MAY_CALL_HOSTAPI: u32 = 0x0004;

    /// Wraps raw flag bits.
    ///
    /// ```rust
    /// use reovim_subsys_domain::state::ViewSlotFlags;
    ///
    /// assert_eq!(ViewSlotFlags::new(3).bits(), 3);
    /// ```
    #[must_use]
    pub const fn new(bits: u32) -> Self {
        Self(bits)
    }

    /// Returns raw flag bits.
    ///
    /// ```rust
    /// use reovim_subsys_domain::state::ViewSlotFlags;
    ///
    /// assert_eq!(ViewSlotFlags::new(5).bits(), 5);
    /// ```
    #[must_use]
    pub const fn bits(self) -> u32 {
        self.0
    }

    /// Returns whether all `mask` bits are present.
    ///
    /// ```rust
    /// use reovim_subsys_domain::state::ViewSlotFlags;
    ///
    /// assert!(ViewSlotFlags::new(ViewSlotFlags::SEND_SAFE).contains(ViewSlotFlags::SEND_SAFE));
    /// ```
    #[must_use]
    pub const fn contains(self, mask: u32) -> bool {
        self.0 & mask == mask
    }

    /// Returns whether the HostApi-reentrant bit is set.
    ///
    /// ```rust
    /// use reovim_subsys_domain::state::ViewSlotFlags;
    ///
    /// assert!(!ViewSlotFlags::new(0).hostapi_reentrant());
    /// ```
    #[must_use]
    pub const fn hostapi_reentrant(self) -> bool {
        self.contains(Self::HOSTAPI_REENTRANT)
    }
}

/// v0.16 register scope vocabulary (§3.4).
///
/// ```rust
/// use reovim_subsys_domain::state::RegisterScope;
///
/// assert_eq!(RegisterScope::Client.lookup_rank(), 0);
/// assert_eq!(RegisterScope::System.name(), "system");
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RegisterScope {
    /// Per-client volatile register.
    Client,
    /// Per-session persisted register.
    Session,
    /// Kernel-global persisted register.
    System,
}

impl RegisterScope {
    /// Stable diagnostic name for this scope.
    ///
    /// ```rust
    /// use reovim_subsys_domain::state::RegisterScope;
    ///
    /// assert_eq!(RegisterScope::Session.name(), "session");
    /// ```
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Client => "client",
            Self::Session => "session",
            Self::System => "system",
        }
    }

    /// Lookup rank; lower ranks win (`client -> session -> system`).
    ///
    /// ```rust
    /// use reovim_subsys_domain::state::RegisterScope;
    ///
    /// assert!(RegisterScope::Client.lookup_rank() < RegisterScope::System.lookup_rank());
    /// ```
    #[must_use]
    pub const fn lookup_rank(self) -> u8 {
        match self {
            Self::Client => 0,
            Self::Session => 1,
            Self::System => 2,
        }
    }
}

/// Register lookup/storage key.
///
/// ```rust
/// use reovim_subsys_domain::{id::RegisterId, state::{RegisterKey, RegisterScope}};
///
/// let key = RegisterKey::new(RegisterScope::Client, RegisterId::new(1));
/// assert_eq!(key.scope, RegisterScope::Client);
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct RegisterKey {
    /// Register scope.
    pub scope: RegisterScope,
    /// Interned opaque register id.
    pub id: RegisterId,
}

impl RegisterKey {
    /// Builds a register key.
    ///
    /// ```rust
    /// use reovim_subsys_domain::{id::RegisterId, state::{RegisterKey, RegisterScope}};
    ///
    /// assert_eq!(RegisterKey::new(RegisterScope::System, RegisterId::new(4)).id, RegisterId::new(4));
    /// ```
    #[must_use]
    pub const fn new(scope: RegisterScope, id: RegisterId) -> Self {
        Self { scope, id }
    }
}

/// Byte-edit origin for undo participation (§4.3 §1).
///
/// ```rust
/// use reovim_subsys_domain::{id::ClientId, state::EditOrigin};
///
/// assert!(EditOrigin::User { client_id: ClientId::new(1) }.participates_in_undo());
/// assert!(!EditOrigin::External.participates_in_undo());
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum EditOrigin {
    /// User-originated edit.
    User {
        /// Client that initiated the edit.
        client_id: ClientId,
    },
    /// Module-originated edit.
    Module {
        /// Owning cdylib.
        cdylib_id: CdylibId,
    },
    /// External source marker: timeline-visible, not undoable.
    External,
    /// Persistence restore: not undoable by default.
    Restore,
    /// Pending-input replay: not undoable by default.
    Replay,
}

impl EditOrigin {
    /// Returns whether edits with this origin belong to undoable groups.
    ///
    /// ```rust
    /// use reovim_subsys_domain::state::EditOrigin;
    ///
    /// assert!(!EditOrigin::Replay.participates_in_undo());
    /// ```
    #[must_use]
    pub const fn participates_in_undo(self) -> bool {
        matches!(self, Self::User { .. } | Self::Module { .. })
    }
}

/// Opaque undo-group id.
///
/// ```rust
/// use reovim_subsys_domain::state::UndoGroupId;
///
/// assert_eq!(UndoGroupId::new(7).as_u64(), 7);
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct UndoGroupId(u64);

impl UndoGroupId {
    /// Wraps a raw undo-group id.
    ///
    /// ```rust
    /// use reovim_subsys_domain::state::UndoGroupId;
    ///
    /// assert_eq!(UndoGroupId::new(8).as_u64(), 8);
    /// ```
    #[must_use]
    pub const fn new(raw: u64) -> Self {
        Self(raw)
    }

    /// Returns the raw id.
    ///
    /// ```rust
    /// use reovim_subsys_domain::state::UndoGroupId;
    ///
    /// assert_eq!(UndoGroupId::new(9).as_u64(), 9);
    /// ```
    #[must_use]
    pub const fn as_u64(self) -> u64 {
        self.0
    }
}

/// One byte-range edit retained by undo (§4.3 §2).
///
/// ```rust,no_run
/// // no_run: requires arch allocator runtime for old/new bytes.
/// ```
pub struct EditRecord {
    /// Inclusive start byte.
    pub start: usize,
    /// Exclusive end byte in the pre-edit buffer.
    pub end: usize,
    /// Bytes removed/replaced by the edit.
    pub old_bytes: Bytes,
    /// Bytes inserted by the edit.
    pub new_bytes: Bytes,
}

impl EditRecord {
    /// Builds a byte-range edit record.
    ///
    /// ```rust,no_run
    /// // no_run: requires arch allocator runtime for old/new bytes.
    /// ```
    #[must_use]
    pub const fn new(start: usize, end: usize, old_bytes: Bytes, new_bytes: Bytes) -> Self {
        Self {
            start,
            end,
            old_bytes,
            new_bytes,
        }
    }

    /// Returns whether this record leaves bytes unchanged.
    ///
    /// ```rust,no_run
    /// // no_run: requires arch allocator runtime for old/new bytes.
    /// ```
    #[must_use]
    pub fn is_noop(&self) -> bool {
        self.old_bytes.as_slice() == self.new_bytes.as_slice()
    }
}

/// One undo group with byte-range edit records.
///
/// ```rust,no_run
/// // no_run: requires arch allocator runtime when edits are pushed.
/// ```
pub struct UndoGroup {
    /// Group id.
    pub id: UndoGroupId,
    /// Origin governing undo participation.
    pub origin: EditOrigin,
    /// Retained byte edits.
    pub edits: Seq<EditRecord>,
    /// Monotonic timestamp in milliseconds from the kernel clock source.
    pub timestamp_ms: u64,
}

impl UndoGroup {
    /// Builds an empty undo group.
    ///
    /// ```rust
    /// use reovim_subsys_domain::state::{EditOrigin, UndoGroup, UndoGroupId};
    ///
    /// let group = UndoGroup::new(UndoGroupId::new(1), EditOrigin::External, 0);
    /// assert!(!group.is_undoable());
    /// ```
    #[must_use]
    pub const fn new(id: UndoGroupId, origin: EditOrigin, timestamp_ms: u64) -> Self {
        Self {
            id,
            origin,
            edits: Seq::new(),
            timestamp_ms,
        }
    }

    /// Returns whether undo should apply this group.
    ///
    /// ```rust
    /// use reovim_subsys_domain::state::{EditOrigin, UndoGroup, UndoGroupId};
    ///
    /// assert!(!UndoGroup::new(UndoGroupId::new(2), EditOrigin::Restore, 0).is_undoable());
    /// ```
    #[must_use]
    pub const fn is_undoable(&self) -> bool {
        self.origin.participates_in_undo()
    }
}

/// Per-buffer undo/redo stack skeleton (§4.3 §2).
///
/// ```rust,no_run
/// // no_run: requires arch allocator runtime when groups are pushed.
/// ```
pub struct UndoStack {
    /// Currently open group, if any.
    pub group_open: Option<UndoGroupId>,
    /// Undo groups, oldest-to-newest.
    pub groups: Seq<UndoGroup>,
    /// Redo groups, newest redo first by kernel policy.
    pub redo_groups: Seq<UndoGroup>,
}

impl UndoStack {
    /// Builds an empty stack.
    ///
    /// ```rust
    /// use reovim_subsys_domain::state::UndoStack;
    ///
    /// let stack = UndoStack::new();
    /// assert!(stack.group_open.is_none());
    /// ```
    #[must_use]
    pub const fn new() -> Self {
        Self {
            group_open: None,
            groups: Seq::new(),
            redo_groups: Seq::new(),
        }
    }
}

impl Default for UndoStack {
    fn default() -> Self {
        Self::new()
    }
}

// L12 layout: tests in sibling state_tests.rs, declared in lib.rs.
