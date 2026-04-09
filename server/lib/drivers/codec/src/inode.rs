//! Inode and mount primitives for codec-aware byte storage.

use std::{collections::HashMap, fmt, io, num::NonZeroU64, path::Path, sync::Arc};

use {
    reovim_driver_vfs::ByteSource,
    reovim_kernel::api::v1::{BufferId, ByteEdit},
};

use {
    crate::errors::{EditError, MountError, UmountError},
    tracing::{debug, trace},
};

use crate::{ContentCodec, DecodedEdit};

// ----------------------------------------------------------------------------
// Ids

/// Strongly typed inode identifier.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[repr(transparent)]
pub struct InodeId(NonZeroU64);

impl InodeId {
    /// Create an identifier from a raw value.
    #[allow(clippy::cast_possible_truncation)]
    ///
    /// # Panics
    ///
    /// Panics if `value` is zero or does not fit in `u64`.
    #[must_use]
    pub fn from_raw(value: usize) -> Self {
        let value = u64::try_from(value).expect("inode id does not fit in u64");
        Self(NonZeroU64::new(value).expect("inode id must be non-zero"))
    }

    /// Create an identifier from a raw `u64`.
    ///
    /// # Panics
    ///
    /// Panics if `value` is zero.
    #[must_use]
    pub const fn from_u64(value: u64) -> Self {
        Self(NonZeroU64::new(value).expect("inode id must be non-zero"))
    }

    /// Get the raw numeric value.
    #[must_use]
    pub const fn as_u64(self) -> u64 {
        self.0.get()
    }

    /// Get the raw numeric value as `usize`.
    #[allow(clippy::cast_possible_truncation)]
    #[must_use]
    pub const fn as_usize(self) -> usize {
        self.as_u64() as usize
    }
}

/// Mount identifier.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[repr(transparent)]
pub struct MountId(NonZeroU64);

impl MountId {
    /// Create an identifier from a raw value.
    #[allow(clippy::cast_possible_truncation)]
    ///
    /// # Panics
    ///
    /// Panics if `value` is zero or does not fit in `u64`.
    #[must_use]
    pub fn from_raw(value: usize) -> Self {
        let value = u64::try_from(value).expect("mount id does not fit in u64");
        Self(NonZeroU64::new(value).expect("mount id must be non-zero"))
    }

    /// Create an identifier from a raw `u64`.
    ///
    /// # Panics
    ///
    /// Panics if `value` is zero.
    #[must_use]
    pub const fn from_u64(value: u64) -> Self {
        Self(NonZeroU64::new(value).expect("mount id must be non-zero"))
    }

    /// Get the raw numeric value.
    #[must_use]
    pub const fn as_u64(self) -> u64 {
        self.0.get()
    }

    /// Get the raw numeric value as `usize`.
    #[allow(clippy::cast_possible_truncation)]
    #[must_use]
    pub const fn as_usize(self) -> usize {
        self.as_u64() as usize
    }
}

impl fmt::Display for InodeId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Inode({})", self.0)
    }
}

impl fmt::Display for MountId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Mount({})", self.0)
    }
}

/// Stable handle to a mounted codec view.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct MountHandle {
    inode: InodeId,
    buffer: BufferId,
    mount: MountId,
}

impl MountHandle {
    /// Construct a handle from identifiers.
    #[must_use]
    pub const fn new(inode_id: InodeId, buffer_id: BufferId, mount_id: MountId) -> Self {
        Self {
            inode: inode_id,
            buffer: buffer_id,
            mount: mount_id,
        }
    }

    /// The owning buffer id for this mount.
    #[must_use]
    pub const fn buffer_id(&self) -> BufferId {
        self.buffer
    }

    /// The mount id embedded in this handle.
    ///
    /// Exposed for the Phase 5 sub-commit 5c wire protocol: the
    /// `MountCodec` RPC response echoes the assigned mount id back to
    /// the client so it can later pass it to `UmountCodec`.
    #[must_use]
    pub const fn mount_id(&self) -> MountId {
        self.mount
    }

    /// Test-only accessor for the mount id embedded in this handle.
    #[cfg(test)]
    #[must_use]
    pub(crate) const fn mount_id_for_tests(&self) -> MountId {
        self.mount
    }
}

// ----------------------------------------------------------------------------
// Mount + Inode

