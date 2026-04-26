//! `TextBufferImpl` — `reovim_subsys_buffer::Buffer` implementation
//! over the rope-backed `provider-text` storage.
//!
//! The buffer holds the canonical byte content under
//! `Arc<RwLock<dyn BufferOps>>` (re-exported from this crate as
//! `BufferOps`), plus a codec attachment table for synchronous
//! fan-out and a tokio broadcast channel for async edit subscriptions.
//!
//! # Edit critical section
//!
//! `apply_edit` performs four steps in order, all under the slot-table
//! write lock so the fan-out order is deterministic:
//!
//! 1. Mutate canonical bytes via the rope's `StorageOps` surface.
//! 2. Synchronously notify each attached codec, ordered by
//!    `CodecAttachmentId`. Each `notify` is wrapped in
//!    `catch_unwind(AssertUnwindSafe(...))`; a panicking slot is logged
//!    and left in place — the buffer subsys docs forbid mid-iteration
//!    detach.
//! 3. Best-effort broadcast send (drop `SendError` when no
//!    subscribers).
//! 4. Mark the buffer as modified.

use {
    crate::BufferOps,
    parking_lot::RwLock,
    reovim_content_codec::ByteNotifiable,
    reovim_kernel::api::v1::{BufferId, ByteEdit},
    reovim_subsys_buffer::{Buffer, BufferError, BufferSubscribable, CodecAttachmentId},
    std::{
        collections::BTreeMap,
        num::NonZeroU32,
        ops::Range,
        panic::{AssertUnwindSafe, catch_unwind},
        sync::{
            Arc,
            atomic::{AtomicBool, AtomicU32, Ordering},
        },
    },
    tokio::sync::broadcast,
};

/// Capacity for the per-buffer broadcast channel. Lag-drop semantics
/// match the session notification stream — slow subscribers do not
/// stall fast publishers.
const BROADCAST_CAPACITY: usize = 256;

/// Concrete `Buffer` impl backed by a `provider-text` rope.
pub struct TextBufferImpl {
    id: BufferId,
    inner: Arc<RwLock<dyn BufferOps>>,
    slots: RwLock<BTreeMap<CodecAttachmentId, Box<dyn ByteNotifiable>>>,
    slot_names: RwLock<BTreeMap<CodecAttachmentId, String>>,
    next_slot_id: AtomicU32,
    broadcast: broadcast::Sender<ByteEdit>,
    file_path: RwLock<Option<String>>,
    modified: AtomicBool,
}

impl TextBufferImpl {
    /// Construct a new buffer over the supplied rope-backed storage.
    ///
    /// `id` should normally be the same `BufferId` already held by the
    /// inner storage (`BufferOps::id()`); the driver passes
    /// `inner.read().id()` at construction time so callers get a single
    /// stable identity.
    #[must_use]
    pub fn new(id: BufferId, inner: Arc<RwLock<dyn BufferOps>>, file_path: Option<String>) -> Self {
        let (tx, _rx) = broadcast::channel(BROADCAST_CAPACITY);
        Self {
            id,
            inner,
            slots: RwLock::new(BTreeMap::new()),
            slot_names: RwLock::new(BTreeMap::new()),
            next_slot_id: AtomicU32::new(1),
            broadcast: tx,
            file_path: RwLock::new(file_path),
            modified: AtomicBool::new(false),
        }
    }

    /// Allocate the next monotonically increasing slot id.
    fn allocate_slot_id(&self) -> CodecAttachmentId {
        // `fetch_add` returns the previous value, so the first slot is
        // 1 — matching the subsys contract that the first slot is `1`.
        let raw = self.next_slot_id.fetch_add(1, Ordering::Relaxed);
        // The `NonZeroU32::new` here is `unwrap`-safe: we start at 1
        // and only ever increment. The only way to hit zero is u32
        // wrap-around after ~4B attachments to a single buffer; that
        // would be an obvious bug we want to surface as a panic.
        let nz = NonZeroU32::new(raw).expect("CodecAttachmentId allocator overflowed u32 range");
        CodecAttachmentId(nz)
    }

    /// Snapshot the current full byte content (for warm-starting a
    /// freshly attached codec). Allocates; caller must hold no lock on
    /// `self.inner`.
    fn snapshot_bytes(&self) -> Vec<u8> {
        self.inner.read().content_bytes()
    }
}

impl Buffer for TextBufferImpl {
    fn id(&self) -> BufferId {
        self.id
    }

    fn file_path(&self) -> Option<String> {
        self.file_path.read().clone()
    }

    fn set_file_path(&self, path: Option<String>) -> Result<(), BufferError> {
        // Mirror the path into the inner provider-text buffer too so
        // legacy text consumers (Phase 5/6 will retire them) see the
        // same value as the buffer-subsys path.
        self.inner.write().set_file_path(path.clone());
        *self.file_path.write() = path;
        Ok(())
    }

    fn is_modified(&self) -> bool {
        self.modified.load(Ordering::Acquire)
    }

    fn size(&self) -> usize {
        self.inner.read().byte_len()
    }

