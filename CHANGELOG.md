# Changelog

For old changelog, see `changelog/CHANGELOG-{version}.md`

## [0.16.0-dev] - Unreleased

### Changed
- `Documentation/` spec made self-contained: draft-version references
  (the prior v3/v4 spec-draft vocabulary) and workflow-local file
  citations removed from every normative chapter; heritage one-liners
  now cite only durable artifacts (folded RFCs, archive paths, issue
  refs). Coverage ledgers live under `Documentation/debt/`
  (`coverage-<issue>-<subject>.md`), named by the DEV1
  physical-measurement-limits clause in
  `Documentation/10-Development/01-Testing.md`.

### Added
- `server/lib/kernel` selftest runner wiring + fixture exec harness + L12 doc-tests — Phase 5 of #796:
  `server/lib/kernel/tests/fixtures/` gains two panic-path fixture bins (`kernel-panic-halt`,
  `kernel-panic-recover`) that boot the kernel via `Init::new(LauncherArgs{disposition:Halt/Recover,..)}.boot()`,
  open a LOG7 sink file from argv[1] via raw arch syscalls, call `set_flush_fd`, emit a few DS12 events
  through `kernel.event_bus` to populate the ring/flush buffer, then panic. The arch panic handler
  flushes the ring's LOG2 lines + the panic line to the sink file and exits 70 (Halt) or 75 (Recover).
  `server/lib/kernel/tests/fixtures_exec.rs` (std libtest integration harness, mirrors
  `arch/tests/fixtures_exec.rs`) builds each fixture via the nested workspace manifest, execs it,
  and asserts: exit code matches the configured disposition; flushed file contains at least one LOG2
  line (ring had content); the panic line parses against the LOG2 §2 grammar (emitter=kernel,
  address=panic, message substring match — not exact bytes, the timestamp varies per run); a grep-assert
  confirms no `catch_unwind` in `server/lib/kernel/src/` (DAG6/AB12). State-record AC (c) is covered
  in-process by `init_tests.rs` via `last_panic_record()`; that hook stores to an `AtomicU32` that
  exits with the process and cannot be observed out-of-process. Selftest round-trip: `kernel_selftest_all_pass_exits_zero`
  and `kernel_selftest_inject_failure_exits_nonzero` guard the shared-artifact hazard with a
  `selftest_lock()` spanning build + exec. `scripts/coverage-fixtures.sh` extended to build and run
  the kernel fixture fleet (`kernel-selftest`, `kernel-panic-halt`, `kernel-panic-recover`, and the
  `kernel-selftest inject-failure` variant) under coverage instrumentation so their profraws land in
  the merge. Doc-test additions: `SUBSCRIBER_CAPACITY` (runnable one-liner); `sink::reset_for_test`
  and `sink::inject_fd_for_test` (no_run with selftest-gated reason). `check-test-layout.sh` path-
  agnostic scan already covers `server/lib/kernel/` (the `server/` tree is in SEARCH_DIRS).

  Coverage closure (DEV1, 10.1 §2): 100% functions, 99.20% lines, 99.04%
  regions on the kernel fixture fleet; zero compound-condition sites so
  MC/DC condition coverage is vacuously complete. Closure mechanisms: the
  arch alloc-fault seam widened to `pub` (`reovim_arch::alloc::fault`,
  kernel is its second consumer), capacity-boundary brute-force sweeps for
  `Bytes` write-site error arms, a per-stage failure-injection sweep, and
  a selftest-gated small-capacity entry for the flush mirror's overflow
  branches. The 7 remaining regions (selftest-unreachable seam-registration
  production arms; fd > `i32::MAX` kernel-contract guard) are individually
  classified in `Documentation/debt/coverage-796-kernel-boot-core.md`.
