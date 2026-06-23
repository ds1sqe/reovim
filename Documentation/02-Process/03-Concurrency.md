# 2.3 — Concurrency

**Scope.** Per-session turn ordering, state lock, RefGuard model,
HostApi reverse-flow lock discipline, panic isolation across the FFI
boundary.

**Heritage.** review item "Mutex<Session> across cdylib calls".

**Locked rules.** `CC1..CC13` carried (restated §10; CC3 reshaped),
`CC14..CC17` new, `AB12..AB13` referenced.

---

## 1. The two-lock session model

Every `Session` owns:

- **`turn_gate: Arc<TurnGate>`** — serialises user-visible
  dispatch order for the session (FIFO fair-queue; in-repo
  primitive over `arch/` sync primitives).
- **`state: Mutex<SessionState>`** — protects field mutation in
  `Session` (the `arch/`-provided Mutex).

The kernel never holds `state` across a cdylib slot invocation.

### 1.1 arch sync-primitive ordering contract (normative)

The `arch/`-provided futex-backed primitives (`Mutex`, `RwLock`,
`Condvar`) pin this memory-ordering contract so their correctness is
specced, not folklore:

- Lock acquisition is an **`Acquire`** atomic operation on the lock
  word; unlock is a **`Release`** store. Every write made under the
  lock happens-before the next holder's first read.
- A futex **wait** re-checks the lock word after every wake; spurious
  and stolen wakes are absorbed by the retry loop (waiting is a
  correctness-neutral hint, never a permission).
- `FUTEX_WAKE` is issued **after** the `Release` store of the unlock,
  so a woken thread that wins the word observes the holder's writes.
- `Condvar::wait` atomically releases its `Mutex` before sleeping and
  re-acquires under the same `Acquire`/`Release` contract before
  returning; a missed-wake window between release and sleep is closed
  by the futex word re-check.
- `RwLock` readers take `Acquire` on entry and `Release` on exit;
  the writer path provides the same edges as `Mutex`.

## 2. Dispatch shape

```
1. acquire session turn_gate
2. lock Session state
   - snapshot focus chain, view ctx, handler/projector rows
   - clone refs needed for cdylib call
   unlock Session state
3. acquire RefGuard for each cdylib row to be invoked
   - generation re-check (CC15)
4. invoke cdylib slots — kernel holds NO EditorCore/Session lock here
5. collect HandlerOutput / kernel mutation commands
6. release RefGuards
7. lock Session state
   - apply commands in deterministic order
   unlock Session state
8. release session turn_gate
```

> **CC14 — No normal EditorCore/Session lock held across cdylib slot
> invocation.** *Class*: spec-asserted, runtime tests with lock
> instrumentation.

## 3. Reverse flow (HostApi during dispatch)

When a cdylib slot calls back into the kernel through HostApi, it
must obey one of:

- operate only on lock-free / copy-at-boundary surfaces,
- enqueue a command to be applied during the current dispatch's
  apply phase (step 7 of §2),
- acquire locks in the documented order without needing the
  already-released `Session.state` lock.

Every HostApi function legal during handler dispatch is listed in
the type catalog with its lock behaviour.

> **CC16 — HostApi calls during slot dispatch obey documented lock
> order.** *Class*: spec-asserted, runtime instrumentation.

> **CC17 — Focus transition publication runs in the apply phase.**
> Q-4: focus mutation runs under `Session.state` (atomic). The
> transition record is published at the apply phase boundary
> (step 7 of §2). No observer closure executes under
> `Session.state`. Observer fan-out is fire-and-forget; cross-observer
> ordering is not guaranteed (observers that need pairing rely on
> the `before/after` snapshot in the record, not on callback
> sequencing).
> *Class*: runtime.

## 4. EditorCore lock tiers

Lock acquisition order is total. A higher tier may not acquire a
lower tier already held. Tier labels are `T1..T13` — deliberately
distinct from the `CC*` rule IDs so a tier reference is never
mistaken for a rule citation. (The tier *ordering discipline*
itself is the reshape of rules CC1..CC13; reshape note in §9.)

