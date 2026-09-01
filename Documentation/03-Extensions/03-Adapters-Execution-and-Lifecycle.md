# Extension Adapters, Execution, and Lifecycle

[Extension contract index](README.md)

## Extension Adapter and Engine Adapter

A language-neutral **Extension Adapter** projects public SDK, Module, and Family
contracts into one authoring toolchain or execution environment and maps
values, calls, callbacks, outcomes, cancellation, lifecycle, and diagnostics
back to the language-neutral authority.

An **Extension Engine Adapter** is an optional specialization hosting one exact
interpreter, VM, managed engine, or equivalent engine instance. It owns only
engine-state construction, module resolution and cache, value/schema
conversion, callback dispatch, engine-local queues/tasks/handles, declared
budget enforcement and observation, diagnostics, and engine shutdown. A
compiled service may use the generated SDK and service boundary without an
Engine Adapter. A direct native Rust path need not traverse either adapter.

The **Extension Adapter Kit** contains only:

- versioned base and Engine Adapter descriptor/manifest schemas with explicit
  conditional applicability;
- generated projection interfaces;
- a deterministic `TestOnly` synthetic host/model adapter;
- semantic/value fixtures and in-memory oracles;
- conformance, lifecycle, fault, and performance harnesses;
- evidence schema, packaging template, and adapter-author documentation.

The Kit contains no production interpreter, VM, compiler, ABI, runtime, or
sandbox. Its `TestOnly` model is outside the product binding matrix and cannot
carry `BindingScope`, `DeliveryGate`, `AdmissionStatus`, production author-
usability, containment, support, or qualification claims.

An adapter claim is keyed by authoring language, SDK projection, binding form,
process placement, execution context, engine technology when applicable,
isolation group, Module role/Family, runtime/ABI/target, support owner, exact
package, and contract version. These axes are independent: an engine technology
does not prove placement or isolation; an out-of-process Python row does not
prove embedded Python; a service binding does not prove a native binding; and
an embedded Lua engine does not prove Neovim compatibility.

Authoring-language and adapter identifiers are open and namespaced for
research-only proposals. The official support matrix remains closed. A new
candidate requires an owner `BindingScope` decision followed by its own
DeliveryGate, AdmissionStatus, dependency review, exact trust/placement row,
and evidence. A newly selected non-Rust product row also inherits the
Rust-first and applicable service-reference predecessor gates unless the
release contract is explicitly revised. Third-party authorship never bypasses
this process.

Comparative engine experiments before scope selection are research-only and
non-shipping. They do not begin product binding or SDK implementation,
admission, or support, and cannot satisfy Supported or Qualified evidence.
Changing a registry status never changes the stable descriptor identity whose
artifact and behavior were evaluated.

## Adapter descriptors, trust, and bounds

Every versioned **ExtensionAdapterDescriptor** declares common:

- adapter, language, SDK, contract/schema, compatibility, package, target, and
  stable registry-row identities;
- roles/Families, Contribution Points, entrypoints, and Activation Predicates;
- binding form, process placement, execution context, isolation group, call
  boundary, lifecycle owners, affinity, and reentrancy;
- value/error/typed-handle mappings and size, depth, work, queue, concurrency,
  output, and memory bounds;
- invocation, completion, delivery, cancellation, and Effect Certainty maps;
- discovery, readiness, callback, replacement, revoke, failure, poison,
  restart, and shutdown semantics;
- authority, code/dependency trust, process-failure domain, containment,
  termination, and fatality claims;
- source location, stack, causal/Event, and redaction behavior; and
- dependency, license, unsafe, exact-artifact, usability, performance, fault,
  and clean-environment evidence mappings.

An **ExtensionEngineAdapterDescriptor** extends the base only when an Engine
Adapter applies. It adds exact engine/runtime/ABI/build identity and technology;
engine-state isolation, module resolution/cache, tasks/handles/timers;
instruction, fuel, checkpoint, stack, heap, bridge, GC, JIT, code-cache, native,
and process-memory bounds or observations; native/FFI loading; exception,
fatality, preemption, poison, and unload behavior; and engine-specific
workloads. Direct-native and ordinary service rows do not fabricate
engine-only values.

