//! `reovim-arch` — the platform floor.
//!
//! This is the `#![no_std]` crate that owns the OS boundary for the v0.16
//! sovereign rebuild (DAG6, 1.2 §10): raw syscall FFI, process entry/exit, the
//! panic handler, the allocator, and the futex/thread primitives. The heap-DS
//! *algorithms* (`Seq`/`Map`/`Bytes`/`Mutex`/`RwLock`/`Condvar`) are NOT here —
//! they are portable Math in `lib/ds`, reaching arch's allocator + futex
//! backend through the boot-installed `kabi` handle (SP03). arch owns the
//! backend *primitives* and implements the `kabi` platform contract; it holds
//! zero heap DS of its own (the panic renderer uses a fixed stack buffer, not a
//! handle-routed DS, so a panic before the handle installs still renders).
//!
//! `unsafe` is allowed here, and only here, because `arch/` is the one
//! crate that owns OS FFI: inline-asm syscalls and raw-pointer memory
//! management cannot be expressed in safe Rust. The workspace lint stays
//! `warn`, so any other crate that introduces `unsafe` still flags. Every
//! `unsafe` block in this crate carries a `// SAFETY:` comment stating the
//! invariants it relies on.
#![no_std]
// SAFETY (lint): arch is the platform floor — inline-asm syscalls and raw
// memory management require unsafe. The workspace lint stays `warn` so every
// crate above arch still flags unsafe; the allow is scoped to this crate.
#![allow(unsafe_code)]

pub mod alloc;
#[cfg(target_os = "linux")]
pub mod net;
pub mod panic;
pub mod sys;
#[cfg(target_os = "linux")]
pub mod term;
pub mod thread;
pub mod time;

// The process entry (`_start`/`rust_entry`/`exit_process`/the `entry!` shim),
// the freestanding mem intrinsics, the LLVM coverage profiler runtime, the
// `#[panic_handler]` lang item, and `rust_eh_personality` were relocated out of
// arch into the per-target `reovim-arch-floor-{target}` crates (SP03). arch no
// longer defines any of those lang items / link symbols; the composition root
// (apps/* + the fixtures) links the matching floor crate. `arch::panic` keeps
// only the product-facing registration shims (the kabi/panic forwards). See
// `arch/src/panic.rs`.

// The no_std test runner (#785 Phase 5): a libtest-free test-collection +
// reporting mechanism the arch test bins boot through. Compiled under
// `runtime` (the bin path) AND under `selftest` alone, because the selftest
// test modules register into it at lib-build time — the module itself is
// plain no_std code (registration + report loop over raw `write`); only the
// BIN that boots `run()` needs the `runtime` lang items.
#[cfg(any(feature = "runtime", feature = "selftest"))]
pub mod testrt;

// Re-export the `arch_test!` macro from the leaf at the arch crate root so
// `reovim_arch::arch_test!` keeps resolving for every external consumer
// (tui/kernel/server fixtures + arch's own test modules). The macro is
// `#[macro_export]` in `reovim-testrt`; this `pub use` republishes it here. It
// emits `$crate::TestCase` — `$crate` resolves to the definition crate
// (`reovim-testrt`), whose root holds `TestCase`, so the expansion is correct
// regardless of the invocation path.
#[cfg(any(feature = "runtime", feature = "selftest"))]
pub use reovim_testrt::arch_test;

// L12 layout (#785 Phase 5): all inline `#[cfg(test)]` blocks have been
// migrated to sibling `*_tests.rs` files compiled under the `selftest`
// feature. The libtest runner (`cargo test -p reovim-arch`) no longer builds
// any arch-internal tests; the arch-selftest binary (feature "selftest" +
// "runtime") is the sole test-execution path for this crate.
//
// The time tests access only public API so they are declared here as a
// parent-declared sibling; every other test module is declared inside its
// source file via the `#[path]` child pattern (giving `super::` access to
// crate-private items).
#[cfg(feature = "selftest")]
pub mod time_tests;

// arch::net and arch::term have their test modules declared via `#[path]`
// children inside `net/mod.rs` and `term/mod.rs` respectively (the standard
// L12 pattern for modules that need `super::` access). No additional
// parent-level declaration is needed here.

// SP07 park: the mem-intrinsic / panic-path / profiler / start selftests are
// NOT re-homed into the floor crates this flight. Their source modules
// (`arch/src/{mem_tests,panic_tests,profiler_tests,start_tests}.rs`) stay in
// place but are no longer declared — `mem`/`panic-handler`/`profiler`/`start`
// moved to `reovim-arch-floor-{target}`, and re-homing their unit tests there
// would force each minimal floor crate to carry a `selftest`-gated
// `reovim-testrt` dep (DAG5/DAG6 wants the floor minimal). The five mem
// intrinsics, the panic path, and the `arch_coverage` boot path are exercised
// at runtime by the SP03 Phase-4 bare-metal fixtures (the boots themselves emit
// the compiler-lowered mem calls; `panic-{ab13,halt,recover}` assert the
// handler; the coverage merge proves the profiler). Re-home is tracked in
// 00-master-plan's deferred list → SP07.

// The `lib/ds` integration selftests. The DS/sync *algorithms* moved to the
// `reovim-lib-ds` crate (SP03), but the no_std runner, the live allocator (with
// its fault-injection seam + `live_bytes` accounting), the thread primitive,
// and the futex backend all live HERE. These tests construct `reovim_lib_ds`
// types AFTER `rust_entry` installs the handle and exercise alloc + park/unpark
// through it end-to-end — so they are arch-hosted integration tests, not
// lib/ds-internal ones. arch enables `reovim-lib-ds/selftest` to reach the
// test-only observation surface (`testhooks`, the growth-ladder constants).
#[cfg(feature = "selftest")]
pub mod lib_ds_tests;
