# Normative Rule Matrix

## Authority

This matrix routes stable normative obligations to one owning contract and
closed verification classes defined in
[Conformance Requirements and Evidence](01-Requirements-and-Evidence.md).
It contains target obligations, never live implementation status. A component
or release claim passes only when every applicable row has current retained
evidence of the named classes.

One row may own several inseparable clauses in its linked contract section.
Explanatory repetition does not create a second rule. Changing a row's meaning,
authority, applicability, or evidence classes is a normative contract change;
an identifier is never reassigned.

This document is **`reovim-rule-matrix/v1`** and every row uses the closed
**`reovim-evidence/v1`** taxonomy. The following ledger supplies explicit
applicability and one responsible boundary for every existing identifier. A
range applies separately to every named existing ID in that range; adding a
future ID never inherits a range entry implicitly. The Obligation column below
may narrow its ledger applicability but cannot silently broaden it.

## Applicability and responsibility ledger

| IDs | Applies when | Responsible boundary |
| --- | --- | --- |
| PROD-001..002 | Any Reovim product, architecture, or release description is made. | Product Contract Authority |
| PROD-003 | Any v0.16 release candidate or Family-scope claim is evaluated. | Release Qualification Authority |
| PROD-004 | The official Module binding registry, SDK package set, or release claim is evaluated. | SDK/support registry authority |
| AUTH-001 | Any normative change, implementation-conformance claim, or release claim is made. | Conformance Authority |
| ARCH-001..003 | Any source role, public type, build edge, or composed adapter boundary is admitted. | Architecture conformance boundary |
| ARCH-004 | Any Assembly-selected Runtime or Client subtree is constructed or activated. | The subtree's Lifecycle Owner |
| ARCH-005 | Any third-party dependency is proposed, admitted, changed, or invalidated. | The dependency's named consumer owner |
| ARCH-006 | Any implementation or release evidence set is produced. | Conformance Authority |
| SAFE-001 | Any opaque carrier crosses a contract, ownership, thread, process, async, or unsafe boundary. | Foundation Carrier Safety boundary |
| BOUND-001 | Any capability or operation claims a finite bound. | The descriptor-owning contract boundary |
| ISOLATE-001 | Any payload, work, state, queue, concurrency, retry, wait, or aggregate scope is admitted or retained. | The owning admission and isolation boundary |
| EFFECT-001 | Any public operation causes an external mutation or relays/reconciles knowledge of one. | The mutation-effect-owning public contract boundary |
| SEC-001 | Any lookup, enumeration, attachment, capability, network, native, or protected-information path is exposed. | Admission Authority |
| PUBLIC-001 | First-party Client, CLI, adapter, or extension behavior is implemented or packaged. | Public SDK/Client contract boundary |
| IR-001 | A general Editing IR, pass manager, or equivalent lowered-form abstraction is proposed or admitted. | Transaction Contract Authority |
| RES-001..005 | A Resource family or Resource Adapter is declared, opened, observed, changed, or closed. | Resource contract boundary |
| CREATE-001 | A Resource family exposes destination creation or create-another. | Destination Creation Authority |
| ADD-001 | A Resource advertises Add-Only mutation or an edit is undone after possible publication. | Resource mutation/publication boundary |
| BUF-001..002 | Any Buffer is opened, edited, resident, evicted, or closed. | Kernel Core Buffer boundary |
| TXN-001..002 | Any Transaction or History transition is admitted or reconstructed. | Buffer State Lane |
| PUB-001..003 | A locally accepted change may cross an external publication boundary. | Publication/reconciliation owner |
| FENCE-001 | Asynchronous work can outlive any listed identity or incarnation. | The identity-owning installation boundary |
| DOM-001 | Concrete semantic meaning or lowering policy is admitted. | Domain boundary |
| COORD-001 | A semantic coordinate/value carrier is declared, transported, or resolved. | Domain/Representation boundary |
| REP-001..002 | A Representation is declared, interpreted, edited, or round-tripped. | Representation boundary |
| PROJ-001 | Correspondence or Representation Update state is computed or installed. | Projection boundary |
| INT-001..002 | A Client or extension submits Intent or consumes Outcome/Update. | Kernel interaction boundary |
| VIEW-001 | Presentation/interaction state or a View-facing type is introduced. | Client boundary |
| PROTO-001 | An external semantic protocol is integrated. | External Protocol Binding |
| EXEC-001..003 | A Runtime, public callable, task, completion, or State Lane is implemented. | Owning Kernel or Client Runtime |
| CONC-001 | A lock/State Lane can overlap a callback, wait, I/O, or other external work. | Owning synchronization boundary |
| LIVE-001..002 | A progress obligation or liveness policy is admitted in development, production, or qualification. | Runtime LivenessViolationPolicy boundary |
| LIVE-003 | A detection or watchdog scope is claimed. | Progress Watchdog owner |
| DBG-001 | Development fail-fast is selected. | Independent fatal supervisor |
| DBG-002 | Causal-debug retention or reverse query is offered. | Causal Debug Graph owner |
| CLIENT-001 | A Client Family is selected, implemented, packaged, or claimed. | Client Family boundary |
| CLIENT-002 | Client-local composition or cross-Client synchronization is introduced. | CLM/CCM contract boundary |
| CCM-001..002 | A Synchronization Scope or mandatory mixed-Family proof is selected. | Synchronization Authority |
| RENDER-001..003 | Rendering/output capability or a Presentation path is selected or claimed. | Client Rendering/Presentation boundary |
| HOST-001..003 | A Host Capability or Host Adapter is proposed, implemented, or used. | Host Capability boundary |
| SDK-001..004 | A public SDK schema, callable, package, binding row, or admission claim is offered. | SDK/support registry authority |
| EXT-001..006 | An Extension Package, Contribution, Adapter, Engine, Quick Profile, or introspection surface is offered. | Extension Contract/admission boundary |
| PERF-001..008 | A performance claim, budget, Scenario, trace, measurement field, or Experiment Bundle is produced. | Performance Authority |
| EVT-001..006 | An Event is submitted, retained, rendered, followed, exported, or used as evidence. | Editor Event Stream Lifecycle Owner |
| MIG-001..002 | Historical behavior or code enters an active migration slice. | Migration slice owner |
| QUAL-001..002 | A feature enters a candidate or an artifact/environment is claimed. | Release Qualification Authority |
| QUAL-003 | Mission-critical or certification language is published. | Product Contract Authority |

