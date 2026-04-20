use {
    reovim_content_codec::{
        CodecError, CodecMetadata, ContentClassifier, ContentClassifierStore,
        ContentCodecFactoryStore, ContentType, DecodeResult,
    },
    std::sync::Arc,
};

use super::*;

// ============================================================================
// Codec pipeline branch coverage helpers
// ============================================================================

/// A classifier that always identifies bytes as the given content type.
struct AlwaysMatchClassifier {
    content_type: ContentType,
}

impl ContentClassifier for AlwaysMatchClassifier {
    fn classify(&self, _raw: &[u8], _path: &str) -> Option<ContentType> {
        Some(self.content_type.clone())
    }

    fn priority(&self) -> u8 {
        100
    }

    fn name(&self) -> &'static str {
        "always-match"
    }
}

/// A codec that always returns a decode error.
struct FailingCodec;

impl reovim_content_codec::ContentCodec for FailingCodec {
    fn decode(&self, _raw: &[u8]) -> Result<DecodeResult, CodecError> {
        Err(CodecError::Other("forced decode failure".to_string()))
    }
}

struct FailingCodecFactory;

impl reovim_content_codec::ContentCodecFactory for FailingCodecFactory {
    fn create(
        &self,
        content_type: &ContentType,
    ) -> Option<Arc<dyn reovim_content_codec::ContentCodec>> {
        if content_type.as_str() == "text/test" {
            Some(Arc::new(FailingCodec))
        } else {
            None
        }
    }

    fn supported_content_types(&self) -> Vec<&str> {
        vec!["text/test"]
    }

    fn name(&self) -> &'static str {
        "failing-codec"
    }
}

/// A codec that decodes as UTF-8, optionally setting truncated = true.
struct SimpleTestCodec {
    truncated: bool,
}

impl reovim_content_codec::ContentCodec for SimpleTestCodec {
    fn decode(&self, raw: &[u8]) -> Result<DecodeResult, CodecError> {
        Ok(DecodeResult {
            content: String::from_utf8_lossy(raw).into_owned(),
            annotations: vec![],
            metadata: CodecMetadata::new(ContentType::new("text/test")),
            lossy: false,
            readonly: false,
            truncated: self.truncated,
        })
    }
}

struct SimpleTestFactory {
    truncated: bool,
}

impl reovim_content_codec::ContentCodecFactory for SimpleTestFactory {
    fn create(
        &self,
        content_type: &ContentType,
    ) -> Option<Arc<dyn reovim_content_codec::ContentCodec>> {
        if content_type.as_str() == "text/test" {
            Some(Arc::new(SimpleTestCodec {
                truncated: self.truncated,
            }))
        } else {
            None
        }
    }

    fn supported_content_types(&self) -> Vec<&str> {
        vec!["text/test"]
    }

    fn name(&self) -> &'static str {
        "simple-test"
    }
}

/// Factory that recognises no content type (simulates missing codec).
struct NoMatchFactory;

impl reovim_content_codec::ContentCodecFactory for NoMatchFactory {
    fn create(
        &self,
        _content_type: &ContentType,
    ) -> Option<Arc<dyn reovim_content_codec::ContentCodec>> {
        None
    }

    fn supported_content_types(&self) -> Vec<&str> {
        vec![]
    }

    fn name(&self) -> &'static str {
        "no-match"
    }
}

#[test]
fn test_edit_command_id() {
    let cmd = EditCommand;
    assert_eq!(cmd.id().name(), "edit");
    assert_eq!(cmd.id().module().as_str(), "commands");
}

#[test]
fn test_edit_command_names() {
    let cmd = EditCommand;
    let names = cmd.names();
    assert_eq!(names.len(), 2);
    assert!(names.contains(&"e"));
    assert!(names.contains(&"edit"));
}

#[test]
fn test_edit_command_description() {
    let cmd = EditCommand;
    let desc = cmd.description();
    assert!(!desc.is_empty());
    assert!(desc.contains("Edit"));
}

#[test]
fn test_edit_command_args() {
    let cmd = EditCommand;
    let args = cmd.args();
    assert_eq!(args.len(), 1);
    assert_eq!(args[0].name, "file");
    assert_eq!(args[0].kind, ArgKind::Rest);
    assert!(!args[0].required);
}

#[test]
fn test_edit_command_complete_returns_empty() {
    let cmd = EditCommand;
    let completions = cmd.complete("some_path");
    assert!(completions.is_empty());
}

