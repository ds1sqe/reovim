//! Tests for the `:umount` ex-command (Phase 6 of #737).
//!
//! Six tests cover each decision point of `UmountCommand::execute`:
//! the no-active-buffer guard, explicit-mount-id success, explicit-zero
//! panic guard, no-arg most-recent-mount path, no-mounts error, and
//! unknown-mount-id error. The "no codec session state" branch is
//! omitted with the same rationale as Phase 5: `TestSessionRuntime`
//! auto-creates the extension on access, making the `None` branch
//! unreachable from the harness.
//!
//! Stub-reuse strategy: Phase 5's `mount_tests.rs` exposes
//! `SimpleTestCodec`, `SimpleTestFactory`, and `factory_store_for` as
//! `pub(super)`. Importing them here avoids a fifth duplication site
//! while keeping the cross-crate hoist deferred to a later flight.

use {
    super::{
        mount_tests::{SimpleTestFactory, factory_store_for},
        umount::UmountCommand,
    },
    reovim_content_codec::{ContentCodecFactoryStore, ContentType, MountId, MountMode},
    reovim_content_codec_text::CodecSessionState,
    reovim_driver_command::{ArgValue, CommandContext, CommandHandler, CommandResult},
    reovim_driver_text_session::testing::TestSessionRuntime,
    std::sync::Arc,
};

fn empty_ctx() -> CommandContext {
    CommandContext::new()
}

fn ctx_with_mount_id(raw: usize) -> CommandContext {
    let mut ctx = CommandContext::new();
    ctx.set("mount_id", ArgValue::Count(raw));
    ctx
}

/// Mount a codec on `buffer_id` for `content_type`, ensuring `factory_store`
/// can produce a codec for that type. Returns the registered `MountId`.
///
/// `unmount_codec` iterates `active_view` keys to locate mounts, so this
/// helper also calls `set_active_view`. The production `:mount` command
/// does the same after a successful `mount_codec`.
fn pre_register_mount(
    harness: &mut TestSessionRuntime,
    factory_store: &Arc<ContentCodecFactoryStore>,
    buffer_id: reovim_kernel::api::v1::BufferId,
    source_bytes: &[u8],
    content_type: &str,
) -> MountId {
    harness.kernel().services.register(factory_store.clone());
    harness
        .shared_extensions
        .get_or_insert::<CodecSessionState>();
    let state = harness
        .shared_extensions
        .get_mut::<CodecSessionState>()
        .unwrap();
    state.set_source(buffer_id, source_bytes.to_vec());
    let mount_id = state
        .mount_codec(
            factory_store,
            buffer_id,
            &ContentType::new(content_type),
            "default".to_string(),
            MountMode::Summary,
        )
        .expect("pre-register mount must succeed")
        .mount_id();
    state.set_active_view(buffer_id, "default".to_string());
    mount_id
}

#[test]
fn umount_with_mount_id_succeeds() {
    let mut harness = TestSessionRuntime::with_buffer("placeholder");
    let buf_id = harness.active_buffer().expect("active buffer");
    let factory = factory_store_for("text/test");
    let mount_id = pre_register_mount(&mut harness, &factory, buf_id, b"bytes", "text/test");

    let ctx = ctx_with_mount_id(mount_id_to_usize(mount_id));
    harness.with_runtime(|runtime| {
        let result = UmountCommand.execute(runtime, &ctx);
        assert_eq!(result, CommandResult::Success);
    });

    let codec_state = harness
        .shared_extensions
        .get::<CodecSessionState>()
        .unwrap();
    assert!(codec_state.list_mounts(buf_id).is_empty(), "no mounts should remain");
}

