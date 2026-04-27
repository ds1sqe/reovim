//! Tests for the `:codecs` ex-command (Phase 7 of #737).
//!
//! Three of the four tests target the pure `format_codecs_listing`
//! helper directly — the formatter is mechanism, easy to verify
//! without involving tracing capture infrastructure. The fourth test
//! invokes `CodecsCommand::execute` end-to-end through
//! `TestSessionRuntime` and asserts only that the result is `Success`
//! (the listing itself is verified by the formatter tests).

use {
    super::{
        codecs::{CodecsCommand, format_codecs_listing},
        mount_tests::factory_store_for,
    },
    reovim_content_codec::{ContentCodecFactoryStore, MountId, MountMode},
    reovim_content_codec_text::{CodecSessionState, MountInfo},
    reovim_driver_command::{CommandContext, CommandHandler, CommandResult},
    reovim_driver_text_session::testing::TestSessionRuntime,
    std::sync::Arc,
};

#[test]
fn format_listing_no_codecs_no_mounts() {
    let lines = format_codecs_listing(None, &[]);
    assert_eq!(
        lines,
        vec![
            "Available codecs:".to_string(),
            "  (none)".to_string(),
            "Active mounts:".to_string(),
            "  (none)".to_string(),
        ]
    );
}

#[test]
fn format_listing_with_codecs_only() {
    let factories = factory_store_for("text/test");
    let lines = format_codecs_listing(Some(&factories), &[]);
    assert_eq!(lines[0], "Available codecs:");
    assert!(
        lines
            .iter()
            .any(|l| l.starts_with("  test-simple-factory:")),
        "expected factory entry, got {lines:?}"
    );
    assert!(
        lines.iter().any(|l| l.contains("text/test")),
        "expected content type listed, got {lines:?}"
    );
    assert!(lines.contains(&"Active mounts:".to_string()));
    assert!(lines.contains(&"  (none)".to_string()));
}

#[test]
fn format_listing_with_active_mounts() {
    let factories = factory_store_for("text/test");
    let mount = MountInfo {
        mount_id: MountId::from_raw(1),
        view_name: "default".to_string(),
        content_valid: true,
        mode: MountMode::Summary,
    };
    let lines = format_codecs_listing(Some(&factories), &[mount]);
    let combined = lines.join("\n");
    assert!(combined.contains("mount_id=1"), "expected mount id in output: {combined}");
    assert!(combined.contains("view=default"), "expected view name in output: {combined}");
    assert!(combined.contains("Summary"), "expected mount mode in output: {combined}");
}

#[test]
fn codecs_command_returns_success() {
    let mut harness = TestSessionRuntime::with_buffer("placeholder");
    harness
        .kernel()
        .services
        .register(Arc::new(ContentCodecFactoryStore::new()));
    harness
        .shared_extensions
        .get_or_insert::<CodecSessionState>();

    let ctx = CommandContext::new();
    harness.with_runtime(|runtime| {
        let result = CodecsCommand.execute(runtime, &ctx);
        assert_eq!(result, CommandResult::Success);
    });
}
