use super::*;

#[test]
fn bnext_identity() {
    let cmd = BnextCommand;
    assert_eq!(cmd.names(), &["bn", "bnext"]);
    assert_eq!(cmd.id().name(), "bnext");
    assert!(!cmd.description().is_empty());
}

#[test]
fn bprevious_identity() {
    let cmd = BpreviousCommand;
    assert_eq!(cmd.names(), &["bp", "bprevious", "bN", "bNext"]);
    assert_eq!(cmd.id().name(), "bprevious");
    assert!(!cmd.description().is_empty());
}

#[test]
fn bdelete_identity() {
    let cmd = BdeleteCommand;
    assert_eq!(cmd.names(), &["bd", "bdelete"]);
    assert_eq!(cmd.id().name(), "bdelete");
    assert!(!cmd.description().is_empty());
}

#[test]
fn qall_identity() {
    let cmd = QuitAllCommand;
    assert_eq!(cmd.names(), &["qa", "qall"]);
    assert_eq!(cmd.id().name(), "qall");
    assert!(!cmd.description().is_empty());
}

#[test]
fn command_handlers_count() {
    let handlers = command_handlers();
    assert_eq!(handlers.len(), 4);
}

// ============================================================================
// BdeleteCommand execute tests (#740 Phase 0, B3)
// ============================================================================

/// B3: `:bd` releases per-buffer codec metadata via `CodecSessionState`.
///
/// Before this fix, `BdeleteCommand` called `kernel.buffers.unregister()`
/// directly and bypassed cleanup hooks (`TextBufferRegistry` and
/// `CodecSessionState`). Every buffer close leaked shared state.
#[test]
fn bdelete_clears_codec_session_state() {
    use {
        reovim_driver_codec::{CodecMetadata, CodecSessionState, ContentType},
        reovim_driver_session::testing::TestSessionRuntime,
    };

    // Two buffers so the delete passes last-buffer protection.
    let mut harness = TestSessionRuntime::with_buffer("first");
    let buf_a = harness.active_buffer().unwrap();
    let buf_b = harness.with_runtime(|runtime| runtime.create_buffer(None, "second"));

    // Pre-populate codec state for the buffer we are about to delete AND
    // for a peer that must NOT be touched.
    harness
        .shared_extensions
        .get_or_insert::<CodecSessionState>();
    let codec_state = harness
        .shared_extensions
        .get_mut::<CodecSessionState>()
        .expect("codec state inserted above");
    codec_state.insert(buf_a, CodecMetadata::new(ContentType::new(ContentType::UTF8)));
    codec_state.set_source(buf_a, b"first".to_vec());
    codec_state.set_active_view(buf_a, "default".to_string());
    codec_state.insert(buf_b, CodecMetadata::new(ContentType::new(ContentType::UTF8)));
    assert!(codec_state.contains(buf_a));
    assert!(codec_state.contains(buf_b));

    // Make `buf_a` active and run `:bd`.
    harness.with_runtime(|runtime| {
        runtime.set_active_buffer(Some(buf_a));
        let cmd = BdeleteCommand;
        let result = cmd.execute(runtime, &CommandContext::new());
        assert!(matches!(result, CommandResult::Success));
    });

    // Codec state for the deleted buffer is gone; the peer survives.
    let codec_state = harness
        .shared_extensions
        .get::<CodecSessionState>()
        .expect("codec state present");
    assert!(!codec_state.contains(buf_a), "buf_a codec metadata should be cleared after :bd");
    assert!(codec_state.contains(buf_b), "buf_b codec metadata must survive a peer delete");
}

/// B3 (negative): when no codec state has ever been registered (e.g. tests
/// that never load a codec module), `:bd` still removes the buffer cleanly
/// and returns `Success`.
#[test]
fn bdelete_without_codec_state_succeeds() {
    use reovim_driver_session::testing::TestSessionRuntime;

    let mut harness = TestSessionRuntime::with_buffer("first");
    let buf_a = harness.active_buffer().unwrap();
    harness.with_runtime(|runtime| {
        runtime.create_buffer(None, "second");
        runtime.set_active_buffer(Some(buf_a));
        let cmd = BdeleteCommand;
        let result = cmd.execute(runtime, &CommandContext::new());
        assert!(matches!(result, CommandResult::Success));
    });
}

/// `:bd` honours last-buffer protection (a side-effect of routing through
/// `BufferApi::delete_buffer` instead of `kernel.buffers.unregister`).
#[test]
fn bdelete_refuses_to_delete_last_buffer() {
    use reovim_driver_session::testing::TestSessionRuntime;

    let mut harness = TestSessionRuntime::with_buffer("only");
    harness.with_runtime(|runtime| {
        let cmd = BdeleteCommand;
        let result = cmd.execute(runtime, &CommandContext::new());
        // BufferError::CannotDeleteLastBuffer surfaces as Error.
        assert!(matches!(result, CommandResult::Error(_)));
    });
}