- `server/lib/kernel` arch panic-seam registration + panic-mirror region — Phase 4 of #796:
  `log/flush.rs`: a 1 MiB `static mut` BSS byte region (demand-paged, no allocator) kept
  current at every ring append (`update_after_push` called from `LogRing::push_event` after
  the ring lock releases); atomic length cursor written with `Release` after bytes are placed
  so the `ring_tail` provider loads with `Acquire` and returns the valid prefix — no
  allocation, no lock in the panic handler (§9.1 Release/Acquire discipline). Overflow
  compaction calls `compact_into_buf` to restart from live ring contents; whole LOG2 lines
  only (no partial lines at boundaries). `Init::boot` registers three arch panic-seam hooks
  in boot stage 0: `set_ring_tail_provider(flush::ring_tail)`, `set_state_record_hook`
  (boot-core stub storing `PanicRecord` into an `AtomicU32` slot readable under `selftest`),
  and `set_disposition` from `LauncherArgs` (gap-4: spec default `Recover` when absent).
  `set_flush_fd` is registered by `FileSink::open_and_subscribe` at sink-open time (the fd
  does not exist at boot; the write-once contract prevents a second registration). A second
  boot in the same process returns `BootError::SeamRegistration` in production; under
  `selftest` `AlreadySet` is accepted silently (process-global arch statics shared across
  the single-process no_std runner). `unsafe` surface: one `static mut` byte array +
  `&raw mut` writes per arch convention; one `#![allow(unsafe_code)]` on `flush.rs`; zero
  new `unsafe` elsewhere in the kernel crate. Sibling `flush_tests.rs` covers initial
  empty state, append discipline, cursor monotonicity, byte-compare against ring, direct
  compaction with a small capacity bound, all four `PanicRecord` combinations, and the
  second-boot-ok selftest rule.

- `server/lib/kernel` log ring, LOG2 renderer, LOG7 file sink — Phase 3 of #796:
  `LogRing` over `arch::ds::Ring<LogEntry>` allocated in boot stage 0 before any
  event (LOG6 pre-sink capture); the bus holds the ring directly as its
  always-present built-in subscriber (`Shared<LogRing>`, written once during
  single-threaded boot — LOG1: one rendering, no second pipeline, no `unsafe`
  in the kernel crate). `render_line` produces byte-deterministic LOG2
  canonical lines per 9.5 §2 grammar with right-aligned-5 seconds and zero-padded-6
  micros, all 6 instance-address variants, and embedded-`\n` escaping. `FileSink`
  (process-global const-initialized state; one sink per kernel this flight)
  opens `<state-dir>/reovim.log` with `O_APPEND | O_CREAT`, replays the ring head on
  open so the file contains the full boot sequence (LOG8), then writes every subsequent
  DS12 event as a LOG2 line; a write failure closes the sink first and then emits one
  `log.sink.fail` (LOG7 non-blocking contract — re-entrant emit is CC6-safe and the
  closed sink ignores it). Early-boot stderr echo gated on sink-open + headless
  posture (LOG8). `arch::ds::Ring<T>` gains explicit `Send + Sync`
  impls (following `Seq<T>` pattern) so `Mutex<Ring<LogEntry>>` compiles. `Kernel`
  gains `log_ring: Shared<LogRing>`; `Init::boot` wires ring → bus built-in before
  stages 1..7. Sibling `*_tests.rs` (LOG2 golden byte-comparison, LOG6 oldest-first
  eviction, LOG1 1:1 invariant, `log.sink.fail` on forced write failure, LOG8 gate,
  full `Init::boot` + sink integration smoke byte-matching file vs ring).

- `server/lib/kernel` DS12 event bus + boot-stage events — Phase 2 of #796:
  `DS12Event` realizing the 9.4 §1 schema view in `no_std` types (9.4 §1
  schema-view note; `BootClock`-derived timestamps, fixed structured field
  set), `DS12EventBus` with CC6 clone-then-invoke fan-out (subscriber list
  copied under the arch `RwLock` read lock, callbacks invoked lock-free —
  re-subscribe during a callback cannot deadlock), registration-order
  delivery, spec-default capacity 64 (9.4 §7) enforced at `subscribe` with
  a typed `SubscribeError`, and a built-in LOG1 slot for the Phase 3 log
  ring. Boot-stage driver emits `boot.stage.{start,ok,fail}` (OBS1) for
  stages 1..7 through `Init::boot`; a failing stage emits `fail` and aborts
  boot (no later stage runs, no `Kernel` is constructed); correlation IDs
  omitted per the OBS2 early-bootstrap exemption. `kernel-selftest` fixture
  bin (nested workspace mirroring `arch/tests/fixtures/`) runs the kernel
  `*_tests.rs` suite on the arch no_std runner: 200 tests green, inject-
  failure variant exits 70.
