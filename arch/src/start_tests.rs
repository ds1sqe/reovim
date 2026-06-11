//! Tests for `start.rs`, compiled into the lib under `selftest` + `runtime`
//! (#785 Phase 5 coverage).
//!
//! L12 layout: declared inside `start.rs` as
//! `#[cfg(all(feature = "selftest", feature = "runtime"))]
//!  #[path = "start_tests.rs"] mod tests;`
//! so `super::` reaches the private `ENVP` and the public `env_block`.
//!
//! Coverage targets:
//! - start.rs L126: `env_block()` returns `&[]` when `ENVP` is null (null arm)
//! - start.rs L133: `while n < ENV_MAX` — the ENV_MAX limit exit (never hit
//!   in a normal process because env blocks are far smaller than 4096 entries;
//!   this is a structurally unreachable arm for any real process)
//!
//! For the ENV_MAX arm: since forcing it requires >4096 env entries (not
//! possible in-process without unsafe pointer surgery on the global), it is
//! recorded as a remaining-open decision in the issue's coverage ledger
//! (`Documentation/debt/coverage-785-platform-floor.md`).
//! The null-arm is forceable by temporarily clearing ENVP.

use core::sync::atomic::Ordering;

use crate::arch_test;

use super::{ENVP, env_block};

arch_test!(start_env_block_null_envp_returns_empty, {
    // Drive the `if envp.is_null() { return &[]; }` arm (start.rs L125-127)
    // by temporarily storing null into ENVP and calling env_block().
    //
    // Save the current ENVP value, null it, call env_block, restore.
    // The arch-selftest runner is single-threaded sequential so no concurrent
    // reader will observe the temporary null.
    let saved = ENVP.load(Ordering::Acquire);
    ENVP.store(core::ptr::null_mut(), Ordering::Release);

    let result = env_block();

    // Restore before any assertion that could panic.
    ENVP.store(saved, Ordering::Release);

    crate::testrt::check(result.is_empty(), "env_block returns empty slice when ENVP is null");
});

arch_test!(start_env_block_normal_envp_returns_nonempty, {
    // With ENVP set (which it always is in the selftest binary since rust_entry
    // stored it at startup), env_block() returns a non-empty slice. This
    // exercises the normal path past the null check.
    let block = env_block();
    crate::testrt::check(!block.is_empty(), "env_block returns non-empty slice with real envp");
    // Every entry should be NUL-terminated (the last byte is 0).
    for entry in block {
        crate::testrt::check(entry.last() == Some(&0u8), "each env entry is NUL-terminated");
    }
});