## Product, authority, and layering

| ID | Obligation | Authority | Evidence |
| --- | --- | --- | --- |
| `PROD-001` | Reovim is a text-neutral editing system; Kernel never means an OS product. | [Concept](../00-Concept/01-Product-System-and-Kernel.md) | `DOC,STATIC,TRACE` |
| `PROD-002` | v0.16 establishes the foundation; v0.17 parity uses v0.16 public contracts. | [Documentation authority](../README.md) | `DOC,TRACE,REVIEW` |
| `PROD-003` | TUI, CLI, and Web are mandatory v0.16 Client Families; none is waived as not-applicable. | [Quality](../07-Mission-Critical-Quality/03-Admission-Qualification-and-Release-Scope.md) | `TRACE,INTEG,QUAL` |
| `PROD-004` | The official SDK role/family BindingScope and DeliveryGate matrix is exact and independently evidenced. | [Extensions](../03-Extensions/01-Packages-Contribution-Points-and-Registry.md) | `SHAPE,COMPAT,INTEG,QUAL` |
| `AUTH-001` | Normative Documentation, plans, trackers, and evidence retain distinct authority. | [Requirements](01-Requirements-and-Evidence.md) | `DOC,TRACE` |
| `ARCH-001` | Source dependencies point inward through the ratified layer model without cycles. | [Layer model](../01-Architecture/01-Layer/01-Roles-Descriptors-and-Dependencies.md) | `STATIC,TRACE` |
| `ARCH-002` | Foundation and Kernel contracts contain no concrete environment, adapter, Domain, transport, Client, presentation, or extension mechanism. | [Layer model](../01-Architecture/01-Layer/01-Roles-Descriptors-and-Dependencies.md) | `STATIC,SHAPE` |
| `ARCH-003` | Resource, Host, Protocol, Presentation, Extension Adapter, and Extension Engine Adapter roles remain distinct. | [Layer model](../01-Architecture/01-Layer/01-Roles-Descriptors-and-Dependencies.md) | `STATIC,INTEG` |
| `ARCH-004` | Assembly selects an inert graph; each subtree has one lifecycle authority and no hidden global discovery. | [Layer model](../01-Architecture/01-Layer/02-Interaction-Assembly-and-Lifecycle.md) | `SHAPE,FAULT,INTEG` |
| `ARCH-005` | Third-party dependencies have concrete consumers and retained dependency reviews. | [Layer model](../01-Architecture/01-Layer/01-Roles-Descriptors-and-Dependencies.md) | `STATIC,REVIEW,PERF` |
| `ARCH-006` | Every evidence set carries one schema-valid Source Role Map with deterministic path classification and graph projection. | [Requirements](01-Requirements-and-Evidence.md) | `SHAPE,STATIC,TRACE` |

