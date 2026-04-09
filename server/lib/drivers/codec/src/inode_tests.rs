//! Tests for inode and mount primitives.

use std::sync::Arc;

use {
    crate::error::CodecError,
    reovim_driver_vfs::HeapByteSource,
    reovim_kernel::api::v1::{BufferId, ByteEdit},
};

use crate::{CodecMetadata, ContentCodec, ContentType, DecodeResult, DecodedEdit};

use super::{
    EditError, InodeId, InodeTable, Mount, MountError, MountHandle, MountId, UmountError,
    byte_edit_from_bytes,
};

fn buf(id: usize) -> BufferId {
    BufferId::from_raw(id)
}

fn mount_with_buffer(
    table: &mut InodeTable,
    inode_id: InodeId,
    buffer_id: usize,
    name: &'static str,
    codec: Arc<dyn ContentCodec>,
) -> Result<MountHandle, MountError> {
    table.bind_file(buf(buffer_id), inode_id);
    table.mount(inode_id, buf(buffer_id), Mount::new(name, codec))
}

#[derive(Default)]
struct CodecNoop;

impl ContentCodec for CodecNoop {
    fn decode(&self, raw: &[u8]) -> Result<DecodeResult, CodecError> {
        Ok(DecodeResult {
            content: String::from_utf8(raw.to_vec()).unwrap_or_else(|_| String::new()),
            annotations: vec![],
            metadata: CodecMetadata::new(ContentType::new("text/utf-8")),
            lossy: false,
            readonly: false,
            truncated: false,
        })
    }
}

#[derive(Default)]
struct CodecReadOnly;

impl ContentCodec for CodecReadOnly {
    fn decode(&self, raw: &[u8]) -> Result<DecodeResult, CodecError> {
        Ok(DecodeResult {
            content: String::from_utf8(raw.to_vec()).unwrap_or_else(|_| String::new()),
            annotations: vec![],
            metadata: CodecMetadata::new(ContentType::new("text/utf-8")),
            lossy: false,
            readonly: false,
            truncated: false,
        })
    }

    fn translate_edit(
        &self,
        _bytes: &dyn reovim_driver_vfs::ByteSource,
        _edit: &DecodedEdit,
    ) -> Option<ByteEdit> {
        None
    }
}

#[derive(Default)]
struct CodecByteTranslator;

impl ContentCodec for CodecByteTranslator {
    fn decode(&self, raw: &[u8]) -> Result<DecodeResult, CodecError> {
        Ok(DecodeResult {
            content: String::from_utf8(raw.to_vec()).unwrap_or_else(|_| String::new()),
            annotations: vec![],
            metadata: CodecMetadata::new(ContentType::new("text/utf-8")),
            lossy: false,
            readonly: false,
            truncated: false,
        })
    }

    fn translate_edit(
        &self,
        bytes: &dyn reovim_driver_vfs::ByteSource,
        edit: &DecodedEdit,
    ) -> Option<ByteEdit> {
        let current = bytes.read(0..bytes.len()).into_owned();

        match edit {
            DecodedEdit::Bytes {
                offset,
                old_len,
                new_bytes,
            } => byte_edit_from_bytes(*offset, &current, *old_len, new_bytes.as_slice()).ok(),
            DecodedEdit::Text { .. } | DecodedEdit::_Reserved => None,
        }
    }
}

#[test]
fn insert_and_get_inode() {
    let mut table = InodeTable::new();
    let id = table.insert(Arc::new(HeapByteSource::new(b"hello")));

    let inode = table.get(id).expect("inode should exist");
    assert_eq!(inode.mounts.len(), 0);
    assert_eq!(inode.bytes.len(), 5);
}

#[test]
fn mount_single_mount_only() {
    let mut table = InodeTable::new();
    let id = table.insert(Arc::new(HeapByteSource::new(Vec::<u8>::new())));

    let first = mount_with_buffer(
        &mut table,
        id,
        1,
        "default",
        Arc::new(CodecNoop) as Arc<dyn ContentCodec>,
    );
    assert!(first.is_ok());

    let second =
        mount_with_buffer(&mut table, id, 1, "hex", Arc::new(CodecNoop) as Arc<dyn ContentCodec>);
    assert!(matches!(
        second,
        Err(MountError::AlreadyMounted { inode_id }) if inode_id == id
    ));
}

#[test]
fn mount_and_unmount_roundtrip() {
    let mut table = InodeTable::new();
    let id = table.insert(Arc::new(HeapByteSource::new("abc".to_owned())));

    let handle = mount_with_buffer(
        &mut table,
        id,
        1,
        "default",
        Arc::new(CodecNoop) as Arc<dyn ContentCodec>,
    )
    .expect("first mount");
    assert_eq!(table.mount_ref(handle).expect("mounted").name, "default");

    let removed = table.unmount(handle);
    assert!(removed.is_ok());
    assert!(table.mount_ref(handle).is_none());
    assert_eq!(table.len(), 1);
}

