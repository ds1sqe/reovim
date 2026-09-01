# Normative Rule Matrix

## Authority

This matrix routes stable normative obligations to one owning contract and
closed verification classes defined in
[Evidence Taxonomy and Authority](03-Evidence-Taxonomy-and-Authority.md).
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
| ARCH-001..003 | Any product source role, Repository Tooling owner, public type, build/packaging/invocation edge, or composed adapter boundary is admitted. | Architecture conformance boundary |
| ARCH-004 | Any Assembly-selected Runtime or Client subtree is constructed or activated. | The subtree's Lifecycle Owner |
| ARCH-005 | Any third-party dependency is proposed, admitted, changed, or invalidated. | The dependency's named consumer owner |
| ARCH-006 | Any classification policy, conformance checker, or implementation/release evidence set is produced or admitted. | Conformance Authority |
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


## Matrix chapters

Each identifier/obligation/authority/evidence tuple appears exactly once in
the focused normative matrices:

1. [Product, Authority, and Layering](02-Rule-Matrix/01-Product-Authority-and-Layering.md)
2. [Safety, Bounds, Effects, and Exposure](02-Rule-Matrix/02-Safety-Bounds-Effects-and-Exposure.md)
3. [Resource and Editing State](02-Rule-Matrix/03-Resource-and-Editing-State.md)
4. [Meaning, Interaction, and View](02-Rule-Matrix/04-Meaning-Interaction-and-View.md)
5. [Execution, Lifecycle, and Debugging](02-Rule-Matrix/05-Execution-Lifecycle-and-Debugging.md)
6. [Clients, Synchronization, and Rendering](02-Rule-Matrix/06-Clients-Synchronization-and-Rendering.md)
7. [Host, SDK, and Extensions](02-Rule-Matrix/07-Host-SDK-and-Extensions.md)
8. [Performance, Events, Migration, and Qualification](02-Rule-Matrix/08-Performance-Events-Migration-and-Qualification.md)
