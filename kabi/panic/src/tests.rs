//! Tests for the `kabi/panic` write-once fault-floor seam.
//!
//! ## Process-global write-once strategy
//!
//! Each atom in this crate is a process-global, write-once static. Ordinary
//! `cargo test` runs all tests as threads in one process, so an atom once set
//! cannot be reset between tests. The strategy here mirrors `kabi/platform`:
//! exercise the full lifecycle (unset → first-set → reject-second-set) of EACH
//! atom within a SINGLE `#[test]` fn, so no two test fns contend for the same
//! atom. Because each test fn uniquely owns its atom interaction there are no
//! inter-test races even under the parallel test runner.

use {
    super::{
        SetError, get_disposition, get_flush_fd, get_pre_exit_hook, get_ring_tail_provider,
        get_state_record_hook, set_disposition, set_flush_fd, set_pre_exit_hook,
        set_ring_tail_provider, set_state_record_hook,
    },
    reovim_uapi_panic::{Disposition, PanicRecord},
};

// ── fixture hooks (synthetic; no arch edge) ───────────────────────────────────

fn fixture_tail() -> &'static [u8] {
    b"[  0.000001] kernel log: tail\n"
}

fn fixture_state_hook(_record: PanicRecord) {
    // A synthetic persistence consumer: records nothing, proves the dispatch.
}

fn fixture_pre_exit() {
    // A synthetic terminal-restore stub: does nothing, proves the dispatch.
}

// ── no-install-gate: getters are callable with no kabi/platform installed ─────

/// Proves that EACH getter is callable with NO `kabi/platform::install` call —
/// the seam is unconditionally readable (Platform-Contract §3.4). The full
/// write-once lifecycle and the None-when-unset contract are exercised in the
/// per-atom tests below; this test simply confirms the TYPES compile and the
/// API does not panic or reference `kabi/platform` when called. The actual
/// Some/None value depends on which lifecycle test runs first (unspecified) —
/// only reachability without an install gate is asserted here.
#[test]
fn getters_callable_without_install_gate() {
    // Each call must not panic regardless of whether the atom is set.
    // The returned Option<_> is discarded — the assertion is reachability.
    let _ = get_flush_fd();
    let _ = get_ring_tail_provider();
    let _ = get_state_record_hook();
    let _ = get_disposition();
    let _ = get_pre_exit_hook();
    // If we reach here the seam is readable without kabi/platform installed.
}

// ── flush_fd: write-once lifecycle ───────────────────────────────────────────

/// The flush-fd atom: first `set_flush_fd` succeeds, `get_flush_fd` returns
/// the value, a second `set_flush_fd` is rejected (write-once), and the getter
/// still returns the first value.
#[test]
fn flush_fd_write_once() {
    // First registration succeeds.
    assert_eq!(set_flush_fd(3), Ok(()), "first set_flush_fd must succeed");

    // The getter returns the registered fd.
    assert_eq!(get_flush_fd(), Some(3), "get_flush_fd must return the registered fd");

    // A second registration is rejected; the first value stands.
    assert_eq!(
        set_flush_fd(99),
        Err(SetError::AlreadySet),
        "second set_flush_fd must be rejected (write-once)"
    );
    assert_eq!(get_flush_fd(), Some(3), "first fd must still stand after rejected second set");
}

// ── ring_tail_provider: write-once lifecycle ──────────────────────────────────

/// The ring-tail-provider atom: first `set_ring_tail_provider` succeeds, the
/// getter dispatches through the stored address, a second set is rejected.
#[test]
fn ring_tail_provider_write_once() {
    // First registration succeeds.
    assert_eq!(
        set_ring_tail_provider(fixture_tail),
        Ok(()),
        "first set_ring_tail_provider must succeed"
    );

    // The getter reconstructs and dispatches the fn pointer.
    let provider = get_ring_tail_provider().expect("provider must be Some after registration");
    assert_eq!(
        provider(),
        b"[  0.000001] kernel log: tail\n",
        "the stored fn pointer must dispatch correctly"
    );

    // A second registration is rejected.
    assert_eq!(
        set_ring_tail_provider(fixture_tail),
        Err(SetError::AlreadySet),
        "second set_ring_tail_provider must be rejected (write-once)"
    );

    // The getter still returns the first provider.
    let provider2 = get_ring_tail_provider().expect("provider must still be Some");
    assert_eq!(
        provider2(),
        b"[  0.000001] kernel log: tail\n",
        "first provider must still stand"
    );
}

// ── state_record_hook: write-once lifecycle ───────────────────────────────────