| Tier | Lock | Notes |
|---|---|---|
| T1 | `correlation_alloc` | shortest |
| T2 | `force_overrides` (read-only after boot) | |
| T3 | `inventory.read` | walking cdylib state |
| T4 | `manifest_registry.read` | |
| T5 | `domains` (router) | codec maps |
| T6 | `services` | service descriptors |
| T7 | (reserved — was CoordinationRegistry; removed) | |
| T8 | `handlers` | dispatch lookup |
| T9 | `projectors` | dispatch lookup |
| T10 | `streams` | stream substrate |
| T11 | `event_bus.subscriber_set` | DS12 fanout |
| T12 | `sessions` (sharded `arch/`-provided RwLock map shard) | per-session map |
| T13 | `Session.state` | innermost session state |

`turn_gate` is an async mutex orthogonal to this tier list; it does
not appear in the lock-order graph.

## 5. RefGuard contract

```rust
pub struct RefGuard<'a> {
    cdylib_id:        CdylibId,
    captured_gen:     u32,
    inventory:        &'a Inventory,
}

impl<'a> RefGuard<'a> {
    fn acquire(inv: &'a Inventory, id: CdylibId) -> Result<Self, AcquireError>;
    fn invoke<F, R>(&self, f: F) -> Result<R, InvokeError> where F: FnOnce() -> R;
    fn release(self);
}
```

Acquire steps (CC15 body):

1. lookup `Inventory.cdylibs[id]` — capture state + generation.
2. if state != Active → `AcquireError::NotActive`.
3. atomically increment refcount IFF state still Active and
   generation unchanged.
4. on success, return `RefGuard`.
5. invoke records the invocation context (owner `CdylibId`, slot)
   in the thread's panic-attribution slot before calling, and
   clears it after — this is what the AB12 panic handler reads to
   attribute a panic to its owning cdylib. There is no
   `catch_unwind` under `DAG6` (AB12, 6.2 §5).

Release: decrement refcount; if state == Draining and refcount
reaches 0, signal the unload waiter.

## 6. Panic isolation

> **AB12 — Single panic isolation rule (panic disposition).** Owner
> body in 6.2 §5. Under `DAG6` (1.2 §10) there is no unwinder and
> no `catch_unwind`: a panic in any slot invocation reaches the
> `arch/`-owned panic handler, which renders the panic into the one
> kernel log ring (with owning-cdylib attribution from the active
> RefGuard context, §5) and synchronously flushes the ring tail to
> the LOG7 file (9.5 §9.1), records the lifecycle consequence in
> the persisted state (2.4), emits the DS12
> `dispatch.handler.panic` event, and terminates the process per
> the configured disposition — `recover` (supervised restart +
> persistence restore + first-panic quarantine of the attributed
> cdylib) or `halt` (stop for analysis). No Rust unwind crosses an
> `extern "C"` boundary. *Class*: compile-time (panic profile) +
> runtime (panic handler).

> **AB13 — Cleanup panics are recorded, then disposed.** Owner body
> in 6.2 §5: the AB12 path with `rollback = failed` in the flushed
> log line and target state `TombstonedFailedUnload` in the
> persisted state; a `recover` restart quarantines the owner.
> *Class*: runtime.

`AB5` and `AB6` are deprecated in favour of `AB12`.

## 7. RefGuard timeout

If a slot invocation exceeds the configured slot-timeout, the
kernel:

1. cancels the invocation if cancellable; otherwise lets it run.
2. records a DS12 `dispatch.handler.panic` event with reason
   `slot_timeout` (even though no panic occurred — the event family
   covers slot misbehaviour).
3. counts toward the per-cdylib slot-timeout threshold gate, with
   namespaced limit `slot-timeout-threshold-count` /
   `slot-timeout-threshold-window-ms` from `editor.host.[limits]`.

(Threshold gate retained, re-scoped under `DAG6`: it
counts slot timeouts only — repeated in-process panics cannot
occur under the AB12 disposition model (the first panic terminates
the process and, under `recover`, quarantines the owner), so the
panic half of the old gate is subsumed by first-panic quarantine.
The old AB6 wording that introduced the gate is deprecated; AB12
carries the panic convention, the lifecycle chapter carries the
gate.)