#[test]
fn umount_no_arg_removes_most_recent_mount() {
    let mut harness = TestSessionRuntime::with_buffer("placeholder");
    let buf_id = harness.active_buffer().expect("active buffer");

    // First mount: text/test.
    let factory_a = factory_store_for("text/test");
    harness.kernel().services.register(factory_a.clone());
    harness
        .shared_extensions
        .get_or_insert::<CodecSessionState>();
    let state = harness
        .shared_extensions
        .get_mut::<CodecSessionState>()
        .unwrap();
    state.set_source(buf_id, b"bytes".to_vec());
    let first_id = state
        .mount_codec(
            &factory_a,
            buf_id,
            &ContentType::new("text/test"),
            "default".to_string(),
            MountMode::Summary,
        )
        .expect("first mount must succeed")
        .mount_id();

    // The first mount must be findable by `unmount_codec`, which scans
    // `active_view`. Set it explicitly (the production `:mount` does the
    // same).
    state.set_active_view(buf_id, "default".to_string());

    // Second mount: same buffer, registered after the first.
    // Use a different view name so the additional mount is accepted.
    let state = harness
        .shared_extensions
        .get_mut::<CodecSessionState>()
        .unwrap();
    let second_id = state
        .mount_codec(
            &factory_a,
            buf_id,
            &ContentType::new("text/test"),
            "secondary".to_string(),
            MountMode::Summary,
        )
        .expect("second mount must succeed")
        .mount_id();

    assert_ne!(first_id, second_id);

    let ctx = empty_ctx();
    harness.with_runtime(|runtime| {
        let result = UmountCommand.execute(runtime, &ctx);
        assert_eq!(result, CommandResult::Success);
    });

    let codec_state = harness
        .shared_extensions
        .get::<CodecSessionState>()
        .unwrap();
    let remaining: Vec<_> = codec_state
        .list_mounts(buf_id)
        .iter()
        .map(|m| m.mount_id)
        .collect();
    assert_eq!(remaining, vec![first_id], "second mount should be removed");
}

#[test]
fn umount_no_mounts_errors() {
    let mut harness = TestSessionRuntime::with_buffer("placeholder");
    harness
        .shared_extensions
        .get_or_insert::<CodecSessionState>();

    let ctx = empty_ctx();
    harness.with_runtime(|runtime| {
        let result = UmountCommand.execute(runtime, &ctx);
        let CommandResult::Error(err) = result else {
            panic!("expected error, got {result:?}");
        };
        assert!(err.contains("no mounts"), "got {err:?}");
    });
}

#[test]
fn umount_unknown_mount_id_errors() {
    let mut harness = TestSessionRuntime::with_buffer("placeholder");
    harness
        .shared_extensions
        .get_or_insert::<CodecSessionState>();

    // Use mount_id 999 — never registered. The wrapper builds a valid
    // MountId from a non-zero raw value but `unmount_codec` fails to
    // find it.
    let ctx = ctx_with_mount_id(999);
    harness.with_runtime(|runtime| {
        let result = UmountCommand.execute(runtime, &ctx);
        let CommandResult::Error(err) = result else {
            panic!("expected error, got {result:?}");
        };
        assert!(err.contains("mount id not found"), "got {err:?}");
    });
}

#[test]
fn umount_zero_mount_id_errors() {
    let mut harness = TestSessionRuntime::with_buffer("placeholder");
    harness
        .shared_extensions
        .get_or_insert::<CodecSessionState>();

    let ctx = ctx_with_mount_id(0);
    harness.with_runtime(|runtime| {
        let result = UmountCommand.execute(runtime, &ctx);
        let CommandResult::Error(err) = result else {
            panic!("expected error, got {result:?}");
        };
        assert!(err.contains("must be non-zero"), "got {err:?}");
    });
}

#[test]
fn umount_no_active_buffer_errors() {
    let mut harness = TestSessionRuntime::new();
    harness
        .shared_extensions
        .get_or_insert::<CodecSessionState>();

    let ctx = empty_ctx();
    harness.with_runtime(|runtime| {
        let result = UmountCommand.execute(runtime, &ctx);
        let CommandResult::Error(err) = result else {
            panic!("expected error, got {result:?}");
        };
        assert!(err.contains("no active buffer"), "got {err:?}");
    });
}

// Silence the lint about the unused `SimpleTestFactory` re-import — the
// import keeps the cross-test-module reuse contract visible at the top
// of the file even though we drive most fixtures through
// `factory_store_for`.
#[allow(dead_code)]
const _STUB_REUSE_MARKER: Option<SimpleTestFactory> = None;

fn mount_id_to_usize(id: MountId) -> usize {
    // MountId wraps NonZeroU64; the test helper extracts the raw value
    // for round-tripping through the Count argument shape.
    id.as_usize()
}
