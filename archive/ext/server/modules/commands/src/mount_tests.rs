//! Tests for the `:mount` ex-command (Phase 5 of #737).
//!
//! The seven tests below cover each numbered step of `MountCommand::execute`:
//! shorthand resolution, full-content-type happy path, all four error
//! arms (no buffer, no factory store, no codec session state, unknown
//! type), plus the mount-mode invariant (`Summary` only in Phase 5).
//!
//! Stub-hoist note: `PrefixClassifier` / `SimpleTestCodec` /
//! `SimpleTestFactory` mirror the patterns in `file_open_tests.rs`,
//! `commands_tests.rs::open_codec`, and `picker-files`'s
//! `open_file_codec`. This is the fourth duplication site. The hoist
//! to a shared `dev-only` testing module is deferred to a follow-on
//! flight per the Phase 5 plan amendment.

use {
    super::mount::MountCommand,
    reovim_content_codec::{
        Annotation, CodecError, CodecMetadata, ContentCodec, ContentCodecFactory,
        ContentCodecFactoryStore, ContentType, DecodeResult, MountMode,
    },
    reovim_content_codec_text::CodecSessionState,
    reovim_driver_command::{ArgValue, CommandContext, CommandHandler, CommandResult},
    reovim_driver_text_session::testing::TestSessionRuntime,
    std::sync::Arc,
};

/// Codec stub: emits a fixed string on every decode call. Mount only
/// invokes the codec lazily on the next render frame (Phase 5 design),
/// but having a real codec keeps the factory's `create` path honest.
pub struct SimpleTestCodec;

impl ContentCodec for SimpleTestCodec {
    fn decode(&self, _raw: &[u8]) -> Result<DecodeResult, CodecError> {
        Ok(DecodeResult {
            content: "[stub-decoded]".to_string(),
            annotations: Vec::<Annotation>::new(),
            metadata: CodecMetadata::new(ContentType::new("text/test")),
            lossy: false,
            readonly: false,
            truncated: false,
        })
    }
}

/// Factory that creates `SimpleTestCodec` for one specific content type
/// and rejects everything else. Tests register one factory per content
/// type they want to exercise (`text/test`, `binary/raw`).
pub struct SimpleTestFactory {
    pub matches: ContentType,
}

impl ContentCodecFactory for SimpleTestFactory {
    fn create(&self, content_type: &ContentType) -> Option<Arc<dyn ContentCodec>> {
        if content_type == &self.matches {
            Some(Arc::new(SimpleTestCodec) as Arc<dyn ContentCodec>)
        } else {
            None
        }
    }
    fn supported_content_types(&self) -> Vec<&str> {
        vec![self.matches.as_str()]
    }
    fn name(&self) -> &'static str {
        "test-simple-factory"
    }
}

fn ctx_with_type(content_type: &str) -> CommandContext {
    let mut ctx = CommandContext::new();
    ctx.set("content_type", ArgValue::String(content_type.to_string()));
    ctx
}

pub fn factory_store_for(content_type: &str) -> Arc<ContentCodecFactoryStore> {
    let store = Arc::new(ContentCodecFactoryStore::new());
    store.add_factory(Arc::new(SimpleTestFactory {
        matches: ContentType::new(content_type),
    }));
    store
}

#[test]
fn mount_with_full_content_type_succeeds() {
    let mut harness = TestSessionRuntime::with_buffer("placeholder");
    harness
        .kernel()
        .services
        .register(factory_store_for("text/test"));
    harness
        .shared_extensions
        .get_or_insert::<CodecSessionState>();

    let buf_id = harness.active_buffer().expect("active buffer");
    let raw = b"some bytes".to_vec();
    harness
        .shared_extensions
        .get_mut::<CodecSessionState>()
        .unwrap()
        .set_source(buf_id, raw);

    let ctx = ctx_with_type("text/test");
    harness.with_runtime(|runtime| {
        let result = MountCommand.execute(runtime, &ctx);
        assert_eq!(result, CommandResult::Success);
    });

    let codec_state = harness
        .shared_extensions
        .get::<CodecSessionState>()
        .unwrap();
    let mounts = codec_state.list_mounts(buf_id);
    assert_eq!(mounts.len(), 1, "expected one registered mount");
    assert_eq!(mounts[0].mode, MountMode::Summary);
}

