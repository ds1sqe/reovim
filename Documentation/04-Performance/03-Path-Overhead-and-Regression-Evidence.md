# Path, Overhead, and Regression Evidence

[Performance Contract index](README.md)

## IPC and transport measurement

One language- and transport-neutral Scenario Runner and public semantic oracle
exercise the applicable matrix:

1. public in-process semantic reference where defined;
2. deterministic in-memory service envelopes, ordering, bounds, and faults;
3. production codec/framing over a controlled carrier;
4. every supported production Protocol Adapter with its real carrier,
   security, process topology, queues, reconnect, and shutdown;
5. every required or supported Rust in-process role/family path;
6. every required or supported Python role/family path through its exact
   process/runtime and production transport; and
7. every required or supported TypeScript Web path through its exact browser/
   runtime, Web Family Contract, rendering path, and production transport.

Each selected target has a **Scenario evidence subrow** independent of
AdmissionStatus. It is `PlannedMissing`, `ExecutedPass`, or `ExecutedFail` for
the exact Scenario mapping and is never hidden as not-applicable until an owner
scope decision excludes the binding. A non-blocking row may remain absent
without blocking release; if offered as Supported it passes the same full
admission and evidence gates. Qualified entails Supported.

Transport descriptors identify contract/schema, External Protocol Binding
where applicable, Protocol Adapter, codec/framing/carrier, SDK/runtime, process
placement/topology, security/compression/batching, resource limits, payload and
concurrency distributions, queues/flow control, and lifecycle variants.
Measurements retain admission, translation, encode/frame, sender queue, carrier
handoff, peer receive, decode/validate/dispatch, terminal result or stream
update, SDK delivery, reconnect/resynchronization, and shutdown costs.

Results include latency distributions and failures; throughput; semantic and
carrier bytes; framing amplification; copies/allocations/retained bytes;
serialization, CPU, RSS and I/O; syscalls/wakeups/context switches when
observable; queue delay/depth/saturation; fairness, loss/gaps; cancellation,
disconnect, restart/rebind, reconciliation, and shutdown. In-memory semantics
do not prove production IPC cost; shared wire goldens prove compatibility, not
runtime behavior.

## Instrumentation overhead

Each instrumentation profile has a prior versioned **cost and capacity model**
naming per-observation and aggregate event/root rates; bytes, allocations,
copies, atomics/locks, clock reads and syscalls; queue/ring/segment capacities;
retention, fan-out, workers/tasks, I/O, CPU/GPU probes; drop/overflow behavior;
and steady, burst, worst-case, and maximum-admitted assumptions. The model
records charge owners, observed and derived inputs, assumptions, uncertainty,
and method. An estimate is not measured evidence.

The exact candidate then runs paired instrumentation-off/on profiles under the
same Scenario, artifact, environment, and analysis policy. Safety enforcement
needed by the product remains enabled; only diagnostic instrumentation under
test changes. Raw results are retained without silently subtracting overhead.
If instrumentation changes a JIT tier, GC, scheduler, timing, or callback
behavior, the profiles are distinct effective configurations.

Paired attempts are interleaved, randomized, or counterbalanced according to
the predeclared Experiment Protocol so warm-up, thermal, scheduler, network,
device, and time-order drift cannot be assigned silently to one profile.

The paired matrix includes the minimum endpoint-timing control; each
individually enabled profile; the claim-bearing production combination; and the
maximum admitted combination, each at idle/low, representative, burst, and
declared maximum-admitted rates where meaningful. For a non-disableable safety
probe, the minimum safe profile is the control and the incremental/maximal
settings are compared without pretending a zero-instrumentation product exists.

Instrumentation accounting is bounded, non-recursive, and readable through a
path independent of the subsystem being measured. It never reports its own
accounting through the same observed Event, trace, lock, queue, sink, transport,
renderer, adapter, or watchdog path. Failure of that independent accounting is
visible and invalidates any claim requiring its missing cost or capacity.

