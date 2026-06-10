# 2.2 — Lifecycle

**Scope.** Boot stages, cdylib load/init/unload state machines, drain
ordering, the rollback rule for partial init, and the
logical-vs-physical unload distinction.

**Heritage.** v3 `02-Process/02-Lifecycle.md`; v4 README §6;
review item "LF7 init state contradicts lifecycle stage 4".

**Locked rules.** `LF1..LF6` carried (restated §10); `LF7..LF16` new.

---

## 1. Boot stages

Boot runs inside `Init` (2.1): stage 0 is `Init::new`; stages 1..6
run inside `Init::boot`; stage 7 begins at the handoff, when
`boot()` moves ownership into `Arc<Kernel>`.

```
stage 0: Init::new  — argv parse, force-override scan, env scan;
                      boot clock + log ring allocated (LOG6)
stage 1: kernel.host config materialised (CFG9)
stage 2: kernel.shell config materialised; runtime caps loaded
stage 3: lockfile + library-root resolution; PM verify
stage 4: module discovery; per-module load (§3..§5)
stage 5: driver discovery; per-driver load (same)
stage 6: framed-protocol runtime / in-memory adapter started
stage 7: handoff — Init::boot returns Arc<Kernel>; serving
```

> **LF13 — Boot is exclusive-ownership; steady state begins at the
> Init→Kernel handoff.** Stages 0..6 run inside `Init` under
> `&mut self` — no locks, no concurrent observers. No hostapi
> call, dispatch, or subscriber can exist before `boot()` returns,
> because `Kernel` has no other constructor; after the handoff,
> `Init` is consumed, so lock-free boot-style mutation is
> unreachable. Boot-only state (`args`, raw config layers) is
> dropped at the handoff. On boot failure `Init` drops whole —
> no partially-constructed kernel state escapes. Boot-time cdylib
> loads are the boot subset of LF1..LF11 operating on the same
> `Inventory` — there is no separate boot loader.
> *Class*: compile (typestate) + runtime.

## 2. Cdylib state machine

```
        [dlopen]                        [drain]                  [physical close
         OK                              done                     allowed?]
Loaded ─────▶ Active ─────▶ Draining ─────▶ Shutting ─────▶ Unloaded ─────▶ <gone>
   │            │              │               │
   │            │              │               └── on cleanup panic ──▶ TombstonedFailedUnload
   │            │              └── on drain timeout ──▶ Active (unload aborted) or TombstonedBusy
   │            └── on shutdown panic ──▶ Panicked (still mapped)
   └── on init failure ──▶ FailedInit (unmapped after rollback)
```

Each cdylib carries a monotonic **generation** counter that
increments on every state transition. `RefGuard` acquisition includes
a generation re-check (§4).

## 3. Load: `dlopen` → `Loaded` → init → `Active`

```
1. dlopen the library
2. read VtableHeader and required cdylib symbols
3. allocate CdylibId; state = Loaded; generation = 1
4. while state == Loaded and no RefGuard exists: call init
5. on init OK:
     publish/reconcile registrations (handler/projector/service/codec/...)
     CAS Loaded -> Active; generation++
6. on init error or panic:
     unregister partial rows owned by this cdylib (LF11)
     destroy any view/service/config resources created during init
     dlclose
     record FailedInit / Panicked in inventory
     state never becomes Active
```

> **LF7 — Init runs only in `Loaded`; `Active` only after success.**
> Init may not run while any prior `RefGuard` exists. Init success is
> a CAS `Loaded → Active`; failure rolls back partial registrations
> before tombstoning. *Class*: runtime.

> **LF11 — Partial init rolls back before tombstoning.** Any rows or
> resources created during init are unregistered and destroyed before
> `dlclose`. *Class*: runtime.

## 4. RefGuard model

`RefGuard` acquisition is atomic with row state and owner generation:

```
1. look up row (handler/projector/service/codec) and capture
   owner_cdylib_id, owner_generation
2. confirm owner_state == Active and row_state == PublicActive
3. increment owner refcount (acquires the RefGuard)
4. re-check owner_generation and row_state — if mismatched, release
   and treat lookup as Draining
5. copy callable pointer; invoke
```

If `Active → Draining` races acquisition, either step 2 or step 4
fails before the function pointer is read.

> **CC15 — RefGuard acquisition is atomic with row generation re-check.**
> *Class*: runtime + tests.

## 5. Unload: drain → hide → shutdown → cleanup → revoke

Registry row lifecycle parallels the cdylib lifecycle:

```
PublicActive ─▶ DrainingHidden ─▶ Destroying ─▶ Revoked
```

Step ordering for unload:

```
1. CAS Active -> Draining; new calls return Draining
2. wait for active RefGuards to drain (bounded; FAIL3)
3. hide public rows (PublicActive -> DrainingHidden)
4. call shutdown(); must drain cdylib-owned threads/tasks/producers
5. run cleanup/destroy callbacks for cdylib-owned resources
   (services, view slots, streams, codecs, capability handles,
   debug subscriptions) while rows are Destroying
6. erase callable/drop metadata (Destroying -> Revoked)
7. physical dlclose only if owner declared dlclose_safe = true and
   shutdown/cleanup proved no callbacks, threads, TLS destructors,
   or foreign runtimes can re-enter; otherwise logical unload —
   library remains mapped until process exit
8. tombstone visible in Inventory
```

> **LF8 — Every callable/drop registry row carries `owner_cdylib_id`.**
> Unload uses the ownership index to find rows before `dlclose`.
> *Class*: runtime.

> **LF9 — Unload step ordering.** §5 above. *Class*: runtime.

> **LF10 — Logical vs physical unload.** Physical `dlclose` requires
> `dlclose_safe` declaration AND verified absence of callback/thread
> re-entry. Otherwise the library stays mapped (logical unload) until
> process exit. *Class*: runtime.

If shutdown cannot prove its tasks are drained within the bounded
wait (FAIL3), unload **fails** and the cdylib returns to `Active`
with `ErrorCode::Busy` (the DS12 `cdylib.unload.fail` event names
the kind). v4 does not silently unmap code while foreign threads
may still call back.

## 6. Row owner index

`Inventory` maintains:

```rust
pub struct Inventory {
    pub cdylibs: HashMap<CdylibId, CdylibEntry>,
    pub rows_by_owner: HashMap<CdylibId, Vec<RegistryRowId>>,
    pub services_by_owner: HashMap<CdylibId, Vec<ServiceKey>>,
    // ... per-row-kind indices
}
```

Row kinds tracked:
- handlers
- projectors
- services
- position codecs
- cursor codecs
- stream scheme registrations
- driver vtable slots
- module entries
- view-state slots / drop functions
- client capability/debug vtables (when client runtime co-resident)

## 7. Drain bounds

Every drain wait is bounded by a configured timeout from
`kernel.host.[limits]`:

| Wait | Limit field | Default |
|---|---|---|
| RefGuard drain on unload | `unload-drain-timeout-ms` | 5000 |
| `shutdown()` body | `shutdown-timeout-ms` | 10000 |
| Stream drain | `stream-drain-timeout-ms` | 3000 |
| Lazy replay | `replay-max-wait-ms` (PM5) | 150 |
| Debug-drive stream completion | `debug-drive-timeout-ms` | 30000 |
| `pkg sync` per-fetch | `pkg-fetch-timeout-ms` | 60000 |
| Detach handler (`OnDetach`) | `detach-timeout-ms` | 1000 |
| Persist handler (`OnPersistSave`) | `persist-timeout-ms` | 3000 |
| Client-notify grace at shutdown | `shutdown-notify-grace-ms` | 2000 |
| Total graceful-shutdown deadline | `shutdown-deadline-ms` | 30000 |

Timeout actions are FAIL3-conformant (status code, event, named
post-timeout state).

## 8. Detach lifecycle

> **LF12 — Detach runs a fixed, non-blocking teardown sequence.**
> For one `(client, buffer, window)` detach event, steps run in
> this order, each bounded per §7:
>
> ```
> 1. OnDetach handler fires        (module sets teardown flags)
> 2. OnPersistSave handler fires   (captures final state, after
>                                   teardown flags are visible)
> 3. subscription cancellations    (DS12 subscribers, debug-drive
>                                   subscribers for this view)
> 4. view-slot drop_fns run        (3.2 §4; AB13 panic-contained)
> 5. buffer handle release         (refcount decrement)
> ```
>
> **Failure policy**: detach is a teardown path — no step's
> failure blocks the remaining steps or the detach itself.
> Failures are recorded, not propagated:
> - step 1/2 failure or timeout → DS12
>   `detach.handler.fail` / `persistence.save.fail`, continue;
> - step 4 panic → AB13 (owner tombstoned-failed-unload), continue
>   with remaining slots.
>
> **Chain order**: with multiple resolved Domains in the focus
> chain, the sequence runs **leaf-first** (deepest Domain
> completes all five steps before its parent starts).
>
> **Buffer-close**: a buffer-close affecting multiple windows runs
> each window's detach sequence first, then buffer-level handlers
> (buffer-scoped slots, buffer Domain detach). Client disconnect
> and session delete iterate their windows/buffers in
> deterministic (id-ascending) order.
>
> *Class*: runtime + detach-trace fixture.

Restore (2.4 §3) is the mirror of this sequence; persistence
restore order was designed against it.

## 9. Shutdown

Shutdown is the mirror of boot with no mirror type: there is no
`Fini`. Steady state ends *inside* `Kernel` — an action, not a
handoff — because there is nothing left to move ownership to.

