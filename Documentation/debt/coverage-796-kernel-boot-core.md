# Coverage Ledger — #796 Kernel Boot Core

Scope: `editor/lib/core` (boot stages, DS12 event bus, log ring/renderer/
sink, AB12 seam registration, panic-flush mirror). Flight environment:
the `editor-core-selftest` + `kernel-panic-{halt,recover}` fixture bins on the
arch no_std runner, instrumented via the arch-owned profiler runtime
(`scripts/coverage-fixtures.sh`), merged with `llvm-profdata`.

## Result

- Functions: 100% (58/58)
- Lines: 99.20% (4 missed of 497)
- Regions: 99.04% (7 missed of 730)
- Conditions: the crate contains no compound boolean conditions
  (zero MC/DC condition sites); condition coverage is vacuously complete.

Every missed region is individually classified below. Each entry stands
only while no covering mechanism exists; a reviewer demonstrating one
invalidates the entry (DEV1, 10.1 §2).

## Classified residue

### 1. `init.rs` L442, L449, L452 — seam-registration error continuations (3 regions, 2 lines)

The `?` continuations on the three boot-time `register_seam(...)` calls
(`set_ring_tail_provider`, `set_state_record_hook`, `set_disposition`).
`register_seam` maps arch's `SetError::AlreadySet` to
`BootError::SeamRegistration` in production builds and accepts it as
success under `cfg(feature = "selftest")` — the arch seams are
process-global write-once statics, and the selftest runner boots the
kernel repeatedly in one process, so the second boot would otherwise be
unable to run at all. The production `Err` arm therefore cannot flow in
the only environment that executes editor-core tests: the arm's reachability
is excluded by the same `cfg` that enables measurement.

Covering mechanism that would invalidate this entry: a per-process-reset
seam in arch (rejected for now — write-once is the AB12 safety property
under test) or a production-cfg test runner (bootstrap-state-1 territory).

### 2. `sink.rs` L215–L216 — fd > `i32::MAX` kernel-contract guard (4 regions, 2 lines)

`open_and_subscribe` refuses an `openat` success value that does not fit
`i32` (surfaced as `EBADF` instead of truncating). The Linux kernel ABI
allocates fds densely from 0 and bounds them by `RLIMIT_NOFILE`; a value
above `i32::MAX` cannot be produced by a real kernel. Same physical-limit
class as the arch profiler's fd bound
(`coverage-785-platform-floor.md`).

## Closure mechanisms used (for the record)

- arch alloc-fault seam (`reovim_arch::alloc::fault`, widened to `pub`
  with #796; the kernel is its second consumer): render-OOM arms,
  boot alloc-error arms via fixed-range fault sweeps.
- Capacity-boundary brute force: `Bytes` write-site error arms only
  allocate when the growth boundary lands on the site; sweeps over
  message length x prefix length x fault point reach every site,
  including the escape-sequence write (prefix-determined offset).
- Per-stage failure injection sweep: every stage's `?` continuation in
  `run_boot_stages`.
- Small-capacity entry (`update_with_small_capacity`, selftest-gated):
  the flush mirror's overflow + compaction branches without writing
  1 MiB per test.
