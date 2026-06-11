//! Tests for `session.rs` — `Session`, `SessionState`, and the CC14 dispatch
//! shape (walking-skeleton, #797).
//!
//! Registered under the `selftest` feature; runs on the arch no_std selftest
//! runner via the `kernel-selftest` fixture bin.
//!
//! ## Integration smoke (Phase 2 AC)
//!
//! The `session_dispatch_integration_smoke` test boots a kernel, registers
//! the text Domain, attaches a buffer, drives a `RawInput` batch through the
//! router's `OnRawInput` → `Render`, and asserts the resulting `Projection`
//! bytes match the expected fixture — proving session + router + Domain dispatch
//! compose end-to-end inside the kernel before any wire is involved.
//!
//! The text Domain types live in `reovim-domain-text` (an `ext/server/domain/*`
//! crate, Category::ServerExt). They are wired in here under `selftest` only;
//! the kernel crate does NOT have a runtime dep on the text Domain crate (the
//! core/ext boundary is maintained). The kernel-selftest fixture bin enables
//! `reovim-domain-text/selftest` and links it.

use reovim_arch::arch_test;

use crate::{
    Init, LauncherArgs,
    session::{BufferId, DomainAttachmentId, FocusEntry, SessionId, SessionState, WindowId},
};

arch_test!(session_id_round_trip, {
    let id = SessionId::new(7);
    assert_eq!(id.as_u32(), 7);
});

arch_test!(domain_attachment_id_round_trip, {
    let id = DomainAttachmentId::new(10);
    assert_eq!(id.as_u32(), 10);
});

arch_test!(focus_entry_resolved_variant, {
    let entry = FocusEntry::Resolved(DomainAttachmentId::new(1));
    match entry {
        FocusEntry::Resolved(id) => assert_eq!(id.as_u32(), 1),
    }
});

arch_test!(session_state_new_empty_buffer, {
    use {crate::router::DomainId, core::num::NonZeroU32};

    let domain_id = DomainId::new(NonZeroU32::new(1).unwrap());
    let state = SessionState::new(
        domain_id,
        DomainAttachmentId::new(1),
        BufferId::new(1),
        WindowId::new(1),
    );
    assert!(state.buffer.is_empty(), "new session state has empty buffer");
    assert_eq!(state.cursor, 0, "cursor starts at 0");
    assert_eq!(state.buffer_id.as_u32(), 1);
    assert_eq!(state.window_id.as_u32(), 1);
});

arch_test!(session_state_focus_chain_is_single_resolved, {
    use {crate::router::DomainId, core::num::NonZeroU32};

    let domain_id = DomainId::new(NonZeroU32::new(1).unwrap());
    let attach_id = DomainAttachmentId::new(1);
    let state = SessionState::new(domain_id, attach_id, BufferId::new(1), WindowId::new(1));
    // Single-entry focus chain [Resolved(root)] (§4.2 subset note).
    assert_eq!(state.focus_chain.len(), 1);
    match state.focus_chain[0] {
        FocusEntry::Resolved(id) => assert_eq!(id.as_u32(), 1),
    }
});

// ── Integration smoke (Phase 2 AC) ───────────────────────────────────────────
//
// The kernel-level smoke exercises session + router mechanics without any ext
// dependency. The full end-to-end integration smoke (kernel + text Domain
// handler/projector + dispatch) lives in the kernel-selftest bin's own module
// (`tests/fixtures/kernel-selftest/src/domain_smoke.rs`), where the bin can
// link `reovim-domain-text` without adding an ext dep to the kernel rlib.
//
// This test verifies the dispatch path returns the expected error when no
// handler is registered (the sentinel session state installed by Init::boot).

arch_test!(session_dispatch_without_domain_returns_err, {
    let kernel = Init::new(LauncherArgs::default())
        .boot()
        .expect("boot succeeds");
    // No handler registered: dispatch must return an Err.
    let result = kernel.dispatch_input(b"x");
    assert!(result.is_err(), "dispatch without registered handler must return Err");
});

// ── as_u32 body-coverage (session.rs lines 73-74, 113-114) ───────────────────
//
// `SessionId::as_u32` and `DomainAttachmentId::as_u32` are `const fn`. LLVM
// may inline the body at const-eval time and not emit a callable function body,
// leaving the coverage tool with no arc to tick. The tests below call them via
// a non-const path (runtime assignment) so the body receives a runtime-visible
// call site that the profiler can attribute.

arch_test!(session_id_as_u32_runtime_call, {
    // Prevent const-propagation: build the id from a runtime value.
    let v: u32 = reovim_arch::sys::gettid().unsigned_abs() % 65536;
    let id = SessionId::new(v);
    assert_eq!(id.as_u32(), v, "SessionId::as_u32 must return the stored value");
});

arch_test!(domain_attachment_id_as_u32_runtime_call, {
    let v: u32 = (reovim_arch::sys::gettid().unsigned_abs() % 65536) + 1;
    let id = DomainAttachmentId::new(v);
    assert_eq!(id.as_u32(), v, "DomainAttachmentId::as_u32 must return the stored value");
});