    fn read_bytes(&self, range: Range<usize>) -> Result<Vec<u8>, BufferError> {
        if range.start > range.end {
            return Err(BufferError::InvalidEdit(format!(
                "inverted range: {}..{}",
                range.start, range.end
            )));
        }
        let inner = self.inner.read();
        let total = inner.byte_len();
        if range.end > total {
            return Err(BufferError::InvalidEdit(format!(
                "range end {} exceeds buffer size {}",
                range.end, total
            )));
        }
        let len = range.end - range.start;
        let mut buf = vec![0u8; len];
        let written = inner.read_bytes(range.start, &mut buf);
        drop(inner);
        buf.truncate(written);
        Ok(buf)
    }

    fn apply_edit(&self, edit: ByteEdit) -> Result<(), BufferError> {
        // Take the slot lock first so fan-out order is deterministic
        // even if multiple writers race on `apply_edit`. The inner
        // write lock is taken inside this critical section.
        let mut slots = self.slots.write();

        // Step 1 — mutate canonical bytes.
        {
            let mut inner = self.inner.write();
            let total = inner.byte_len();
            let end = edit
                .offset
                .checked_add(edit.old_bytes.len())
                .ok_or_else(|| BufferError::InvalidEdit("edit offset+len overflow".into()))?;
            if end > total {
                return Err(BufferError::InvalidEdit(format!(
                    "edit range {}..{} exceeds buffer size {}",
                    edit.offset, end, total
                )));
            }
            // Splice: delete old bytes (if any), then insert new bytes
            // (if any). `StorageOps::delete_bytes` and `insert_bytes`
            // map `StorageError` → `BufferError::InvalidEdit`.
            if !edit.old_bytes.is_empty() {
                inner
                    .delete_bytes(edit.offset, edit.old_bytes.len())
                    .map_err(|e| BufferError::InvalidEdit(e.to_string()))?;
            }
            if !edit.new_bytes.is_empty() {
                inner
                    .insert_bytes(edit.offset, &edit.new_bytes)
                    .map_err(|e| BufferError::InvalidEdit(e.to_string()))?;
            }
        }

        // Step 2 — synchronous fan-out, in CodecAttachmentId order
        // (BTreeMap iterates in key order). Per-slot panic isolation:
        // log + leave the slot in place (subsys contract — the slot is
        // marked poisoned but not detached mid-iteration).
        for (slot_id, codec) in slots.iter_mut() {
            let result = catch_unwind(AssertUnwindSafe(|| codec.notify(&edit)));
            if let Err(panic_payload) = result {
                let msg = panic_message(&*panic_payload);
                tracing::error!(
                    target: "reovim_driver_text_buffer",
                    slot = slot_id.0.get(),
                    panic = %msg,
                    "codec slot panicked during notify; slot left in place",
                );
            }
        }
        // Drop the slot lock before broadcast send so a slow subscriber
        // can't stall a future writer.
        drop(slots);

        // Step 3 — best-effort async broadcast (drop on no subscribers).
        let _ = self.broadcast.send(edit);

        // Step 4 — mark modified.
        self.modified.store(true, Ordering::Release);

        Ok(())
    }

    fn write_to(&self, writer: &mut dyn std::io::Write) -> std::io::Result<()> {
        let inner = self.inner.read();
        // BufferOps has a `write_to` default that forwards through the
        // rope chunks; delegate to it for non-allocating writes on
        // VirtualBuffer-backed inners.
        inner.write_to(writer)
    }

    fn attach_codec(
        &self,
        name: &str,
        mut codec: Box<dyn ByteNotifiable>,
    ) -> Result<CodecAttachmentId, BufferError> {
        // Warm-start the index BEFORE taking the slot locks — the
        // subsys docs require a `build` call on the current bytes,
        // and `build` may run arbitrarily long codec-side work that
        // we don't want to hold either lock during.
        let bytes = self.snapshot_bytes();
        codec.build(&bytes);

        // Take both locks atomically (slot-then-names lock order
        // matches `apply_edit`/`detach_codec`). The duplicate-name
        // check moves under the write lock so the check + insert is
        // a single critical section — no TOCTOU window.
        let mut slots = self.slots.write();
        let mut names = self.slot_names.write();
        if names.values().any(|n| n == name) {
            return Err(BufferError::DuplicateCodecName(name.to_string()));
        }
        let id = self.allocate_slot_id();
        slots.insert(id, codec);
        names.insert(id, name.to_string());
        drop(names);
        drop(slots);
        Ok(id)
    }

    fn detach_codec(&self, id: CodecAttachmentId) -> Result<Box<dyn ByteNotifiable>, BufferError> {
        let mut slots = self.slots.write();
        let mut names = self.slot_names.write();
        let Some(codec) = slots.remove(&id) else {
            return Err(BufferError::NotFound(self.id));
        };
        names.remove(&id);
        drop(names);
        drop(slots);
        Ok(codec)
    }

    fn list_codecs(&self) -> Vec<(CodecAttachmentId, String)> {
        self.slot_names
            .read()
            .iter()
            .map(|(id, name)| (*id, name.clone()))
            .collect()
    }
}

impl BufferSubscribable for TextBufferImpl {
    fn subscribe_edits(&self) -> broadcast::Receiver<ByteEdit> {
        self.broadcast.subscribe()
    }
}

/// Best-effort recovery of a panic payload's message.
fn panic_message(payload: &(dyn std::any::Any + Send)) -> String {
    payload.downcast_ref::<&'static str>().map_or_else(
        || {
            payload
                .downcast_ref::<String>()
                .cloned()
                .unwrap_or_else(|| "<non-string panic payload>".to_string())
        },
        |s| (*s).to_string(),
    )
}
