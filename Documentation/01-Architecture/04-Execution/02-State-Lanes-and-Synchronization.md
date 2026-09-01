# State Lanes and Synchronization

> **Normative target contract.** This chapter is part of the
> [Architecture contract](../README.md). Present-tense requirements define the
> v0.16 target and do not assert current implementation or qualification.

## Buffer State Lane

Each Buffer has one logical **Buffer State Lane**. It is a linearization and
ownership boundary, not a requirement for one physical thread per Buffer.
Concurrent consumers may compute from immutable stamped state, but every
authoritative Buffer transition serializes through this lane, including:

- Transaction, History, Buffer-version, and Projection-invalidation commit;
- base Snapshot adoption and compatibility, freshness, and conflict state;
- publication or creation selection, terminal result, and Reconcile Status;
  and
- close, reset, and Buffer-incarnation transition.

An alternative atomic state machine is conforming only when it proves the same
single-authority behavior. It may not create a second unordered Buffer writer.

### Local Transaction path

An ordinary local Transaction follows this sequence:

1. Validate envelope, contract/schema, authority, structural bounds, and
   initial stamps before policy traversal or unbounded allocation.
2. Perform Domain and Representation computation and complete lowering outside
   commit ownership. Input is bounded owned state or an immutable stamped lease
   whose storage lifetime and transitive retained-memory charge are independent
   of the lane.
3. Establish every fallible capacity and rollback resource required by the
   selected commit strategy.
4. Enter the Buffer State Lane and acquire its one commit authority.
5. Revalidate Buffer identity, incarnation, version, and every fully lowered
   Transaction Precondition against current state.
6. Apply the cancellation cut. Cancellation before commit admission is
   definitely not applied. After in-place mutation begins, the critical section
   neither suspends nor observes cancellation and must reach a safe terminal
   state.
7. Apply only Buffer-valid Operations, using in-place mutation or bounded
   structural sharing/root replacement, without callback, I/O, wait, or
   suspension.
8. Atomically publish working state, History, the new Buffer version, and
   Projection invalidation, or expose no accepted version.
9. Release commit authority before deferred Projection, Resource, Host,
   Protocol, extension, Event-consumer, or Client work.

If immutable read access cannot remain valid outside the lane, its provider
declares a serialized contention mode and its blocking and measurement
consequences. It may not describe such access as a nonblocking borrow.

### Publication and failure

Asynchronous or streaming publication and creation retain the exact selected
payload through a stable charged state lease or complete bounded reconstruction
witness until terminal completion or reconciliation. A version stamp alone is
not a payload lease.

Allocation, pressure, apply failure, or an unwinding panic must not expose a
partial accepted version. The commit strategy either restores exact pre-state
or fences or poisons the Buffer so partial state cannot be observed as current
until contracted recovery or close completes. No storage or rollback algorithm
is selected here.

Order and precondition defects are detected before commit authority or mutation
where possible. No watchdog, Event emission, supervisor callback, panic hook,
allocation, I/O, wait, or suspension occurs inside the critical section. If a
panic begins during in-place mutation, rollback or fencing completes before the
unwind is observable. Development termination is not a substitute for this
rule. Abort and undefined behavior are never described as contained.

Abrupt termination preserves only the Effect Certainty established at the
applicable cut. It may leave `OutcomeUnknown`; it never implies rollback or
clean shutdown.

## Client State Lane

Each Client Runtime owns one logical **Client State Lane** for generation-
checked installation of Client-local Family Core, Module, View, layout, render,
and frame state. It is not a Kernel type and does not require a particular
thread.

Client computation occurs outside the lane against immutable stamped inputs.
Completion separately enters the lane and revalidates:

- Client, Family Contract, and Module identities and generations;
- View, layout, render, frame, and Rendering Capability generations; and
- consumed Representation, Projection, and CCM state stamps.

The lane installs only bounded Client-local state and exits before
Presentation or Protocol I/O, callbacks, waits, or later deferred work. It
never enters the Buffer State Lane and never treats a Representation or CCM
input as Client editing authority.

Facility pressure and Client State Lane pressure are distinct typed outcomes.
Inline Client work does not pay a facility hop merely because a facility
exists.

The Client ownership and Rendering Capability rules are specified in
[Client, CLM, CCM, and Rendering](../05-Client/README.md).

## Synchronization and reentrancy

Public contracts define ownership, contention, and outcomes without exposing a
private mutex inventory.

- No authoritative Buffer or Client lock, lane token, or commit token is held
  across adapter I/O, user/Domain/extension callbacks, Event fan-out,
  suspension, queue wait, carrier/response wait, or another component's
  lifecycle call.
- Buffer State Lane and Client State Lane entry is non-reentrant. Nested entry
  returns a typed refusal and never waits for itself or silently mutates.
- Every operation declares contention and admission behavior, cancellation
  while waiting, promised ordering and fairness, and hard bounds on queued
  work, attempts, and critical-section work units.
- Cross-Buffer orchestration uses an explicit higher-level ordering and
  atomicity contract. Arbitrary multi-Buffer lock nesting is forbidden.
- No process-global lock or thread-local hidden authority selects components or
  grants editing access.

Hosted scheduling means a hard wall-clock lock-hold maximum cannot be inferred
from code shape alone. Wait and hold thresholds are performance budgets set
from reproducible baselines. Overruns remain visible and may disqualify a
claim.

Implementation evidence inventories every actual lock, logical lane,
condition/wakeup primitive, material atomic-contention point, and bounded work
queue on a claimed path. It records protected state, owner and waiter
identities, declared total or partial acquisition order, affinity,
poisoning/failure, acquisition/refusal, wait and hold distributions, queue
high-water, fairness/starvation, priority inversion, cancellation, and shutdown.

Measurement probes must not acquire the synchronization authority they observe,
invoke user code, perform unbounded allocation or I/O, or introduce a new lock-
order edge.