## Cross-cutting safety, bounds, effects, and exposure

| ID | Obligation | Authority | Evidence |
| --- | --- | --- | --- |
| `SAFE-001` | Every opaque carrier has an enforced Carrier Safety Descriptor covering ownership/lifetime, transfer, sharing, panic/unwind, and unsafe boundaries. | [Layer model](../01-Architecture/01-Layer/01-Roles-Descriptors-and-Dependencies.md) | `SHAPE,STATIC,FAULT` |
| `BOUND-001` | Every bounded capability/operation has one Bound Descriptor declaring units, per-object/aggregate maxima, checked arithmetic/overflow/zero semantics, pre-allocation validation order, charge ownership, and typed exhaustion. | [Layer model](../01-Architecture/01-Layer/01-Roles-Descriptors-and-Dependencies.md) | `SHAPE,MODEL,FAULT` |
| `ISOLATE-001` | Every payload, retained state, work unit, queue, concurrency group, retry, callback, drain, and wait is aggregate-bounded, charged, pressure-visible, deadline-bounded where applicable, and isolated from unrelated consumers. | [Quality](../07-Mission-Critical-Quality/02-Lifecycle-Isolation-and-Authority.md) | `MODEL,FAULT,MEM,PERF` |
| `EFFECT-001` | Every public external mutation or operation relaying its result preserves Effect Certainty and Outcome Cause through cancellation, timeout, response loss, shutdown, and reconciliation. | [Quality](../07-Mission-Critical-Quality/01-Integrity-Effects-and-Fencing.md) | `MODEL,CONTRACT,FAULT,IO` |
| `SEC-001` | Authority precedes lookup/exposure; protected metadata, network/native access, and unclassified behavior fail closed with bounded redaction and revocation. | [Quality](../07-Mission-Critical-Quality/02-Lifecycle-Isolation-and-Authority.md) | `SHAPE,SEC,FAULT` |
| `PUBLIC-001` | First-party Clients, CLI paths, adapters, and extensions reduce through the same supported public contracts as equivalent third parties, without private state shortcuts. | [Requirements](01-Requirements-and-Evidence.md) | `STATIC,CONTRACT,INTEG` |
| `IR-001` | A general Editing IR or pass manager is admitted only after two independent implemented consumers prove the same Transaction gap, measured boundary costs, and Transaction Contract Authority approval. | [Layer model](../01-Architecture/01-Layer/02-Interaction-Assembly-and-Lifecycle.md) | `TRACE,INTEG,PERF,REVIEW` |

## Resource and editing state