- `server/lib/kernel` boot-core scaffold — Phase 1 of #796 (#778 Phase 2):
  `reovim-kernel` crate at `server/lib/kernel`, `#![no_std]` over `arch/` +
  `uapi/abi` (L9/DAG5 + L10/DAG6). `Init`/`Kernel` typestate (LF13): `Init`
  is the sole boot actor; `Init::boot` is the only `Kernel` constructor; using
  `Init` after `boot()` is a compile error (moved value). `BootClock` captures
  the `CLOCK_MONOTONIC` zero and `CLOCK_REALTIME` wall-clock anchor at stage 0
  (7.5 §4). Boot stages 0..7 scaffolded as structural stubs (2.2 §1 stub rule:
  stages 1..6 hold their ordering position and emit no events yet; DS12 bus
  lands in Phase 2). `Kernel` boot-core subset (2.1 §3 note): `abi`
  (`Shared<KernelAbi>`) and `boot_anchor` realized; all deferred fields carry
  `()` placeholders per the rule-of-three (no registry type before its
  walking-skeleton consumer). `reovim-kernel` registered in the depgraph
  category table (`ServerKernel`, exact path `server/lib/kernel`) with
  `ServerKernel → Foundation` allowed edges (DAG5/DAG6 green). Sibling test
  files (`*_tests.rs`) and crate-level doc-tests on every `pub` item follow the
  L12 convention.


