# Host Lifecycle, Adapters, and Conformance

[Host API index](../02-Host-API.md)

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
repeated shutdown returns the same terminal state. Every lifecycle phase has
an outer deadline or another declared hard completion bound; a local timeout
does not justify an unbounded Runtime wait.

An immediate observation such as a clock read may justify no per-call
cancellation, but instance ownership, close behavior, and evidence for that
classification remain explicit.

A mutating Host result preserves Effect Certainty separately from Outcome
Cause. A failure whose external effect is unknown is not relabeled transient
or blindly retried. Reconciliation or proved idempotency is required before
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
I/O. A **Presentation Adapter** owns Client-side terminal, window, display,
GPU, and target mechanics behind a Presentation Port. Neither becomes Host
merely because its implementation uses the environment.

Hardware is classified by the role it serves:

- hardware being inspected or edited is a Resource reached through a Resource
  Adapter;
- an environmental service consumed by portable Kernel or Client code uses an
  admitted exact Host Capability;
- Client input/output hardware uses a Presentation Port and Presentation
  Adapter; and
- hardware carrying a remote contract is hidden by a Protocol Adapter.

The same physical device may serve separate roles, but their grants,
contracts, and authorities remain distinct. A hardware driver is never Kernel
Core code. Terminal target setup and restoration belong to Client
Presentation, not a general Kernel Host Capability.

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
  entries name workloads from the
  [Performance contract][performance].

This contract selects the two initial capabilities and their required evidence.
Defining or implementing only their public contract types does not satisfy
`HOST-003`. That claim additionally requires the adapters, shared suites,
lifecycle faults, Events, and measurements named by the applicable rows.

[performance]: ../04-Performance/01-Reaction-Metrics-Baselines-and-Workloads.md
