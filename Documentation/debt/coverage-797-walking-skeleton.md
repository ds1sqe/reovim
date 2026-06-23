# Coverage Ledger — #797 Walking Skeleton

Scope: `server/lib/server` (framed carrier, listener, connection state, notify push,
error types), `server/lib/subsys/domain` (Domain contract tier, Projection
encode/decode, ids), `ext/server/domain/text` (text handler + projector),
`ext/client/platforms/tui` (UDS carrier, ANSI frame composer, input classifier,
paint loop), and the composed `apps/reovim` launcher binary. Flight environment:
the `server-selftest` + `apps/reovim` DEV2 E2E fixture under coverage
instrumentation via `scripts/coverage-fixtures.sh`, merged with `llvm-profdata`.

## Result

Walking-skeleton crates (server-rt, subsys-domain, domain-text, platform-tui,
apps/reovim composed binary), all fleets + the DEV2 E2E run merged:

- Lines: 96.44% (49 missed of 1,376)
- Regions: 93.09% (147 missed of 2,127)
- Functions: 90.91% by llvm-cov's multi-object view — inflated downward by a
  measurement artifact: each fixture binary carries its own monomorphized
  copy of generic/closure functions under a distinct crate hash, and
  llvm-cov counts a copy uncovered in one binary even when another binary
  covered the same source (the line column merges correctly; CI's
  lcov-based merge is line-keyed and unaffected).
- Conditions: the slice contains no compound boolean conditions (zero
  MC/DC condition sites); condition coverage is vacuously complete.

The missed-line residue concentrates in two files: `paint.rs` (15 lines —
PTY-gated raw-mode/restore branches and paint-loop exit arms, §below) and
`apps/reovim/src/main.rs` (16 lines — composition-root bin without a
selftest lane, §below). The remaining ~18 lines are the classified
sub-line error arms below.

## Classified residue

### 1. `init.rs` L444, L451 — seam-registration `?` continuations (inherited from #796)

