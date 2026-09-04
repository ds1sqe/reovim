# Editing-System Vocabulary

> **Normative target contract.** These are the canonical cross-chapter terms.
> Present-tense definitions state required target meaning, not current
> implementation or qualification status.

## Use of this vocabulary

The vocabulary is text-neutral and distinguishes data connectivity, editing
state, portable Kernel mechanism, semantic interpretation, Client presentation,
extension execution, Assembly, and the process environment. Chapter-local
implementation terms remain in their owning chapter.

Use the qualified adapter term—such as **Resource Adapter**, **Host Adapter**,
**Protocol Adapter**, **Presentation Adapter**, or **Extension Adapter**—when
the role could otherwise be unclear.

## Product and ownership

| Term | Definition |
| --- | --- |
| **Editing System** | The complete Reovim product: portable Kernel, public contracts, injected adapters, extensions, Clients, presentation, and Assembly. It is not synonymous with the Kernel or one Text frontend. |
| **Mission-Critical Quality** | The discipline of admitting and qualifying only a specifically scoped surface with evidenced fidelity, atomicity, Effect Certainty, recovery, bounds, isolation, diagnostics, and performance. It is not a blanket certification or feature-count claim. |
| **Foundation** | Environment-, transport-, presentation-, and Domain-neutral language primitives and opaque carriers used by Kernel Interface. Each carrier is safe only under its Carrier Safety Descriptor. Foundation is subject to dependency-closure and forbidden-type checks. |
| **Carrier Safety Descriptor** | The versioned declaration for one opaque carrier family: ownership, borrow/copy/drop behavior, lifetime/incarnation, type/schema identity and mismatch outcome, thread transfer/affinity, sharing/concurrency, panic/unwind behavior, and permitted unsafe boundary. Undeclared behavior is unsupported. |
| **Reovim Kernel** | The portable, Domain-neutral execution and lowering substrate owning editing contracts and generic mechanisms for revision-bound state transitions, Representation Updates, lifecycle, isolation, and observation. The managed-runtime/compiler-substrate analogy is limited to role and does not imply bytecode, JIT, compiler passes, garbage collection, or code generation. |
| **Kernel Interface** | Dependency-light, environment- and transport-neutral public contract families through which Kernel Core, Kernel Runtime, Domains, adapters, Clients, and extensions interact. It owns semantics and values, never concrete provider or presentation types. |
| **Inter-Layer Contract** | The inward-owned versioned semantic boundary between two logical roles, complete for identity, authority, bounds, operations, terminal outcomes, lifecycle, failure, Events, causal relations, and evidence before either side's inner mechanism is implemented. |
| **Intra-Tree Interface** | A private unstable interface wholly owned within one Runtime, Core, Family, adapter, or other ownership tree. Its owner may break it at any commit while updating all consumers coherently; it is never an SDK or compatibility surface. |
| **Kernel Core** | The portable authoritative editing mechanism for Buffer transitions, Transactions, History, conflicts, residency, publication state, correspondence, and Projection generations. It contains no concrete Domain, adapter, transport, or Client policy. |
| **Kernel Runtime** | The portable bounded execution and lifecycle mechanism for Kernel-owned work and Contract Sessions: negotiation, admission, generation fencing, readiness, rollback, Event admission, supervision, and ordered shutdown. It receives concrete components from Assembly without importing their implementations. |
| **Client** | A public consumer owning its applicable presentation/output state and negotiated Contract Sessions. Any View, viewport, layout, focus, input mapping, drawing, or frame state it instantiates is Client-owned; a Family may instead expose a typed no-View output path. It never reaches private Kernel state or concrete storage. |
| **Client Family** | One complete Client implementation family with its own Family Core, public Family Contract, extension point and Module surface, and presentation/output path. TUI, CLI, and Web are mandatory v0.16 families; Android and Native are future families. |
| **Family Core** | Family-local mechanism for public-contract adaptation, Module composition/dispatch, Client-local state coordination, family lifecycle, and its private Runtime ExecutionFacility. It knows no concrete feature Module and exposes no private Kernel path. |
| **Family Contract** | The public versioned family-specific contract for input, View, negotiated Rendering Capabilities, presentation/output, lifecycle, bounds, and failure semantics. It specializes Client behavior without redefining editing semantics. |
| **Family Module** | Feature policy implementing one Family Contract through public common Client semantics. It never depends on Family Core internals, another family, private Kernel types, or a concrete Protocol or Presentation Adapter. |
| **Client Runtime** | Lifecycle Owner for one Client subtree: Family Core and Modules, View, Presentation Port/Adapter, client-side Protocol endpoint, Contract Sessions, readiness, rollback, supervision, and ordered shutdown. It remains outside the Kernel. |
| **Client Layer Model (CLM)** | The composition, extension, and presentation model inside one Client Runtime. It owns no cross-Client synchronization authority. |
| **Client–Client Model (CCM)** | The versioned synchronization model among two or more Client instances for explicitly admitted shared semantic, collaboration, or Module state. It owns no Buffer, View, rendering, Resource, or transport authority. |
| **Assembly** | The composition boundary resolving configuration, selecting concrete components, constructing an inert dependency/ownership graph, and triggering root Lifecycle Owners. It does not share their readiness or shutdown state. |
| **Lifecycle Owner** | The one authority for activation, readiness publication, supervision, rollback, and shutdown of a declared subtree. Assembly may trigger it but does not share its state. |
| **Admission Authority** | The named authority validating trust policy and minting a bounded, revocable Contract Session or capability grant before lookup, attachment, or mutation. Identity alone is not authority. |
| **Contract Authority** | The one named chapter or contract owning a normative question. Another document may explain or implement it but cannot silently redefine it. |
| **Target Contract** | Required behavior stated by normative Documentation. Present tense does not by itself assert implementation or release completion. |
| **Implementation Evidence** | Reviewable committed code, public-boundary tests, conformance results, measurements, and artifacts proving only the exact requirement and scope exercised. Intent or a narrower passing test is not equivalent evidence. |
| **Source Role Map** | Versioned exact-revision generated evidence correlating classified paths/exclusions with the workspace, package, target, dependency, tool/input, finding, and result fields required by that schema version. It is generated from separate version-matched policy and never serves as its own input; a later version does not add meaning to an older report. |
| **Shipping Closure** | The transitive Product target, dependency, source/include/macro, generated-source, helper/invocation, public-type/re-export, and bundled package/asset set selected for a shipping claim. Any participant is Product-owned Production; Repository Tooling never enters it. |
| **Validation Closure** | A separately selected nonshipping Product-owned Test/Example compilation used to prove a bounded validation question. Its allowed inward Product edge grants no shipping authority, and it remains absent from shipping compilation/package membership unless explicitly promoted to Production. |
| **Product Closure** | One explicitly selected Shipping Closure or Validation Closure with exact workspace, lock, target, profile, features, source/dependency participation, mechanism support, and applicable package/public-consumer evidence. Ambient build defaults grant no closure. |
| **Product Proof Profile** | A versioned closed set of Product mechanisms for which the checker has positive compiler, package, source, public-surface, bound, and cleanup evidence. An unsupported mechanism remains typed unavailable rather than weakening the profile. |
| **Prepared Execution Root** | Linux exact-evidence authority rooted in one quota-backed private scratch filesystem, exposing only exact read-only source/tool/runtime mounts plus bounded process and output channels. It is tooling containment, not a Reovim Host API or Product runtime abstraction. |
| **Package Membership Evidence** | Evidence of Cargo-selected source-package member paths correlated to declared copied repository paths and closed Cargo-generated names. Without staged/archive byte inspection, it does not prove the bytes Cargo copied or generated. |
| **Public Consumer Evidence** | Compile-time accessibility/type-checking of an exact declared public-item set by a separate Product-owned external-crate Validation Closure. It is not behavioral conformance, semantic completeness, ABI stability, or qualification. |
| **Dependency Review Record** | A retained decision for one dependency and consumer recording approver, re-review conditions, maintenance, actual license rationale, transitive graph and cost, unsafe/native surface, build/runtime behavior, targets, and measured overhead. |
| **Supported and Qualified Environment Matrix** | The versioned release record whose rows identify claimed environments, prerequisites, support state, exact artifact, mapped conformance/workloads, limitations, evidence state, and deterministic invalidators. An absent row implies no claim. |
| **Qualified Environment** | A matrix row whose exact packaged candidate passed every mapped public journey, conformance rule, fault case, and workload required by the scoped claim in the declared clean environment. |
| **Build-Verified Environment** | A row for which an explicitly named build/test subset passed and every omitted qualification obligation is listed. It is not release qualification. |
| **Best-Effort Environment** | A row with no current compatibility or qualification promise. Promotion requires the destination state's evidence; stale or failed evidence is not ignored. |
| **Portable Core Profile** | The `#![no_std]`, Rust-`core`-only Foundation closure. It claims no allocator or full editing composition. |
| **Portable Allocation Profile** | The exact `#![no_std]`, `core + alloc` Product closure for portable Kernel, SDK/policy, and at least one Client path, with explicit allocator, atomic, panic, execution, and adapter prerequisites. |
| **Hosted Profile** | The outside portable closure containing `std`-using adapters, supervisors, and application Assembly roots. It cannot leak hosted types into portable contracts. |

