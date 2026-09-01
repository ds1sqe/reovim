# Extension Packages, Contribution Points, and Registry

[Extension contract index](README.md)

## Extension Packages and module roles

An **Extension Package** may contain a Server Module Definition, zero or more
family-specific Client Module Definitions, shared schemas/assets, or any valid
combination. A package may be server-only, client-only, or paired. Shipping the
TUI, CLI, and Web Client Families does not require every package to implement
all three family modules.

Every package manifest declares at least:

- package and module identities and versions;
- language binding, Server or Client role, and exact Client Family;
- compatible public contract and schema ranges;
- Contribution Points and Activation Predicates;
- required and optional Server/Client pair shape plus CCM schema when used;
- requested capabilities, grants, scope, and authority lifetime;
- payload, record, byte, work, concurrency, queue, and memory bounds;
- trust class, binding form, process placement, execution context, and
  isolation group;
- entrypoint, lifecycle, dependencies, native/FFI policy, and shutdown; and
- documentation, examples, conformance, performance, and qualification
  workload mappings.

A **Server Module Instance** attaches to one explicit server-side scope and may
implement only registry-listed Domain, Representation, command,
synchronization, or other public extension points. It owns no Buffer, History,
Resource Adapter, View, or presentation policy and never mutates storage
directly. A separately admitted Resource Adapter remains its own boundary; a
module cannot absorb its authority merely by sharing a package.

A **family Client Module Instance** binds to exactly one Client identity and
incarnation, Client Family, and Client Runtime subtree. It contributes local
input, mode, View, layout, decoration, output, render, or presentation policy
only through that Family Contract. It consumes stamped common semantic or
module-synchronization state and adapts it locally. It does not duplicate
server authority, import another Family Module, or cause Kernel to know a
viewport or drawing model.

Every Client Family owns a Core, public Family Contract, and Family Module
surface. TUI, CLI, and Web are mandatory v0.16 Client Families; their extension
surfaces and evidence are independent. A Family Module may consume only its
negotiated Rendering Capabilities and never receives terminal, DOM, window,
codec, rasterizer, GPU, device, or platform handles.

Each Client Runtime owns one Client Module Set, which may be empty in a running
instance. A mandatory Family Module surface requires released attachment,
lifecycle, public SDK, example/conformance, failure, performance, and
qualification contracts; it does not require every Client instance to load a
feature module.

Server and Client Modules use separate identities, Contract Sessions, grants,
queues, charges, generations, lifecycle owners, and shutdown. A paired package
uses a versioned Module Synchronization Contract and a named Synchronization
Authority. Auxiliary shared state is schema-qualified and bounded; editing
effects still return through Interaction Intent, Domain/Representation
lowering, and Transaction. Matching names or arbitrary bytes never create a
private Server–Client channel.

An outside-Kernel **Pair Lifecycle Coordinator** may own only the aggregate
pair readiness and outcome. It invokes public attach, close, and rollback
operations and never writes a child owner's state. Required-pair readiness is
atomic across every declared child; optional pairing may publish only its
manifest-declared degraded result.

## Contribution Points and activation

A **Contribution Point** is a closed, versioned public contract family for a
declarative or runtime-backed contribution. Examples include commands,
Domain/Representation providers, Resource Adapter declarations, interaction
mappings, Client decoration/layout/render contributions, diagnostics, and
module/CCM synchronization. Naming an example does not select its release
status.

Every Contribution Point declares identity/version, value and handle schemas,
authority, bounds, invocation, completion, delivery, Effect Certainty,
lifecycle, reentrancy, cancellation, failures, compatibility, and conformance.
It is never an arbitrary callback map, unbounded object, JSON bag, byte bag, or
raw-handle escape hatch.

An **Activation Predicate** names a bounded public condition such as invoking a
command, opening an admitted Domain or Resource family, attaching a Client
Family, negotiating a capability, or explicit user enablement. Discovery and
manifest validation execute no extension code. Activation is lazy only where
the manifest permits it and declares readiness, rollback, deadline, pressure,
and shutdown behavior.

Imperative registration during activation must be a typed subset of the
manifest's declared roles, families, Contribution Points, predicates, grants,
dependencies, and aggregate bounds. Any widening rejects activation and
reverses its provisional state. A package cannot acquire authority by
registering a name at runtime.