#[test]
fn test_edit_command_debug() {
    let cmd = EditCommand;
    let debug = format!("{cmd:?}");
    assert!(debug.contains("EditCommand"));
}

#[test]
fn test_edit_command_is_send_sync() {
    fn assert_send_sync<T: Send + Sync>() {}
    assert_send_sync::<EditCommand>();
}

#[test]
fn test_edit_command_clone_copy() {
    let cmd = EditCommand;
    let copy = cmd;
    assert_eq!(cmd.id(), copy.id());
}

// ========================================================================
// Execute tests (using TestSessionRuntime + MockVfs)
// ========================================================================

#[test]
fn test_edit_execute_no_filename() {
    use reovim_driver_text_session::testing::TestSessionRuntime;

    let mut harness = TestSessionRuntime::with_buffer("hello");
    harness.with_runtime(|runtime| {
        let cmd = EditCommand;
        let ctx = CommandContext::new();
        let result = cmd.execute(runtime, &ctx);
        assert!(result.is_error());
    });
}

#[test]
fn test_edit_execute_no_buffer_id() {
    use reovim_driver_text_session::testing::TestSessionRuntime;

    let mut harness = TestSessionRuntime::with_buffer("hello");
    harness.with_runtime(|runtime| {
        let cmd = EditCommand;
        let mut ctx = CommandContext::new();
        ctx.set("file", reovim_driver_command::ArgValue::String("test.rs".to_string()));
        // No buffer_id set
        let result = cmd.execute(runtime, &ctx);
        assert!(result.is_error());
    });
}

#[test]
fn test_edit_execute_no_vfs() {
    use reovim_driver_text_session::testing::TestSessionRuntime;

    let mut harness = TestSessionRuntime::with_buffer("hello");
    let buffer_id = harness.active_buffer().unwrap();
    harness.with_runtime(|runtime| {
        let cmd = EditCommand;
        let mut ctx = CommandContext::new();
        ctx.set("file", reovim_driver_command::ArgValue::String("test.rs".to_string()));
        ctx.set_buffer_id(buffer_id);
        // No VFS set
        let result = cmd.execute(runtime, &ctx);
        assert!(result.is_error());
    });
}

#[test]
fn test_edit_execute_file_not_found() {
    use {
        reovim_driver_text_session::testing::TestSessionRuntime, reovim_subsys_vfs::MockVfs,
        std::sync::Arc,
    };

    let mut harness = TestSessionRuntime::with_buffer("hello");
    let buffer_id = harness.active_buffer().unwrap();
    let mock_vfs = Arc::new(MockVfs::new());
    // No files added → read will fail

    harness.with_runtime(|runtime| {
        let cmd = EditCommand;
        let mut ctx = CommandContext::new();
        ctx.set("file", reovim_driver_command::ArgValue::String("nonexistent.rs".to_string()));
        ctx.set_buffer_id(buffer_id);
        ctx.set_vfs(Arc::clone(&mock_vfs) as Arc<dyn reovim_subsys_vfs::VfsDriver>);
        let result = cmd.execute(runtime, &ctx);
        assert!(result.is_error());
    });
}

#[test]
fn test_edit_execute_success() {
    use {
        reovim_driver_text_session::testing::TestSessionRuntime, reovim_subsys_vfs::MockVfs,
        std::sync::Arc,
    };

    let mut harness = TestSessionRuntime::with_buffer("original");
    let buffer_id = harness.active_buffer().unwrap();
    let mock_vfs = Arc::new(MockVfs::new());
    mock_vfs.add_file_str("/tmp/test.rs", "fn main() {}");

    harness.with_runtime(|runtime| {
        let cmd = EditCommand;
        let mut ctx = CommandContext::new();
        ctx.set("file", reovim_driver_command::ArgValue::String("/tmp/test.rs".to_string()));
        ctx.set_buffer_id(buffer_id);
        ctx.set_vfs(Arc::clone(&mock_vfs) as Arc<dyn reovim_subsys_vfs::VfsDriver>);
        let result = cmd.execute(runtime, &ctx);
        assert!(result.is_success());
    });
}