## Adapter and protocol boundaries

| Term | Definition |
| --- | --- |
| **Resource Adapter** | The connector translating a Resource-native subject's observation, revision, publication, conflict, resize, creation, and change-discovery semantics into the Resource contract. It supplies connectivity, not Domain meaning or presentation. |
| **Host Capability** | A small semantic Kernel-owned contract through which one named consumer requests an environment service, including bounds, typed errors, lifecycle/cancellation, deterministic substitution, Events, and measurements where applicable. |
| **Host Adapter** | A concrete implementation of one or more Host Capabilities. Environment-specific handles, constants, branching, and dependencies remain private. It is not a Resource Adapter. |
| **Standard Host Adapter** | A Host Adapter for normal hosted execution using language-standard facilities and selectively reviewed dependencies. “Standard” names its role, not a universal Host object. |
| **Deterministic Test Adapter** | A controllable Host Adapter exercising limits, faults, cancellation, ordering, and lifecycle under the same semantic contract as production variants. |
| **Portable Host Requirements** | The declared allocator, atomic, panic/entry, private ExecutionFacility, exact Host Capability, Resource, Protocol, Presentation, and Assembly obligations an external custom host satisfies to compose a claimed portable Reovim path. |
| **Protocol Adapter** | A concrete component owning encoding, framing, connection, carrier I/O, queues, and reconnect mechanics for a language-neutral operation contract. It cannot redefine semantics, Effect Certainty, or authority. |
| **External Protocol Binding** | An outside-Kernel semantic translator between one external protocol and public Reovim Resource, Domain, Representation, Client, or service contracts. It does not own framing/carrier mechanics and never equates an external reference or version with Reovim identity/stamps without an explicit mapping. |
| **Presentation Port** | A Client-owned contract between presentation policy and a target mechanism. It carries bounded input/output and handoff semantics, is not Kernel Interface, and grants no editing authority. |
| **Presentation Adapter** | An Assembly-selected Client-side mechanism behind a Presentation Port. It owns terminal, window, display, rasterizer, GPU/device, or equivalent target I/O and presentation handoff. |
| **Rendering Capability** | A negotiated Family Contract capability describing accepted cell/raster/pixel/vector/video/output forms, bounds, ordering, handoff, loss/recovery, and fallback. A Client Family name does not imply a render format. |