## SDK registry, introspection, and API state

One versioned **SDK/support registry** is authoritative for every public
extension point, language SDK, adapter row, package, and Module binding. An
entry names:

- stable identity and contract/schema compatibility range;
- responsible owner and support owner;
- role, Family, language, binding form, process/trust class, and exact package;
- `BindingScope`, `DeliveryGate` when selected, and `AdmissionStatus`;
- stable, experimental, or private API state;
- supported journeys, examples, and conformance commands; and
- exact performance and qualification workload mappings.

Package metadata cannot self-assert registry status. “Approved,” “maintained,”
or “available” is not a support claim outside this registry and its retained
evidence.

Every public API entry is exactly one of:

- **Stable:** a supported registry row with compatibility policy, public docs,
  conformance, deprecation/replacement rules, and release evidence;
- **Experimental:** a separately named opt-in contract/version, unavailable to
  stable packages by default and never represented as supported compatibility;
  or
- **Private:** unavailable to first-party and third-party extension code and
  forbidden from public examples.

Promotion from Experimental to Stable is an evidence-backed admission decision,
not a rename. Stable surfaces evolve additively only where the schema explicitly
marks unknown values as safely ignorable. Incompatible meaning uses a new
contract identity or version. Product, SDK, contract, and package versions
remain independent.

The same registry and schema authority generates API reference, compatibility
tables, completion/type metadata, `describe` introspection, capability and limit
inspection, package/module status, examples, and conformance commands. Public
documentation and runtime introspection must not silently disagree.

Introspection is bounded and read-only. Before resolving existence or
enumerating, the registry validates Contract Session/incarnation, caller-
authority generation, requested scope, and work, record, byte, and deadline
bounds. Results are immutable, redacted snapshots stamped with registry and
caller-authority generations and use bounded pagination. Existence, every
field, aggregate count, error, and omission reason is classified public or
protected; unclassified information is denied. A continuation token binds its
original scope, bounds, registry generation, and authority generation.
Revocation or generation change makes it typed stale or denied. Pagination,
ordering, totals, error shape, and avoidable timing distinctions must not reveal
protected entries.

## Official v0.16 SDK matrix

The official Module binding matrix has three independent axes:

1. **`BindingScope`:** `Selected` permits one exact language × role/Family ×
   binding/placement target; `Excluded` forbids exposing or claiming it; and
   `Unselected` records an unresolved scope decision. Changing Excluded or
   Unselected scope requires another owner decision.
2. **`DeliveryGate`:** `RequiredV0.16` blocks release when absent or failing;
   `TargetNonBlocking` remains a visible target but may remain absent without
   blocking release.
3. **`AdmissionStatus`:** `PlannedMissing`, `Experimental`, `Supported`, or
   exact-environment `Qualified`, with retained pass/fail evidence.

`Qualified` entails `Supported`. A non-blocking row may become Supported after
its own full admission without becoming release-required. If an optional
artifact or support claim is included and fails, it must be fixed or removed;
its evidence cannot be waived.

| SDK language | Server Module | TUI Client Module | CLI Client Module | Web Client Module |
| --- | --- | --- | --- | --- |
| Rust | Selected; `RequiredV0.16`; first | Selected; `RequiredV0.16`; first | Selected; `RequiredV0.16`; first | Excluded |
| Python | Selected; `RequiredV0.16`; later | Selected; `TargetNonBlocking`; later | Selected; `TargetNonBlocking`; later | Excluded |
| TypeScript | Excluded | Excluded | Excluded | Selected; `RequiredV0.16`; later |

On selection, each of the seven rows enters the registry with `PlannedMissing`
AdmissionStatus and may advance only through its own evidence gate. Excluded
cells have no AdmissionStatus. This is an initial-state transition law, not a
live implementation record. The matrix governs public Module authoring only;
it does not select the implementation language of Kernel, Client Family Core/
Runtime, Presentation Adapter, schema generator, or product artifact.

Every SDK and generated package hides or rejects Excluded operations. Generic
helpers must not enable a forbidden role or Family. Every selected row retains
its own registry identity, package mapping, trust/process class, contracts,
grants, journey, example, conformance, workload, and qualification subrow. One
row never proves another.