Inherited from coverage-796-kernel-boot-core.md §1 (same class; that ledger
entry covers L442, L449, L452 for the editor core's own boot seams). The `?`
continuations on seam-registration calls in editor-core `init.rs` cannot be reached
in the selftest runner because the arch seams are process-global write-once
statics; the selftest binary boots the editor core multiple times per process, so
`register_seam` silently accepts a second registration under
`cfg(feature = "selftest")`. The `Err` continuation is excluded by the same
`cfg` that enables coverage measurement.

Covering mechanism that would invalidate this entry: a per-process-reset seam
in arch (rejected — write-once is the AB12 safety property; see #796 §1).

### 2. `sink.rs` L232–L233 — `emit_sink_fail` branch on `bus.subscribe` error (inherited from #796)

Inherited from coverage-796-kernel-boot-core.md §2. The `sink.rs`
`open_and_subscribe` path carries a `bus.subscribe` failure arm that requires
an `EventBus` at subscription capacity. The kernel boot path does not exhaust
bus capacity in the selftest environment. The same physical-limit shape as the
arch fd-bound guard: the environment required to trigger it cannot be produced
within the selftest binary.

### 3. `apps/reovim/src/main.rs` — `#[no_main]` binary, no test lane

Lines 57, 68, 75–76, 84–85, 103–104, 120 (error returns and the
`DEFAULT_SOCKET` fallback, `write_stderr`):

The `apps/reovim` composition root uses `#![no_main]` with `reovim_arch::entry!`.
There is no test lane for `#[no_main]` binary crates in the arch selftest
framework (the selftest fixture pattern applies only to `lib` and `rlib`
crates; `entry!`-based bins cannot embed selftest registration). The error
returns and the `DEFAULT_SOCKET` branch are unreachable via the E2E fixture
because: (a) editor-core boot in the fixture never fails (no fault injection at
the bin level), (b) the `argc < 2` branch uses the default path in the E2E
run (which supplies a socket path), and (c) `start_listener` succeeds.

The `write_stderr` helper (lines 125–133) is only called from the error paths
above; its inner `Ok(0) | Err(_) => break` arm is dead for the same reason.

Covering mechanism that would invalidate this entry: fork/exec-based E2E
fixture that can inject failures into the composition root binary (requires
`execve` in the arch floor; tracked under #774 and #753).

### 4. `notify.rs` L72 — oversized-projection cap guard (SP10)

`push_projection` guards `body_len > WIRE_MAX_FRAME_BYTES` (4 MiB + 1). In the
walking-skeleton E2E the projection body is a handful of bytes (a short text
buffer). Reaching the cap in a no_std test would require allocating a
`Projection` carrying ~4 MiB of content — impractical in the selftest
environment (no heap backing that kind of synthetic payload, and the arch alloc
fault seam cannot make an allocation *succeed* at that size).

Covering mechanism that would invalidate this entry: a test that builds a
`Projection` with 4 MiB + 1 bytes of buffer content. This requires the arch
allocator to service a ~4 MiB alloc in the selftest runner — acceptable if
the memory budget is raised; deferred pending a capacity-test harness at the
notify layer.

### 5. `paint.rs` `restore_terminal_on_panic` (L97–L118) — PTY-gated panic handler

`restore_terminal_on_panic` is registered as a panic hook via
`arch::set_disposition`. It fires only when the TUI process panics. The
coverage E2E fixture does not panic, so the hook body is never reached.
The body itself (`TCGETS` ioctl → set `ICANON | ECHO` → `TCSETS` ioctl)
requires a real PTY (stdin must be a terminal device). The selftest runner
does not attach a PTY; `ioctl(STDIN_FD, TCGETS, ...)` would fail with `ENOTTY`
in that environment regardless, so even an injected panic would not exercise
the `TCSETS` branch.

This is the same physical-measurement-limit shape as the arch profiler's
no-PTY guard (`coverage-785-platform-floor.md`). The function is a live
safety net in production but structurally unreachable in the test environment
by design.

### 6. `paint.rs` L127, L205, L210 — run-loop exit arms

- `Ok(0) | Err(_) => break` at L127: the `write_all_fd` inner loop exits on a
  zero or error write. The coverage E2E terminates via a clean `Ctrl-C` or
  `Quit` command that signals the paint loop via a shared atomic; the write fd
  does not error or return 0 in that flow.
- `return Ok(())` at L205, L210: the `run_loop` inner match arms for
  `recv_notify_frame` returning `Err(CarrierError::Io)` and for a disconnect
  sentinel. The clean quit path exits via the `Ctrl-C` branch before these
  arms are reached.

These are structural loop-exit residues (the clean termination path dominates
the coverage run). Covering mechanism: a dedicated paint-loop error-path test
using a closed fd for the write side or a fake server that drops the connection
after the attach; deferred because the paint loop has no in-process test lane
(it blocks on `read` and requires a thread).

### 7. `paint.rs` L166, L168 — `PanicGuard` acquire arms

`PanicGuard::new()` returns `Ok(guard)` (L166) or `Err(_) => None` (L168).
The `Err` arm fires when `set_disposition` returns `AlreadySet`, which
requires a second TUI run in the same process — the selftest E2E fixture runs
one TUI session per process. Physical-limit: the `Ok` arm is the only live
path in the E2E run; the `Err` arm cannot be reached without multi-session
in-process TUI recycling.

### 8. `ext/client/platforms/tui/src/carrier.rs` L247–L249 — `is_disconnect` function body

`is_disconnect` returns `e == reovim_arch::net::EBADF`. This is covered by
`carrier_tests.rs` unit tests (`carrier_is_disconnect_true_for_ebadf` and
`carrier_is_disconnect_false_for_other_error`). If the coverage report still
marks it uncovered, verify that the TUI-selftest binary is included in the
`llvm-profdata merge` step.

### 9. Structural dead code — `push_decimal(n=0)` in `frame.rs`

`push_decimal` has an early-return for `n == 0`. In `compose_ansi_frame` the
cursor column is computed as `cursor_byte.saturating_add(1)`, which is always
≥ 1 for any `usize` value. The `n = 0` branch is therefore structurally
unreachable from the only caller. Verified in `frame_tests.rs`
(`frame_compose_cursor_byte_zero_gives_col_one`). No restructuring is required:
the zero guard is a legitimate defensive check in the private helper that
happens to be unreachable from the current call site.

### 10. Structural dead code — `delete_at(buf, pos)` where `pos >= slice.len()`

`delete_at` returns `buf` unchanged when `pos >= slice.len()`. The only caller
is `TextHandler::on_raw_input`, which calls `delete_at(buf, cur - 1)` under
the guard `cur > 0`. Because `cur` is invariantly `≤ current.len()`, `cur - 1`
is always `< current.len()`. The `pos >= slice.len()` branch is structurally
unreachable from the current handler. Documented in `handler_tests.rs`.

## Closure mechanisms used

### 1. Alloc-fault sweep (`reovim_arch::alloc::fault`)

Used to cover `try_extend_from_slice` / `try_push` error returns that are
unreachable via happy-path tests alone. Mechanism: `fail_after(k)` injects
an OOM on the k-th allocation; a loop increments `k` until the target
operation succeeds, asserting the expected fallback at each OOM step. Applied
to:

- `TextHandler::on_raw_input` → `insert_at` alloc-error return
  (`handler_insert_alloc_fault_returns_original` in `handler_tests.rs`)
- `TextHandler::on_raw_input` → `delete_at` alloc-error return
  (`handler_delete_alloc_fault_returns_original` in `handler_tests.rs`)
- `compose_ansi_frame` → every `try_push` site in `push_bytes` /
  `push_decimal` (`frame_compose_alloc_fault_sweep` in `frame_tests.rs`)

### 2. Crafted fake-server over in-process UDS

Used to cover protocol-error paths in `carrier.rs` (both TUI and server-rt)
that require a peer that sends wrong responses. Each test binds a
TID-unique `/tmp` socket via `reovim_arch::testrt::unique_path`, spawns a
server thread via `reovim_arch::thread::spawn`, writes crafted `FrameHeader`
+ body bytes, and asserts the expected `CarrierError` or `RuntimeError` on
the client / server side. Applied to:

- TUI `carrier.rs` lines 155, 159, 176, 203 (`carrier_tests.rs`:
  `wrong_reply_to_hello_returns_protocol_error`,
  `incompatible_hello_ack_major_returns_protocol_error`,
  `wrong_reply_to_attach_returns_protocol_error`,
  `wrong_notify_tag_returns_protocol_error`)
- Server-rt `carrier.rs` lines 61, 86, 129–132, 170–171, 202–203, 239–240,
  266, 297–300 (`runtime_smoke.rs`:
  `non_hello_first_frame_causes_reject_and_close`,
  `incompatible_protocol_major_causes_reject`,
  `send_input_before_attach_causes_protocol_violation`,
  `second_attach_causes_conflict_reject`,
  `oversized_body_len_causes_resource_exhausted_close`,
  `truncated_frame_body_causes_close`,
  `send_reject_detail_is_non_empty`)

### 3. `BytesWriter` + `core::fmt::write` for `Display` coverage

Used to cover `impl fmt::Display for RuntimeError` (error.rs lines 35–43).
`BytesWriter<'_>` is the arch-provided `fmt::Write` adapter for `Bytes`;
`core::fmt::write` drives the formatter. Each variant is exercised by a
dedicated `arch_test!` in `error_tests.rs`.

### 4. Guard-failure exercises for `_ => {}` arms

Used to cover the default arms in `TextHandler::on_raw_input` match
expressions. Inputs that route to `_ => {}`:

- Unknown escape direction (`\x1b[Z`) → falls through escape inner match
  `_ => {}` (line 65) and then outer `_ => {}` (line 95) because `0x1b`
  is not in `0x20..=0x7e`
- NUL (`\x00`) and Ctrl-A (`\x01`) → outer `_ => {}` (line 95)
- Ctrl-F (`\x06`) with cursor at end of buffer → guard
  `0x06 if cur < current.len()` fails → `_ => {}` (line 95)

All three patterns covered in `handler_tests.rs`.

### 5. `Default::default()` explicit call to cover derived `Default` bodies

`impl Default for ConnState` and `impl Default for DomainRouter` delegate to
`Self::new()`. LLVM may inline the `Default` body at compile time without
emitting an instrumented call site. Explicit calls to `Default::default()`
at runtime force the body to receive a coverage arc. Applied to:

- `ConnState::default()` in `conn_tests.rs`
  (`conn_state_default_starts_in_handshake`)
- `DomainRouter::default()` in `router_tests.rs`
  (`domain_router_default_equals_new`)

### 6. Runtime-value call to cover `const fn` bodies

`SessionId::as_u32` and `DomainAttachmentId::as_u32` are `const fn`. LLVM
const-propagates them when the argument is a literal, leaving no instrumented
arc in the binary. Constructing the ID from a runtime value
(`reovim_arch::sys::gettid().unsigned_abs() % 65536`) prevents
constant-folding and forces the body to execute under the profiler. Applied in
`session_tests.rs` (`session_id_as_u32_runtime_call`,
`domain_attachment_id_as_u32_runtime_call`).