## Resource model

| Term | Definition |
| --- | --- |
| **Resource** | An external or synthetic addressable subject exposed through the Resource contract with explicit identity, native coordinate and Unit types, capabilities, and revision semantics. Read-only and editable Resources are valid. |
| **Resource Identity** | The value by which an adapter determines that two handles refer to the same logical Resource under declared lifetime and comparison rules. It is not necessarily a path or process-global name. |
| **Provisional Access Token** | A private identity for one Resource open attempt. It may stamp provisional state inside readiness but cannot authorize public work. Successful atomic readiness promotes it; failure retires it. |
| **Resource Access Incarnation** | The public opaque identity atomically promoted with a ready Resource Adapter handle and validated initial Snapshot. Replacement, reopen, or adapter restart creates a new incarnation even when Resource Identity and Revision match. |
| **Capability** | A negotiated semantic promise, its Bound Descriptor, and its failure behavior. Unsupported behavior is a typed outcome, never an implicit fallback. |
| **Bound Descriptor** | The declaration of units and hard maxima for payloads, counts, owned bytes, retained objects, concurrent operations, queues, aggregate scope, and completion waits. It owns checked arithmetic, overflow, zero-limit semantics, validation order, accounting, and exhaustion outcomes. |
| **Source Access Authority** | The Resource Access Incarnation grant to observe only or to publish specified mutation families to the same Resource. It does not grant destination creation. |
| **Read-Only Access** | Source Access Authority that cannot mutate the same Resource. The subject may still evolve externally, and a Buffer may accept a local derived branch when its contract permits. |
| **Stable Resource** | A Resource-family guarantee that state cannot change under the same Resource Identity for the declared lifetime. It is distinct from Read-Only Access and Snapshot immutability. |
| **Mutation Grammar** | The Resource-native set of permitted external changes: none, replacement, Add-Only, resize, or another explicitly defined operation family. |
| **Add-Only** | A mutation grammar allowing only additions under a Resource-family monotonicity relation. It may add records, keys, facts, samples, or other native values; it does not imply byte append. |
| **Destination Creation Authority** | An independent grant over a destination family or namespace to submit a bounded Resource Creation Intent. Source access never implies it. |
| **Address** | A Resource-native coordinate identifying where an observation or edit applies. It is not universally a byte offset, integer, text position, or stable random-access location. |
| **Extent** | A Resource-native measure bounding an addressed region or amount. Its arithmetic and validity belong to the Resource coordinate contract. |
| **Unit** | The Resource-native value observed or replaced at an addressable location. It is not universally a byte, character, record, or terminal cell. |
| **Revision** | A value identifying Resource state under declared observation and conflict semantics. It need not be numeric, globally ordered, or time. |
| **Snapshot Descriptor** | The Resource-family schema and validator for Snapshot structure, native region/address/extent invariants, capture/consistency/freshness stamps, capability and bound provenance, and compatible unknown fields. |
| **Snapshot** | An immutable observation stamped with Resource Identity, Resource Access Incarnation, and Revision, with stable semantics for the regions exposed. It may be sparse, lazy, or partial; it does not imply a complete in-memory copy. |
| **Snapshot Freshness** | A family-owned predicate yielding `Fresh`, `Stale`, or `Indeterminate` under declared capture, consistency, time-authority, uncertainty, age/expiry, and discontinuity semantics. `Unavailable` separately means the requested historical state cannot be served. |
| **Residency** | Resource or Buffer state presently retained in owned memory. Bounded partial residency has explicit limits and does not require whole-Resource materialization by default. |
| **Resource Change Notification** | Optional bounded invalidation evidence requiring a new authoritative observation. It is not a Snapshot or source-state stream. |
| **Resource Change Feed** | A stronger optional Resource-native stream with declared ordering, completeness, cursor/resume, retention, coalescing, duplicates, pressure, gaps, and resynchronization. It is not the Editor Event Stream. |
| **Watch** | The general optional Resource capability for change discovery. A concrete family exposes Notification or Feed strength explicitly; “watchable” alone grants neither completeness nor resumability. |
| **Resource Creation Intent** | An immutable, fully stamped selection naming the source Snapshot or Buffer version, independently authorized destination family/namespace, identity policy, fidelity, bounds, conflict, cancellation, and recovery. It never mutates or rebinds the source. |
| **Resource Creation Proof** | A validated destination-family proof that creation produced the declared new Resource Identity and initial Revision/Snapshot or equivalent family-defined result. |