/// A single mounted codec view of an inode.
#[derive(Clone)]
pub struct Mount {
    /// Display name for this mount (example: codec view name).
    pub name: String,

    /// Codec responsible for translating decoded edits for this mount.
    pub codec: Arc<dyn ContentCodec>,

    /// `false` when this mount's decoded view is known to be stale with
    /// respect to the underlying `Inode.bytes` (e.g. because a sibling
    /// mount applied an edit). Phase 5 sub-commit 5e wires re-decode on
    /// read via the `StaleCheck` hook; Phase 5 sub-commit 5a only marks.
    pub content_valid: bool,
}

impl std::fmt::Debug for Mount {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Mount")
            .field("name", &self.name)
            .field("codec", &"<dyn ContentCodec>")
            .field("content_valid", &self.content_valid)
            .finish()
    }
}

impl Mount {
    /// Create a new mount. `content_valid` starts `true` since the mount's
    /// decoded view is fresh at creation time.
    #[must_use]
    pub fn new(name: impl Into<String>, codec: Arc<dyn ContentCodec>) -> Self {
        Self {
            name: name.into(),
            codec,
            content_valid: true,
        }
    }
}

/// Maximum inode size (in bytes) the Phase 5 5d edit pipeline will
/// allow into the heap-promotion path.
///
/// `apply_byte_edit_to_source` reads the current `ByteSource` into a
/// fresh `HeapByteSource` when the edit lands; for mmap-backed inodes
/// this is a one-shot O(N) copy. Past 512 MB the memory cost is
/// unreasonable, so the guard returns `EditError::InvalidEdit` with a
/// user-actionable message. Phase 7 will lift the cap via an
/// `OverlayByteSource`.
pub const MMAP_PROMOTION_BUDGET: u64 = 512 * 1024 * 1024;

/// Raw-byte source and active mount registry for a file.
#[derive(Clone)]
pub struct Inode {
    /// Canonical bytes for this file.
    pub bytes: Arc<dyn ByteSource>,
    /// Optional on-disk path the inode was loaded from. Populated by
    /// `:e` via [`InodeTable::insert_with_path`] and by the first
    /// successful [`InodeTable::flush`] when a scratch buffer is saved
    /// to a new filename.
    pub path: Option<Arc<Path>>,
    /// Mounts keyed by `MountId`. Phase 5 5a relaxed the single-mount
    /// invariant; use [`InodeTable::mount_additional`] to attach more
    /// than one mount.
    pub mounts: HashMap<MountId, Mount>,
}

impl Inode {
    /// Construct an inode with initial bytes and no mounts.
    #[must_use]
    pub fn new(bytes: Arc<dyn ByteSource>) -> Self {
        Self {
            bytes,
            path: None,
            mounts: HashMap::new(),
        }
    }

    /// Construct an inode with initial bytes, an on-disk path, and no mounts.
    #[must_use]
    pub fn with_path(bytes: Arc<dyn ByteSource>, path: Arc<Path>) -> Self {
        Self {
            bytes,
            path: Some(path),
            mounts: HashMap::new(),
        }
    }

    /// Replace canonical bytes with a new byte slice.
    fn set_bytes(&mut self, bytes: Vec<u8>) {
        self.bytes = Arc::new(reovim_driver_vfs::HeapByteSource::new(bytes));
    }
}

// ----------------------------------------------------------------------------
// Table

/// Map of mounted files, where each inode owns source bytes and its mounts.
#[derive(Default)]
pub struct InodeTable {
    inodes: HashMap<InodeId, Inode>,
    /// Buffer->inode source index for canonical source of truth.
    files: HashMap<BufferId, InodeId>,
    /// Secondary index for fast mount lookups and to enforce mount-ownership checks.
    mount_idx: HashMap<MountId, InodeId>,
    next_inode_id: usize,
    next_mount_id: u64,
}

impl InodeTable {
    /// Create an empty inode table.
    #[must_use]
    pub fn new() -> Self {
        Self {
            inodes: HashMap::new(),
            files: HashMap::new(),
            mount_idx: HashMap::new(),
            next_inode_id: 0,
            next_mount_id: 0,
        }
    }

    fn next_inode_id(&mut self) -> InodeId {
        let next = self
            .next_inode_id
            .checked_add(1)
            .expect("inode id counter overflowed usize");
        self.next_inode_id = next;
        let as_u64 = u64::try_from(next).expect("inode id does not fit in u64");
        InodeId::from_u64(as_u64)
    }

