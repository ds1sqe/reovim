//! `reovim-arch` — the platform floor.
//!
//! This is the single `#![no_std]` `core`-only crate that owns the OS
//! boundary for the v0.16 sovereign rebuild (DAG6, 1.2 §10): raw syscall
//! FFI, process entry/exit, the panic handler, the allocator, and every
//! heap data structure consumed above it. Crates above `arch/` never
//! declare OS bindings or heap primitives — they consume `arch/` alone.
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
pub mod ds;
#[cfg(target_os = "linux")]
pub mod net;
pub mod panic;
pub mod sync;
pub mod sys;
#[cfg(target_os = "linux")]
pub mod term;
pub mod thread;
pub mod time;

// The process entry (`_start`, the `entry!` shim, the exit/profraw shim) is
// runtime-only: defining the `_start` global symbol in arch's pre-migration
// libtest builds (bootstrap state 1) clashes with the std runtime's entry.
#[cfg(feature = "runtime")]
pub mod start;

// The freestanding mem intrinsics are runtime-only for the same reason:
// libc already defines memset/memcpy/memmove/memcmp/bcmp in libtest builds.
#[cfg(feature = "runtime")]
pub mod mem;

// The minimal LLVM coverage profiler runtime (#785 Phase 5). It defines the
// `__llvm_profile_runtime` marker and `__llvm_profile_write_file`, so it is
// compiled only when coverage instrumentation is in effect (`arch_coverage`),
// where the `__llvm_prf_*` sections exist; it reads the captured env block, so
// it also needs `runtime`. An ordinary build compiles none of it.
#[cfg(all(feature = "runtime", arch_coverage))]
pub mod profiler;

// The no_std test runner (#785 Phase 5): a libtest-free test-collection +
// reporting mechanism the arch test bins boot through. Compiled under
// `runtime` (the bin path) AND under `selftest` alone, because the selftest
// test modules register into it at lib-build time — the module itself is
// plain no_std code (registration + report loop over raw `write`); only the
// BIN that boots `run()` needs the `runtime` lang items.
#[cfg(any(feature = "runtime", feature = "selftest"))]
pub mod testrt;

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

// The mem intrinsic tests: runtime-gated (the functions are `#[no_mangle]`
// symbols that clash with libc in libtest builds; only the arch-selftest
// binary enables both "selftest" and "runtime").
#[cfg(all(feature = "selftest", feature = "runtime"))]
pub mod mem_tests;