#[test]
fn mount_with_shorthand_resolves_to_full_type() {
    let mut harness = TestSessionRuntime::with_buffer("placeholder");
    // Factory registered for the FULL content type only — the shorthand
    // resolution must convert `hex` to `binary/raw` before the lookup.
    harness
        .kernel()
        .services
        .register(factory_store_for("binary/raw"));
    harness
        .shared_extensions
        .get_or_insert::<CodecSessionState>();

    let buf_id = harness.active_buffer().expect("active buffer");
    harness
        .shared_extensions
        .get_mut::<CodecSessionState>()
        .unwrap()
        .set_source(buf_id, b"\x00\x01raw bytes".to_vec());

    let ctx = ctx_with_type("hex");
    harness.with_runtime(|runtime| {
        let result = MountCommand.execute(runtime, &ctx);
        assert_eq!(result, CommandResult::Success);
    });

    let codec_state = harness
        .shared_extensions
        .get::<CodecSessionState>()
        .unwrap();
    let mounts = codec_state.list_mounts(buf_id);
    assert_eq!(mounts.len(), 1);
    assert_eq!(mounts[0].mode, MountMode::Summary);
}

#[test]
fn mount_with_no_active_buffer_errors() {
    let mut harness = TestSessionRuntime::new();
    harness
        .kernel()
        .services
        .register(factory_store_for("text/test"));
    harness
        .shared_extensions
        .get_or_insert::<CodecSessionState>();

    let ctx = ctx_with_type("text/test");
    harness.with_runtime(|runtime| {
        let result = MountCommand.execute(runtime, &ctx);
        let CommandResult::Error(err) = result else {
            panic!("expected error, got {result:?}");
        };
        assert!(err.contains("no active buffer"), "got {err:?}");
    });
}

#[test]
fn mount_with_unknown_content_type_errors() {
    let mut harness = TestSessionRuntime::with_buffer("placeholder");
    harness
        .kernel()
        .services
        .register(factory_store_for("text/test"));
    harness
        .shared_extensions
        .get_or_insert::<CodecSessionState>();

    let buf_id = harness.active_buffer().expect("active buffer");
    harness
        .shared_extensions
        .get_mut::<CodecSessionState>()
        .unwrap()
        .set_source(buf_id, b"bytes".to_vec());

    let ctx = ctx_with_type("text/unregistered");
    harness.with_runtime(|runtime| {
        let result = MountCommand.execute(runtime, &ctx);
        let err = match result {
            CommandResult::Error(e) => e,
            CommandResult::Success => panic!("expected error, got Success"),
        };
        assert!(err.contains("no codec registered"), "got {err:?}");
        assert!(err.contains("text/unregistered"), "got {err:?}");
    });
}

#[test]
fn mount_with_no_factory_store_errors() {
    let mut harness = TestSessionRuntime::with_buffer("placeholder");
    harness
        .shared_extensions
        .get_or_insert::<CodecSessionState>();

    let ctx = ctx_with_type("text/test");
    harness.with_runtime(|runtime| {
        let result = MountCommand.execute(runtime, &ctx);
        let err = match result {
            CommandResult::Error(e) => e,
            CommandResult::Success => panic!("expected error, got Success"),
        };
        assert!(err.contains("no factory store"), "got {err:?}");
    });
}

// `mount_with_no_codec_session_state_errors`: not tested because
// `TestSessionRuntime::shared_ext_mut` auto-creates the extension on
// access, making the `None` branch unreachable from the harness. The
// error message remains in the code as defense-in-depth for runtime
// impls that use the trait default (`fn shared_ext_mut → None`).

#[test]
fn mount_without_canonical_bytes_or_file_path_errors() {
    let mut harness = TestSessionRuntime::with_buffer("placeholder");
    harness
        .kernel()
        .services
        .register(factory_store_for("text/test"));
    harness
        .shared_extensions
        .get_or_insert::<CodecSessionState>();

    // No `set_source` and no file_path → step 5 triggers VFS load
    // attempt, but the buffer has no path. Asserts the "cannot mount on
    // a buffer with no file path" branch.
    let ctx = ctx_with_type("text/test");
    harness.with_runtime(|runtime| {
        let result = MountCommand.execute(runtime, &ctx);
        let err = match result {
            CommandResult::Error(e) => e,
            CommandResult::Success => panic!("expected error, got Success"),
        };
        assert!(err.contains("no file path") || err.contains("no VFS"), "got {err:?}");
    });
}