| ID | Obligation | Authority | Evidence |
| --- | --- | --- | --- |
| `RES-001` | Resource identity, Address, Extent, Unit, Revision, capabilities, limits, and errors are explicit and Resource-native. | [Resource contract](../01-Architecture/02-Resource-and-Buffer/01-Resource-Access-Snapshots-and-Change.md) | `SHAPE,CONTRACT` |
| `RES-002` | Resource capabilities compose independently; unsupported behavior is typed and never inferred. | [Resource contract](../01-Architecture/02-Resource-and-Buffer/01-Resource-Access-Snapshots-and-Change.md) | `SHAPE,CONTRACT,FAULT` |
| `RES-003` | Open establishes authority, coherent initial Snapshot/watch state, and one live Access Incarnation atomically. | [Resource contract](../01-Architecture/02-Resource-and-Buffer/01-Resource-Access-Snapshots-and-Change.md) | `MODEL,CONTRACT,FAULT` |
| `RES-004` | Snapshot consistency, capture/freshness/time authority, structure, limits, and gaps are explicit. | [Resource contract](../01-Architecture/02-Resource-and-Buffer/01-Resource-Access-Snapshots-and-Change.md) | `SHAPE,MODEL,FAULT` |
| `RES-005` | External change notification/feed and initial snapshot/subscription handoff are race-free and gap-visible. | [Resource contract](../01-Architecture/02-Resource-and-Buffer/01-Resource-Access-Snapshots-and-Change.md) | `MODEL,CONTRACT,FAULT` |
| `CREATE-001` | Create-another uses an independently authorized immutable Resource Creation Intent without mutating or rebinding its source. | [Resource contract](../01-Architecture/02-Resource-and-Buffer/03-External-Effects-and-Conformance.md) | `MODEL,FAULT,IO,INTEG` |
| `ADD-001` | Add-Only uses a Resource-native monotonic relation; Undo never misstates an applied external addition as rollback. | [Resource contract](../01-Architecture/02-Resource-and-Buffer/03-External-Effects-and-Conformance.md) | `MODEL,CONTRACT,FAULT` |
| `BUF-001` | One Buffer is transactional working state for exactly one Resource and editing context. | [Buffer contract](../01-Architecture/02-Resource-and-Buffer/02-Buffer-Transactions-History-and-Residency.md) | `SHAPE,MODEL,INTEG` |
| `BUF-002` | Opening and editing support bounded partial residency without default full-resource cloning/materialization. | [Buffer contract](../01-Architecture/02-Resource-and-Buffer/02-Buffer-Transactions-History-and-Residency.md) | `MODEL,MEM,IO,PERF` |
| `TXN-001` | Transactions validate Preconditions and atomically accept or reject fully lowered Buffer-valid Operations. | [Transaction contract](../01-Architecture/02-Resource-and-Buffer/02-Buffer-Transactions-History-and-Residency.md) | `MODEL,CONTRACT,FAULT` |
| `TXN-002` | History/Undo/Redo preserve coherent reconstructable accepted state within declared bounds. | [Transaction contract](../01-Architecture/02-Resource-and-Buffer/02-Buffer-Transactions-History-and-Residency.md) | `MODEL,FAULT,MEM` |
| `PUB-001` | Local acceptance and external publication are separate outcomes. | [Publication contract](../01-Architecture/02-Resource-and-Buffer/03-External-Effects-and-Conformance.md) | `CONTRACT,FAULT,INTEG` |
| `PUB-002` | Effect Certainty and Outcome Cause remain orthogonal through cancellation, timeout, failure, and response loss. | [Publication contract](../01-Architecture/02-Resource-and-Buffer/03-External-Effects-and-Conformance.md) | `MODEL,CONTRACT,FAULT` |
| `PUB-003` | Outcome-unknown publication retains a bounded public reconciliation owner/witness and prevents blind retry. | [Publication contract](../01-Architecture/02-Resource-and-Buffer/03-External-Effects-and-Conformance.md) | `MODEL,FAULT,IO` |
| `FENCE-001` | Resource creation/access, Buffer, Projection, Contract/Protocol Session, IPC, Client/Family/Module/render/CCM, Host, adapter, and engine identities/generations fence stale work at every listed cut. | [Quality fence matrix](../07-Mission-Critical-Quality/01-Integrity-Effects-and-Fencing.md) | `MODEL,FAULT,INTEG` |

## Meaning, interaction, and View

