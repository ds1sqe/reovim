# Runtime ExecutionFacility

> **Normative target contract.** This chapter is part of the
> [Architecture contract](../README.md). Present-tense requirements define the
> v0.16 target and do not assert current implementation or qualification.

## Runtime-owned private ExecutionFacility

Every Kernel Runtime and Client Runtime instance owns one Assembly-selected
**ExecutionFacility** instance. The facility is a private Runtime port. It is
not:

- a Host Capability;
- a public SDK, module, or service API;
- a global executor, component registry, or editing authority;
- a source of ambient task spawning; or
- the Buffer State Lane or Client State Lane.

The initial Host contract remains limited to its separately admitted
capabilities; adding an ExecutionFacility does not add a task-execution Host
Capability.

### Kernel facility scope

The Kernel facility admits bounded deferred `LocalOnly` computation whose
semantic owner is Kernel Runtime, including admitted Domain/Representation
lowering and Projection work. It does not:

- perform Resource, Host, Protocol, Presentation, CCM, or other external I/O;
- mutate Buffer state or publish an Outcome or Update by itself;
- select components;
- accept private Buffer or storage handles, unchecked `Any`, or unbounded
  payloads; or
- accept direct arbitrary SDK or module submission.

Kernel Runtime remains the semantic completion owner. A result becomes current
only through the appropriate separately owned validation and installation path.

### Facility descriptor

Each facility descriptor names:

- facility identity and incarnation;
- admitted Work Classes;
- global and per-owner, Buffer, Client, Family, View, and module queue,
  in-flight-count, and charged-byte bounds;
- concurrency and worker/poll/drive bounds;
- priority, fairness, starvation, affinity, and transfer rules;
- bounded-to-completion or cooperative step/yield rules;
- cancellation and deadline observation;
- panic, unwind, abort, and worker/facility failure behavior;
- completion delivery, instrumentation, readiness, and shutdown; and
- the concrete realization selected for that instance.

“Bounded” names these dimensions. It does not promise forcible preemption or a
hard wall-clock bound for arbitrary native code.

### Admission and ownership

Admission is one bounded `NonBlockingInvocation`. A request carries:

- unique Work, facility, owner, and task-group identities and incarnations;
- Work Class, priority, charge, and deadline/cancellation authority;
- exact applicable causal, Resource, Buffer, Representation, Projection,
  Client, Family, Module, View, layout, render, frame, Rendering Capability,
  and consumed CCM identities and generations; and
- owned inputs or independently valid charged leases whose carrier contract
  permits their lifetime and transfer.

Admission validates before retaining work. It returns an owned handle or a
typed invalid, stale, unsupported, pressure, closing, or closed outcome.
Rejection admits nothing. No authoritative lock or state-lane token crosses
admission.

Accepted work has exactly one owned execution record. An execution-terminal
state means the work actually stopped or an isolation owner proved its process
was reaped. Valid terminal states include bounded successful completion,
cancellation before start, cooperative cancellation, a caught panic at the
selected boundary, and stopped worker/facility failure.

A caller timeout, deadline observation, cancellation request, or owner fence
may close semantic installation while execution remains owned. Late results
remain fenced and visible. Completion uses a bounded Runtime-owned path and
does not call semantic, module, or user code while facility synchronization is
held. Every unwind reaching a selected catch/join boundary is observed by the
Lifecycle Owner; abort is observable only from an independent supervisor or
process result. Detached or ignored failure is nonconforming.

### Realizations

Each production facility instance selects exactly one recorded realization:

1. **Owned worker.** A bounded Runtime-owned worker set executes only
   bounded-to-completion or cooperatively yielding Work Classes.
2. **Caller driven.** No hidden worker exists. The root Runtime driver supplies
   explicit bounded `drive(work_budget)` opportunities under a liveness
   contract. The drive surface is private, not a module API.
3. **Deterministic test.** A test-only controller selects runnable order,
   yield/resume, completion, pressure, cancellation, panic/failure, and
   shutdown without ambient timing or hidden threads.

These are realizations of one contract, not distinct Host architectures. No
hidden fallback changes realization after readiness. Different Runtime
instances may select different evidenced realizations.

Kernel and Client facility instances do not share authority or a physical
scheduler/backend in v0.16. A future sharing contract requires an explicit
backend Lifecycle Owner, readiness and reference ownership, aggregate and per-
owner charges, isolation/fairness/failure domains, reverse shutdown, and proof
that one Runtime cannot stop or leak another.

### State-lane installation

Facility admission, queueing, execution, cancellation, and completion delivery
occur outside state lanes. Work observes an immutable charged version or lease
and exact stamps, never a live mutable Buffer or Client-state borrow.

Kernel completion presents a bounded candidate, then separately enters the
Buffer State Lane, revalidates every identity, version, and Precondition,
performs only the synchronous non-suspending commit through the
[Buffer State Lane](02-State-Lanes-and-Synchronization.md), and leaves
before later facility, adapter, callback, Event, or Client work.

Client completion follows the
[Client State Lane](02-State-Lanes-and-Synchronization.md). It does not
enter the Buffer lane.

### Modules and extension hosts

A module Lifecycle Owner or Extension Host Supervisor is not an
ExecutionFacility realization. Trusted in-process Rust modules may reach a
facility only through their owning Runtime wrapper and a bounded task group
stamped with module identity/incarnation, grant, admitted Work Classes,
charges, concurrency, and failure policy. A module receives neither the
facility nor private Runtime or Buffer handles.

Stop, reload, or revoke closes task-group admission, advances its fence,
requests cancellation, drains while code remains loaded, and rejects late
installation. Code and state are released only after every execution is
terminal or an isolation owner proves reaping. Otherwise the subtree remains
failed-owned with `IncompleteShutdown`. Facility use is not process
containment.

An out-of-process language Module does not own a language runtime, event loop,
or process mechanics merely by semantic role. For a supervised child, the
outside-Kernel Extension Host Supervisor owns process, engine, code-load,
invocation, event-loop, and reap mechanics. An externally managed process has
one explicitly recorded external owner for those mechanics. Protocol Adapters
separately own carrier mechanics. Kernel and Client facilities do not spawn or
run those Modules, wait on their IPC, own Protocol or CCM queues, or perform
another Runtime's View/render work.

### Failure and shutdown

Only work with a bounded-to-completion proof or cooperative step/yield contract
is admitted. Indefinite, foreign-blocking, cancellation-ignoring, or forcibly
terminated work is rejected or placed behind a separately admitted process-
isolation boundary. No task is deliberately detached.

On an internal facility invariant defect, production stops affected admission,
fences or poisons the smallest owner whose safe continuation is proved, and
fails closed. It escalates to Runtime or process failure when safe continuation
cannot be proved. A production watchdog remains observation-only and is not an
automatic recovery authority.

Shutdown:

1. stops admission;
2. advances owner and task-group fences;
3. closes dependants while the facility can still drive required work;
4. requests cancellation;
5. boundedly drains work and completion delivery; and
6. reports every unresolved item.

The facility publishes `Closed` only after all execution has stopped or been
reaped and owned code/resources can be released. Remaining native work leaves
the facility and Runtime subtree failed-owned in `IncompleteShutdown`; they may
not claim close, unload, detach, or clean shutdown.
