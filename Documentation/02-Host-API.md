# Host API

## Purpose

The **Host API** is a family of small Reovim Kernel Interface contracts through
which a specific portable consumer requests one semantic service from its
process environment. It exposes Reovim-owned capability meaning, values,
limits, and failures. It never exposes raw environment handles, global
platform tables, numeric platform constants, or target-specific branches.

Host is not a universal object, architecture layer, registry, service locator,
or generic environment abstraction. Kernel Core, Kernel Runtime, Resource
Adapters, and Clients receive only the independently negotiated capabilities
that their accepted public paths actually use.

A Host Capability supplies an environmental service. A **Resource Adapter**
connects one external or synthetic Resource and may consume individually
injected Host Capabilities. These responsibilities never merge.

## Ownership and dependency boundary

| Part | Owner | May depend on | Must not own |
| --- | --- | --- | --- |
| Host Capability contract | Kernel Interface | Foundation | environment mechanism, Resource semantics, or concrete configuration |
| Kernel consumer | Kernel Core or Kernel Runtime where justified | only the exact Host Interface contracts it consumes | concrete Host Adapter types or hidden discovery |
| Client consumer | Client Family or Client Runtime where justified | public Client contracts and only the exact Host Interface contracts it consumes | concrete Host Adapter types, a universal Host handle, or Kernel-private state |
| Resource Adapter consumer | concrete adapter outside Kernel | Resource contracts and only the exact Host Capabilities it consumes | a universal Host handle |
| Standard Host Adapter | concrete component outside Kernel | Host contract plus private standard-library or admitted dependencies | Kernel policy or Resource interpretation |
| Deterministic Test Adapter | concrete test component outside Kernel | the same Host contract plus a test-only controller | a weakened production contract |
| Assembly | product composition | selected consumers and adapters | semantic redefinition or lifecycle-state ownership |

Kernel Interface must not depend on a Host implementation. Kernel Core and
Kernel Runtime must not import concrete adapter or environment types. A Host
Adapter must not call private Buffer or Core mechanisms. A Resource Adapter may
depend on an individual Host Capability contract; that edge never permits Host
to depend back on Resource.

Assembly selects and constructs inert Host Adapters and records their ownership
graph. The named Lifecycle Owner for the consuming subtree activates, grants,
supervises, and closes each injected capability instance and its admitted work.
Client Runtime is that owner for a Client subtree; Kernel Runtime is that owner
for a Kernel subtree. No consumer discovers an implementation through a global.

## What Host does not include

The initial Host API contains exactly the two capabilities defined below:
**MonotonicClock** and **DiagnosticOutput**. In particular:

- the Assembly-selected, Runtime-owned private **ExecutionFacility** is not a
  Host Capability, public service, or extension API;
- an Extension Adapter, Extension Engine Adapter, Extension Host Supervisor,
  interpreter, VM, task runtime, worker pool, scheduler, or callback executor
  is not a Host Capability; and
- Protocol and Presentation Adapters remain independently owned even when
  their implementations use environmental services.

Kernel or Client Runtime may use its private ExecutionFacility to realize its
own bounded work, but neither an extension nor a Host consumer receives that
facility as a handle. Extension execution and hosting follow
[Extension Contracts](03-Extensions/README.md).

## Capability admission

A new capability enters the Host Interface family only when all of the
following exist together:

- one named accepted public path, owning boundary, concrete consumer, and exact
  operation;
- contract identity and compatible-version negotiation independent of the
  Reovim package version;
- contract-owned values, typed errors, and hard limits;
- authority validation before lookup, expansion, allocation, or external work;
- separate Effect Certainty and Outcome Cause for mutation, plus explicit
  unsupported, pressure, cancellation, closing, closed, timeout, and failure
  outcomes where applicable;
- ownership, lifecycle, and cancellation behavior for work that may wait,
  queue, or outlive a call;
- a retained operation-specific rationale and evidence for any justified
  immediate/no-waiting lifecycle or cancellation case;
- one Standard Host Adapter and one Deterministic Test Adapter;
- one shared behavioral conformance suite;
- bounded, redacted lifecycle and failure events, or a justified
  non-recursive bounded health record when event emission would require the
  failing capability itself; and
- measurements mapped to exact workload identifiers for latency, allocation,
  memory, queue, and pressure behavior.

Optional Host behavior is negotiated before first use. Unsupported service is
an ordinary typed outcome, never a panic, environment probe, or fallback to a
broader Host object.

The initial two-capability set is not a permanent ceiling. A later capability
requires another explicit owner decision plus this complete admission record;
implementation convenience or historical usage cannot add it implicitly.

## MonotonicClock

**MonotonicClock** supplies monotonic instants for duration measurement,
deadlines, event timestamps, and benchmark boundaries.

- An instant is opaque and meaningful only within the clock instance that
  created it.
- A later observation never precedes an earlier observation from the same
  instance.
- Wall time is not part of the capability and cannot order Transactions,
  events, lifecycle, or work.
- Duration arithmetic returns a typed overflow or invalid-cross-clock outcome.
- The Standard Host Adapter uses the hosted implementation's monotonic clock.
- The Deterministic Test Adapter advances only under test control and requires
  no sleeping.

Wall-clock presentation metadata is a separate future capability only if a
named consumer satisfies the full admission rule. MonotonicClock does not imply
it.

## DiagnosticOutput

**DiagnosticOutput** accepts already-redacted, canonically rendered bytes from
the [Editor Event Stream](05-Editor-Event-Stream/README.md). It owns only the
private mechanics needed to route those bytes to configured diagnostic destinations,
such as a file or standard error.