| ID | Obligation | Authority | Evidence |
| --- | --- | --- | --- |
| `DOM-001` | Domain policy supplies meaning and lowers semantic intent without owning Resource connectivity or Buffer state. | [Semantic contract](../01-Architecture/03-Semantics-and-View/01-Domains-Representations-and-Projections.md) | `STATIC,CONTRACT,INTEG` |
| `COORD-001` | Semantic coordinates and values use descriptor-qualified carriers plus exact authority and state stamps; presentation coordinates remain Client-owned. | [Semantic contract](../01-Architecture/03-Semantics-and-View/01-Domains-Representations-and-Projections.md) | `SHAPE,CONTRACT,FAULT` |
| `REP-001` | Each Representation declares address/value model, fidelity, editability, correspondence, bounds, and oracle. | [Semantic contract](../01-Architecture/03-Semantics-and-View/01-Domains-Representations-and-Projections.md) | `SHAPE,MODEL,GOLDEN` |
| `REP-002` | Editable high-level forms preserve unexpressed source data or reject the edit; lossy forms are read-only. | [Semantic contract](../01-Architecture/03-Semantics-and-View/01-Domains-Representations-and-Projections.md) | `MODEL,GOLDEN,INTEG` |
| `PROJ-001` | Projection owns generation-fenced correspondence/invalidation and never becomes a second edit/history authority. | [Semantic contract](../01-Architecture/03-Semantics-and-View/01-Domains-Representations-and-Projections.md) | `MODEL,FAULT,INTEG` |
| `INT-001` | Every Interaction Intent is authority-, identity-, revision-, version-, and generation-stamped and receives one terminal Outcome. | [Interaction contract](../01-Architecture/03-Semantics-and-View/02-Interaction-Updates-Protocols-and-View.md) | `CONTRACT,FAULT,INTEG` |
| `INT-002` | Representation Updates and Interaction Outcomes remain distinct; gaps force explicit resynchronization. | [Interaction contract](../01-Architecture/03-Semantics-and-View/02-Interaction-Updates-Protocols-and-View.md) | `CONTRACT,FAULT,INTEG` |
| `VIEW-001` | View/viewport/layout/input/drawing/frame policy is Client-owned and absent from Kernel contracts. | [View contract](../01-Architecture/03-Semantics-and-View/02-Interaction-Updates-Protocols-and-View.md) | `STATIC,SHAPE,INTEG` |
| `PROTO-001` | External Protocol Binding owns semantic translation while Protocol Adapter owns framing/carrier mechanics; neither redefines Reovim identity or authority. | [External protocol contract](../01-Architecture/03-Semantics-and-View/02-Interaction-Updates-Protocols-and-View.md) | `STATIC,COMPAT,INTEG,FAULT` |

## Execution, lifecycle, and debugging

| ID | Obligation | Authority | Evidence |
| --- | --- | --- | --- |
| `EXEC-001` | Each Kernel/Client Runtime owns a private non-Host ExecutionFacility with bounded admission and terminal completion. | [Execution contract](../01-Architecture/04-Execution/03-Runtime-Execution-Facility.md) | `SHAPE,MODEL,FAULT` |
| `EXEC-002` | Invocation, completion, delivery, effect, callback, and reentrancy classes remain explicit and orthogonal; any BlockingClass alias maps only to Invocation. | [Execution contract](../01-Architecture/04-Execution/01-Execution-Laws-and-Operation-Classes.md) | `SHAPE,CONTRACT` |
| `EXEC-003` | Buffer and Client State Lanes are separate logical linearization authorities; guest/external work never runs under them. | [Execution contract](../01-Architecture/04-Execution/02-State-Lanes-and-Synchronization.md) | `STATIC,MODEL,FAULT` |
| `CONC-001` | No authoritative lock or State Lane is held across user, SDK, adapter, IPC, Host, presentation, or other external work. | [Execution contract](../01-Architecture/04-Execution/02-State-Lanes-and-Synchronization.md) | `STATIC,FAULT,PERF` |
| `LIVE-001` | Progress Points, stalls, freezes, deadlocks, starvation, and livelock are typed and independently evidenced. | [Execution contract](../01-Architecture/04-Execution/04-Liveness-Watchdog-and-Fail-Fast.md) | `MODEL,FAULT,PERF,EVENT` |
| `LIVE-002` | Production observes liveness defects without diagnostic-driven recovery mutation; qualification fails disallowed defects. | [Quality](../07-Mission-Critical-Quality/02-Lifecycle-Isolation-and-Authority.md) | `FAULT,INTEG,QUAL` |
| `LIVE-003` | Every claimed watchdog has an observation and clock path independent of the suspected stall, lock, lane, facility, IPC, and diagnostic output. | [Execution contract](../01-Architecture/04-Execution/04-Liveness-Watchdog-and-Fail-Fast.md) | `SHAPE,FAULT,PERF` |
| `DBG-001` | Development fatal policy captures bounded independently readable evidence before process/test termination. | [Execution contract](../01-Architecture/04-Execution/04-Liveness-Watchdog-and-Fail-Fast.md) | `FAULT,EVENT,INTEG` |
| `DBG-002` | Reverse causal queries are bounded, authority-scoped, gap-visible, generation-exact, and non-mutating. | [Execution contract](../01-Architecture/04-Execution/05-Causal-Debugging-and-Evidence.md) | `MODEL,FAULT,EVENT,PERF` |