#[test]
fn mount_after_unmount_allows_remount() {
    let mut table = InodeTable::new();
    let id = table.insert(Arc::new(HeapByteSource::new("abc".to_owned())));

    let first = mount_with_buffer(
        &mut table,
        id,
        1,
        "default",
        Arc::new(CodecNoop) as Arc<dyn ContentCodec>,
    )
    .expect("first mount");
    let _ = table.unmount(first);

    let second =
        mount_with_buffer(&mut table, id, 1, "hex", Arc::new(CodecNoop) as Arc<dyn ContentCodec>)
            .expect("second mount after unmount");
    assert_ne!(second, first);
    assert_eq!(table.mount_ref(second).expect("remounted").name, "hex");
}

#[test]
fn mount_handle_fields() {
    let mut table = InodeTable::new();
    let id = table.insert(Arc::new(HeapByteSource::new(vec![])));
    let first = mount_with_buffer(
        &mut table,
        id,
        1,
        "default",
        Arc::new(CodecNoop) as Arc<dyn ContentCodec>,
    )
    .expect("first mount");

    assert_eq!(first.buffer_id(), buf(1));
}

#[test]
fn lookup_helpers_round_trip() {
    let mut table = InodeTable::new();
    let id = table.insert(Arc::new(HeapByteSource::new(vec![1, 2, 3])));

    assert!(table.lookup_inode(id).is_some());
    assert!(
        table
            .lookup_inode(id)
            .is_some_and(|inode| inode.bytes.len() == 3)
    );

    let handle = mount_with_buffer(
        &mut table,
        id,
        1,
        "default",
        Arc::new(CodecByteTranslator) as Arc<dyn ContentCodec>,
    )
    .expect("mount");
    assert!(table.lookup_mount(handle).is_some());
    assert_eq!(table.lookup_mount(handle).expect("mounted").name, "default");

    let inode = table
        .lookup_inode_mut(id)
        .expect("inode exists for mutation");
    inode.set_bytes(vec![4, 5, 6]);
    assert_eq!(table.read_bytes(id).expect("bytes available"), vec![4, 5, 6]);
}

#[test]
fn missing_inode_mount_errors() {
    let mut table = InodeTable::new();
    let err = table
        .mount(
            InodeId::from_raw(999),
            buf(1),
            Mount::new("x", Arc::new(CodecNoop) as Arc<dyn ContentCodec>),
        )
        .expect_err("mount should fail");
    assert!(
        matches!(err, MountError::InodeNotFound { inode_id } if inode_id == InodeId::from_raw(999))
    );
}

#[test]
fn apply_edit_bytes_insertion_translated_via_codec() {
    let mut table = InodeTable::new();
    let id = table.insert(Arc::new(HeapByteSource::new(b"hello")));
    let handle = mount_with_buffer(
        &mut table,
        id,
        1,
        "default",
        Arc::new(CodecByteTranslator) as Arc<dyn ContentCodec>,
    )
    .expect("mount");

    let edit = DecodedEdit::Bytes {
        offset: 5,
        old_len: 0,
        new_bytes: b"!".to_vec(),
    };

    let byte_edit = table.apply_edit(handle, &edit).expect("apply edit");
    assert_eq!(byte_edit, ByteEdit::insert(5, b"!"));

    let inode = table.get(id).expect("inode exists");
    let written = inode.bytes.read(0..inode.bytes.len()).into_owned();
    assert_eq!(written, b"hello!".to_vec());
}

#[tracing_test::traced_test]
#[test]
fn mount_and_apply_edit_emit_debug_traces() {
    let mut table = InodeTable::new();
    let id = table.insert(Arc::new(HeapByteSource::new(b"hello")));
    let handle = mount_with_buffer(
        &mut table,
        id,
        1,
        "default",
        Arc::new(CodecByteTranslator) as Arc<dyn ContentCodec>,
    )
    .expect("mount");

    let edit = DecodedEdit::Bytes {
        offset: 5,
        old_len: 0,
        new_bytes: b"!".to_vec(),
    };

    assert_eq!(
        table
            .apply_edit(handle, &edit)
            .expect("decoded edit should apply"),
        ByteEdit::insert(5, b"!"),
    );
}