> **LF14 — Shutdown triggers.** Three sanctioned triggers, all
> converging on the same LF15 sequence:
>
> 1. **OS signal** — first `SIGTERM` or `SIGINT` delivery;
> 2. **composition root** — the embedding binary calls
>    `Kernel::shutdown()` directly (the embedded launcher does this
>    when its client runtime exits, 1.3);
> 3. **debug drive** — `hostapi_debug_drive_shutdown` (7.2 §4),
>    gated on `debug.mutate` and audited like every drive op.
>
> Each trigger emits `shutdown.start` with a `source` field. There
> is no session-protocol shutdown RPC — the 7.3 inventory is
> unchanged; remote shutdown is the debug surface's job.
> *Class*: runtime.

> **LF15 — Graceful shutdown sequence (persist-before-unload).**
>
> ```
> phase 0  shutdown.start emitted; intake closed — new connections
>          refused (protocol UNAVAILABLE), new attaches/RPCs fail
>          ErrorCode::Busy (the DS12 event names the reason);
>          pending lazy-replay queues cancelled
> phase 1  notify — ServerDraining { grace_ms } on every Attach
>          stream (SP8, 7.3); wait ≤ shutdown-notify-grace-ms (§7)
>          for voluntary detach
> phase 2  session teardown — sessions in id-ascending order; every
>          remaining (client, buffer, window) runs the LF12 sequence
>          leaf-first, then buffer-level handlers; session state and
>          MRU written per 2.4 (PS1, PS2)
> phase 3  task drain — connection tasks, then fan-out tasks (CC10)
> phase 4  cdylib unload — LF3 protocol per loaded cdylib, in
>          reverse lockfile order (dependents before dependencies;
>          the DT17 load order, mirrored)
> phase 5  shutdown.ok | shutdown.fail emitted; file sink flushed
>          and closed (LOG7)
> phase 6  release the last Arc<Kernel>; field-order Drop; exit
> ```
>
> The load-bearing edge is **phase 2 before phase 4**: every
> `OnPersistSave` and module persist vtable runs while its owning
> cdylib is still `Active` — persist code is never unloaded before
> it runs. Phase failures follow the LF12 policy: record (DS12)
> and continue; a phase is never silently skipped.
> *Class*: runtime + shutdown-trace fixture.

> **LF16 — Immediate shutdown and exit status.** A second
> `SIGTERM`/`SIGINT` during graceful shutdown, or expiry of the
> total deadline `shutdown-deadline-ms` (§7), escalates to
> immediate: emit `shutdown.forced`, best-effort flush the file
> sink, exit. No further cdylib code runs — no persist handlers,
> no shutdown vtables, no unload protocol; libraries stay mapped
> through process exit (always safe; stronger than LF10's logical
> unload). Torn or missing saves are restore's problem by
> construction: the PS1 commit never happened, so the next boot
> restores the previous complete state.
>
> Exit status: `0` = `shutdown.ok`; `1` = graceful completed with
> recorded failures (`shutdown.fail`); `2` = immediate path
> (`shutdown.forced`). Death by unhandled signal is the OS's
> `128+N`; the spec assigns nothing there.
> *Class*: runtime.

## 10. Carried rules (LF1..LF6)

The v3 rule bodies, restated in v4 vocabulary. These are the
normative texts; the v3 chapter is heritage.

> **LF1 — Every cdylib has an atomic refcount.** Guard acquisition
> (`RefGuard::acquire`, 2.3 §5) fails with
> `AcquireError::NotActive` whenever the cdylib's state is
> anything other than `Active`. *Class*: kernel-enforced (runtime).

> **LF2 — Per-cdylib state machine.**
> `Loaded → Active → Draining → Shutting → Unloaded` (§2),
> CAS-only transitions. The **only** sanctioned backward edge is
> the LF4 drain-timeout revert `Draining → Active` (unload
> aborted); no other state regresses. Failure branches land in
> `FailedInit`, `Panicked`, `TombstonedBusy`, or
> `TombstonedFailedUnload`. The generation counter increments on
> every transition. *Class*: kernel-enforced (runtime).

> **LF3 — Unload protocol.** Acquire the per-cdylib unload-mutex;
> CAS `Active → Draining`; wait bounded
> (`unload-drain-timeout-ms`, §7) for the refcount to reach 0;
> CAS `Draining → Shutting`; call `vtable.shutdown` under
> `catch_unwind` (AB12/AB13); unregister owned rows (LF8/LF9);
> physical `dlclose` only per LF10; CAS `Shutting → Unloaded`.
> *Class*: kernel-enforced (runtime).

> **LF4 — Drain timeout is a recoverable failure.** Default 5 s
> (`unload-drain-timeout-ms`). On timeout: revert
> `Draining → Active`, log the offending guard holders (DS12),
> return `ErrorCode::Busy`. *Class*: kernel-enforced (runtime).