## Clients, synchronization, and rendering

| ID | Obligation | Authority | Evidence |
| --- | --- | --- | --- |
| `CLIENT-001` | Every Client Family owns a separate Core, public Contract, Module surface, Runtime, and Presentation path. | [Client contract](../01-Architecture/05-Client/01-Client-Ownership-and-Family-Contracts.md) | `STATIC,CONTRACT,INTEG,QUAL` |
| `CLIENT-002` | CLM owns one Client's local composition; CCM alone owns explicit cross-Client synchronization. | [Client contract](../01-Architecture/05-Client/01-Client-Ownership-and-Family-Contracts.md) | `SHAPE,MODEL,INTEG` |
| `CCM-001` | CCM state has explicit authority/schema/privacy/bounds/gaps/resynchronization and never applies peer Buffer edits. | [Client contract](../01-Architecture/05-Client/03-Extension-Modules-and-CCM.md) | `MODEL,FAULT,INTEG` |
| `CCM-002` | TUI, CLI, and Web prove join/leave/reconnect/gap/resync plus one mixed-family isolation journey. | [Client contract](../01-Architecture/05-Client/04-Lifecycle-Families-and-Release-Evidence.md) | `INTEG,FAULT,PERF,QUAL` |
| `RENDER-001` | Rendering is negotiated capability, not inferred from Client Family. | [Rendering contract](../01-Architecture/05-Client/02-Rendering-Capabilities.md) | `SHAPE,CONTRACT` |
| `RENDER-002` | Presentation Adapter alone owns concrete terminal/window/browser/GPU/device handles and loss/recreation. | [Rendering contract](../01-Architecture/05-Client/02-Rendering-Capabilities.md) | `STATIC,FAULT,INTEG` |
| `RENDER-003` | Every selected cell/raster/pixel/video/GPU path has independent correctness and performance evidence. | [Rendering contract](../01-Architecture/05-Client/04-Lifecycle-Families-and-Release-Evidence.md) | `GOLDEN,PERF,MEM,QUAL` |

## Host, SDK, and extensions

