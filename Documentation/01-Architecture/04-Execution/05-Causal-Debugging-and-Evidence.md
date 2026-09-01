# Causal Debugging and Evidence

> **Normative target contract.** This chapter is part of the
> [Architecture contract](../README.md). Present-tense requirements define the
> v0.16 target and do not assert current implementation or qualification.

## Causal relation and reverse debugging

The [Performance contract](../../04-Performance/README.md) exclusively owns Causal
Interaction Trace capture, sampling, completeness, clock-domain treatment,
retention, gaps, and Experiment Bundle evidence. This chapter owns the typed
cause-to-effect relation used by execution and the Causal Debug Graph that
reverse-indexes it. It does not create a second Trace authority.

The causal relation records actual fan-out, joins, coalescing, supersession,
transport branches, state changes, View/render changes, and typed terminal
outcomes from admitted input, command, or external change. Timestamp adjacency
never creates a causal edge. Performance traces and debug graphs may consume
the same relation and share correlation identity, but Event presence does not
prove causal completeness.

### Causal Debug Graph

Each admitted debug scope may maintain a bounded **Causal Debug Graph**: a
read-only reverse index over retained execution-relation nodes, available
Performance-owned Causal Interaction Trace nodes, Editor Event correlations,
and a separately typed synchronization overlay.

Cause-to-effect edges remain acyclic. Reverse traversal follows stored incoming
edges from an Event, Outcome, Progress Point, freeze snapshot, lock wait,
Buffer version or Transaction, Projection generation, CCM Update, IPC message,
module callback, View, render generation, or frame toward known roots. It does
not reverse execution, mutate state, or infer ancestry from clocks or matching
labels.

The synchronization `waits-for` and `held-by` overlay may contain cycles and is
the only graph relation that can prove a lock-cycle deadlock. Correlation,
temporal adjacency, or similar numeric stamps do not establish causality without
a typed edge contract.

### Identity and state relation

Nodes and edges carry exact applicable before/after identities and generations,
including:

- Runtime, Contract Session, authority, request, and root;
- Resource Identity, Access Incarnation, Revision, and Snapshot;
- Buffer identity/incarnation and predecessor/result version;
- Transaction and History transition;
- Representation and Projection predecessor/result generation;
- Protocol Session, stream, message, and send/receive edge;
- Client, Family, Module, View, layout, render, and frame;
- CCM scope, membership, and state;
- Rendering Capability, Presentation Adapter/surface/device incarnation, and
  render fence/completion; and
- synchronization owner, waiter, and acquisition.

Remap, coalesce, supersede, fan-out, join, device/capability loss and recreation,
accepted fallback, and many-root relations use explicit edge types. A missing
render or device observation is `unavailable` or a gap; it is never bridged
across incarnations by assumption.

A node may reference an immutable retained state version/Snapshot and a bounded
redacted debug projection or digest. Retention never implicitly clones or pins
full state. The owner is charged for all transitive resident or pinned bytes,
with an explicit lease lifetime, eviction rule, and release outcome. A state
query returns an exact retained reference or typed `evicted`, `not-captured`,
`redacted`, `incomparable`, or `gap`; it does not reconstruct missing history
from current state.

### Reverse query

A reverse query declares anchors, allowed edge classes and direction, maximum
depth, nodes, edges, bytes, work, and deadline, plus access and redaction
authority. It returns an immutable bounded subgraph, root/frontier identities,
completeness class, and continuation or gap result. Traversal tracks visited
nodes and never follows unbounded fan-out or recursively allocates without a
charge. Query output has no Buffer, Projection, View, CCM, or lock authority.

### Retention and gaps

A versioned Causal Debug Graph retention descriptor bounds pending and terminal
roots, graph nodes, edges, reverse-index entries, state references, bytes,
age/epochs, per-root fan-out, snapshots, queries, transitive retained state,
lease lifetime, and aggregate worker/I/O cost. Performance separately owns
Causal Interaction Trace retention, sampling, completeness, and trace-gap
classification; this descriptor cannot redefine them.