#[test]
fn test_edit_execute_invalid_utf8() {
    use {
        reovim_driver_text_session::testing::TestSessionRuntime, reovim_subsys_vfs::MockVfs,
        std::sync::Arc,
    };

    let mut harness = TestSessionRuntime::with_buffer("original");
    let buffer_id = harness.active_buffer().unwrap();
    let mock_vfs = Arc::new(MockVfs::new());
    mock_vfs.add_file("/tmp/binary.bin", [0xFF, 0xFE, 0x80, 0x90]);

    harness.with_runtime(|runtime| {
        let cmd = EditCommand;
        let mut ctx = CommandContext::new();
        ctx.set("file", reovim_driver_command::ArgValue::String("/tmp/binary.bin".to_string()));
        ctx.set_buffer_id(buffer_id);
        ctx.set_vfs(Arc::clone(&mock_vfs) as Arc<dyn reovim_subsys_vfs::VfsDriver>);
        let result = cmd.execute(runtime, &ctx);
        assert!(result.is_error());
    });
}

// ========================================================================
// `decode_file_content` tests (#740 Phase 0, B5)
// ========================================================================

/// B5: `decode_file_content` returns `Err("no active buffer")` instead of
/// silently dropping codec metadata when the runtime has no active buffer.
/// Previously the function fell through to the UTF-8 fallback and a later
/// `:w` round-trip corrupted binary files.
#[test]
fn decode_file_content_no_active_buffer_errors() {
    use reovim_driver_text_session::testing::TestSessionRuntime;

    // Construct a runtime with NO active buffer. `with_buffer` would set
    // one; `new()` leaves `active_buffer = None`.
    let mut harness = TestSessionRuntime::new();
    harness.with_runtime(|runtime| {
        let result = decode_file_content(b"hello", "test.txt", runtime);
        assert!(result.is_err(), "expected error, got {result:?}");
        let err = result.unwrap_err();
        assert!(
            err.contains("no active buffer"),
            "expected 'no active buffer' in error, got {err:?}"
        );
    });
}

/// B5 (positive control): with an active buffer and valid UTF-8 input but
/// no codec stores in `kernel.services`, the function falls through to the
/// UTF-8 fallback and returns the decoded string.
#[test]
fn decode_file_content_utf8_fallback_succeeds() {
    use reovim_driver_text_session::testing::TestSessionRuntime;

    let mut harness = TestSessionRuntime::with_buffer("placeholder");
    harness.with_runtime(|runtime| {
        let result = decode_file_content(b"hello world", "test.txt", runtime);
        assert_eq!(result, Ok("hello world".to_string()));
    });
}

/// B5 (negative control): UTF-8 fallback rejects invalid byte sequences with
/// a clear error pointing at the offset.
#[test]
fn decode_file_content_invalid_utf8_errors() {
    use reovim_driver_text_session::testing::TestSessionRuntime;

    let mut harness = TestSessionRuntime::with_buffer("placeholder");
    harness.with_runtime(|runtime| {
        let result = decode_file_content(&[0xFF, 0xFE, 0x80, 0x90], "binary.bin", runtime);
        assert!(result.is_err(), "expected error, got {result:?}");
        let err = result.unwrap_err();
        assert!(
            err.contains("not valid UTF-8"),
            "expected 'not valid UTF-8' in error, got {err:?}"
        );
    });
}

// ============================================================================
// Codec pipeline branch coverage (#740 Phase 0)
// Branch 368:0 — both classifier + factory stores present (enters codec path)
// Branch 375:0 — factory.find returns Some (codec found)
// Branch 375:1 — factory.find returns None (no matching codec, UTF-8 fallback)
// Branch 378:0 — result.truncated == true
// Branch 378:1 — result.truncated == false
// Branch 389:0 — CodecSessionState is Some (stores metadata)
// Branch 389:1 — CodecSessionState is None (skips metadata storage)
// ============================================================================

/// Branches 368:0, 375:0, 378:1, 389:1 — codec pipeline engaged, codec found,
/// not truncated, no `CodecSessionState` registered.
#[test]
fn decode_file_content_codec_found_not_truncated_no_codec_state() {
    use reovim_driver_text_session::testing::TestSessionRuntime;

    let mut harness = TestSessionRuntime::with_buffer("placeholder");

    let classifier_store = Arc::new(ContentClassifierStore::new());
    classifier_store.add(Arc::new(AlwaysMatchClassifier {
        content_type: ContentType::new("text/test"),
    }));
    let factory_store = Arc::new(ContentCodecFactoryStore::new());
    factory_store.add_factory(Arc::new(SimpleTestFactory { truncated: false }));

    harness.kernel().services.register(classifier_store);
    harness.kernel().services.register(factory_store);

    harness.with_runtime(|runtime| {
        let result = decode_file_content(b"hello codec", "test.txt", runtime);
        assert_eq!(result, Ok("hello codec".to_string()));
    });
}