| ID | Obligation | Authority | Evidence |
| --- | --- | --- | --- |
| `HOST-001` | Host capabilities are small, semantic, consumer-driven, and exclude platform handles/branching. | [Host API](../02-Host-API.md) | `STATIC,SHAPE,REVIEW` |
| `HOST-002` | Initial Host scope contains only MonotonicClock and DiagnosticOutput; ExecutionFacility/engines are non-Host. | [Host API](../02-Host-API.md) | `STATIC,CONTRACT` |
| `HOST-003` | Every Host capability has standard/test adapters, shared conformance, lifecycle, limits, events, and measurements. | [Host API](../02-Host-API.md) | `CONTRACT,FAULT,PERF` |
| `SDK-001` | Language-neutral schemas own semantics; Rust/Python/TypeScript projections cannot redefine wire, retry, certainty, or lifecycle. | [Extensions](../03-Extensions/02-SDKs-Service-Contracts-and-Compatibility.md) | `SHAPE,COMPAT,GOLDEN` |
| `SDK-002` | Rust-first and service-reference predecessor gates complete before Python/TypeScript admission. | [Extensions](../03-Extensions/02-SDKs-Service-Contracts-and-Compatibility.md) | `TRACE,CONTRACT,PERF` |
| `SDK-003` | Each required or supported role/family/language row has public examples, docs, conformance, exact package, and independent evidence. | [Extensions](../03-Extensions/04-Quick-Authoring-Admission-and-Compatibility.md) | `DOC,COMPAT,INTEG,PERF,QUAL` |
| `SDK-004` | Every public callable declares orthogonal Invocation, Completion, Delivery, Effect, Callback, and Reentrancy classes. | [Extensions](../03-Extensions/02-SDKs-Service-Contracts-and-Compatibility.md) | `SHAPE,CONTRACT,FAULT` |
| `EXT-001` | Manifests declare Contribution Points, activation, grants, bounds, dependencies, placement, and lifecycle before code executes. | [Extensions](../03-Extensions/01-Packages-Contribution-Points-and-Registry.md) | `SHAPE,SEC,FAULT` |
| `EXT-002` | Introspection validates Session/authority/scope before lookup and prevents metadata/pagination/continuation leakage. | [Extensions](../03-Extensions/01-Packages-Contribution-Points-and-Registry.md) | `SEC,CONTRACT,FAULT` |
| `EXT-003` | First-party extensions use public-only paths and no private Kernel/Client shortcut. | [Extensions](../03-Extensions/README.md) | `STATIC,INTEG` |
| `EXT-004` | Adapter and Engine descriptors separate stable identity from registry-owned scope/delivery/admission status. | [Extensions](../03-Extensions/03-Adapters-Execution-and-Lifecycle.md) | `SHAPE,CONTRACT` |
| `EXT-005` | v0.16 Adapter Kit/TestOnly model selects no production engine or compatibility profile. | [Extensions](../03-Extensions/03-Adapters-Execution-and-Lifecycle.md) | `STATIC,SHAPE,TRACE` |
| `EXT-006` | Quick local manifests are synthesized only before execution from explicit inputs; installed, shared, and published packages use explicit manifests. | [Extensions](../03-Extensions/04-Quick-Authoring-Admission-and-Compatibility.md) | `SHAPE,SEC,INTEG` |

## Performance, events, migration, and qualification