    #[allow(clippy::missing_const_for_fn)]
    fn next_mount_id(&mut self) -> MountId {
        let next = self
            .next_mount_id
            .checked_add(1)
            .expect("mount id counter overflowed u64");
        self.next_mount_id = next;
        MountId::from_u64(next)
    }

    /// Register a new inode and return its id.
    ///
    /// # Panics
    ///
    /// Panics if the inode id counter overflows `usize`.
    pub fn insert(&mut self, bytes: Arc<dyn ByteSource>) -> InodeId {
        let id = self.next_inode_id();
        let previous = self.inodes.insert(id, Inode::new(bytes));
        debug_assert!(previous.is_none());
        id
    }

    /// Register a new inode with an on-disk path and return its id.
    ///
    /// Phase 5 5d: `:e` routes through this constructor so the
    /// consolidated `:w` flush path can resolve the destination without
    /// re-asking the caller for a filename.
    ///
    /// # Panics
    ///
    /// Panics if the inode id counter overflows `usize`.
    pub fn insert_with_path(&mut self, bytes: Arc<dyn ByteSource>, path: Arc<Path>) -> InodeId {
        let id = self.next_inode_id();
        let previous = self.inodes.insert(id, Inode::with_path(bytes, path));
        debug_assert!(previous.is_none());
        id
    }

    /// Replace the on-disk path for an existing inode.
    ///
    /// # Errors
    ///
    /// - [`EditError::InodeNotFound`] if the target inode does not exist.
    pub fn set_path(&mut self, inode_id: InodeId, path: Arc<Path>) -> Result<(), EditError> {
        let inode = self
            .lookup_inode_mut(inode_id)
            .ok_or(EditError::InodeNotFound { inode_id })?;
        inode.path = Some(path);
        Ok(())
    }

    /// Bind a buffer to an inode id.
    pub fn bind_file(&mut self, buffer_id: BufferId, inode_id: InodeId) -> Option<InodeId> {
        self.files.insert(buffer_id, inode_id)
    }

    /// Unbind a buffer and return its inode id, if any.
    pub fn unbind_file(&mut self, buffer_id: BufferId) -> Option<InodeId> {
        self.files.remove(&buffer_id)
    }

    /// Lookup the inode for a buffer.
    #[must_use]
    pub fn file_inode(&self, buffer_id: BufferId) -> Option<InodeId> {
        self.files.get(&buffer_id).copied()
    }

    /// Find an inode by id.
    #[must_use]
    pub fn lookup_inode(&self, inode_id: InodeId) -> Option<&Inode> {
        self.inodes.get(&inode_id)
    }

    /// Find a mutable inode by id.
    pub fn lookup_inode_mut(&mut self, inode_id: InodeId) -> Option<&mut Inode> {
        self.inodes.get_mut(&inode_id)
    }

    /// Get an inode by id.
    #[must_use]
    pub fn get(&self, inode_id: InodeId) -> Option<&Inode> {
        self.lookup_inode(inode_id)
    }

    /// Get a mutable inode by id.
    pub fn get_mut(&mut self, inode_id: InodeId) -> Option<&mut Inode> {
        self.lookup_inode_mut(inode_id)
    }

    /// Find a mount by its handle.
    #[must_use]
    pub fn lookup_mount(&self, handle: MountHandle) -> Option<&Mount> {
        let inode_id = self.mount_idx.get(&handle.mount)?;

        if self
            .files
            .get(&handle.buffer)
            .is_none_or(|inode| *inode != *inode_id)
        {
            return None;
        }

        self.inodes
            .get(inode_id)
            .and_then(|inode| inode.mounts.get(&handle.mount))
    }

    /// Find a mount by its handle (mutable inode context), if needed by callers.
    pub fn lookup_mount_mut(&mut self, handle: MountHandle) -> Option<&mut Mount> {
        let inode_id = self.mount_idx.get(&handle.mount)?;
        let mount_id = handle.mount;

        if self
            .files
            .get(&handle.buffer)
            .is_none_or(|inode| *inode != *inode_id)
        {
            return None;
        }

        self.inodes
            .get_mut(inode_id)
            .and_then(|inode| inode.mounts.get_mut(&mount_id))
    }