- `arch/` aarch64-linux platform floor (#790): per-target backend
  structure under `arch/src/sys/` (`linux_x86_64/` and `linux_aarch64/`,
  each with `raw.rs` asm + syscall-number table + fused `clone_into`
  trampoline; the aarch64 backend also carries a `getauxval` stub);
  `errno.rs`/`wrap.rs` stay at the `sys/` level as shared Linux-kernel-ABI
  family code; `arch/src/sys/mod.rs` cfg-selects the active backend and
  emits a `compile_error!` fallback for unsupported targets; per-target
  `_start` entry arms in `arch/src/start.rs`; cross-target fixture
  harness env contract (`ARCH_FIXTURE_TARGET` + `ARCH_FIXTURE_RUNNER` on
  `arch/tests/fixtures_exec.rs`, native default unchanged); asm-confinement
  source-grep probe added to `lib/depgraph/tests/dag6_conformance.rs`
  enforcing that every `asm!`/`naked_asm!` token in `arch/src/` is confined
  to `arch/src/sys/<target>/` or `arch/src/start.rs`.
- `uapi/abi` frozen `#[repr(C)]` type catalog (#786 Phase 1): the 6.3/6.2/6.4
  ABI type catalog as a `#![no_std]` core-only crate (`reovim-uapi-abi`);
  zero dependencies (L9/DAG5); every type mirrors its spec section with a
  doc comment and doc-test (L12.2).  Workspace members added for all four
  uapi crates (`uapi/abi`, `uapi/protocol`, `uapi/module-macros`,
  `uapi/driver-macros`).  Foundation sub-DAG grants table added to
  `lib/depgraph` (`uapi/protocol → uapi/abi`).  `CHANGELOG.md` entry.
- `uapi/protocol` sans-IO framed-protocol codec and message inventory
  (#786 Phase 2): the deterministic byte codec (`Encoder`/`Decoder` over
  caller buffers, §6), the 36 hand-written message structs (§7) with the
  `encode`/`decode`/`encoded_size` triple (SP13) and borrowing decoded views
  (SP17), zero-copy list views (`StrList`/`RawInputList`/`DomainEntryList`/
  `CarrierList`), frame assembly with the SP10 `wire-max-frame-bytes` cap
  check, and the pure `Send + 'static` handshake/correlation/unknown-tag state
  machine (§10).  `#![no_std]`, no-alloc, no `arch/` edge; every `pub` item
  carries a doc-test.
- L9-clean declare macros for module and driver vtable exports (#786 Phase 4):
  `reovim-uapi-module-macros` (`declare_module!`, `declare_module_client!`) and
  `reovim-uapi-driver-macros` (`declare_driver_server!`, `declare_driver_client!`,
  `declare_capability_client!`, `declare_domain_server!`, `declare_provider_server!`,
  `declare_stream_scheme!`) implemented as `macro_rules!` in `#![no_std]`
  ordinary lib crates (NOT `proc-macro = true`; `syn`/`quote`/`proc-macro2`
  are absent per L9/DAG5). AB12 disposition shape: no `catch_unwind` anywhere;
  panics in vtable slots reach the `arch/`-owned handler under `panic = "abort"`
  directly. Textual-path discipline: macros emit `::reovim_uapi_abi::*` tokens;
  the macro crates have zero shipped deps (DAG5). `[dev-dependencies]` on
  `reovim-uapi-abi` added to both Cargo.toml files for doc-test compilation
  (test-only; shipped dep graph unaffected). Each macro exports the 6.2 §2
  canonical symbol (`REOVIM_MODULE_SERVER_VTABLE`, `REOVIM_DRIVER_SERVER_VTABLE`,
  etc.) as a `#[unsafe(no_mangle)] pub static` with a scoped `#[allow(unsafe_code)]`
  and a SAFETY comment citing the FFI export seam.
- uapi no_std selftest runner migration (#786 Phase 5): all uapi test bodies
  migrated to the arch `testrt` no_std selftest runner in a new
  `uapi-selftest` bin added to the existing `arch/tests/fixtures` nested
  workspace.  Test groups: `layout_goldens` (all ABI size/offset/align
  assertions), `codec_goldens` (57-byte Hello golden + all 37 message
  round-trips + full failure-mode suite), `cf5_crosscheck` (§7 inventory
  parity, std-free linear uniqueness check), `macro_smoke` (vtable header
  kind/size_of_self for all four macro variants).  Two exec tests added to
  `arch/tests/fixtures_exec.rs` (all-pass exit-0, inject-failure exit-70).
  `uapi-selftest` added to `scripts/coverage-fixtures.sh` fleet so its
  profraw lands in the merge.  Ledger created at
  `Documentation/debt/coverage-786-uapi-foundation.md` (no
  physical-limit claims; genuine 100% MC/DC expected for pure
  computation).  Existing libtest integration tests in `uapi/*/tests/`
  remain as bootstrap-state-1 mirrors.
- Golden-test apparatus for frozen uapi surfaces (#786 Phase 3): ABI layout
  goldens (`uapi/abi/tests/layout_goldens.rs`) asserting `size_of`,
  `align_of`, and `offset_of` for every 6.3/6.2/6.4 catalog type against the
  spec values (mechanism: `core::mem::offset_of!`, stable since Rust 1.77,
  gap-4 pin); codec round-trip goldens (`uapi/protocol/tests/codec_goldens.rs`)
  including the permanent 57-byte Hello frame golden (§"worked frame", SP13)
  and a round-trip + determinism check for all 37 §7 message types, plus a
  full failure-mode suite (truncated frames, bad bool, non-UTF-8 str, over-cap
  encode, body_len over cap, out-of-range Reject.code → Generic, unknown tag
  state-machine transitions); CF5 inventory cross-check
  (`uapi/protocol/tests/cf5_crosscheck.rs`) asserting count=37, every tag
  value, and direction class against the §7 table encoded as a static golden;
  L11 purity probe (`lib/depgraph/src/lib.rs` + `lib/depgraph/tests/uapi_purity.rs`)
  asserting `uapi/protocol` source imports nothing outside `core` and
  `reovim_uapi_abi`, with negative fixtures for `arch`, `std`, and `alloc`
  imports, a cfg(test) exemption control, and a positive control over the
  real source tree.
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
- Spec: three normative spec deltas (#789): PS1 (2.4 §8) restated
  storage-neutrally — atomic-visibility / durability-point /
  forward-recoverability contract; POSIX directory-swap retained as
  reference realization; LF17 (2.2 §3) added for statically registered
  modules — load via boot section-walk, LF3 drain/shutdown applies
  unchanged, physical `dlclose` vacuous, re-load within one process
  lifetime out of contract; DAG6 (1.2 §10) amended with three target
  classes (kernel-ABI: Linux raw syscall, libc FORBIDDEN; system-library:
  macOS/illumos/Windows vendor system library; freestanding: bare metal) —
  Linux floor unchanged in effect.
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
