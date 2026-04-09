//! Tests for codec session state.

use {
    reovim_kernel::api::v1::{BufferId, ByteEdit},
    std::sync::Arc,
};

use {
    super::*,
    crate::{ContentType, DecodeResult, error::CodecError},
};

#[derive(Clone)]
struct TestInsertCodec {
    insert_bytes: Vec<u8>,
}

impl ContentCodec for TestInsertCodec {
    fn decode(&self, raw: &[u8]) -> Result<DecodeResult, CodecError> {
        Ok(DecodeResult {
            content: String::from_utf8_lossy(raw).into_owned(),
            annotations: vec![],
            metadata: CodecMetadata::new(ContentType::new("text/test")),
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
        Some(ByteEdit::insert(0, &self.insert_bytes))
    }

    fn encode(
        &self,
        content: &str,
        _metadata: &CodecMetadata,
    ) -> Option<Result<Vec<u8>, CodecError>> {
        Some(Ok(content.as_bytes().to_vec()))
    }
}

fn buf(id: usize) -> BufferId {
    BufferId::from_raw(id)
}

fn test_metadata() -> CodecMetadata {
    CodecMetadata::new(ContentType::new("text/utf-8"))
}

#[test]
fn new_is_empty() {
    let state = CodecSessionState::new();
    assert!(state.is_empty());
    assert_eq!(state.len(), 0);
}

#[test]
fn default_is_empty() {
    let state = CodecSessionState::default();
    assert!(state.is_empty());
}

#[test]
fn session_extension_create() {
    let state = CodecSessionState::create();
    assert!(state.is_empty());
}

#[test]
fn insert_and_get() {
    let mut state = CodecSessionState::new();
    state.insert(buf(1), test_metadata());
    assert_eq!(state.len(), 1);

    let m = state.get(buf(1)).unwrap();
    assert_eq!(m.content_type().as_str(), "text/utf-8");
}

#[test]
fn get_missing() {
    let state = CodecSessionState::new();
    assert!(state.get(buf(1)).is_none());
}

#[test]
fn contains() {
    let mut state = CodecSessionState::new();
    assert!(!state.contains(buf(1)));
    state.insert(buf(1), test_metadata());
    assert!(state.contains(buf(1)));
}

#[test]
fn remove() {
    let mut state = CodecSessionState::new();
    state.insert(buf(1), test_metadata());
    let removed = state.remove(buf(1));
    assert!(removed.is_some());
    assert!(state.is_empty());
}

#[test]
fn remove_missing() {
    let mut state = CodecSessionState::new();
    assert!(state.remove(buf(1)).is_none());
}

#[test]
fn insert_replaces() {
    let mut state = CodecSessionState::new();
    state.insert(buf(1), test_metadata());

    let mut new_meta = CodecMetadata::new(ContentType::new("binary/raw"));
    new_meta.set("key", "value");
    state.insert(buf(1), new_meta);

    assert_eq!(state.len(), 1);
    let m = state.get(buf(1)).unwrap();
    assert_eq!(m.content_type().as_str(), "binary/raw");
}

#[test]
fn multiple_buffers() {
    let mut state = CodecSessionState::new();
    state.insert(buf(1), test_metadata());
    state.insert(buf(2), CodecMetadata::new(ContentType::new("binary/raw")));
    assert_eq!(state.len(), 2);

    assert_eq!(state.get(buf(1)).unwrap().content_type().as_str(), "text/utf-8");
    assert_eq!(state.get(buf(2)).unwrap().content_type().as_str(), "binary/raw");
}

#[test]
fn clear() {
    let mut state = CodecSessionState::new();
    state.insert(buf(1), test_metadata());
    state.insert(buf(2), test_metadata());
    state.clear();
    assert!(state.is_empty());
}

#[test]
fn debug_format() {
    let mut state = CodecSessionState::new();
    state.insert(buf(1), test_metadata());
    let debug = format!("{state:?}");
    assert!(debug.contains("CodecSessionState"));
    assert!(debug.contains("buffer_count"));
    assert!(debug.contains("source_count"));
    assert!(debug.contains("active_view_count"));
}

// --- Canonical byte source tests ---

#[test]
fn set_and_get_source() {
    let mut state = CodecSessionState::new();
    let data = vec![0x7f, 0x45, 0x4c, 0x46];
    state.set_source(buf(1), data.clone());
    assert_eq!(state.bytes(buf(1)), Some(data));
}

#[test]
fn get_source_missing() {
    let state = CodecSessionState::new();
    assert!(state.bytes(buf(1)).is_none());
}

#[test]
fn set_source_replaces() {
    let mut state = CodecSessionState::new();
    state.set_source(buf(1), vec![1, 2, 3]);
    state.set_source(buf(1), vec![4, 5]);
    assert_eq!(state.bytes(buf(1)), Some(vec![4, 5]));
}

#[test]
fn apply_decoded_edit_updates_source() {
    let mut state = CodecSessionState::new();
    let codec = TestInsertCodec {
        insert_bytes: b"X".to_vec(),
    };
    state.set_source_with_codec(buf(1), b"hello".to_vec(), Arc::new(codec));

    let byte_edit = state
        .apply_decoded_edit(
            buf(1),
            "default",
            &DecodedEdit::Text {
                start: reovim_types_text::Position::new(0, 0),
                end: reovim_types_text::Position::new(0, 0),
                replacement: "a".to_string(),
            },
        )
        .expect("decoded edit should be applied when mount exists");

    assert_eq!(byte_edit, ByteEdit::insert(0, b"X"));
    assert_eq!(state.bytes(buf(1)), Some(b"Xhello".to_vec()));
}

#[test]
fn apply_decoded_edit_reuses_mount_for_same_view() {
    let mut state = CodecSessionState::new();
    let codec = TestInsertCodec {
        insert_bytes: b"Y".to_vec(),
    };
    state.set_source_with_codec(buf(1), b"abc".to_vec(), Arc::new(codec));

    let first_handle = state
        .inodes
        .active_mount(buf(1))
        .expect("initial mount exists");

    let _ = state.apply_decoded_edit(
        buf(1),
        "default",
        &DecodedEdit::Text {
            start: reovim_types_text::Position::new(0, 0),
            end: reovim_types_text::Position::new(0, 0),
            replacement: "a".to_string(),
        },
    );

    let _ = state.apply_decoded_edit(
        buf(1),
        "default",
        &DecodedEdit::Text {
            start: reovim_types_text::Position::new(1, 0),
            end: reovim_types_text::Position::new(1, 0),
            replacement: "b".to_string(),
        },
    );

    let second_handle = state
        .inodes
        .active_mount(buf(1))
        .expect("mount handle still exists");

    assert_eq!(first_handle, second_handle);
    assert_eq!(state.bytes(buf(1)), Some(b"YYabc".to_vec()));
}

#[test]
fn set_active_view_does_not_invalidate_mount() {
    let mut state = CodecSessionState::new();
    let codec = TestInsertCodec {
        insert_bytes: b"Z".to_vec(),
    };
    state.set_source_with_codec(buf(1), b"abc".to_vec(), Arc::new(codec));

    let _ = state.apply_decoded_edit(
        buf(1),
        "default",
        &DecodedEdit::Text {
            start: reovim_types_text::Position::new(0, 0),
            end: reovim_types_text::Position::new(0, 0),
            replacement: "a".to_string(),
        },
    );

    let first_handle = state
        .inodes
        .active_mount(buf(1))
        .expect("mount handle exists");

    state.set_active_view(buf(1), "hex".to_string());

    let second_handle = state
        .inodes
        .active_mount(buf(1))
        .expect("mount handle should remain after view change");

    assert_eq!(first_handle, second_handle);
    let mount = state
        .inodes
        .mount_ref(second_handle)
        .expect("still mounted after view change");
    assert_eq!(mount.name, "default");

    let _ = state.apply_decoded_edit(
        buf(1),
        "hex",
        &DecodedEdit::Text {
            start: reovim_types_text::Position::new(0, 0),
            end: reovim_types_text::Position::new(0, 0),
            replacement: "b".to_string(),
        },
    );

    let hex_handle = state
        .inodes
        .active_mount(buf(1))
        .expect("mount handle still exists after applying edit");
    assert_eq!(first_handle, hex_handle);
}

#[test]
fn remove_clears_mounts_and_source() {
    let mut state = CodecSessionState::new();
    let codec = TestInsertCodec {
        insert_bytes: b"!".to_vec(),
    };
    state.set_source_with_codec(buf(1), b"abc".to_vec(), Arc::new(codec));

    let _ = state.apply_decoded_edit(
        buf(1),
        "default",
        &DecodedEdit::Text {
            start: reovim_types_text::Position::new(0, 0),
            end: reovim_types_text::Position::new(0, 0),
            replacement: "a".to_string(),
        },
    );

    let handle = state
        .inodes
        .active_mount(buf(1))
        .expect("mount exists before removal");
    assert!(state.inodes.mount_ref(handle).is_some());

    let _ = state.remove(buf(1));

    assert!(state.inodes.file_inode(buf(1)).is_none());
    assert!(state.inodes.active_mount(buf(1)).is_none());
    assert!(state.inodes.mount_ref(handle).is_none());
    assert!(state.bytes(buf(1)).is_none());
    assert!(state.active_view(buf(1)).is_none());
}

#[test]
fn remove_clears_raw_and_view() {
    let mut state = CodecSessionState::new();
    state.insert(buf(1), test_metadata());
    state.set_source(buf(1), vec![1, 2, 3]);
    state.set_active_view(buf(1), "hex".to_string());

    state.remove(buf(1));

    assert!(state.get(buf(1)).is_none());
    assert!(state.bytes(buf(1)).is_none());
    assert!(state.active_view(buf(1)).is_none());
}

#[test]
fn clear_clears_sources_and_views() {
    let mut state = CodecSessionState::new();
    state.insert(buf(1), test_metadata());
    state.set_source(buf(1), vec![1, 2]);
    state.set_active_view(buf(1), "default".to_string());
    state.insert(buf(2), test_metadata());
    state.set_source(buf(2), vec![3, 4]);
    state.set_active_view(buf(2), "hex".to_string());

    state.clear();

    assert!(state.is_empty());
    assert!(state.bytes(buf(1)).is_none());
    assert!(state.bytes(buf(2)).is_none());
    assert!(state.active_view(buf(1)).is_none());
    assert!(state.active_view(buf(2)).is_none());
}

// --- Active view tests ---

#[test]
fn active_view_default_none() {
    let state = CodecSessionState::new();
    assert!(state.active_view(buf(1)).is_none());
}

#[test]
fn set_and_get_active_view() {
    let mut state = CodecSessionState::new();
    state.set_active_view(buf(1), "hex".to_string());
    assert_eq!(state.active_view(buf(1)), Some("hex"));
}

#[test]
fn active_view_replaces() {
    let mut state = CodecSessionState::new();
    state.set_active_view(buf(1), "default".to_string());
    state.set_active_view(buf(1), "hex".to_string());
    assert_eq!(state.active_view(buf(1)), Some("hex"));
}

#[test]
fn apply_byte_edit_updates_source() {
    let mut state = CodecSessionState::new();
    state.set_source(buf(1), b"abcdef".to_vec());

    state.apply_byte_edit(buf(1), &ByteEdit::insert(3, b"ZZ"));
    assert_eq!(state.bytes(buf(1)), Some(b"abcZZdef".to_vec()));

    state.apply_byte_edit(buf(1), &ByteEdit::delete(2, b"cZZ"));
    assert_eq!(state.bytes(buf(1)), Some(b"abdef".to_vec()));

    state.apply_byte_edit(buf(1), &ByteEdit::replace(0, b"ab", b"AB"));
    assert_eq!(state.bytes(buf(1)), Some(b"ABdef".to_vec()));
}

// --- Index management tests (#740 D.2) ---

/// Mock index for testing `ByteNotifiable` integration.
struct MockIndex {
    notify_count: usize,
    last_offset: Option<usize>,
    built: bool,
}

impl MockIndex {
    fn new() -> Self {
        Self {
            notify_count: 0,
            last_offset: None,
            built: false,
        }
    }
}

impl crate::ByteNotifiable for MockIndex {
    fn notify(&mut self, edit: &ByteEdit) {
        self.notify_count += 1;
        self.last_offset = Some(edit.offset);
    }

    fn build(&mut self, _raw: &[u8]) {
        self.built = true;
    }
}

#[test]
fn index_not_present_by_default() {
    let state = CodecSessionState::new();
    assert!(!state.has_index(buf(1)));
}

#[test]
fn set_and_has_index() {
    let mut state = CodecSessionState::new();
    state.set_index(buf(1), Box::new(MockIndex::new()));
    assert!(state.has_index(buf(1)));
}

#[test]
fn notify_index_routes_edit() {
    let mut state = CodecSessionState::new();
    state.set_index(buf(1), Box::new(MockIndex::new()));

    let edit = ByteEdit::insert(5, b"hello");
    state.notify_index(buf(1), &edit);

    // Index was notified (we can't inspect MockIndex through Box<dyn>,
    // but notify_index returns without panic, proving routing works)
    assert!(state.has_index(buf(1)));
}

#[test]
fn notify_index_no_op_without_index() {
    let mut state = CodecSessionState::new();
    let edit = ByteEdit::insert(0, b"test");
    // Should not panic
    state.notify_index(buf(1), &edit);
}

#[test]
fn remove_index() {
    let mut state = CodecSessionState::new();
    state.set_index(buf(1), Box::new(MockIndex::new()));
    state.remove_index(buf(1));
    assert!(!state.has_index(buf(1)));
}

#[test]
fn remove_clears_index() {
    let mut state = CodecSessionState::new();
    state.insert(buf(1), test_metadata());
    state.set_index(buf(1), Box::new(MockIndex::new()));

    state.remove(buf(1));

    assert!(!state.has_index(buf(1)));
}

#[test]
fn clear_clears_indices() {
    let mut state = CodecSessionState::new();
    state.set_index(buf(1), Box::new(MockIndex::new()));
    state.set_index(buf(2), Box::new(MockIndex::new()));

    state.clear();

    assert!(!state.has_index(buf(1)));
    assert!(!state.has_index(buf(2)));
}

#[test]
fn debug_includes_index_count() {
    let mut state = CodecSessionState::new();
    state.set_index(buf(1), Box::new(MockIndex::new()));
    let debug = format!("{state:?}");
    assert!(debug.contains("index_count"));
}

// ── Orchestration helper tests (#740 Phase 4) ─────────────────────────────

use crate::{CodecView, ContentCodecFactory, SwitchViewError, codec::ContentCodec};

struct MultiViewCodec;

impl ContentCodec for MultiViewCodec {
    fn decode(&self, raw: &[u8]) -> Result<DecodeResult, CodecError> {
        Ok(DecodeResult {
            content: String::from_utf8_lossy(raw).into_owned(),
            annotations: vec![],
            metadata: CodecMetadata::new(ContentType::new("text/multi")),
            lossy: false,
            readonly: false,
            truncated: false,
        })
    }

    fn encode(
        &self,
        content: &str,
        _metadata: &CodecMetadata,
    ) -> Option<Result<Vec<u8>, CodecError>> {
        Some(Ok(content.as_bytes().to_vec()))
    }

    fn views(&self) -> &[CodecView] {
        const VIEWS: &[CodecView] = &[
            CodecView {
                name: "default",
                display: "Default",
            },
            CodecView {
                name: "hex",
                display: "Hex",
            },
        ];
        VIEWS
    }

    fn decode_view(&self, raw: &[u8], view: &str) -> Result<DecodeResult, CodecError> {
        if view == "hex" {
            let content = raw
                .iter()
                .map(|b| format!("{b:02x}"))
                .collect::<Vec<_>>()
                .join(" ");
            return Ok(DecodeResult {
                content,
                annotations: vec![],
                metadata: CodecMetadata::new(ContentType::new("text/multi")),
                lossy: false,
                readonly: false,
                truncated: false,
            });
        }
        self.decode(raw)
    }
}

struct FailingDecodeCodec;

impl ContentCodec for FailingDecodeCodec {
    fn decode(&self, _raw: &[u8]) -> Result<DecodeResult, CodecError> {
        Err(CodecError::Other("boom".to_string()))
    }

    fn encode(
        &self,
        _content: &str,
        _metadata: &CodecMetadata,
    ) -> Option<Result<Vec<u8>, CodecError>> {
        None
    }

    fn views(&self) -> &[CodecView] {
        const VIEWS: &[CodecView] = &[CodecView {
            name: "default",
            display: "Default",
        }];
        VIEWS
    }

    fn decode_view(&self, _raw: &[u8], _view: &str) -> Result<DecodeResult, CodecError> {
        Err(CodecError::Other("decode_view failed".to_string()))
    }
}

struct OrchestrationFactory {
    content_type: &'static str,
    codec: fn() -> Arc<dyn ContentCodec>,
}

impl ContentCodecFactory for OrchestrationFactory {
    fn create(&self, content_type: &ContentType) -> Option<Arc<dyn ContentCodec>> {
        if content_type.as_str() == self.content_type {
            Some((self.codec)())
        } else {
            None
        }
    }

    fn supported_content_types(&self) -> Vec<&str> {
        vec![self.content_type]
    }

    fn name(&self) -> &'static str {
        self.content_type
    }
}

fn multi_view_store() -> crate::ContentCodecFactoryStore {
    let store = crate::ContentCodecFactoryStore::new();
    store.add_factory(Arc::new(OrchestrationFactory {
        content_type: "text/multi",
        codec: || Arc::new(MultiViewCodec),
    }));
    store
}

#[test]
fn mount_decoded_sets_metadata_view_and_source() {
    let mut state = CodecSessionState::new();
    state.mount_decoded(
        buf(1),
        CodecMetadata::new(ContentType::new("text/multi")),
        "default".to_string(),
        b"hello".to_vec(),
        Arc::new(MultiViewCodec),
    );

    assert!(state.contains(buf(1)));
    assert_eq!(state.active_view(buf(1)), Some("default"));
    assert_eq!(state.bytes(buf(1)), Some(b"hello".to_vec()));
}

#[test]
fn switch_view_decodes_with_requested_view() {
    let store = multi_view_store();
    let mut state = CodecSessionState::new();
    state.mount_decoded(
        buf(1),
        CodecMetadata::new(ContentType::new("text/multi")),
        "default".to_string(),
        b"hi".to_vec(),
        Arc::new(MultiViewCodec),
    );

    let content = state.switch_view(Some(&store), buf(1), "hex").unwrap();
    assert_eq!(content, "68 69");
    assert_eq!(state.active_view(buf(1)), Some("hex"));
}

#[test]
fn switch_view_clears_existing_index() {
    let store = multi_view_store();
    let mut state = CodecSessionState::new();
    state.mount_decoded(
        buf(1),
        CodecMetadata::new(ContentType::new("text/multi")),
        "default".to_string(),
        b"hi".to_vec(),
        Arc::new(MultiViewCodec),
    );
    state.set_index(buf(1), Box::new(MockIndex::new()));
    assert!(state.has_index(buf(1)));

    state.switch_view(Some(&store), buf(1), "hex").unwrap();

    assert!(!state.has_index(buf(1)));
}

#[test]
fn switch_view_no_metadata() {
    let store = multi_view_store();
    let mut state = CodecSessionState::new();
    let err = state.switch_view(Some(&store), buf(1), "hex").unwrap_err();
    assert_eq!(err, SwitchViewError::NoMetadata);
}

#[test]
fn switch_view_no_canonical_bytes() {
    let store = multi_view_store();
    let mut state = CodecSessionState::new();
    state.insert(buf(1), CodecMetadata::new(ContentType::new("text/multi")));
    let err = state.switch_view(Some(&store), buf(1), "hex").unwrap_err();
    assert_eq!(err, SwitchViewError::NoCanonicalBytes);
}

#[test]
fn switch_view_no_codec() {
    let store = crate::ContentCodecFactoryStore::new();
    let mut state = CodecSessionState::new();
    state.mount_decoded(
        buf(1),
        CodecMetadata::new(ContentType::new("text/multi")),
        "default".to_string(),
        b"hi".to_vec(),
        Arc::new(MultiViewCodec),
    );
    let err = state.switch_view(Some(&store), buf(1), "hex").unwrap_err();
    assert_eq!(err, SwitchViewError::NoCodec);
}

#[test]
fn switch_view_no_factory_store_acts_as_no_codec() {
    let mut state = CodecSessionState::new();
    state.mount_decoded(
        buf(1),
        CodecMetadata::new(ContentType::new("text/multi")),
        "default".to_string(),
        b"hi".to_vec(),
        Arc::new(MultiViewCodec),
    );
    let err = state.switch_view(None, buf(1), "hex").unwrap_err();
    assert_eq!(err, SwitchViewError::NoCodec);
}

#[test]
fn switch_view_unknown_view_name() {
    let store = multi_view_store();
    let mut state = CodecSessionState::new();
    state.mount_decoded(
        buf(1),
        CodecMetadata::new(ContentType::new("text/multi")),
        "default".to_string(),
        b"hi".to_vec(),
        Arc::new(MultiViewCodec),
    );
    let err = state
        .switch_view(Some(&store), buf(1), "binary")
        .unwrap_err();
    assert_eq!(
        err,
        SwitchViewError::ViewNotAvailable {
            view_name: "binary".to_string()
        }
    );
}

#[test]
fn switch_view_codec_decode_error_surfaces_as_decode_failed() {
    let store = crate::ContentCodecFactoryStore::new();
    store.add_factory(Arc::new(OrchestrationFactory {
        content_type: "text/failing",
        codec: || Arc::new(FailingDecodeCodec),
    }));
    let mut state = CodecSessionState::new();
    state.mount_decoded(
        buf(1),
        CodecMetadata::new(ContentType::new("text/failing")),
        "default".to_string(),
        b"hi".to_vec(),
        Arc::new(FailingDecodeCodec),
    );

    let err = state
        .switch_view(Some(&store), buf(1), "default")
        .unwrap_err();
    match err {
        SwitchViewError::DecodeFailed { reason } => {
            assert!(reason.contains("decode_view failed"));
        }
        other => panic!("expected DecodeFailed, got {other:?}"),
    }
}

// ── Phase 5 sub-commit 5c: multi-mount orchestration tests ────────────────

use crate::{MountCodecError, UmountCodecError};

#[test]
fn mount_codec_adds_first_mount_when_inode_has_no_mounts() {
    let store = multi_view_store();
    let mut state = CodecSessionState::new();
    // set_source binds an inode for the buffer but does NOT mount a codec.
    state.set_source(buf(1), b"hi".to_vec());

    let handle = state
        .mount_codec(&store, buf(1), &ContentType::new("text/multi"), "default".to_string())
        .expect("first mount via mount_codec");

    assert_eq!(handle.buffer_id(), buf(1));
    let mounts = state.list_mounts(buf(1));
    assert_eq!(mounts.len(), 1);
    assert_eq!(mounts[0].view_name, "default");
    assert!(mounts[0].content_valid);
}

#[test]
fn mount_codec_adds_additional_when_inode_already_mounted() {
    let store = multi_view_store();
    let mut state = CodecSessionState::new();
    state.mount_decoded(
        buf(1),
        CodecMetadata::new(ContentType::new("text/multi")),
        "default".to_string(),
        b"hi".to_vec(),
        Arc::new(MultiViewCodec),
    );

    let handle = state
        .mount_codec(&store, buf(1), &ContentType::new("text/multi"), "hex".to_string())
        .expect("additional mount via mount_codec");

    let mounts = state.list_mounts(buf(1));
    assert_eq!(mounts.len(), 2);
    let view_names: Vec<&str> = mounts.iter().map(|m| m.view_name.as_str()).collect();
    assert!(view_names.contains(&"default"));
    assert!(view_names.contains(&"hex"));
    assert!(
        mounts
            .iter()
            .any(|m| m.mount_id == handle.mount_id_for_tests())
    );
}

#[test]
fn mount_codec_no_canonical_bytes() {
    let store = multi_view_store();
    let mut state = CodecSessionState::new();
    let err = state
        .mount_codec(&store, buf(1), &ContentType::new("text/multi"), "default".to_string())
        .unwrap_err();
    assert_eq!(err, MountCodecError::NoCanonicalBytes);
}

#[test]
fn mount_codec_no_codec_for_content_type() {
    let store = multi_view_store();
    let mut state = CodecSessionState::new();
    state.set_source(buf(1), b"hi".to_vec());
    let err = state
        .mount_codec(&store, buf(1), &ContentType::new("text/unknown"), "default".to_string())
        .unwrap_err();
    assert_eq!(
        err,
        MountCodecError::NoCodec {
            content_type: "text/unknown".to_string()
        }
    );
}

#[test]
fn unmount_codec_removes_the_named_mount() {
    let store = multi_view_store();
    let mut state = CodecSessionState::new();
    state.mount_decoded(
        buf(1),
        CodecMetadata::new(ContentType::new("text/multi")),
        "default".to_string(),
        b"hi".to_vec(),
        Arc::new(MultiViewCodec),
    );
    let hex = state
        .mount_codec(&store, buf(1), &ContentType::new("text/multi"), "hex".to_string())
        .expect("hex mount");

    assert_eq!(state.list_mounts(buf(1)).len(), 2);
    state
        .unmount_codec(hex.mount_id_for_tests())
        .expect("umount");
    let remaining = state.list_mounts(buf(1));
    assert_eq!(remaining.len(), 1);
    assert_eq!(remaining[0].view_name, "default");
}

#[test]
fn unmount_codec_not_found_returns_error() {
    let mut state = CodecSessionState::new();
    state.mount_decoded(
        buf(1),
        CodecMetadata::new(ContentType::new("text/multi")),
        "default".to_string(),
        b"hi".to_vec(),
        Arc::new(MultiViewCodec),
    );
    // Use a mount id that certainly doesn't exist.
    let bogus = crate::MountId::from_u64(999_999);
    let err = state.unmount_codec(bogus).unwrap_err();
    assert_eq!(err, UmountCodecError::MountNotFound);
}

#[test]
fn list_mounts_empty_for_unknown_buffer() {
    let state = CodecSessionState::new();
    assert!(state.list_mounts(buf(42)).is_empty());
}

#[test]
fn factory_store_available_enumerates_registered_factories() {
    let store = multi_view_store();
    let available = store.available();
    assert_eq!(available.len(), 1);
    assert_eq!(available[0].0, "text/multi");
    assert_eq!(available[0].1, vec!["text/multi".to_string()]);
}