## 8. Send / Sync invariants

Every cdylib-callable function pointer must satisfy:

- callable from any worker thread of the `arch/`-threaded
  thread-per-connection runtime (Send-safe by construction);
- re-entrant safe with respect to its own owner (per `flags` in
  vtable header);
- `hostapi_reentrant` flag declares whether the slot may call
  HostApi during execution (default: yes for handlers, no for
  drop/cleanup).

ServiceDescriptor flags carry the same vocabulary; see
`03-State/03-Service-Registry.md`.

## 9. Reshape notes

(The CC1..CC13 collective note now closes §10, beside the
restated bodies.)

## 10. Carried rules (CC1..CC13)

The rule bodies below are the normative texts.

> **CC1 — `EditorCore` is `Send + Sync`, shared as `Arc<EditorCore>`** across
> the server runtime's worker threads (`arch/`-threaded,
> thread-per-connection). There is no central actor task and no
> EditorCore mailbox. *Class*: kernel-enforced (compile).

> **CC2 — Per-field locks on EditorCore; no global kernel lock.** Each
> lockable EditorCore field is its own tier in the §4 table (T1..T12);
> sessions live in a sharded map (T12) with per-session state
> innermost (T13); shutdown is signalled by a cancellation token,
> not a lock. *Class*: kernel-enforced (compile).

> **CC3 (reshaped) — Per-session dispatch is serialized.** The
> prior single `Mutex<Session>` is superseded by the two-lock model
> (§1): `turn_gate` serialises user-visible dispatch order;
> `state` protects field mutation and is never held across a
> cdylib slot invocation (CC14). *Class*: kernel-enforced
> (compile).

> **CC4 — Every cdylib vtable slot is `Send + Sync`-callable from
> any worker thread of the server runtime** (§8). This is the default
> contract; CC13 is the per-cdylib opt-out. *Class*: spec-asserted.

> **CC5 — Handler dispatch is clone-then-invoke.** DomainRouter
> dispatch reads under a read lock, clones the resolved handler
> list, drops the lock, then invokes — no lock is held across
> cdylib re-entry (generalised by the §2 dispatch shape).
> *Class*: kernel-enforced (compile).

> **CC6 — Subscriber notification is clone-then-invoke.**
> Buffer-edit subscriber lists are cloned to a local Vec under the
> read lock, the lock dropped, then notified — same pattern as
> CC5. *Class*: kernel-enforced (compile).

> **CC7 — Lock-acquisition order is total.** The §4 tier table
> (T1..T13) is the order. Below T13 sit, in order: driver-internal
> locks, the per-cdylib serialization mutex (CC13/PM9, only when
> opted in), and `RefGuard`. A higher position never acquires a
> lower position already held. *Class*: spec-asserted + lock
> instrumentation.

> **CC8 — Observation fan-out is parallel-spawn.** Each
> overlapping observer (DT6) runs on its own spawned task;
> ordering across observers is non-deterministic. Observers that
> need pairing rely on the DT14 snapshot record (CC17), not on
> completion order. *Class*: spec-asserted.

> **CC9 — Cdylibs never receive `Arc<EditorCore>`.** All cdylib→kernel
> traffic goes through `HostApi` (6.1 §6). *Class*:
> kernel-enforced (compile).

> **CC10 — EditorCore shutdown lock and Drop discipline.** The full
> phase sequence, triggers, and escalation are LF14..LF16
> (2.2 §9): cancel intake → notify clients → per-session
> detach/persist (LF12, leaf-first) → drain connection tasks →
> drain fan-out tasks → LF unload per loaded cdylib (reverse
> lockfile order) → release the last `Arc<EditorCore>` → field-order
> Drop. CC10 owns the concurrency half: no T1..T13 lock is held
> across a phase boundary, and Drop runs single-threaded after the
> last `Arc<EditorCore>` releases.
> *Class*: kernel-enforced (runtime).

