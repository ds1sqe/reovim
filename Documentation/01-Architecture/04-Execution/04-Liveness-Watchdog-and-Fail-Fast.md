# Liveness, Watchdog, and Fail-Fast

> **Normative target contract.** This chapter is part of the
> [Architecture contract](../README.md). Present-tense requirements define the
> v0.16 target and do not assert current implementation or qualification.

## Semantic progress and liveness classifications

Every execution implementation and applicable Scenario declares semantic
**Progress Points** for its pending obligations. Examples include admission,
Transaction terminal, version publication, Projection/Update terminal, Client
application, frame submission, presentation handoff or acknowledgment,
cancellation terminal, and bounded shutdown phase. A heartbeat or active worker
is progress only when the contract names the obligation it advances.

The following classifications remain distinct:

| Classification | Required evidence |
| --- | --- |
| **Proved Lock-Order Violation** | An observed acquisition attempt or hold nesting contradicts the declared total or partial order. It is proved for that observation even without deadlock. |
| **Observed Progress Stall** | A named pending obligation passes its threshold without a declared Progress Point. The interval ends at recovery, terminal outcome, close, or current observation. |
| **Freeze Interval** | A required Client presentation or other named interaction terminal, allowed no-visible-change, rejection, cancellation, intentional suspension, or disconnect outcome fails to occur by its threshold. |
| **Proved Deadlock** | A complete wait-for cycle or other contract-specific impossibility proof establishes that no allowed completion, timeout, cancellation, runnable owner, or external terminal can advance the scoped obligations. |
| **Starvation** | Eligible work is repeatedly denied progress while other work advances. |
| **Livelock** | Activity continues without the declared semantic Progress Point. |

A threshold crossing proves only its scoped stall or freeze interval. It does
not prove cause or deadlock. Long lock hold, scheduler delay, external wait,
lost wakeup, starvation, priority inversion, or UI freeze may support diagnosis
but do not prove deadlock. A UI freeze is a Client symptom and does not prove a
Kernel deadlock.

Every threshold declares scope, pending condition, monotonic clock, start and
stop points, pause/exclusion rules, and uncertainty. Numeric values follow
reproducible baselines and are not hard real-time guarantees by implication.

## Progress Watchdog and diagnostic snapshot

Every claimed production detection scope has a bounded **Progress Watchdog**
whose observation opportunity and monotonic-clock path are independent of the
suspected lock or lane, facility queue/drive path, stalled owner, IPC request,
and DiagnosticOutput. The watchdog reads published Progress Points and bounded
synchronization summaries without acquiring contested authority.

Its descriptor declares Lifecycle Owner, observation opportunity, clock,
scheduling/liveness assumptions, thresholds, and exact detection scope. This
contract selects no thread, process, timer, signal, runtime, or Host Capability.
If an independent path cannot run during a stall, detection is `unavailable` or
explicitly narrower and cannot support a broader claim.

On threshold crossing, the watchdog attempts one bounded read-only **Progress
Diagnostic Snapshot**. Where observable, it contains:

- scope, classification, proof level, and stall/freeze interval;
- the last applicable Progress Point;
- causal, request, Session, Buffer, Projection, Client, View, and frame
  identities;
- task, worker, lock-owner, waiter, wait-for, and queue summaries;
- state generations and bounded recent transitions; and
- explicit unavailable, racy, redacted, truncated, incomparable, failure, and
  loss fields.

Capture uses non-waiting or try-read paths, performs no unbounded task/stack
enumeration, and records snapshot failure. Count, bytes, work, attempts,
frequency, retention, redaction, and shutdown are bounded. Saturating health
counters remain queryable when snapshot or Event admission fails.

In supported production and exact-artifact qualification, the watchdog is
observation-only. It does not unlock, kill, retry, restart, rewrite state, or
declare Effect Certainty. Recovery requires a separately admitted lifecycle,
state, and effect contract.

The Editor Event Stream receives only bounded, rate-limited, actionable
summaries. Full snapshots remain in a separate bounded, access-controlled
evidence store and never enter the Event ring or form another normal logger.

## LivenessViolationPolicy

Each assembled Runtime incarnation fixes one immutable
**LivenessViolationPolicy** before readiness:

| Policy | Applicability and behavior |
| --- | --- |
| `ObserveOnly` | Mandatory for supported production and the exact packaged candidate under qualification. The product observes and reports; an external qualification runner may fail the Scenario. |
| `DevelopmentFailFast` | Permitted for an explicitly identified development or test composition with a named independent fatal supervisor. It performs bounded capture before requesting fatal disposition. |

The policy descriptor records artifact/profile identity, policy, panic/unwind/
abort profile, supervisor scope, snapshot handoff, and thresholds. Policy is not
inferred only from compiler assertions, optimization level, an environment
variable read by Kernel code, or an untrusted Client/module request.

The standard development composition uses `DevelopmentFailFast`. Supported
production uses `ObserveOnly`.

### Development fatal triggers

Under `DevelopmentFailFast`:

- a Proved Lock-Order Violation is rejected before the acquisition where
  observable and is fatal;
- a Proved Deadlock is fatal; and
- a Freeze Interval is fatal only for a descriptor-named scope with a declared
  obligation, threshold, pause/exclusion rules, monotonic clock, uncertainty,
  and independent confirmation or grace rule.

A fatal freeze remains `FreezeThresholdExceeded`; it is not relabeled deadlock
without a complete deadlock proof. Suspected cycles, long holds, debugger or
host pauses, scheduler delay, external waits, lost wakeups, starvation, and
livelock are not proved-deadlock triggers, although they may satisfy an
independently declared freeze policy.

Expected invalid input, stale state, unsupported work, pressure, cancellation,
timeout, and closure remain typed outcomes. Duplicate terminal completion,
ownerless admitted work, lost charge ownership, facility work mutating state,
illegal lane re-entry, and other proved internal invariant violations use the
same development defect policy.

### One-shot capture and fatal disposition

Development escalation is one bounded state machine:

```text
Armed -> Capturing -> (Sealed | SealFailed) -> TerminationRequested
```

The first trigger wins. Concurrent triggers append only within the descriptor
bound or increment loss-visible counters. The independent observer:

1. freezes the typed trigger, proof, scope, and Effect Certainty known at the
   cut;
2. captures the Progress Diagnostic Snapshot without using the suspected path;
3. attaches a bounded reverse causal-debug subgraph and all typed gaps;
4. attempts bounded handoff of diagnostic evidence and any crash-survivable
   reconciliation witness to an independently resumable owner/supervisor,
   recording acknowledgment or `seal_failed`;
5. best-effort admits one correlated actionable Editor Event summary; and
6. requests the configured process, test, or Scenario fatal disposition.

Snapshot, Event, or sink failure cannot postpone termination beyond the
descriptor bound. Event delivery is never a prerequisite. Process-local memory
does not prove that evidence survived the failed process; survival requires
independent acknowledgment.

### Panic and termination

A development implementation may use a panic as its final disposition only
after the escalation state reaches `Sealed` or `SealFailed` and records
`TerminationRequested`. A bare policy-triggered panic or abort may not bypass
capture ordering. A watchdog-task panic alone is insufficient because a
facility may catch it or only that task may stop.

Every unwind reaching a selected catch/join boundary is observed by its
Lifecycle Owner. No unwind crosses an SDK, user callback, native/FFI, or other
non-unwind-safe boundary. Catching unwind may support rollback/fencing and
evidence; it does not itself prove invariants restored or authorize continuation
with uncertain Buffer state.

Abort performs no Drop, flush, rollback, join, or graceful shutdown; none is
claimed. The fatal path never unlocks or forcibly terminates an individual
thread as recovery, waits through the proved cycle, or injects a panic into it.
An independent supervisor may terminate the process after bounded capture.

Fatal termination preserves only externally evidenced Effect Certainty. Before
irreversible external handoff, a mutation admitted to a fail-fast composition
already has a crash-survivable independently resumable reconciliation owner, or
the capture phase transfers that witness and receives acknowledgment. Otherwise
`orphaned_reconciliation` is a defect and the composition cannot claim
containment or qualification. A terminated Runtime is not described as the
remaining live owner. A development crash is diagnosis, not rollback or proof
of correction.