Its descriptor declares at least contract identity/version, maximum record
size, queue record and charged-byte capacity, invocation behavior, lifecycle
owner, and shutdown policy. One bounded submission returns exactly one of:

- **accepted:** the adapter took ownership within its declared bounds;
- **dropped:** it did not accept the record, with a typed pressure or output
  failure cause; or
- **closed:** its lifecycle no longer accepts records.

Accepted does not imply crash durability. Output failure never changes an
editing result and must not synchronously call back into the Event Stream.
Every drop is visible through non-recursive bounded counters owned by the
stream. Closing stops new admission; shutdown has a bounded drain-or-discard
outcome and is idempotent.

Clock, Event Stream, and DiagnosticOutput bootstrap and failure reporting must
not recursively require the failed capability. A fixed, bounded bootstrap
health state records failures before the Event Stream is ready or while output
is unavailable. Its descriptor names one state owner, schema/version, maximum
record and resident-byte counts, counter widths and saturation behavior,
record-order rule, lifecycle, and one-time import order relative to ordinary
Event Stream admission. If the stream later becomes ready, it imports that
state as provenance-stamped events without erasing or taking ownership of the
counters.

DiagnosticOutput must not assign event sequence or timestamp, redact, render,
retain the Event Ring, or create another logging pipeline.

## Transactional readiness and shutdown

A constructed Host Adapter is inert, not ready. Assembly supplies the selected
instance and ownership graph. The named Lifecycle Owner starts it
provisionally, negotiates and validates every granted capability and bound,
publishes no externally visible grant until all required initialization
succeeds, and then commits one readiness result.

If activation fails, the owner fences the adapter generation, cancels admitted
work, revokes provisional grants, and reverses successfully initialized state
with dependants before the dependencies they consume. Grouping several
capabilities in one implementation is a private composition choice; it never
creates a universal Host contract.

Closing rejects new work before draining. Outstanding operations are
generation-fenced so completion from a replaced or closed adapter cannot
install as current. Owned queued or in-flight work is boundedly cancelled,
drained, or reported discarded. Dependants close before dependencies, and
repeated shutdown returns the same terminal state. Every lifecycle phase has an
outer deadline or another declared hard completion bound; a local timeout does
not justify an unbounded Runtime wait.

An immediate observation such as a clock read may justify no per-call
cancellation, but instance ownership, close behavior, and the evidence for that
classification remain explicit.

A mutating Host result preserves Effect Certainty separately from Outcome
Cause. A failure whose external effect is unknown is not relabeled transient or
blindly retried. Reconciliation or proved idempotency is required before
another mutation can be admitted.

## Consumer-driven growth

Storage, wall time, randomness, process signals, and other environmental
services are not preselected Host architecture. Each can become an independent
Host Interface contract only after a concrete editing-system consumer proves
the semantic need and satisfies the full admission rule.

Raw file descriptors, environment flags, allocator hooks, synchronization
primitives, thread identifiers, boot data, device tables, and panic globals do
not cross Host merely because an older implementation used them.

A **Protocol Adapter** owns carrier, codec, framing, connection, and transport
I/O. A **Presentation Adapter** owns Client-side terminal, window, display, GPU,
and target mechanics behind a Presentation Port. Neither becomes Host merely
because its implementation uses the environment.

Hardware is classified by the role it serves:

- hardware being inspected or edited is a Resource reached through a Resource
  Adapter;
- an environmental service consumed by portable Kernel or Client code uses an
  admitted exact Host Capability;
- Client input/output hardware uses a Presentation Port and Presentation
  Adapter; and
- hardware carrying a remote contract is hidden by a Protocol Adapter.

The same physical device may serve separate roles, but their grants, contracts,
and authorities remain distinct. A hardware driver is never Kernel Core code.
Terminal target setup and restoration belong to Client Presentation, not a
general Kernel Host Capability.

## Adapter substitution and third-party admission

Standard Host Adapters keep environment types, paths, handles, blocking
details, configuration, and error translation private. Deterministic Test
Adapters expose control through a test-only controller, not the production
capability surface.

The deterministic adapter implements the same success and failure semantics,
limits, negotiation, lifecycle, and outcomes as the standard adapter. It adds
deliberate control of time, completion order, pressure, partial progress,
failure, cancellation, replacement, and shutdown wherever the capability can
exhibit them. The shared conformance suite runs against both.

Each selected third-party Host Adapter has a retained admission record naming
its approver, accepted public consumer, contract, dependency, authority, trust,
target, lifecycle, and workload findings. It satisfies the same conformance
obligations as an in-repository adapter; third-party status is never a waiver.

## Conformance evidence

A Host conformance claim requires:

- Standard and Deterministic adapter substitution through normal Assembly;
- dependency-edge, source-import, and public-type checks;
- contract/version/capability/authority negotiation;
- applicable limit, pressure, generation-fencing, and cancellation tests;
- transactional readiness, failure rollback, concurrent close, and
  reverse-order idempotent shutdown;
- fault and outcome-unknown injection;
- redacted Event Stream and non-recursive failure reporting; and
- retained latency, allocation, memory, queue, and pressure measurements whose
  entries name their exact
  [Performance](04-Performance/01-Reaction-Metrics-Baselines-and-Workloads.md)
  workloads.

This contract selects the two initial capabilities and their required evidence.
It does not claim that an implementation or conformance artifact already
exists.