    /// Read the full byte contents of an inode.
    ///
    /// # Errors
    ///
    /// - [`EditError::InodeNotFound`] if the target inode does not exist.
    /// - [`EditError::ApplyFailed`] if the byte source cannot be read contiguously.
    pub fn read_bytes(&self, inode_id: InodeId) -> Result<Vec<u8>, EditError> {
        let inode = self
            .lookup_inode(inode_id)
            .ok_or(EditError::InodeNotFound { inode_id })?;
        read_all_bytes(inode.bytes.as_ref())
    }

    /// Store new bytes for an inode.
    ///
    /// # Errors
    ///
    /// - [`EditError::InodeNotFound`] if the target inode does not exist.
    pub fn set_bytes(&mut self, inode_id: InodeId, bytes: Vec<u8>) -> Result<(), EditError> {
        let inode = self
            .lookup_inode_mut(inode_id)
            .ok_or(EditError::InodeNotFound { inode_id })?;
        inode.set_bytes(bytes);
        Ok(())
    }

    /// Borrow a mount for the given handle.
    #[must_use]
    pub fn mount_ref(&self, handle: MountHandle) -> Option<&Mount> {
        self.lookup_mount(handle)
    }

    /// Mount into an inode as the **first** mount.
    ///
    /// Phase 5 retains `mount()` as the single-mount bootstrap — it
    /// rejects a second call with [`MountError::AlreadyMounted`]. Use
    /// [`InodeTable::mount_additional`] to attach a second (or Nth)
    /// codec view onto an inode that already has a mount.
    ///
    /// # Errors
    ///
    /// - [`MountError::InodeNotFound`] if the inode does not exist.
    /// - [`MountError::AlreadyMounted`] if the inode already has a mount.
    pub fn mount(
        &mut self,
        inode_id: InodeId,
        buffer_id: BufferId,
        mount: Mount,
    ) -> Result<MountHandle, MountError> {
        let inode = self
            .inodes
            .get_mut(&inode_id)
            .ok_or(MountError::InodeNotFound { inode_id })?;

        if !inode.mounts.is_empty() {
            return Err(MountError::AlreadyMounted { inode_id });
        }

        let mount_id = self.next_mount_id();
        let handle = MountHandle::new(inode_id, buffer_id, mount_id);
        let mount_name = mount.name.clone();
        let inode = self
            .inodes
            .get_mut(&inode_id)
            .ok_or(MountError::InodeNotFound { inode_id })?;
        let previous = inode.mounts.insert(mount_id, mount);
        debug_assert!(previous.is_none());

        self.mount_idx.insert(mount_id, inode_id);

        debug!(
            inode_id = %inode_id,
            mount_id = %mount_id,
            mount_name = %mount_name,
            "inode-mounted"
        );

        Ok(handle)
    }

    /// Attach a second (or Nth) codec view onto an inode that already has
    /// a first mount.
    ///
    /// This is the Phase 5 multi-mount extension: two or more mounts can
    /// coexist on a single inode, each presenting a different codec view
    /// (e.g. UTF-8 text alongside hex). Edits applied through one mount
    /// mark every peer mount's `content_valid` as `false` via
    /// [`InodeTable::apply_edit`] so the 5e `StaleCheck` hook can trigger
    /// a re-decode on the next read.
    ///
    /// # Errors
    ///
    /// - [`MountError::InodeNotFound`] if the inode does not exist.
    /// - [`MountError::AlreadyMounted`] is **not** returned here — the
    ///   single-mount invariant is relaxed. If no initial mount exists
    ///   yet, use [`InodeTable::mount`] instead.
    ///
    /// # Panics
    ///
    /// Panics if the inode exists but the mount id counter overflows.
    pub fn mount_additional(
        &mut self,
        inode_id: InodeId,
        buffer_id: BufferId,
        mount: Mount,
    ) -> Result<MountHandle, MountError> {
        if !self.inodes.contains_key(&inode_id) {
            return Err(MountError::InodeNotFound { inode_id });
        }

        let mount_id = self.next_mount_id();
        let handle = MountHandle::new(inode_id, buffer_id, mount_id);
        let mount_name = mount.name.clone();

        let inode = self
            .inodes
            .get_mut(&inode_id)
            .ok_or(MountError::InodeNotFound { inode_id })?;
        let previous = inode.mounts.insert(mount_id, mount);
        debug_assert!(previous.is_none());

        self.mount_idx.insert(mount_id, inode_id);

        debug!(
            inode_id = %inode_id,
            mount_id = %mount_id,
            mount_name = %mount_name,
            peer_count = inode.mounts.len(),
            "inode-mounted-additional"
        );

        Ok(handle)
    }

