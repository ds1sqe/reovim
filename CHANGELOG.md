# Changelog

For old changelog, see `changelog/CHANGELOG-{version}.md`

## [0.16.0-dev] - Unreleased

### Added
- `arch/` + `lib/depgraph/` doc-test coverage (#785 Phase 6): every `pub`
  item in `arch/` and `lib/depgraph/` now carries a runnable doc-test
  exercising its primary contract; runtime-gated items (`_start`, `entry!`,
  `mem.rs` intrinsics, `profiler.rs`) and `testrt` items use `ignore`
  fences; process-terminating and write-once process-global items use
  `no_run`; `TomlDoc`, `TomlValue`, `parse_text`, `parse_file`, `classify`,
  `run_probe`, and all `has_*`/`strip_*` helpers carry in-memory runnable
  examples; L12 test conventions (sibling `*_tests.rs` layout +
  doc-tests on every `pub` item) enforced by
  `scripts/check-test-layout.sh` and `cargo test --doc` in
  `scripts/check.sh`.
- `arch/` 100% MC/DC coverage closure (#785 Phase 5): DEV1 restructures
  removing structurally-unreachable branches (`class_for` dead-None tail →
  scan-N-1-return-last; post-noreturn `exit_group`/`exit` spin loops →
  `unreachable_unchecked` with kernel-ABI SAFETY; `render_panic_message`
  `location()` None arm → `unwrap_unchecked` citing the Rust Reference
  guarantee); selftest seam `spawn_with_clone_flags` (gated `selftest`)
  forcing the clone-Err teardown path via CLONE_THREAD without CLONE_SIGHAND
  (EINVAL); new test files `mem_tests.rs` (memset/memcpy/memmove/memcmp/bcmp
  coverage, both memmove branches), `start_tests.rs` (env_block null-ENVP
  arm), `profiler_tests.rs` (write_all/write_padding/profile_path helper
  coverage, gated `arch_coverage`), `testrt_tests.rs` (current_test None arm,
  write_all Err arm, check unknown-label path); extended tests in all existing
  `*_tests.rs` files covering ring head-wrap pop, rwlock FUTEX_WAIT contention,
  condvar multi-iteration wait, map backward-shift stay arm, thread
  is_finished ctid-0 branch, clone-Err teardown, and panic write_all Err arm;
  remaining OOM `?`-propagation Err branches (seq/bytes/ring/map/shared alloc
  failure arms) and mprotect-failure arm ledgered in
  `Documentation/debt/coverage-785-platform-floor.md` (not forceable
  in-process without RLIMIT_AS).
- `arch/` no_std test runner + minimal profiler runtime (#785 Phase 5):
  an in-repo libtest-free test runner (`arch::testrt`) with declarative
  `arch_test!` registration via a `#[used]` link-section distributed slice
  (no central list), raw `write`-to-fd progress reporting, and a fail-fast
  failure model (a failing test panics; the arch panic handler exits the
  halt code, so the process exit code is the pass/fail signal) that names
  the failing test via a published current-test name; an arch-owned LLVM
  coverage profiler runtime (`arch::profiler`, gated `arch_coverage`)
  defining `__llvm_profile_runtime` + `__llvm_profile_write_file` that
  walks the `__llvm_prf_{data,cnts,bits,names}` ELF sections and serializes
  a raw-profile file via arch syscalls, reading `LLVM_PROFILE_FILE` from the
  captured envp (the toolchain's compiler-rt runtime needs ~38 libc symbols
  a no_std bin cannot satisfy), with the profraw version read from the
  toolchain's `__llvm_profile_get_version` rather than hardcoded; a public
  `Instant::from_timespec` constructor and an `env_block` env accessor on
  the entry path; the `time.rs` test suite migrated onto the runner as the
  pilot (L12 layout), exec'd by integration tests asserting exit 0 on
  all-pass and the halt code on a deliberately-failing variant; and the
  bulk L12 migration: ALL inline `#[cfg(test)]` blocks removed from every
  arch impl file; per-module sibling `*_tests.rs` files compiled into the
  lib under the `selftest` feature (bin enables both `runtime` + `selftest`);
  `arch-testrt-pilot` renamed to `arch-selftest` and extended to run the full
  arch suite (ds, sys, sync, thread, time, panic modules); `read` syscall
  (nr 0) added with success- and failure-path coverage; `reset_registry()`
  discipline (called at end of each panic test); file-based output capture
  replacing `pipe()`/`std::fs::File`; no `testutil::serial()` (runner is
  single-threaded sequential); `std::sync::Arc`→`Shared`, `std::vec::Vec`→
  `Seq`, `std::thread::spawn`→`crate::thread::spawn`, sleep→bounded spin;
  and `scripts/coverage-fixtures.sh` documenting the instrumented build
  (`-C instrument-coverage -Z no-profiler-runtime --cfg arch_coverage`).
- `arch/` process entry + AB12 panic handler (#785): naked `_start`
  (`#![no_main]` bins via the `arch::entry!` macro) that unpacks
  argc/argv/envp and `exit_group`s the shim's return code, with an
  instrumentation-gated `__llvm_profile_write_file` exit shim (`--cfg
  arch_coverage`) so exec'd no_std fixtures emit coverage profraw; a
  `#[panic_handler]` (gated behind the `runtime` feature) rendering the
  panic as a LOG2 kernel-emitter line (9.5 §2), performing the final
  flush to a registered fd ahead of the line (9.5 §9.1), firing the
  state-record hook, and exiting per disposition (recover→75
  `EX_TEMPFAIL`, halt→70 `EX_SOFTWARE`; 6.2 §5.1); the four write-once
  hook seam (`set_flush_fd`/`set_ring_tail_provider`/`set_state_record_hook`/
  `set_disposition`, Release/Acquire, `Err(AlreadySet)` on re-register;
  6.2 §5.2); the AB13 cleanup-panic marker recording `rollback = failed`;
  and `#![no_std] #![no_main]` fixture bins (charter smoke + halt/recover/
  AB13 panic paths) exec'd by integration tests asserting exit codes and
  LOG2-grammar-parsed lines. Adds the `O_WRONLY`/`O_CREAT`/`O_TRUNC`
  `openat` flags to the syscall floor.
- `arch/` sync, thread, and clock floor (#785): futex-backed `Mutex`
  (three-state lock word), `RwLock` (single-word writer-flag + reader-count
  protocol), and `Condvar` (sequence-number wait closing the missed-wake
  window), all implementing the normative arch sync-primitive ordering
  contract (2.3 §1.1) with no poisoning under `panic = "abort"`;
  `clone`-based `spawn`/`JoinHandle` over an mmap stack with a `PROT_NONE`
  guard page, a `CLONE_CHILD_CLEARTID` futex join word, and joiner-owned
  teardown (TLS-free, join-only); `Instant`/`monotonic`/`realtime` clocks
  over `clock_gettime` (the LOG2 timestamp source). Adds the `mprotect` (nr
  10) and `exit` (nr 60, thread-only) syscalls to the floor.
- v0.16 workspace scaffold: sovereign depgraph probe engine (`lib/depgraph`,
  zero third-party dependencies in all three dependency tables) enforcing
  DAG1..DAG5 including the three-dep-table sovereignty walk and the
  `Cargo.lock` resolved-graph gate (`sovereignty_gate.rs`); `scripts/check.sh`
  and `scripts/coverage.sh`; per-PR CI with a dedicated `sovereignty` job
  (`cargo test -p reovim-depgraph --test sovereignty_gate`). (#783)

### Changed
- Spec: Zero-Std Sovereignty is law (`DAG6`, 1.2 §10) — every product
  crate is `#![no_std]` and `alloc`-free including `arch/`, which owns
  the platform floor (syscall FFI, `_start`, panic handler, allocator,
  sync, all heap data structures); workspace `panic = "abort"`; the
  North Star gains the Mission-to-Mars reliability doctrine. Protocol
  purity + carrier seam (`SP15`/`SP16`, 7.3 §1a): the wire protocol is
  sans-IO pure over caller-provided buffers (`SP13` re-signed,
  `encoded_size` sizing contract, `ErrorCode::BufferTooSmall`); the
  carrier (UDS/TCP default; HTTP/WebSocket/gRPC/file possible) is a
  replaceable byte-mover. AB12 panic isolation re-specified for
  no-unwinder reality: panic disposition (`recover`/`halt`), the
  panic-time final ring flush (one kernel log buffer, no second
  log), first-panic quarantine via persisted state.
  `std::` realization claims scrubbed to `arch/` contracts. (#784)
- Spec: Dependency Sovereignty is law (`DAG5`, 1.2 §9) — the workspace
  dependency graph is closed to std + in-repo crates across all three
  dependency tables; OS access via `arch/`-owned FFI; North Star
  (fastest-reaction, 50-year survivability) stated in the spec README. (#782)
- Spec: the server-client wire protocol is redesigned from gRPC to an
  in-house framed protocol (7.3 full rewrite: SP9..SP14, message
  inventory with tags, deterministic byte codec aligned with the 6.3
  catalog, Hello/HelloAck handshake, reject/ErrorCode model, worked
  byte-level golden); 6.3 registers `FrameHeader` and fixes the
  fieldless-enum repr convention; ~20 chapters scrubbed of
  third-party mechanism assumptions. (#782)
- `Documentation/` is now the normative spec SSOT (v4 draft): architecture,
  process, state, domain substrate, view, ABI, surfaces, client,
  conformance, and development process chapters. (#777)
- Pre-0.16 docs and CI workflows moved to `archive/` (reference only). (#777)
