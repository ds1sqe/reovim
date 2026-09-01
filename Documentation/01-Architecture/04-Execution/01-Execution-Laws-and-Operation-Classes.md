# Execution Laws and Operation Classes

> **Normative target contract.** This chapter is part of the
> [Architecture contract](../README.md). Present-tense requirements define the
> v0.16 target and do not assert current implementation or qualification.

## Scope and authority

This Execution contract family owns:

- the Buffer and Client state linearization boundaries;
- invocation, completion, delivery, and effect classifications;
- the private per-Runtime `ExecutionFacility` contract;
- task, queue, synchronization, reentrancy, and shutdown behavior;
- semantic progress, stall, freeze, deadlock, starvation, and livelock terms;
- production observation and development fail-fast behavior; and
- the causal relation used for reverse debugging.

The Performance contract owns Scenario definitions, datasets, numeric budgets,
statistical methods, instrumentation profiles, and raw Experiment Bundles. The
Editor Event Stream owns bounded operational summaries. Neither surface may
redefine the execution semantics in this Execution contract family.

Assembly selects concrete components, one facility realization for each
Runtime instance, and one liveness policy for each assembled Runtime
incarnation. Assembly does not become the semantic owner, execution owner, or
Lifecycle Owner of the selected components.

No scheduler, async runtime, thread count, lock type, panic mechanism,
watchdog mechanism, numeric latency budget, or third-party dependency is
selected by this Execution contract family.

## Execution laws

All execution paths obey these laws:

1. Logical ownership and observable behavior are public contract; private
   threads, locks, queues, and storage structures are implementation detail.
2. One accepted state transition has one linearization authority. Two
   unordered authorities may not mutate the same state.
3. External observation, I/O, policy computation, user code, callbacks, and
   waiting occur outside authoritative state critical sections.
4. Deferred work remains owned until it has actually stopped or an isolation
   owner has proved its containing process was reaped.
5. A deadline, cancellation request, generation fence, or caller timeout may
   end a caller's wait or prevent installation; none proves execution stopped.
6. Expected invalid, stale, unsupported, pressure, cancellation, timeout,
   closing, and closed conditions are typed outcomes rather than panics.
7. A local edit, mapping, or Projection path must not clone or materialize
   state proportional to the complete Resource, Buffer, Representation, View,
   or resident working set as an intermediate correctness mechanism.

Bounded scalar and carrier copies, reference-count changes, copied touched
regions, charged retained versions, preallocated rollback metadata, and
bounded structural sharing are permitted. An operation whose declared
semantics touch complete state may perform proportional work only under an
explicit workload and budget; it may not be reported as a localized edit.

## Orthogonal operation classifications

Every public callable or operation descriptor declares four independent
execution/effect axes plus explicit callback and reentrancy axes. Names such as
`sync` and `async` do not replace these classifications. Any retained SDK field
named `BlockingClass` describes invocation return only and maps to the
Invocation Class; it does not describe semantic completion.

### Invocation class

| Class | Contract |
| --- | --- |
| `NonBlockingInvocation` | Returns an admission or terminal result after bounded in-process work, with no external I/O or wait. |
| `CallerMayWait` | May wait for owned deferred work only within its declared deadline and cancellation contract and while holding no authoritative state. |

`CallerMayWait` is forbidden on the Buffer State Lane and Client State Lane.

### Completion class

| Class | Contract |
| --- | --- |
| `InlineTerminal` | Semantic terminal completion occurs inside one `NonBlockingInvocation`. The body does not suspend, wait, retain borrowed input, or invoke user code. |
| `DeferredTerminal` | Work is owned after admission and terminates later. The descriptor names request identity, owner, stable inputs, charges, deadline/cancellation, and generation-fenced installation. |

State-lane admission is a separate immediate-or-bounded-queue operation. An
`InlineTerminal` callable may not hide a state-lane wait.

### Delivery class

| Class | Contract |
| --- | --- |
| `Single` | Delivers one bounded terminal result. |
| `Stream` | Delivers a bounded sequence with ordering, cursor and gap behavior, backpressure, cancellation, lifecycle, and resynchronization. |

A stream that depends on selected state retains an owned, charged stable input
lease or a complete bounded reconstruction witness for as long as delivery can
use it.

### Effect class

| Class | Contract |
| --- | --- |
| `LocalOnly` | Pure computation or local state whose result is determined within Reovim. |
| `ExternalObservation` | External read or observation with declared consistency, freshness, cancellation, and completion, and no claimed target mutation. |
| `ExternalMutation` | External mutation with explicit admission and irreversible-handoff cuts, Effect Certainty, and reconciliation where required. |

A destructive read or consume operation is `ExternalMutation`. Method naming
does not weaken effect semantics.

### Callback class

| Class | Contract |
| --- | --- |
| `NoUserCallback` | The classified operation phase invokes no user, Domain-policy, or extension callback. Completion is delivered through its declared bounded result path. |
| `MayInvokeUserCallback` | The descriptor names the callback phase, context/affinity, inputs, work and depth bounds, permitted outward calls, panic/failure behavior, and cancellation. |

No callback runs while an authoritative lock or State Lane is held. A callback
classification does not grant reentrancy or editing authority.

### Reentrancy class

| Class | Contract |
| --- | --- |
| `NonReentrant` | Nested entry into the same declared operation, Session, or authority scope returns a typed refusal or follows an explicitly bounded deferred queue rule. It never waits for itself. |
| `Reentrant` | Nested entry is permitted only within declared depth/work bounds, ordering, state isolation, and failure behavior. It may not re-enter a State Lane or form an undeclared synchronous wait cycle. |

Unclassified callback or reentrancy behavior is not public API.

### Valid combinations

- `InlineTerminal` requires `NonBlockingInvocation` and `Single` delivery.
- `DeferredTerminal` may return a handle through `NonBlockingInvocation`, or
  use `CallerMayWait` within its declared bound.
- `Stream` requires `DeferredTerminal` after bounded admission.
- Any valid completion/delivery combination uses only the Effect Class its
  semantics justify.

Borrowed data does not outlive its declared invocation or cross an execution
context unless its carrier contract proves the lifetime and transfer safe and
charges any owned or shared lease.