## Editing state

| Term | Definition |
| --- | --- |
| **Buffer** | Reovim-owned transactional working state associated with exactly one Resource. It combines a known base, accepted local changes, History, conflict/publication state, and bounded residency. One Resource may have several explicit Buffers; it is neither a string nor a mandatory full copy. |
| **Buffer Incarnation** | The identity of one opened Buffer lifetime, distinct from Resource Identity and Buffer version. Reopen, replacement, or explicit reset creates a new incarnation. |
| **Base Revision** | The Resource Revision against which Buffer state or a Transaction states its assumptions. |
| **Buffer State Lane** | The one logical linearization authority for Buffer Transaction, History, compatibility, publication/reconcile, and lifecycle installation. It is not necessarily a thread and is never held across external work, callbacks, waits, or I/O. |
| **Client State Lane** | The logical per-Client Runtime linearization authority for generation-checked installation of Client-local Core, Module, View, layout, render, and frame state. Work runs outside it on immutable stamped inputs; installation revalidates Client, capability, Representation, and CCM generations and exits before presentation/protocol I/O, callbacks, or waits. It is not a Kernel type or mandatory thread. |
| **Transaction** | An ordered, atomic lowered local edit against one Buffer incarnation and observed version, containing Buffer-valid Operations and optional Preconditions. Local acceptance remains distinct from publication. |
| **Operation** | A typed change understood by the Buffer operation contract and executable without Client or concrete Domain policy inside atomic acceptance. Operation families remain Resource/Buffer-native. |
| **Precondition** | A test that must hold for a Transaction or publication to proceed. |
| **Conflict** | A typed outcome showing that revision, precondition, or concurrent-change assumptions do not hold. It is never silently converted into overwrite or rebase. |
| **Local Acceptance** | Validation and atomic incorporation of a Transaction into Buffer working state and History. It does not assert an external Resource effect. |
| **Publication** | An attempt through a Resource Adapter to apply one immutable Publication Intent under explicit revision, atomicity, cancellation, recovery, and failure semantics. |
| **Publication Intent** | The immutable Resource/Access/base-Revision, adapter-contract, Buffer identity/incarnation/version/transitions, and recovery selection submitted for publication. Later edits are excluded. |
| **Publication Proof** | A Resource-family validated proof that an applied publication produced the declared resulting state. An opaque success flag is insufficient. |
| **Reconcile Status** | Bounded public state for an unresolved Publication or Creation Intent, including identities, recovery witness, selected state, Effect Certainty, Outcome Cause, proof, retention/expiry, duplicate behavior, and permitted next operation. |
| **Effect Certainty** | The orthogonal mutation-effect axis: `Applied`, `DefinitelyNotApplied`, or `OutcomeUnknown`. Unknown work requires reconciliation or evidenced idempotency and is not blindly retried. |
| **Outcome Cause** | The semantic reason associated with an outcome, such as success, conflict, stale, unsupported, denial, cancellation, pressure, closure, timeout, or failure. It never erases Effect Certainty. |
| **Generation Fence** | A binding between admitted/asynchronous work and every exact applicable identity, incarnation, authority, and generation it observed—including Resource access/creation, Buffer, Projection, Contract/Protocol Session, Client/Family/Module/View/render/CCM, Host, Protocol/Presentation/Extension Adapter, and Engine state. Retired work cannot install as current. |
| **Discard Accounting** | Contract-owned visibility for stale or discarded work. Caller-visible work receives a terminal outcome or Reconcile Status; no-waiter speculative work uses bounded saturating counters and stable Event families. |
| **History** | The ordered record required to reason about accepted Buffer transitions, with declared ordering, position, retention bounds, undo/redo behavior, reconstruction/checkpoint invariants, and eviction rules. It is not external Resource history. |
| **Undo** | A new Buffer transition reversing an eligible accepted transition. Before external application it may remove a pending Add-Only addition from a later Publication Intent; it never proves external rollback. |
| **Redo** | A new Buffer transition reapplying a transition made eligible by Undo under the History contract. |