    /// Flush inode bytes to a path. Replaces encode-on-save.
    ///
    /// Path resolution rules:
    /// - `path_override = Some(p)` → write to `p`; if `inode.path` is
    ///   `None`, populate it with `p` (the `:w newfile.txt` scratch
    ///   buffer flow).
    /// - `path_override = None`, `inode.path = Some(p)` → write to `p`.
    /// - Both `None` → `io::ErrorKind::InvalidInput` ("no filename").
    ///
    /// Bytes are written via [`ByteSource::write_to`] so heap-backed
    /// and mmap-backed sources both stream contiguously without an
    /// extra `Vec` allocation. No `encode` is invoked — Plan 06 Phase 5
    /// sub-commit 5d deletes the encode-on-save path by architecture.
    ///
    /// # Errors
    ///
    /// - [`io::ErrorKind::NotFound`] if `mount_id` is not registered on
    ///   any inode in this table.
    /// - [`io::ErrorKind::InvalidInput`] if no destination path is
    ///   resolvable (both `path_override` and `inode.path` are `None`).
    /// - Propagates any I/O error from creating or writing to the file.
    pub fn flush(&mut self, mount_id: MountId, path_override: Option<&Path>) -> io::Result<()> {
        let inode_id = self
            .mount_idx
            .get(&mount_id)
            .copied()
            .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "mount id not found"))?;

        let inode = self
            .inodes
            .get_mut(&inode_id)
            .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "inode not found for mount"))?;

        let target: Arc<Path> = match (path_override, inode.path.clone()) {
            (Some(p), _) => Arc::<Path>::from(p.to_path_buf()),
            (None, Some(existing)) => existing,
            (None, None) => {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "no filename: buffer has no on-disk path and no explicit target was provided",
                ));
            }
        };

        let mut file = std::fs::File::create(&target)?;
        inode.bytes.write_to(&mut file)?;

        // Populate inode.path for the `:w newfile.txt` scratch flow so
        // subsequent `:w` calls without an override resolve to the same
        // file.
        if inode.path.is_none() {
            inode.path = Some(Arc::clone(&target));
        }

        debug!(
            inode_id = %inode_id,
            mount_id = %mount_id,
            path = %target.display(),
            bytes = inode.bytes.len(),
            "inode-flushed"
        );

        Ok(())
    }

    /// Count of active file bindings.
    #[must_use]
    pub fn file_len(&self) -> usize {
        self.files.len()
    }

    /// Remove a mount from an inode.
    ///
    /// # Errors
    ///
    /// - [`UmountError::InodeNotFound`] if the inode does not exist.
    /// - [`UmountError::MountNotFound`] if the mount is not present on the inode.
    pub fn unmount(&mut self, handle: MountHandle) -> Result<Mount, UmountError> {
        let inode_id = handle.inode;
        let mount_id = handle.mount;

        let inode = self
            .inodes
            .get_mut(&inode_id)
            .ok_or(UmountError::InodeNotFound { inode_id })?;

        let removed = inode
            .mounts
            .remove(&mount_id)
            .ok_or(UmountError::MountNotFound { inode_id, mount_id })?;

        self.mount_idx.remove(&mount_id);

        if !self.files.values().any(|bound| *bound == inode_id)
            && let Some(inode) = self.inodes.get(&inode_id)
        {
            debug_assert!(inode.mounts.is_empty());
        }

        debug!(
            inode_id = %inode_id,
            mount_id = %mount_id,
            mount_name = %removed.name,
            "inode-unmounted"
        );

        Ok(removed)
    }

    /// Find the currently mounted view for a buffer, if any.
    #[must_use]
    pub fn active_mount(&self, buffer_id: BufferId) -> Option<MountHandle> {
        let inode_id = self.files.get(&buffer_id).copied()?;
        let inode = self.inodes.get(&inode_id)?;
        let mount_id = inode.mounts.keys().next().copied()?;

        Some(MountHandle::new(inode_id, buffer_id, mount_id))
    }

    /// Get the active mount name/view for a buffer.
    #[must_use]
    pub fn active_mount_name(&self, buffer_id: BufferId) -> Option<&str> {
        self.active_mount(buffer_id)
            .and_then(|handle| self.mount_ref(handle))
            .map(|mount| mount.name.as_str())
    }

    /// Alias for locating the inode bound to a buffer.
    #[must_use]
    pub fn lookup_file(&self, buffer_id: BufferId) -> Option<InodeId> {
        self.file_inode(buffer_id)
    }

    /// Check whether a buffer is bound in this table.
    #[must_use]
    pub fn contains_file(&self, buffer_id: BufferId) -> bool {
        self.files.contains_key(&buffer_id)
    }

    /// Remove all mounts for a buffer and return whether anything changed.
    pub fn unmount_all(&mut self, buffer_id: BufferId) -> bool {
        let Some(inode_id) = self.files.get(&buffer_id).copied() else {
            return false;
        };

        let mounts: Vec<MountId> = self
            .inodes
            .get(&inode_id)
            .map(|inode| inode.mounts.keys().copied().collect())
            .unwrap_or_default();

        let mut changed = false;
        for mount_id in mounts {
            let handle = MountHandle::new(inode_id, buffer_id, mount_id);
            if self.unmount(handle).is_ok() {
                changed = true;
            }
        }

        changed
    }

    /// Apply a decoded edit by translating it to bytes and mutating canonical content.
    ///
    /// After the edit lands, every **peer** mount on the same inode
    /// (i.e. mounts other than `handle.mount`) has its `content_valid`
    /// flag set to `false`. Phase 5 sub-commit 5e wires the
    /// `StaleCheck` hook so the next read through a stale mount
    /// re-decodes from `inode.bytes`. The source mount stays
    /// `content_valid = true` because the edit originated there.
    ///
    /// # Errors
    ///
    /// - [`EditError::InodeNotFound`] if the target inode does not exist.
    /// - [`EditError::MountNotFound`] if the mount is not attached to the inode.
    /// - [`EditError::ReadOnly`] if the codec does not support edit translation.
    /// - [`EditError::Unsupported`] if the decoded edit variant is unsupported.
    /// - [`EditError::InvalidEdit`] if the edit references invalid ranges.
    /// - [`EditError::ApplyFailed`] if the translated byte edit cannot be applied.
    pub fn apply_edit(
        &mut self,
        handle: MountHandle,
        edit: &DecodedEdit,
    ) -> Result<ByteEdit, EditError> {
        let inode_id = handle.inode;
        let mount_id = handle.mount;

        let mount_codec = self
            .lookup_mount(handle)
            .map(|mount| mount.codec.clone())
            .ok_or(EditError::MountNotFound { inode_id, mount_id })?;

        if self.mount_ref(handle).is_none() {
            return Err(EditError::MountNotFound { inode_id, mount_id });
        }

        let inode = self
            .lookup_inode_mut(inode_id)
            .ok_or(EditError::InodeNotFound { inode_id })?;

        let current = read_all_bytes(inode.bytes.as_ref())?;
        let Some(byte_edit) = mount_codec.translate_edit(inode.bytes.as_ref(), edit) else {
            return match edit {
                DecodedEdit::_Reserved => Err(EditError::Unsupported {
                    reason: "reserved edit variant is not supported",
                }),
                DecodedEdit::Text { .. } | DecodedEdit::Bytes { .. } => Err(EditError::ReadOnly),
            };
        };

        let next = apply_byte_edit(current, &byte_edit)?;
        inode.set_bytes(next);

        // Phase 5 sub-commit 5a: mark every peer mount on this inode
        // stale. The source mount stays valid; others need re-decode on
        // next read (wired in 5e via StaleCheck).
        let mut stale_peers = 0usize;
        for (peer_id, peer) in &mut inode.mounts {
            if *peer_id == mount_id {
                peer.content_valid = true;
            } else {
                peer.content_valid = false;
                stale_peers += 1;
            }
        }

        debug!(
            inode_id = %inode_id,
            mount_id = %mount_id,
            start = byte_edit.offset,
            replaced = byte_edit.old_bytes.len(),
            inserted = byte_edit.new_bytes.len(),
            "inode-byte-edit-applied"
        );
        if stale_peers > 0 {
            trace!(
                inode_id = %inode_id,
                source_mount = %mount_id,
                stale_peers,
                "inode-peer-mounts-marked-stale"
            );
        }

        Ok(byte_edit)
    }

    /// Apply a raw byte edit directly to canonical bytes.
    ///
    /// This path bypasses decoded coordinates and codecs, and is intended for
    /// edits generated from already-decoded text operations (e.g. runtime
    /// `insert_text` mutations).
    ///
    /// # Errors
    ///
    /// - [`EditError::InodeNotFound`] if the target inode does not exist.
    /// - [`EditError::ApplyFailed`] if the edit range is invalid.
    pub fn apply_byte_edit(&mut self, inode_id: InodeId, edit: &ByteEdit) -> Result<(), EditError> {
        let inode = self
            .inodes
            .get_mut(&inode_id)
            .ok_or(EditError::InodeNotFound { inode_id })?;

        let current = read_all_bytes(inode.bytes.as_ref())?;
        let next = apply_byte_edit(current, edit)?;
        inode.set_bytes(next);
        Ok(())
    }

    /// Remove an inode and drop all mounted metadata.
    ///
    /// Returns the removed inode for debugging and assertions.
    pub fn remove(&mut self, inode_id: InodeId) -> Option<Inode> {
        let inode = self.inodes.remove(&inode_id)?;

        for mount_id in inode.mounts.keys() {
            self.mount_idx.remove(mount_id);
        }

        Some(inode)
    }

    /// Remove buffer binding and backing inode.
    pub fn remove_file(&mut self, buffer_id: BufferId) -> Option<Inode> {
        let inode_id = self.files.remove(&buffer_id)?;
        self.remove(inode_id)
    }

    /// Number of registered inodes.
    #[must_use]
    pub fn len(&self) -> usize {
        self.inodes.len()
    }

    /// Whether the table has no inodes.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.inodes.is_empty()
    }
}