A `DevelopmentFailFast` scope reserves a minimum causal-debug spine for every
admitted pending obligation: root, current Progress Point, exact state stamps,
IPC/CCM handoffs, and synchronization owner/wait relation. This reservation is
not conditional on Performance-trace sampling. Exhausting that capacity
produces typed diagnostic pressure before admission; the composition may not
promise traceability and then silently omit all causes.

Graph eviction, pre-trigger absence, pressure/overflow, unavailable imported
trace nodes, IPC or CCM loss, process death, unsupported probes, redaction,
expiry, snapshot failure, and the termination cut are distinct debug-gap
classes. Gaps retain known counts and identity frontiers and never invent
predecessors. A graph is complete only when every mandatory predecessor chain
reaches its root or declared terminal. Partial graphs remain useful but cannot
prove attribution or a deadlock cycle across a missing relation.

The Editor Event Stream contains only a bounded summary with trigger/proof,
policy/disposition, scope, safe stamps, snapshot/graph reference or
`unavailable`, gap counts/classes, supervisor handoff, and remediation
reference. Full graph and snapshot data never enter the Event ring. Neither
surface is a second logger or state authority.

## Evidence and conformance obligations

Conformance must cover the public behavior in this Execution contract family
without making a claim from a nearby implementation or synthetic-only result.

### Deterministic conformance

The deterministic facility realization, controlled schedule, and manually
advanced MonotonicClock cover:

- reorder and stale completion;
- cancellation at every admission and installation cut;
- reentrant lane entry and lock-order inversion;
- complete and gapped wait cycles, lost wakeup, long hold, starvation,
  livelock, and priority inversion;
- pressure, panic/failure, poisoning, fencing, close/reopen, and bounded
  shutdown;
- Client View, render, and presentation stalls;
- concurrent fail-fast triggers and snapshot-before-termination ordering;
- failed Event, snapshot, and supervisor handoff;
- reverse traversal through Buffer, Projection, Client, rendering, module, CCM,
  and IPC nodes; and
- unwind, FFI, and post-handoff `OutcomeUnknown` cuts.

Tests independently pause schedule while advancing time and pause time while
advancing schedule. They do not use real sleeps to conflate elapsed time with
semantic progress. Partial evidence remains suspected and does not become a
proved deadlock.

### Real hosted evidence

Deterministic models establish contract behavior and rare cuts; they do not
establish hosted wait/hold, contention, freeze, watchdog-latency, or overhead
distributions. Every applicable product, transport, or release claim therefore
uses executed real Scenarios against the actual built or packaged artifact and
records the exact:

- Runtime and facility identities, descriptors, realizations, Work Classes,
  queue/worker/drive configuration, and task-group isolation;
- Buffer and Client lanes, processes, locks, queues, Rendering Capability,
  transport, watchdog, and liveness policy;
- queue delay, run/yield duration, transfers/wakeups, active/peak work and
  workers, charged bytes, pressure, cancellation, panic/failure, stale
  completion, and shutdown;
- synchronization owner/waiter/order, acquisition/refusal, wait/hold
  distributions, queue high-water, fairness, starvation, and priority
  inversion; and
- functional outcomes and all failure rates.

Large-Resource evidence proves localized edits do not scale copying or
allocation with complete state.

### Measurement overhead

Every trace, lock, watchdog, snapshot, and causal-debug profile has a prior
bounded cost/capacity estimate and exact-artifact paired measurements with
detailed capture off, the claim-bearing profile on, every other admitted
profile, and the maximum admitted profile. The minimum endpoint-timing control
remains active and is calibrated.

Raw results remain side by side. No result silently subtracts overhead to
construct an imagined zero-cost latency. Capture is non-recursive, bounded,
loss-visible, and charged through an independent path. Probe behavior may not
change lock order or wait on the component it diagnoses.

### Admission consequences

Supported production uses `ObserveOnly`. A Proved Lock-Order Violation, Proved
Deadlock, disallowed Freeze Interval, lost mandatory causal spine, or unobserved
task panic fails applicable admission or qualification. A development fail-fast
result is diagnostic evidence, not correction evidence.

No deterministic model, mock, plan, estimate, separately instrumented build,
or favorable selected run substitutes for a correct real Scenario or the exact
packaged artifact required by a release claim.