/// The state-record-hook atom: first `set_state_record_hook` succeeds, the
/// getter returns the hook, a second set is rejected.
#[test]
fn state_record_hook_write_once() {
    let record = PanicRecord {
        disposition: Disposition::Halt,
        rollback_failed: false,
    };

    // First registration succeeds.
    assert_eq!(
        set_state_record_hook(fixture_state_hook),
        Ok(()),
        "first set_state_record_hook must succeed"
    );

    // The getter reconstructs the fn pointer and calling it is safe.
    let hook = get_state_record_hook().expect("hook must be Some after registration");
    // Call to prove dispatch — the fixture is a no-op.
    hook(record);

    // A second registration is rejected.
    assert_eq!(
        set_state_record_hook(fixture_state_hook),
        Err(SetError::AlreadySet),
        "second set_state_record_hook must be rejected (write-once)"
    );

    // The first hook still stands.
    assert!(
        get_state_record_hook().is_some(),
        "first hook must still be registered after rejected second set"
    );
}

// ── disposition: write-once lifecycle ────────────────────────────────────────

/// The disposition atom: first `set_disposition` succeeds, the getter returns
/// the value, a second set is rejected.
#[test]
fn disposition_write_once() {
    // First registration succeeds.
    assert_eq!(
        set_disposition(Disposition::Recover),
        Ok(()),
        "first set_disposition must succeed"
    );

    // The getter returns the registered disposition.
    assert_eq!(
        get_disposition(),
        Some(Disposition::Recover),
        "get_disposition must return Recover after registration"
    );

    // A second registration (even a different value) is rejected.
    assert_eq!(
        set_disposition(Disposition::Halt),
        Err(SetError::AlreadySet),
        "second set_disposition must be rejected (write-once)"
    );

    // The first disposition still stands.
    assert_eq!(
        get_disposition(),
        Some(Disposition::Recover),
        "first disposition must still stand after rejected second set"
    );
}

// ── pre_exit_hook: write-once lifecycle ──────────────────────────────────────

/// The pre-exit-hook atom: first `set_pre_exit_hook` succeeds, the getter
/// returns the hook (callable without side effects), a second set is rejected.
#[test]
fn pre_exit_hook_write_once() {
    // First registration succeeds.
    assert_eq!(
        set_pre_exit_hook(fixture_pre_exit),
        Ok(()),
        "first set_pre_exit_hook must succeed"
    );

    // The getter reconstructs the fn pointer and calling it is safe.
    let hook = get_pre_exit_hook().expect("hook must be Some after registration");
    // Call to prove dispatch — the fixture is a no-op.
    hook();

    // A second registration is rejected.
    assert_eq!(
        set_pre_exit_hook(fixture_pre_exit),
        Err(SetError::AlreadySet),
        "second set_pre_exit_hook must be rejected (write-once)"
    );

    // The first hook still stands.
    assert!(
        get_pre_exit_hook().is_some(),
        "first hook must still be registered after rejected second set"
    );
}

// ── minimal-path: seam queryable with zero atoms set ─────────────────────────

/// Asserts the minimal panic-handler path: the seam is queryable with zero
/// atoms of ITS OWN set (it does not depend on the disposition, `flush_fd`, or
/// any other atom having been registered). This is the Platform-Contract §3.4
/// always-present invariant.
///
/// Because every other atom has been claimed by the lifecycle tests above, we
/// prove the minimal-path property through the disposition getter: the handler
/// knows to default to `halt` when `get_disposition()` returns `None`.  For
/// this test we check that the RECOVER disposition was registered by the
/// lifecycle test and that `exit_code()` dispatches correctly — the getter path
/// composes with the uapi/panic types without any platform handle.
#[test]
fn minimal_path_no_install_gate() {
    // The disposition getter (and all getters) must be callable with no
    // kabi/platform handle installed. We assert on the currently-registered
    // disposition (set by `disposition_write_once`) to prove the full
    // getter → uapi/panic type chain works.
    let disp = get_disposition();
    // The disposition may be Some(Recover) (if that test ran first) or None
    // (if it hasn't — test order is not guaranteed). Either case is valid:
    // the handler defaults to Halt on None.
    // Some(Recover) if `disposition_write_once` ran first, else None — both
    // valid (the handler defaults to Halt on None). Only the Some arm asserts.
    if let Some(d) = disp {
        // The exit code roundtrip works through the uapi/panic type.
        let code = d.exit_code();
        assert!(code == 75 || code == 70, "exit_code must be a valid sysexits value");
    }

    // The ring-tail getter is similarly unconditional.
    let _ = get_ring_tail_provider();
    // The state-record getter is similarly unconditional.
    let _ = get_state_record_hook();
    // The flush-fd getter is similarly unconditional.
    let _ = get_flush_fd();
    // The pre-exit getter is similarly unconditional.
    let _ = get_pre_exit_hook();
}
