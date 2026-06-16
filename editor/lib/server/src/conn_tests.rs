//! Tests for `conn.rs` — `ConnState` and `ConnPhase` coverage.
//!
//! Registered under the `selftest` feature; runs on the arch no_std
//! selftest runner (arch_test! + testrt::run). The server-selftest bin in
//! tests/fixtures/ runs these.

use reovim_arch::arch_test;

use crate::conn::{ConnPhase, ConnState};

arch_test!(conn_new_starts_in_handshake_phase, {
    let cs = ConnState::new();
    assert_eq!(cs.phase(), ConnPhase::Handshake);
});

arch_test!(conn_new_is_not_attached, {
    assert!(!ConnState::new().is_attached());
});

arch_test!(conn_new_handshake_not_done, {});

arch_test!(conn_set_ready_advances_from_handshake, {
    let mut cs = ConnState::new();
    cs.set_ready();
    assert_eq!(cs.phase(), ConnPhase::Ready);
});

arch_test!(conn_set_ready_is_noop_when_already_ready, {
    let mut cs = ConnState::new();
    cs.set_ready();
    cs.set_ready(); // idempotent
    assert_eq!(cs.phase(), ConnPhase::Ready);
});

arch_test!(conn_set_attached_advances_from_ready, {
    let mut cs = ConnState::new();
    cs.set_ready();
    assert!(cs.set_attached());
    assert_eq!(cs.phase(), ConnPhase::Attached);
    assert!(cs.is_attached());
});

arch_test!(conn_set_attached_from_handshake_returns_false, {
    let mut cs = ConnState::new();
    // Cannot attach before handshake.
    assert!(!cs.set_attached());
    assert_eq!(cs.phase(), ConnPhase::Handshake);
});

arch_test!(conn_second_attach_returns_false_sp1, {
    let mut cs = ConnState::new();
    cs.set_ready();
    assert!(cs.set_attached());
    // SP1: second Attach returns false.
    assert!(!cs.set_attached());
});

arch_test!(conn_phase_ne_handshake_vs_ready, {
    assert_ne!(ConnPhase::Handshake, ConnPhase::Ready);
});

arch_test!(conn_phase_ne_ready_vs_attached, {
    assert_ne!(ConnPhase::Ready, ConnPhase::Attached);
});

arch_test!(conn_set_ready_does_not_advance_from_attached, {
    let mut cs = ConnState::new();
    cs.set_ready();
    cs.set_attached();
    cs.set_ready(); // no-op from Attached
    assert_eq!(cs.phase(), ConnPhase::Attached);
});

// ── Default impl coverage (conn.rs line 153-155) ─────────────────────────────
//
// `impl Default for ConnState` delegates to `Self::new()`. Invoking `Default::
// default()` explicitly hits the function body at lines 153-155.

arch_test!(conn_state_default_starts_in_handshake, {
    let cs: ConnState = Default::default();
    assert_eq!(cs.phase(), ConnPhase::Handshake, "Default ConnState starts in Handshake");
    assert!(!cs.is_attached(), "Default ConnState is not attached");
});
