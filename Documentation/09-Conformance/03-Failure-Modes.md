# 9.3 — Failure Modes

**Scope.** Cross-cutting failure rules: every fallible boundary has
named errors; rollback failure is first-class; drains are bounded.

**Heritage.** v4 README §15.3.

**Locked rules.** `FAIL1..FAIL3`.

---

## 1. Failure shape

> **FAIL1 — Every fallible boundary operation has:**
> - a named error code (ErrorCode or status),
> - a documented rollback rule,
> - a caller-visible result,
> - a DS12 event when the failure is not already visible through
>   protocol response.
>
> *Class*: spec / runtime.

A "fallible boundary" is any function the kernel exposes through
HostApi, framed-protocol request, or cdylib vtable.

## 2. Rollback as first-class

> **FAIL2 — Rollback failure is first-class.** If rollback fails:
> - the operation returns `ErrorCode::RollbackFailed` (= 25 in
>   the catalog, 6.3 §2.3); one shared code for all resource
>   families — the DS12 `resource_state` field carries the kind,
> - emits a DS12 event containing both the original error and the
>   rollback error,
> - leaves the affected resource in a named degraded or
>   tombstoned state.
>
> *Class*: runtime.

Rollback states:

| Resource | Rollback failed → state |
|---|---|
| Cdylib init | `FailedInit` |
| Cdylib unload | `TombstonedFailedUnload` |
| Service registration | `Conflict` (rollback unwinds) |
| ConfigSlice construction | `ConfigSliceTooLarge` (rollback drops partial slice) |
| Domain attachment | `AttachmentFailed` (struct_refs == 0; cleaned at next sweep) |

## 3. Bounded drains

> **FAIL3 — Drain operations are bounded.** Every wait carries a
> timeout from `kernel.host.[limits]`. Timeout behaviour:
> - returns a distinct timeout status,
> - emits a DS12 event with `outcome = timeout`,
> - transitions the resource to a named post-timeout state.
>
> *Class*: runtime.

Bounded operations and their fields (per 2.2 §7):

| Operation | Limit field | Default ms |
|---|---|---|
| Unload RefGuard drain | `unload-drain-timeout-ms` | 5000 |
| `shutdown()` body | `shutdown-timeout-ms` | 10000 |
| Stream drain | `stream-drain-timeout-ms` | 3000 |
| Lazy replay (PM5) | `replay-max-wait-ms` | 150 |
| Debug-drive stream completion | `debug-drive-timeout-ms` | 30000 |
| `pkg sync` per-fetch | `pkg-fetch-timeout-ms` | 60000 |

Post-timeout states:

| Operation | Post-timeout state |
|---|---|
| Unload drain | resource → `Active` (unload aborted with `Busy`) |
| Shutdown | resource → `TombstonedFailedUnload` |
| Stream drain | stream → `Closed` with `outcome = drain_timeout` |
| Lazy replay | replay queue → cancelled; user notice via PM8 |
| Debug-drive | drive op → cancelled; subscriber notified |
| `pkg sync` fetch | sync → `Failed` with partial-state rollback |

## 4. Forbidden failure modes

- **Silent failure.** Every failed operation emits SOMETHING
  (return code, event, or both).
- **Half-rolled-back state.** A failure that leaves resources in
  a named-elsewhere state must be FAIL2-tombstoned.
- **Unbounded waits.** Any wait without a timeout is a v4 lock
  blocker.

## 5. Required DS12 fields for failure events

| Field | Required when |
|---|---|
| `error_code` | always |
| `rollback` | for any operation with a rollback rule |
| `correlation_id` | external operations (per OBS2) |
| `cdylib_id` | when the failure is per-cdylib |
| `resource_state` | post-failure state of the affected resource |
| `original_error` | when rollback fails (per FAIL2) |

## Open items

1. ~~Per-resource `RollbackFailed` codes~~ — resolved: one shared
   code (`ErrorCode::RollbackFailed = 25`); `resource_state`
   carries the kind (FAIL2 body).
2. Default values for the FAIL3 limit fields above — current
   defaults are guesses; tune at integration time.
3. Whether a `rollback = unnecessary` outcome warrants an event.
   Default: no — silent on success.

## Conformance

| Rule | Fixture |
|---|---|
| FAIL1 | Inject failure at every fallible boundary in turn; verify error + DS12 + caller result. |
| FAIL2 | Force rollback to fail (e.g. panic in cleanup); verify `RollbackFailed` + tombstoned state + event. |
| FAIL3 | Force unload drain timeout; verify resource transitions back to Active per §3 table. |