> **CC11 — Runtime `dlclose` is permitted once the LF unload
> protocol completes**, gated by LF10: physical close requires the
> `dlclose_safe` declaration and verified non-re-entry; otherwise
> the unload is logical and the library stays mapped. *Class*:
> kernel-enforced (runtime).

> **CC12 — RefGuard discipline is Active-only.** Every cdylib
> symbol invocation made while the owner is `Active` holds a
> `RefGuard` for the duration of the call (§5). Acquisition fails
> `NotActive` for `Loaded`, so init is the carve-out (LF7).
> `RefGuard` sits below every tier in the CC7 order. *Class*:
> kernel-enforced (runtime).

> **CC13 — Per-cdylib serialization opt-in.** Manifest
> `[load] requires_serialization = false` (default; PM9 owns the
> manifest side). When `true`, the kernel takes a per-cdylib mutex
> around every slot call. CC4 is the default contract; CC13 is the
> escape hatch for cdylibs whose internal state is not safe under
> concurrent slot calls. Stream schemes MUST NOT opt in (S1).
> *Class*: kernel-enforced (runtime).

**Reshape note (CC1..CC13).** Carried forward with this spec's vocabulary.
CC3 is the one body reshape: the single `Mutex<Session>` became
the two-lock model (§1), which is what makes CC14 satisfiable.
CC2's original list of concrete lock types generalised to the §4 tier
table (the CoordinationRegistry tier is retired — T7 reserved);
CC7's prose ladder became T1..T13 plus the named below-T13 order;
CC11 is now explicitly gated by LF10's logical-vs-physical
distinction; CC12 gained the generation re-check refinement
(CC15). Tier labels were renamed T1..T13 so a tier reference is
never mistaken for a rule citation. Intent unchanged throughout:
the order is total and violations are bugs.

## Open items

1. Async vs sync slot dispatch — current model assumes sync slots
   that may enqueue async work via HostApi. Streaming slots
   (debug-drive, stream substrate) need explicit async contract.
2. ~~Whether `turn_gate` is fair (FIFO) or LIFO~~ — resolved
   (resolved #782 — FIFO fairness is provided by the in-repo
   `TurnGate`'s explicit ticket queue; no third-party runtime
   dependency).
3. Cancellation token propagation into slots — currently no
   contract.

## Conformance

| Rule | Fixture |
|---|---|
| CC1 | Compile probe: `EditorCore: Send + Sync` static assertion; no actor-task spawn in kernel construction. |
| CC5/CC6 | Handler registered during another handler's dispatch → no deadlock (lock not held across invoke). |
| CC7 | Lock instrumentation: acquire `sessions` then `domains` → ordering violation detected in test build. |
| CC8 | Two observers on one edit → both run; completion order varies across runs (non-determinism asserted, pairing via snapshot). |
| CC9 | Compile probe: no `Arc<EditorCore>` (or `EditorCore` reference) type appears in any `extern "C"` signature in `uapi/`. |
| CC10 | Shutdown trace: cancel precedes connection drain precedes fan-out drain precedes per-cdylib unload. |
| CC12 | Slot invocation observed holding a guard; guard count returns to zero after the call. |
| CC13 | Serialized cdylib: two concurrent slot calls run sequentially; non-serialized cdylib: concurrently. |
| CC14 | Lock instrumentation: invoke handler that calls long HostApi op; verify `Session.state` is not held during the slot. |
| CC15 | Race: unload begins between RefGuard generation capture and invoke; verify `AcquireError::NotActive`. |
| CC16 | Handler issues HostApi acquiring `domains`; verify ordering against §4 tiers. |
| AB12 | Panicking handler under `halt`: the panic line (with cdylib attribution) is present in the flushed log file; DS12 event recorded; process stops. Under `recover` (supervision fixture): restart restores persisted state and the attributed cdylib is quarantined. |
| AB13 | Panicking shutdown → flushed log line carries `rollback = failed`; persisted state records `TombstonedFailedUnload`; `recover` restart shows the owner quarantined. |
