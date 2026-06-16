//! Integration selftests for the `reovim-lib-ds` data structures + sync
//! primitives, hosted by `arch` (#785 Phase 5 coverage, relocated in SP03).
//!
//! ## Why arch hosts these (not `lib/ds`)
//!
//! `lib/ds` is portable Math: it depends on `reovim-kabi-platform` ONLY and may
//! name no `arch` symbol (master invariant 2, the `lib/ds ⊄ arch` probe). These
//! tests, however, require arch-owned machinery the algorithm itself never
//! touches: the no_std test runner (`arch_test!`/`testrt`), the live allocator
//! with its `live_bytes` accounting and `fault` injection seam, the thread
//! primitive (`crate::thread`), and the monotonic clock (`crate::time`). So the
//! tests live here, in arch, and import the types under test from
//! `reovim_lib_ds::*`. They run AFTER `rust_entry` installs the platform handle,
//! exercising alloc + a park/unpark cycle through the handle end-to-end — the
//! SP03 integration smoke.
//!
//! Each submodule reaches the lib/ds test-observation surface
//! (`reovim_lib_ds::testhooks`, the `selftest`-gated growth-ladder constants and
//! helper methods) through the `reovim-lib-ds/selftest` feature arch enables.

mod bytes_tests;
mod condvar_tests;
mod ds_tests;
mod map_tests;
mod mutex_tests;
mod ring_tests;
mod rwlock_tests;
mod seq_tests;
mod shared_tests;