fn read_all_bytes(bytes: &dyn ByteSource) -> Result<Vec<u8>, EditError> {
    let len = usize::try_from(bytes.len()).map_err(|_| EditError::InvalidEdit {
        reason: "inode bytes are too large for memory address space",
    })?;

    let current = bytes.read(0..bytes.len()).into_owned();
    if current.len() != len {
        return Err(EditError::ApplyFailed {
            reason: "could not read full inode bytes for edit",
        });
    }

    Ok(current)
}

#[cfg(test)]
fn byte_edit_from_bytes(
    offset: usize,
    current: &[u8],
    old_len: usize,
    new_bytes: &[u8],
) -> Result<ByteEdit, EditError> {
    let end = offset.checked_add(old_len).ok_or(EditError::InvalidEdit {
        reason: "offset overflow in byte edit",
    })?;

    if end > current.len() {
        return Err(EditError::InvalidEdit {
            reason: "byte edit range is out of bounds",
        });
    }

    Ok(ByteEdit {
        offset,
        old_bytes: current[offset..end].to_vec(),
        new_bytes: new_bytes.to_vec(),
    })
}

fn apply_byte_edit(mut current: Vec<u8>, edit: &ByteEdit) -> Result<Vec<u8>, EditError> {
    let end = edit
        .offset
        .checked_add(edit.old_bytes.len())
        .ok_or(EditError::InvalidEdit {
            reason: "offset overflow while applying byte edit",
        })?;

    if edit.offset > current.len() || end > current.len() {
        return Err(EditError::ApplyFailed {
            reason: "byte edit range is out of bounds",
        });
    }

    // Phase 5 5d: reject edits on very large inodes. The heap
    // promotion path for mmap-backed sources is O(N) and we don't
    // want to silently allocate > 512 MB on a single keystroke.
    // Phase 7 lifts this via OverlayByteSource.
    if current.len() as u64 > MMAP_PROMOTION_BUDGET {
        return Err(EditError::InvalidEdit {
            reason: "file too large for in-memory edit — mount hex alongside for byte-level editing, \
                 or use an external tool (#740 Phase 5 MMAP_PROMOTION_BUDGET)",
        });
    }

    current.splice(edit.offset..end, edit.new_bytes.iter().copied());

    Ok(current)
}

#[cfg(test)]
#[path = "inode_tests.rs"]
mod tests;