## Meaning, coordinates, and presentation

| Term | Definition |
| --- | --- |
| **Domain** | A package of meaning and editing rules for a class of data: parsing, validation, navigation, commands, semantic selection, and Transaction construction. Text is one Domain. |
| **Representation** | An addressable interpretation of Resource or Buffer state for inspection or editing, declaring input requirements, coordinate/value model, correspondence, fidelity, editability, and round-trip behavior. It is not storage or layout. |
| **Semantic Coordinate Space Descriptor** | The Domain/Representation-owned schema for coordinate-space identity/incarnation, coordinate/region/value carriers, canonical encoding, bounds, supported operations, units/origin/axes/normalization, stability/remap, typed failures, and carrier safety. |
| **Semantic Location** | A descriptor-qualified semantic coordinate or region plus exact Resource/Access/Revision, Buffer/incarnation/version, Representation, Projection, and coordinate-space stamps. An unqualified number has no semantic authority. |
| **Projection** | Kernel Core mechanism owning stamped source correspondence, dependency tracking, invalidation, generation fencing, ordering, and bounded invocation of concrete Representation policy. It is not a second edit executor. |
| **Representation Update** | A bounded semantic full snapshot, predecessor-bound delta, invalidation, Representation Gap, or resynchronization result stamped with exact source and Projection state. It may contain schema-qualified intrinsic source values, including image pixels, but never target-screen pixels, terminal cells, draw lists, or frame instructions. |
| **Representation Gap** | An explicit indication that a required predecessor/update is unavailable. The Client resynchronizes rather than guessing. |
| **Interaction Intent** | A typed semantic Client request submitted through a Contract Session against the exact Resource, Buffer, Representation, Projection, and semantic-coordinate state observed. It lowers to a Transaction or typed Outcome and is never silently retargeted. |
| **Interaction Outcome** | The one bounded correlated terminal response to an Interaction Intent, recording acceptance, Effect Certainty where applicable, Outcome Cause, and resulting Buffer version. It is separate from Update delivery. |
| **View** | Client-owned presentation and interaction state over a Representation: viewport, layout, clipping, focus, scrolling, local selection, decoration, hit testing, drawing, and frame policy. It owns no Resource or Buffer authority. |
| **Viewport** | Client-owned visible-range and clipping policy for a View. It is not Resource residency, Kernel cache state, or a source coordinate authority. |
| **Faithful Representation** | A Representation that accounts for and can reproduce every source Unit in its declared scope without unreported loss. |
| **Constrained Editable Representation** | A form exposing declared constructs while preserving every unexpressed source Unit for each accepted edit or rejecting the edit. |
| **Derived Read-Only Representation** | A form that may summarize or lose source information and therefore cannot directly create Resource edits. |
| **Unexpressed Data** | Source state in a Representation's scope that the form cannot directly encode. Editable high-level paths preserve it or reject the edit. |