/// `:bd` is a no-op when there is no active buffer (matches vim behaviour).
#[test]
fn bdelete_no_active_buffer_is_noop() {
    use reovim_driver_session::testing::TestSessionRuntime;

    let mut harness = TestSessionRuntime::new();
    harness.with_runtime(|runtime| {
        let cmd = BdeleteCommand;
        let result = cmd.execute(runtime, &CommandContext::new());
        assert!(matches!(result, CommandResult::Success));
    });
}

// ============================================================================
// cycle_buffer branch coverage
// Branch 157:0 — active buffer exists (continues to body)
// Branch 157:1 — no active buffer (returns early)
// Branch 162:0 — only one buffer (returns early)
// Branch 162:1 — two or more buffers (continues to cycle)
// Branch 172:0 — current_idx == 0, prev wraps to end
// Branch 172:1 — current_idx > 0, prev decrements
// ============================================================================

/// Branch 157:1 — no active buffer → `:bn` is a no-op.
#[test]
fn bnext_no_active_buffer_is_noop() {
    use reovim_driver_session::testing::TestSessionRuntime;

    let mut harness = TestSessionRuntime::new();
    harness.with_runtime(|runtime| {
        let cmd = BnextCommand;
        let result = cmd.execute(runtime, &CommandContext::new());
        assert!(matches!(result, CommandResult::Success));
    });
}

/// Branch 162:0 — only one buffer → `:bn` is a no-op.
#[test]
fn bnext_single_buffer_is_noop() {
    use reovim_driver_session::testing::TestSessionRuntime;

    let mut harness = TestSessionRuntime::with_buffer("only");
    let buf = harness.active_buffer().unwrap();
    harness.with_runtime(|runtime| {
        let cmd = BnextCommand;
        let result = cmd.execute(runtime, &CommandContext::new());
        assert!(matches!(result, CommandResult::Success));
    });
    assert_eq!(harness.active_buffer(), Some(buf));
}

/// Branches 157:0, 162:1 — active buffer, two buffers → `:bn` advances.
#[test]
fn bnext_cycles_to_next_buffer() {
    use reovim_driver_session::testing::TestSessionRuntime;

    let mut harness = TestSessionRuntime::with_buffer("first");
    let buf_a = harness.active_buffer().unwrap();
    let buf_b = harness.with_runtime(|runtime| runtime.create_buffer(None, "second"));

    harness.with_runtime(|runtime| {
        runtime.set_active_buffer(Some(buf_a));
        let cmd = BnextCommand;
        let result = cmd.execute(runtime, &CommandContext::new());
        assert!(matches!(result, CommandResult::Success));
    });

    assert_eq!(harness.active_buffer(), Some(buf_b));
}

/// Branch 172:0 — `current_idx` == 0 → `:bp` wraps to last buffer.
#[test]
fn bprevious_at_first_buffer_wraps_to_last() {
    use reovim_driver_session::testing::TestSessionRuntime;

    let mut harness = TestSessionRuntime::with_buffer("first");
    let buf_a = harness.active_buffer().unwrap();
    let buf_b = harness.with_runtime(|runtime| runtime.create_buffer(None, "second"));

    // Determine which ID sorts first so we can set the smallest as "current"
    // and verify that `:bp` wraps to the other.
    let first = buf_a.min(buf_b);

    harness.with_runtime(|runtime| {
        runtime.set_active_buffer(Some(first));
        let cmd = BpreviousCommand;
        let result = cmd.execute(runtime, &CommandContext::new());
        assert!(matches!(result, CommandResult::Success));
    });

    let active = harness.active_buffer().unwrap();
    assert_ne!(active, first, "should have cycled away from first buffer");
}

/// Branch 172:1 — `current_idx` > 0 → `:bp` decrements.
#[test]
fn bprevious_not_at_first_decrements() {
    use reovim_driver_session::testing::TestSessionRuntime;

    let mut harness = TestSessionRuntime::with_buffer("first");
    let buf_a = harness.active_buffer().unwrap();
    let buf_b = harness.with_runtime(|runtime| runtime.create_buffer(None, "second"));

    // Set the larger ID as active so current_idx > 0 and `:bp` decrements.
    let first = buf_a.min(buf_b);
    let last = buf_a.max(buf_b);

    harness.with_runtime(|runtime| {
        runtime.set_active_buffer(Some(last));
        let cmd = BpreviousCommand;
        let result = cmd.execute(runtime, &CommandContext::new());
        assert!(matches!(result, CommandResult::Success));
    });

    assert_eq!(harness.active_buffer(), Some(first));
}