/// Branches 368:0, 375:0, 378:0, 389:1 — codec found, truncated=true,
/// no `CodecSessionState` registered.
#[test]
fn decode_file_content_codec_found_truncated_no_codec_state() {
    use reovim_driver_text_session::testing::TestSessionRuntime;

    let mut harness = TestSessionRuntime::with_buffer("placeholder");

    let classifier_store = Arc::new(ContentClassifierStore::new());
    classifier_store.add(Arc::new(AlwaysMatchClassifier {
        content_type: ContentType::new("text/test"),
    }));
    let factory_store = Arc::new(ContentCodecFactoryStore::new());
    factory_store.add_factory(Arc::new(SimpleTestFactory { truncated: true }));

    harness.kernel().services.register(classifier_store);
    harness.kernel().services.register(factory_store);

    harness.with_runtime(|runtime| {
        let result = decode_file_content(b"truncated content", "test.txt", runtime);
        assert_eq!(result, Ok("truncated content".to_string()));
    });
}

/// Branches 368:0, 375:0, 378:1, 389:0 — codec found, not truncated,
/// `CodecSessionState` IS registered.
#[test]
fn decode_file_content_codec_found_stores_codec_state() {
    use {
        reovim_content_codec_text::CodecSessionState,
        reovim_driver_text_session::testing::TestSessionRuntime,
    };

    let mut harness = TestSessionRuntime::with_buffer("placeholder");

    let classifier_store = Arc::new(ContentClassifierStore::new());
    classifier_store.add(Arc::new(AlwaysMatchClassifier {
        content_type: ContentType::new("text/test"),
    }));
    let factory_store = Arc::new(ContentCodecFactoryStore::new());
    factory_store.add_factory(Arc::new(SimpleTestFactory { truncated: false }));

    harness.kernel().services.register(classifier_store);
    harness.kernel().services.register(factory_store);
    harness
        .shared_extensions
        .get_or_insert::<CodecSessionState>();

    harness.with_runtime(|runtime| {
        let result = decode_file_content(b"data for codec", "test.txt", runtime);
        assert_eq!(result, Ok("data for codec".to_string()));
    });

    let buf_id = harness.active_buffer().unwrap();
    let codec_state = harness
        .shared_extensions
        .get::<CodecSessionState>()
        .expect("codec state should exist");
    assert!(codec_state.contains(buf_id), "codec state should be populated after decode");
}

/// Branches 368:0, 375:1 — both stores present but no codec matches the
/// content type; falls back to UTF-8.
#[test]
fn decode_file_content_codec_not_found_falls_back_to_utf8() {
    use reovim_driver_text_session::testing::TestSessionRuntime;

    let mut harness = TestSessionRuntime::with_buffer("placeholder");

    let classifier_store = Arc::new(ContentClassifierStore::new());
    classifier_store.add(Arc::new(AlwaysMatchClassifier {
        content_type: ContentType::new("text/test"),
    }));
    let factory_store = Arc::new(ContentCodecFactoryStore::new());
    factory_store.add_factory(Arc::new(NoMatchFactory));

    harness.kernel().services.register(classifier_store);
    harness.kernel().services.register(factory_store);

    harness.with_runtime(|runtime| {
        let result = decode_file_content(b"plain utf8 fallback", "test.txt", runtime);
        assert_eq!(result, Ok("plain utf8 fallback".to_string()));
    });
}

/// Lines 407-408: codec decode returns `Err(_)` — the error is logged and
/// the function falls through to the UTF-8 fallback path.
///
/// Branch 376:1 — `codec.decode(bytes)` returns `Err`.
#[test]
fn decode_file_content_codec_decode_error_falls_back_to_utf8() {
    use reovim_driver_text_session::testing::TestSessionRuntime;

    let mut harness = TestSessionRuntime::with_buffer("placeholder");

    let classifier_store = Arc::new(ContentClassifierStore::new());
    classifier_store.add(Arc::new(AlwaysMatchClassifier {
        content_type: ContentType::new("text/test"),
    }));
    let factory_store = Arc::new(ContentCodecFactoryStore::new());
    factory_store.add_factory(Arc::new(FailingCodecFactory));

    harness.kernel().services.register(classifier_store);
    harness.kernel().services.register(factory_store);

    harness.with_runtime(|runtime| {
        // `FailingCodec` returns `Err`; the function should warn and fall back
        // to UTF-8, returning `Ok` for valid UTF-8 input.
        let result = decode_file_content(b"valid utf8 after codec fail", "test.txt", runtime);
        assert_eq!(result, Ok("valid utf8 after codec fail".to_string()));
    });
}
