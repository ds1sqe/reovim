# Coverage Debt — #785 `arch/` Platform Floor

Classified coverage residue for the `arch/` zero-std platform floor
under the 100% MC/DC gate. Classification authority: the physical
measurement limits clause in
`Documentation/10-Development/01-Testing.md` §2 (DEV1).

## Status

99.04% regions; 7 missed lines repo-wide. Every missed line is
individually classified below. No `coverage(off)` annotations exist;
the coverage EXCLUDE list is empty.

## Classified lines

### Terminal horizon (DEV1: terminal syscalls)

- `arch/src/sys/wrap.rs` — `exit_group` entry + the `noreturn`
  syscall call (2 lines): the coverage profile is flushed *before*
  the final exit syscall, so these lines' own counter increments can
  never persist. `exit` (thread exit) IS covered — threads call it
  and the process-global counters are dumped at process exit.

### No reachable caller under the platform contract

- `arch/src/panic.rs` — `rust_eh_personality` body (2 lines):
  invoked only by an unwinder, which `panic = "abort"` (DAG6)
  forbids from existing. No in-process path can reach it.

### Structural guards (build-defect tripwires, not runtime inputs)

- `arch/src/profiler.rs` — `num_data_entries` refuse arm inside
  `write_profile` (1 line): fires only if the LLVM profile
  `DATA_ENTRY_SIZE` drifts from the pinned toolchain value — the
  guard IS the toolchain-version tripwire. Its pure-function arms
  are unit-tested directly.
- `arch/src/profiler.rs` — `fd > i32::MAX` `try_from` arm (1 line):
  the kernel ABI bounds file descriptors to `int`; no fd the kernel
  can return trips it.

### Runtime-only build gap

- `arch/src/panic.rs` — `write!` Err arm inside the panic-message
  render (1 line): reaching it requires an allocation failure
  mid-format during a panic in a `runtime` build; the fixture
  binaries cannot arm the selftest fault seam (it is compiled out of
  runtime builds by design).

## Standing closure mechanisms

The classes that an earlier measurement left open were closed by
selftest-only seams, all compiled out of flight builds:

- **Allocator fault-injection seam** (`arch/src/alloc/fault.rs`,
  `selftest` feature only): `fail_after(n)` countdown on `alloc()`
  entry and a one-shot refill hook that injects `Err` into the
  `mmap` result (never calls `mmap`, no page leak) — these force
  every OOM `?`-propagation Err arm in `seq`/`bytes`/`ring`/`map`/
  `shared` and the allocator refill-refusal arm.
- **Thread teardown seam** (`thread` testhooks): a one-shot
  guard-`mprotect`-failure flag drives the guard-failure
  unmap+`Err` teardown; the three spawn entry points route through
  one `spawn_inner` so the forced branch IS the flight branch.
- **Sync wait counters** (`sync` testhooks): reader `FUTEX_WAIT`,
  writer lost-CAS, and reader CAS-retry arms assertable
  deterministically.
- **No-UB never-return shape**: `exit`/`exit_group` carry
  `syscall1_noreturn` (asm with `options(noreturn)`) — the
  never-returns contract is carried by the asm shape, with no
  `unreachable_unchecked` and no dead spin loop.

## Falsifiability

A line claimed on this ledger that anyone can show is coverable — by
a mid-run invocation, a fixture exec, or a restructure — falls off
the ledger and must be covered. The claims above fail the gate the
moment a covering mechanism is demonstrated.