#[test]
fn apply_edit_bytes_variant_requires_codec_translation() {
    let mut table = InodeTable::new();
    let id = table.insert(Arc::new(HeapByteSource::new(b"hello")));
    let handle = mount_with_buffer(
        &mut table,
        id,
        1,
        "default",
        Arc::new(CodecReadOnly) as Arc<dyn ContentCodec>,
    )
    .expect("mount");

    let edit = DecodedEdit::Bytes {
        offset: 1,
        old_len: 0,
        new_bytes: b"!".to_vec(),
    };

    let err = table
        .apply_edit(handle, &edit)
        .expect_err("no translation path");
    assert!(matches!(err, EditError::ReadOnly));
}

#[test]
fn apply_edit_text_requires_translatable_codec() {
    let mut table = InodeTable::new();
    let id = table.insert(Arc::new(HeapByteSource::new(b"hello")));
    let handle = mount_with_buffer(
        &mut table,
        id,
        1,
        "default",
        Arc::new(CodecReadOnly) as Arc<dyn ContentCodec>,
    )
    .expect("mount");

    let edit = DecodedEdit::Text {
        start: reovim_types_text::Position::new(0, 0),
        end: reovim_types_text::Position::new(0, 0),
        replacement: String::new(),
    };

    let err = table
        .apply_edit(handle, &edit)
        .expect_err("read-only codecs cannot translate text edits");

    assert!(matches!(err, EditError::ReadOnly));
}

#[test]
fn unmount_unknown_handle_errors() {
    let mut table = InodeTable::new();
    let id = table.insert(Arc::new(HeapByteSource::new(vec![])));
    let _ = mount_with_buffer(
        &mut table,
        id,
        1,
        "default",
        Arc::new(CodecNoop) as Arc<dyn ContentCodec>,
    )
    .expect("mount");

    let missing = table.unmount(MountHandle::new(
        InodeId::from_raw(id.as_usize()),
        buf(1),
        MountId::from_u64(999),
    ));
    assert!(matches!(
        missing,
        Err(UmountError::MountNotFound {
            inode_id,
            mount_id
        }) if inode_id == id && mount_id.as_usize() == 999
    ));
}

#[test]
fn mount_ref_returns_none_after_unmount() {
    let mut table = InodeTable::new();
    let id = table.insert(Arc::new(HeapByteSource::new(vec![])));
    let handle = mount_with_buffer(
        &mut table,
        id,
        1,
        "default",
        Arc::new(CodecNoop) as Arc<dyn ContentCodec>,
    )
    .expect("first mount");
    assert!(table.mount_ref(handle).is_some());

    let _ = table.unmount(handle);
    assert!(table.mount_ref(handle).is_none());
}

// ── Phase 5 sub-commit 5a: multi-mount relaxation ─────────────────────────

#[test]
fn mount_additional_allows_second_mount_on_same_inode() {
    let mut table = InodeTable::new();
    let id = table.insert(Arc::new(HeapByteSource::new(b"hi".to_vec())));
    let first = mount_with_buffer(
        &mut table,
        id,
        1,
        "default",
        Arc::new(CodecNoop) as Arc<dyn ContentCodec>,
    )
    .expect("first mount");

    let second = table
        .mount_additional(
            id,
            buf(1),
            Mount::new("hex", Arc::new(CodecNoop) as Arc<dyn ContentCodec>),
        )
        .expect("second mount");

    assert_ne!(first, second);
    assert_eq!(
        table
            .lookup_inode(id)
            .map(|inode| inode.mounts.len())
            .unwrap_or_default(),
        2
    );
}

#[test]
fn mount_additional_rejects_missing_inode() {
    let mut table = InodeTable::new();
    let err = table
        .mount_additional(
            InodeId::from_raw(99),
            buf(1),
            Mount::new("default", Arc::new(CodecNoop) as Arc<dyn ContentCodec>),
        )
        .unwrap_err();
    assert!(matches!(err, MountError::InodeNotFound { .. }));
}

#[test]
fn mount_content_valid_default_true() {
    let mut table = InodeTable::new();
    let id = table.insert(Arc::new(HeapByteSource::new(b"hi".to_vec())));
    let handle = mount_with_buffer(
        &mut table,
        id,
        1,
        "default",
        Arc::new(CodecByteTranslator) as Arc<dyn ContentCodec>,
    )
    .expect("first mount");

    let mount = table.mount_ref(handle).expect("mount visible");
    assert!(mount.content_valid);
}