For every matrix cell, retained analysis compares the prior cost/capacity model
with observed rates, CPU, memory, queues, I/O, loss, and perturbation, and
explains material error rather than silently revising the estimate. Clock reads,
identity/correlation propagation, probe bodies, serialization, collector
queues, storage, transport/export, and watchdog bookkeeping have separately
isolatable calibration cells before their admitted combined profiles are used.

At minimum, this matrix covers the normal production observer, Event Stream
and each sink class, causal trace and Interaction Trace Manifest collection,
Causal Debug retention/query, lock/liveness probes, IPC/transport probes,
rendering/GPU probes, and every Extension Adapter profile claimed by the
release. It retains saturation, loss, changed scheduling/GC/JIT, and collector
failure outcomes at every rate. Development snapshot, fatal-capture, panic, and
process termination cost is reported separately from production.

## Synchronization, stall, freeze, and deadlock evidence

Synchronization evidence measures acquisition attempt/success, wait and hold
duration, contention and timeout, owner/waiter identities, acquisition class,
queue depth, lane/facility state, blocking reason, cancellation, and terminal
result. It includes the Buffer and Client State Lanes, Runtime facilities,
extension/IPC/CCM/render queues, Event/trace collectors, and applicable process
or device synchronization. Instrumentation cannot acquire the observed lock to
report it.

The [Execution and Concurrency contract](../01-Architecture/04-Execution/04-Liveness-Watchdog-and-Fail-Fast.md)
exclusively defines Progress Point, stall, freeze, Proved Deadlock, starvation,
livelock, their proof levels, and development/production dispositions. A
Performance result uses those exact classifications and records the referenced
execution-contract version; it does not redefine or infer one from elapsed
time or activity.

A long wait, debugger pause, scheduler delay, external wait, lost wakeup,
starvation, or livelock is not measured or reported as deadlock without the
Execution contract's proof. Real hosted Scenarios and deterministic schedules
cover uncontended, contended, order violation, complete and gapped cycle, slow
external dependency, lost wakeup, pressure, starvation, livelock,
debugger/host pause, and shutdown. Production observation and qualification
consequences, and development fatal behavior, remain owned by the Execution and
Mission-Critical Quality contracts.

## Extension Adapter evidence

Every required or supported adapter row owns four controls: adapter absent;
adapter packaged but inactive; adapter/host Ready with no extension and engine
Ready when applicable; and exact extension active. Real Scenarios measure
author setup, discovery, startup/activation, warm calls and conversion,
callback/queue/lock delay, end-to-end effect, fairness/backpressure, memory/GC/
JIT/native cost, cancellation stages, stale completion, replacement, failure/
restart, and shutdown. A TestOnly model or injected loop/bomb/storm/fault may
prove its exact contract transition or calibrate a probe but not production
usability, general containment, support, or qualification.

When an adapter participates in a causal/stage claim, all applicable adapter-
dispatch, engine-entry, guest-callback, SDK-request, callback-terminal, and
fenced-installation nodes and their adapter/engine/module/Session generations
are mandatory. One language, role, family, placement, engine, native policy, or
state-sharing topology never qualifies another.

## Validity and regression decisions

Benchmarks first prove the same functional outcome as the corresponding
behavior test. Runs outside the schema's environment-acceptance predicates or
carrying a declared measurement-error code remain visible and follow the
recorded bounded rerun and aggregation policy; they are not silently discarded
or replaced by a preferred attempt. A regression decision compares only
results whose compatibility keys and allowed differences match, and reports
both absolute and relative change with the declared run-to-run method.

A budget changes only through explicit review of retained evidence. Improving
one median does not excuse a p99, memory, I/O, correctness, recovery,
qualification, or loss regression. Feature breadth never compensates for a
failed reliability or performance gate.

## Conformance evidence

Conformance requires reproducible runners, raw artifacts, schema
validation, endpoint tests, functional-result checks, memory accounting,
correlated Kernel/Client stages, and paired overhead measurements for every
applicable boundary. Release evidence runs the exact artifact on each claimed
supported environment. This contract sets no numeric performance budget and
claims no current result.