> **LF5 — Tombstones outlive the slot.** After `Unloaded`, the
> `CdylibId` slot is reusable; the tombstone stays visible in the
> DS9 inventory; a new load receives a fresh `CdylibId` and a
> fresh generation. *Class*: kernel-enforced (runtime).

> **LF6 — Concurrent unloads serialize on the per-cdylib
> unload-mutex.** That mutex is a lifecycle mutex held across the
> whole LF3 protocol; it is NOT part of the T1..T13 acquisition
> ladder (2.3 §4). *Class*: kernel-enforced (runtime).

**Reshape note (LF1..LF6).** Carried from v3 with the §2 state
vocabulary and the generation counter added. Two sharpenings, no
intent change: v3 LF2's "never returns to a prior state" and v3
LF4's `Draining → Active` revert were latently contradictory —
LF2 now names the revert as the single sanctioned backward edge.
v3 LF4's `ErrorCode::ModuleBusy` maps to `ErrorCode::Busy` (AB14:
one error vocabulary; the DS12 event names the kind). v3 LF1's
`try_get → Option<RefGuard>` is the v4
`RefGuard::acquire → Result<_, AcquireError>` shape.

## 11. Reshape notes

**Reshape note (LF7).** v3 LF7 said "init runs after `Loaded`".
v4 keeps the number and sharpens the body: init runs only while
`state == Loaded` with no prior RefGuard, and `Active` is the CAS
target on success. Intent unchanged; concurrency precision
tightened to match the RefGuard model (2.3). The v3 wording is
superseded, not contradicted.

## Open items

1. ~~Detach lifecycle ordering and rollback policy~~ — resolved
   (§8, LF12).
2. ~~v3 LF7 vs v4 LF7 numbering~~ — resolved (hybrid renumbering
   policy): number retained, reshape note in §11.
3. Per-row-kind list completeness (§6).
4. Whether `dlclose_safe = true` ever holds for typical Rust
   cdylibs given TLS destructors. If not, all unload is logical
   for v4; physical close becomes process-exit-only.
5. ~~kernel shutdown ordering~~ — resolved (§9, LF14..LF16): full
   phase sequence with persist-before-unload; CC10 (2.3) keeps the
   lock and Drop discipline. Streams drain inside LF unload per
   S8/FAIL3; services per SVC3.

## Conformance

| Rule | Fixture |
|---|---|
| LF1 | Acquire against a `Draining` cdylib → `NotActive`; against `Loaded` → `NotActive` (LF7 carve-out applies to init only). |
| LF2 | State-transition trace over a full load/unload cycle: only forward CAS edges plus at most one `Draining → Active` revert. |
| LF3 | Trace test: unload steps run in protocol order under the unload-mutex. |
| LF4 | Pin a guard past the timeout → unload returns `Busy`; cdylib observable as `Active`; DS12 names the holder. |
| LF5 | Unload then load a new cdylib into the reused slot → fresh `CdylibId`; tombstone still listed in inventory. |
| LF6 | Two concurrent unload requests → one protocol run; second observes the outcome (no interleaving). |
| LF7 | Force init failure; verify `Active` never set; rollback runs; `dlclose`. |
| LF8 | Inventory snapshot mid-load shows owner index covers all created rows. |
| LF9 | Trace test verifies §5 step order. |
| LF10 | Cdylib without `dlclose_safe`: unload runs steps 1-6, library stays mapped. |
| LF11 | Init fails after partial registrations; verify all rolled back before tombstone. |
| LF12 | Detach-trace fixture: two-Domain chain, window close → steps 1..5 leaf-first; panicking `OnPersistSave` → `persistence.save.fail` emitted, detach completes; buffer-close with two windows → window sequences before buffer handlers. |
| LF13 | Compile probe: `Kernel` has no public constructor other than `Init::boot`; compile-fail fixture: using `Init` after `boot()` is rejected (moved value). Boot-failure fixture: `boot()` error → no serving socket, no kernel observable, boot-stage `fail` event in the ring. |
| LF14 | Per-trigger fixtures: `SIGTERM`, embedded `Kernel::shutdown()`, drive op → `shutdown.start` with matching `source`; drive op without `debug.mutate` → `PermissionDenied`. |
| LF15 | Shutdown-trace fixture: phase order holds; every persist handler runs while its owner is `Active` (inventory state in trace); `Attach` after phase 0 → `UNAVAILABLE`; unload order is reverse-lockfile. |
| LF16 | Second signal mid-drain → `shutdown.forced`, exit 2, no cdylib call after escalation in trace; deadline expiry behaves identically; session torn by escalation restores its previous state at next boot (PS1). |
| CC15 | Race fixture: unload begins between row lookup and pointer read; verify guard release without invocation. |