Values and handles are schema-qualified, bounded, generation-fenced, and
converted only after validation. No generic unbounded object, `Any`, callback,
JSON bag, byte bag, raw pointer, or raw handle can escape the descriptor.

Every callback has admitted work, queue, payload, output, handle/timer, stack,
code/cache, and memory-charge bounds. A preemption claim does not permit
unbounded work. Fuel or checkpoint claims name units, checking frequency, and
coverage over interpreted, JIT/compiled, GC, host-call, blocking, native, and
FFI paths. Unsupported enforcement produces a typed placement/admission
consequence, not a containment claim. Unavailable memory observation is never
recorded as zero.

Restricted profiles deny ambient native/FFI loading, module search, filesystem,
network, process, and environment access by default. Every enabled native
artifact has an exact digest, ABI/target, dependency and unsafe review, and
process-level trust treatment. Native code inherits no interpreter sandbox,
instruction budget, memory-accounting, or cancellation claim and remains
pinned until process exit unless safe unload is independently proved.

## Execution, ownership, and containment

An Extension Adapter or Engine Adapter is not a Host Capability, Protocol
Adapter, Kernel mechanism, Family Core, semantic owner, or private
ExecutionFacility. Guest work never runs on or receives the Kernel or Client
Runtime's private ExecutionFacility as extension authority.

Every Extension Host Instance has explicit mechanical and semantic lifecycle
owners:

- product-process adapter/engine mechanics are subordinate to the applicable
  Server or Client Module lifecycle owner and are not independently terminable
  by a fictional supervisor;
- for a supervised child, an outside-Kernel Extension Host Supervisor owns
  process/engine/code-load/invocation/reap mechanics;
- Kernel Runtime owns Server Module attachment, semantic dispatch, and result
  installation;
- each Client Runtime owns its Client Module attachment and Client-local state;
- the Synchronization Authority owns CCM membership/state; and
- Protocol Adapter owns framing and carrier I/O.

Process-per-module, process-per-package, shared-language-host, and externally
managed topologies remain unselected. Whichever topology is later selected
must preserve logical instance identities, grants, charges, generations,
isolation groups, and owners.

No owner executes or waits on guest code while holding the Buffer State Lane,
Client State Lane, or another authoritative lock. Bounded validation,
conversion, dispatch, and installation wrappers run outside those lanes and
revalidate applicable stamps before installation.

Same-process code shares the containing process's crash, allocator, memory-
corruption, global-runtime, native-module, OOM, and denial-of-service failure
domain. Separate capability authority does not prove process containment.
Separate-process code may claim only the isolation and termination actually
proved by its process and sandbox composition.

A slow, failed, or pressured extension host must not block Kernel editing,
another Client, or presentation outside an explicitly declared bounded
dependency. Its queues, callbacks, and restart policy have both per-instance
and aggregate bounds with typed pressure behavior.

Cancellation separately records the request, adapter or engine observation,
semantic-installation fence, and actual execution terminal. A deadline or
generation fence prevents late installation but does not prove execution
stopped. Hard termination is claimable only when the process owner proves that
the containing process stopped and was reaped. Thread kill, FFI unwind, or
engine destruction with uncertain effects is not cancellation or rollback.

## Atomic lifecycle and replacement

Activation creates an inert adapter or engine incarnation and provisionally
asks each existing owner to create its Contract Session, Module registration,
required pair, and CCM membership. `Ready` becomes externally visible only
after every required predicate succeeds; no partial contribution is visible.
Failure reverses public child operations with dependants before their
dependencies rather than writing another owner's lifecycle state.

The common lifecycle is:

```text
Discovered -> Validated -> Activating -> Ready -> Quiescing
  -> Closed | Failed | IncompleteShutdown
```

Replacement creates new adapter/engine, Module, Session, membership, callback,
and handle generations and reaches readiness before routing changes. The old
generation closes admission, fences callbacks and handles, requests
cancellation, drains bounded terminal work, reconciles accepted or outcome-
unknown effects, removes contributions and membership, and releases code/state
only after execution is terminal or its process is reaped.

Nonterminal same-process guest or native work becomes failed-owned
`IncompleteShutdown` and may require containing-process termination. State
migration, if separately selected, is a bounded versioned schema operation.
Cache invalidation or source re-execution alone is not reload.