#[test]
fn apply_edit_marks_peer_mounts_stale_but_preserves_source() {
    let mut table = InodeTable::new();
    let id = table.insert(Arc::new(HeapByteSource::new(b"hello".to_vec())));
    let source = mount_with_buffer(
        &mut table,
        id,
        1,
        "default",
        Arc::new(CodecByteTranslator) as Arc<dyn ContentCodec>,
    )
    .expect("first mount");
    let peer = table
        .mount_additional(
            id,
            buf(1),
            Mount::new("hex", Arc::new(CodecByteTranslator) as Arc<dyn ContentCodec>),
        )
        .expect("second mount");

    table
        .apply_edit(
            source,
            &DecodedEdit::Bytes {
                offset: 0,
                old_len: 0,
                new_bytes: b"X".to_vec(),
            },
        )
        .expect("apply edit");

    let inode = table.lookup_inode(id).expect("inode present");
    let source_mount = inode
        .mounts
        .get(&source_mount_id(source))
        .expect("source mount present");
    let peer_mount = inode
        .mounts
        .get(&source_mount_id(peer))
        .expect("peer mount present");

    assert!(source_mount.content_valid, "source mount stays valid after its own edit");
    assert!(!peer_mount.content_valid, "peer mount is marked stale after sibling edit");
}

#[test]
fn apply_edit_with_no_peers_does_not_panic() {
    let mut table = InodeTable::new();
    let id = table.insert(Arc::new(HeapByteSource::new(b"hello".to_vec())));
    let handle = mount_with_buffer(
        &mut table,
        id,
        1,
        "default",
        Arc::new(CodecByteTranslator) as Arc<dyn ContentCodec>,
    )
    .expect("first mount");

    table
        .apply_edit(
            handle,
            &DecodedEdit::Bytes {
                offset: 0,
                old_len: 0,
                new_bytes: b"!".to_vec(),
            },
        )
        .expect("apply edit");

    let inode = table.lookup_inode(id).expect("inode present");
    let only = inode
        .mounts
        .get(&source_mount_id(handle))
        .expect("only mount present");
    assert!(only.content_valid);
}

#[test]
fn flush_no_path_and_no_override_returns_invalid_input() {
    let mut table = InodeTable::new();
    let id = table.insert(Arc::new(HeapByteSource::new(b"hi".to_vec())));
    let handle = mount_with_buffer(
        &mut table,
        id,
        1,
        "default",
        Arc::new(CodecNoop) as Arc<dyn ContentCodec>,
    )
    .expect("first mount");

    let err = table.flush(handle_mount_id(handle), None).unwrap_err();
    assert_eq!(err.kind(), std::io::ErrorKind::InvalidInput);
}

#[test]
fn flush_writes_inode_bytes_to_path_override() {
    let dir = tempfile::tempdir().expect("tempdir");
    let target = dir.path().join("out.bin");

    let mut table = InodeTable::new();
    let id = table.insert(Arc::new(HeapByteSource::new(b"hello 5d".to_vec())));
    let handle = mount_with_buffer(
        &mut table,
        id,
        1,
        "default",
        Arc::new(CodecNoop) as Arc<dyn ContentCodec>,
    )
    .expect("first mount");

    table
        .flush(handle_mount_id(handle), Some(&target))
        .expect("flush succeeds");
    let written = std::fs::read(&target).expect("file readable");
    assert_eq!(written, b"hello 5d");

    // Inode path should have been populated by the path override.
    let inode = table.lookup_inode(id).expect("inode present");
    assert_eq!(inode.path.as_deref(), Some(target.as_path()));
}

#[test]
fn flush_uses_inode_path_when_override_is_none() {
    let dir = tempfile::tempdir().expect("tempdir");
    let target = dir.path().join("preset.bin");

    let mut table = InodeTable::new();
    let id = table.insert_with_path(
        Arc::new(HeapByteSource::new(b"preset".to_vec())),
        Arc::<std::path::Path>::from(target.clone()),
    );
    table.bind_file(buf(1), id);
    let handle = table
        .mount(id, buf(1), Mount::new("default", Arc::new(CodecNoop) as Arc<dyn ContentCodec>))
        .expect("first mount");

    table
        .flush(handle_mount_id(handle), None)
        .expect("flush succeeds");
    let written = std::fs::read(&target).expect("file readable");
    assert_eq!(written, b"preset");
}

#[test]
fn flush_missing_mount_returns_not_found() {
    let mut table = InodeTable::new();
    // Bogus mount id with no matching entry in mount_idx.
    let bogus = MountId::from_u64(987_654);
    let err = table.flush(bogus, None).unwrap_err();
    assert_eq!(err.kind(), std::io::ErrorKind::NotFound);
}

/// Helper: extract `MountId` from a `MountHandle` for test assertions.
fn source_mount_id(handle: MountHandle) -> MountId {
    handle_mount_id(handle)
}

fn handle_mount_id(handle: MountHandle) -> MountId {
    // `MountHandle::mount` is private; go through `InodeTable::lookup_mount`
    // indirectly by reading the observable side effect of the apply_edit
    // pair in tests. Since we need direct access here, expose via a
    // test-only accessor on MountHandle (added below).
    handle.mount_id_for_tests()
}
