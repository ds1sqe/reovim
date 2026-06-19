//! The `no_std` uapi self-test runner (#786 Phase 5).
//!
//! Boots through the arch `_start` and runs the full uapi test suite via
//! [`reovim_arch::testrt`]. Test bodies live in sub-modules of this bin crate
//! (one sub-module per test group), registered via `arch_test!`. The bin owns
//! no tests of its own beyond the inject-failure variant; all substantive tests
//! are in the sub-modules.
//!
//! Covered test groups:
//! - `layout_goldens` — ABI size/offset/align golden table (uapi/abi, Phases 1/3).
//! - `codec_goldens` — 57-byte Hello frame golden + all 37 message round-trips
//!   + failure modes (uapi/protocol, Phases 2/3).
//! - `cf5_crosscheck` — §7 inventory ↔ struct TAG/DIRECTION parity (Phase 3).
//! - `macro_smoke` — declare_module!/declare_*_driver! emit a correct vtable
//!   shape (uapi/{module,driver}-macros, Phase 4).
//! - `edge_cases` — coverage-closure edge cases: all enum arms, non-empty list
//!   encode/decode paths, truncation sweeps, frame error arms, state-machine
//!   branches (#786 Phase 5 coverage-closure slice).
//!
//! The `inject-failure` feature adds one deliberately-failing test (mirrors the
//! arch-selftest pattern) so the integration test can assert a non-zero exit.
#![no_std]
#![no_main]
// `entry!` expands to `#[unsafe(...)]` symbol declarations; a test-runner bin
// requires unsafe by nature.
#![allow(unsafe_code)]

use reovim_arch::testrt;

mod codec_goldens;
mod cf5_crosscheck;
mod edge_cases;
mod layout_goldens;
mod macro_smoke;

// One deliberately-failing test, present only under `inject-failure`.
#[cfg(feature = "inject-failure")]
reovim_arch::arch_test!(deliberately_fails, {
    reovim_arch::testrt::check_eq(1 + 1, 3);
});

#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
use reovim_arch_floor_linux_x86_64::entry;
#[cfg(all(target_os = "linux", target_arch = "aarch64"))]
use reovim_arch_floor_linux_aarch64::entry;

entry!(|_argc, _argv, _envp| {
    // Install the platform handle as the closure's first statement, before any
    // test reads it. Enabling `reovim-arch/selftest` links arch's `lib_ds_tests`
    // into the shared link-section registry, and those cases allocate + park
    // through the handle, so `testrt::run` reads it. This runner is host-only
    // (it self-skips under system-image mode), so it installs the real POSIX
    // provider unconditionally — no bare-metal scaffold branch.
    let _ = reovim_platform_linux_native::install_platform();
    testrt::run()
});