| ID | Obligation | Authority | Evidence |
| --- | --- | --- | --- |
| `PERF-001` | Input-to-present p99 is primary; command-to-present, CLI command-to-output, and stages are separately named and never substituted. | [Performance](../04-Performance/01-Reaction-Metrics-Baselines-and-Workloads.md) | `PERF,TRACE` |
| `PERF-002` | Every product/support or exact-release claim maps respectively to its predeclared `RealProductScenario` or `QualificationProductScenario`, admissible Dataset provenance, Experiment Protocol, and content-addressed Bundle with all raw attempts. | [Performance](../04-Performance/02-Scenarios-Bundles-and-Traces.md) | `SHAPE,PERF,REVIEW` |
| `PERF-003` | Every run has Interaction Trace Status; captured traces have a schema-valid Manifest and are loss-aware, generation-exact, clock-domain-correct, and mandatory for stage claims. | [Performance](../04-Performance/02-Scenarios-Bundles-and-Traces.md) | `SHAPE,TRACE,PERF,EVENT` |
| `PERF-004` | IPC/transport, SDK, Client, rendering, adapter, and language rows are measured on real exact paths without substitution. | [Performance](../04-Performance/03-Path-Overhead-and-Regression-Evidence.md) | `PERF,MEM,IO,QUAL` |
| `PERF-005` | Instrumentation has a prior cost/capacity model plus exact-artifact individual, combined, maximum-profile, and rate-matrix calibration against observations. | [Performance](../04-Performance/03-Path-Overhead-and-Regression-Evidence.md) | `PERF,MEM,IO,REVIEW` |
| `PERF-006` | Lock/wait/hold/contention and freeze/deadlock/liveness behavior are measured through deterministic fault evidence and `RealProductScenario` or `QualificationProductScenario` evidence as applicable. | [Performance](../04-Performance/03-Path-Overhead-and-Regression-Evidence.md) | `FAULT,PERF,EVENT` |
| `PERF-007` | Every claim field has a Measurement Capability Record proving semantics, coverage, uncertainty, overhead, blind spots, and unavailable/observed-zero treatment. | [Performance](../04-Performance/02-Scenarios-Bundles-and-Traces.md) | `SHAPE,PERF,REVIEW` |
| `PERF-008` | One Performance Authority owns each budget, baseline, workload mapping, rationale, and deterministic invalidation/re-baseline decision. | [Performance](../04-Performance/01-Reaction-Metrics-Baselines-and-Workloads.md) | `TRACE,PERF,REVIEW` |
| `EVT-001` | One structured Event Stream owns operational diagnostics; the ring is exactly 1 MiB and all loss is visible. | [Event stream](../05-Editor-Event-Stream/01-Records-Admission-Retention-and-Consumers.md) | `MODEL,EVENT,MEM` |
| `EVT-002` | Canonical rendering is byte-identical across ring, file, stderr, replay, export, and `reovim log`. | [Event stream](../05-Editor-Event-Stream/01-Records-Admission-Retention-and-Consumers.md) | `GOLDEN,EVENT` |
| `EVT-003` | Event, ring, renderer, subscriber, and sink failures remain bounded, non-recursive, and non-authoritative. | [Event stream](../05-Editor-Event-Stream/02-Scope-Failure-and-Conformance.md) | `FAULT,EVENT,PERF` |
| `EVT-004` | Event provenance is editor-stamped, redacted before fan-out, and paired with typed Actionability/remediation where required. | [Event stream](../05-Editor-Event-Stream/01-Records-Admission-Retention-and-Consumers.md) | `SHAPE,SEC,EVENT` |
| `EVT-005` | One follow operation returns each available sequence at most once; cursor reuse replays deterministically, gaps stay explicit, snapshot/follow handoff is atomic, and foreign-incarnation cursors return typed stale/resynchronize. | [Event stream](../05-Editor-Event-Stream/01-Records-Admission-Retention-and-Consumers.md) | `MODEL,CONTRACT,FAULT` |
| `EVT-006` | Producer admission is a bounded NonBlockingInvocation with non-waiting synchronization, explicit rejection/drop/closed outcomes, and no synchronous sink or subscriber work. | [Event stream](../05-Editor-Event-Stream/01-Records-Admission-Retention-and-Consumers.md) | `MODEL,FAULT,PERF` |
| `MIG-001` | Every donor slice is adopt/adapt/rewrite/retire with a public target, evidence, cutover, and removal record. | [Migration](../05-Migration/01-Selective-Migration.md) | `TRACE,REVIEW,INTEG` |
| `MIG-002` | Duplicate/stale paths and invalid compatibility shims are removed after the conforming path is proved. | [Migration](../05-Migration/01-Selective-Migration.md) | `STATIC,INTEG,REVIEW` |
| `QUAL-001` | Features enter a candidate only with complete public, fault, bounds, lifecycle, dependency, performance, Event, and qualification mapping. | [Quality](../07-Mission-Critical-Quality/03-Admission-Qualification-and-Release-Scope.md) | `TRACE,REVIEW,QUAL` |
| `QUAL-002` | Exact TUI/CLI/Web artifacts and required SDK rows qualify on `x86_64-unknown-linux-gnu` before v0.16 release. | [Quality](../07-Mission-Critical-Quality/03-Admission-Qualification-and-Release-Scope.md) | `INTEG,PERF,QUAL` |
| `QUAL-003` | Mission-critical language is scoped evidence discipline and never implies unnamed formal certification. | [Quality](../07-Mission-Critical-Quality/01-Integrity-Effects-and-Fencing.md) | `DOC,REVIEW` |