## Extensions and public contracts

| Term | Definition |
| --- | --- |
| **Extension** | A deliberately admitted first- or third-party component adding behavior through public SDK, Module, Family, or service contracts under explicit authority, bounds, lifecycle, compatibility, trust, and failure-containment rules. |
| **Extension Package** | A manifest-declared package containing Server Module definitions, zero or more Family Client Module definitions, shared schemas/assets, or a valid subset. Discovery executes no package code. |
| **Extension Adapter** | A language-neutral adapter projecting public SDK, Module, and Family contracts into a language toolchain or execution environment. It owns projection mechanics only and grants no authority. |
| **Extension Engine Adapter** | Conditional Extension Adapter specialization for one interpreter, VM, managed engine, or equivalent engine instance, owning only engine construction, module resolution, value conversion, callback mechanics, local queues/tasks/handles, declared budget enforcement, diagnostics, and shutdown. |
| **Extension Adapter Kit** | The v0.16 language-neutral descriptor and manifest schemas, generated projection interfaces, TestOnly synthetic model, semantic/value fixtures, conformance/fault harnesses, evidence schema, packaging template, and adapter-author documentation. It contains no production engine or runtime. |
| **Quick Extension Profile** | A local one-file or small-package workflow using the same public contracts as installed packages. Its deterministic manifest may be synthesized before source execution from explicit inputs; installing, sharing, or publishing the Extension Package requires an explicit manifest. |
| **Contribution Point** | A closed versioned public contract family for a declarative or runtime-backed contribution. It declares authority, values, bounds, invocation/delivery/effect classes, lifecycle, reentrancy, cancellation, failures, and conformance. |
| **Activation Predicate** | A bounded public condition controlling code activation. Discovery and static registration execute no code; activation cannot widen manifest authority. |
| **Rust SDK** | The trusted, latency-sensitive in-process public extension surface. It is a semantic API, not a permanent native ABI promise. |
| **SDK Stability Release** | The first full release that explicitly admits named SDK and service-contract rows as Stable after compatibility, migration, public-journey, and qualification evidence. No earlier release or support state implies it. |
| **Service Contract** | The language- and transport-neutral out-of-process extension and Client operation contract, independently versioned from product and SDK packages. |
| **Contract Session** | One negotiated Client or extension attachment incarnation fixing contract/version, role, capabilities, Resource scope, bounds, authority generation/lifetime, request identity, and reconciliation behavior. |
| **Python SDK** | An out-of-process Service Contract projection for selected Python Server/TUI/CLI Module rows. Its role/family support and delivery status are recorded per exact row. |
| **TypeScript SDK** | An out-of-process Service Contract projection selected for the Web Client Module row. It grants no Server/TUI/CLI binding. |
| **BindingScope** | Owner-selected binding scope: `Selected`, `Excluded`, or `Unselected`. It is independent of delivery and admission status. |
| **DeliveryGate** | Release obligation for a selected binding: `RequiredV0.16` or `TargetNonBlocking`. |
| **AdmissionStatus** | Evidence-backed state `PlannedMissing`, `Experimental`, `Supported`, or exact-environment `Qualified`. `Qualified` entails `Supported`; no status is inferred from BindingScope or DeliveryGate. |
| **Capability Negotiation** | The explicit exchange selecting contract identity/version, authority, operations, bounds, and optional behavior before lookup, attachment, or mutation. |

## Execution, performance, and observability

| Term | Definition |
| --- | --- |
| **ExecutionFacility** | An Assembly-selected, Runtime-owned private bounded execution mechanism. Kernel and Client Runtime instances own separate facilities. It is not a Host Capability, public SDK executor, or Buffer State Lane. |
| **Progress Point** | A contract-declared semantic milestone for one pending obligation, such as admission, Transaction terminal, version publication, Projection/Update terminal, Client application, frame submission, presentation handoff, cancellation terminal, or shutdown phase. Heartbeats, worker activity, and elapsed time are not semantic progress unless the owning contract explicitly says so. |
| **Scenario** | An executable versioned workflow declared before execution with exactly one evidence class, exact artifacts/environment, Dataset Manifest, product/Client/SDK/adapter topology, operation and fault mix, correctness oracle, sampling, metrics, evidence profile, and invalidators. Real Product and Qualification Product Scenarios alone support product/support and exact-release claims respectively; replay, calibration, fault, and mechanism classes have narrower evidence scope. |
| **Dataset Manifest** | The immutable identity and provenance of every captured-real, representative-generated, adversarial-generated, or synthetic-model input used by a Scenario, including upstream acquisition/parents, population/sampling/exclusions, collection or generator/version/seed, size/shape, digests and transformation implementation/configuration, license/consent/privacy/access/secret screening/retention, validation/reconstruction, representativeness, bias, and limitations. |
| **Experiment Bundle** | The content-addressed raw evidence for one Scenario execution: resolved Scenario/Dataset/configuration/Experiment Protocol manifests, commands/tools/checksums, all attempts/samples, setup/environment/cleanup acceptance, correctness, collector and measurement-capability records, per-attempt Interaction Trace Status and zero or more referenced trace manifests/segments/gaps, transformation/redaction ledger, analysis, summaries, claim mapping, and bundle index digest. |
| **Experiment Protocol** | The immutable pre-execution hypothesis, claim/baseline/budget role, treatment/control/calibration matrix, commands/tools/output schemas, attempt/sample/order/randomization, fault schedule, stop/statistical/rerun/environment/exclusion rules, and result-contribution policy retained by digest in an Experiment Bundle. |
| **Performance Authority** | The named authority for a claim-to-workload matrix and evidence-backed numeric budget. It cannot waive correctness or reuse invalidated evidence. |
| **Measurement Capability Record** | The versioned declaration for one measurement source: metric semantics/unit, observer and charge owner, collector/tool identity/configuration, method, scope, clock domain, accuracy/resolution/uncertainty/calibration, admission/sampling/completeness/loss, privileges, overhead profile, blind spots, supported claim classes, provenance, and exact unavailable or valid-observed-zero semantics. |
| **Causal Interaction Trace** | Experiment-owned bounded loss-aware cause-to-effect DAG from admitted input/command through editing, modules/adapters, Client render/output, and terminal presentation. It is separate from the Event Stream and reverse-indexed Causal Debug Graph. |
| **Interaction Trace Manifest** | The versioned capture contract retained with a Causal Interaction Trace: root-sampling and propagation policy, mandatory/optional nodes/edges/terminals, capture bounds, process segments, clock domains/calibration, collector identities, authority/privacy/redaction, transformations, digests, gaps, and completeness rules. |
| **Interaction Trace Status** | Per-attempt `Captured`, `Disabled`, or `NotApplicable` state. A non-captured state retains its predeclared reason, claim limitation, endpoint/root and terminal timing identities, and supporting Measurement Capability Record; it grants no stage attribution. |
| **MonotonicClock** | Initial Host Capability for process-elapsed order and duration without wall-time meaning, with standard and deterministic adapters. |
| **DiagnosticOutput** | Initial bounded Host Capability accepting already-redacted canonical diagnostic bytes and returning accepted, dropped, or closed. |
| **Input-to-present** | Interval measured by one Client clock authority from admission of raw device input through mapping, Intent, Kernel work, Update, layout/render, and Presentation Adapter handoff. |
| **Command-to-present** | Separate interval from admission of an already semantic command or Intent to presentation handoff. It does not include device mapping. |
| **Command-to-output** | CLI interval from admission of an already semantic command to the declared terminal handoff of its bounded text, byte, or structured output. It does not imply a View, frame, or physical display. |
| **Workload Record** | Reproducible input, Resource shape, operation mix, environment, build, sample method, and accounting boundaries for a measurement. |
| **Evidence Artifact** | Retained workload, environment, samples, errors, and provenance sufficient to evaluate one conformance or performance claim. |
| **Editor Event Stream** | The single structured operational diagnostic stream. It admits immutable versioned records, assigns total sequence within an incarnation, redacts before fan-out, bounds memory/queues, and makes loss visible. Raw measurement traces and causal graphs are separate evidence. |
| **Event Stream Incarnation** | Opaque identity of one Editor Event Stream lifetime. Events, cursors, gaps, snapshots, and high-water marks are scoped to it. |
| **Event Record** | One immutable structured item with stable family/version meaning, stamped provenance, Stream incarnation/sequence, severity, and optional correlation. |
| **Correlation ID** | Identifier propagated across records and causal nodes for one logical operation. It does not replace stream sequence. |
| **Event Ring** | Editor-owned oldest-first bounded in-memory retention of canonical Event Records and rendering. Its exact resident-byte limit belongs to the Event contract, not this vocabulary. |
| **Event Gap** | Explicit unavailable admitted-sequence range caused by retention or consumer loss. Pre-admission drops have no sequence and use separate counters. |
| **Canonical Rendering** | The one contract-defined human-readable byte rendering of an Event Record, reused byte-identically by every supported operational text sink. |
| **Causal Debug Graph** | A bounded, versioned, loss-aware graph optimized for reverse traversal from an event or state to related inputs, work, locks/queues, changes, rendering, presentation, and failures. It is not the Editor Event Stream. |

## Conformance and migration

| Term | Definition |
| --- | --- |
| **Conformance Rule** | A stable testable obligation derived from a Target Contract with an owner and verification class. |
| **Donor** | Historical code, fixture, or behavior consulted during selective migration. It supplies evidence, not architectural authority. |
| **Selective Migration** | Per-slice adopt/adapt/rewrite/retire treatment followed by public-boundary, integration, fault, cancellation, performance, and Event evidence. |
| **Vertical Proof** | End-to-end demonstration that accepted contracts compose through a real Resource, Buffer, multiple Representations, editing, publication, Host variants, measurements, and Events. |
| **Transactional Readiness** | Lifecycle discipline in which inspectable child predicates pass before one atomic Ready publication; failure fences and rolls back with dependants before dependencies; shutdown follows reverse readiness order. |
| **Feature Admission** | Decision allowing an implementation into a release candidate after contract, public-path, bound, lifecycle/failure, diagnostic, measurement, and qualification-plan gates pass. It is not Release Qualification. |
| **Release Qualification** | Evidence that the exact packaged candidate passed every mapped conformance test and workload in each specifically claimed environment. It implies no untested scope or certification. |

See [Concept](00-Concept.md) and the
[Architecture contract map](01-Architecture/README.md).
