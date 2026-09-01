# Reovim v0.16 Normative Documentation

**Status:** Ratified target contract for the v0.16 editing-system
re-foundation.

## Authority

`Documentation/` is the normative authority for Reovim product goals,
terminology, architectural ownership, public contracts, conformance, migration,
and release acceptance. Present-tense normative language defines the target; it
does not claim that current code implements it or that a release is supported
or qualified.

Implementation status requires committed code plus matching public-boundary
tests and evidence. Release qualification additionally requires the exact
packaged candidate to pass its mapped clean-environment suite. Plans own order
and progress, flight logs own chronology, trackers own public work state, and
Experiment Bundles own results. None may redefine this specification.

## Product direction

Reovim is a high-performance, extensible editing system pursuing mission-
critical quality. It inspects and modifies addressable Resources through
faithful low-level and Domain-aware high-level Representations while preserving
one authoritative Buffer and History per editing context. Text is one Domain,
not the shape of the core.

The **Reovim Kernel** is the portable, Domain-neutral execution and lowering
substrate. The limited JVM-runtime/LLVM analogy describes its role: it supplies
common contracts, lifecycle, state-transition, lowering, isolation, and
observation mechanisms over concrete components. It is not an operating-system
kernel and implies no bytecode, JIT, compiler pass framework, garbage collector,
machine-code generator, hardware abstraction, or platform-services product.

Clients own their applicable presentation or output state. Any concrete View,
viewport, layout, focus, input mapping, drawing, rendering, or frame state that
exists is Client-owned. The Kernel knows what semantic state changed and which
revision/generation it describes; it never knows how a concrete viewport draws
it.

## Release split

- **v0.16 — editing-system re-foundation:** establish and prove the neutral
  Kernel, Resource/Buffer/semantic/Client contracts, Host API, SDK/extension and
  adapter boundaries, execution/lifecycle, empirical performance, bounded
  diagnostics, and exact qualification machinery.
- **v0.17 — Neovim feature parity:** implement parity behavior only through
  accepted v0.16 contracts. Parity does not automatically select Neovim plugin,
  Lua, MessagePack-RPC, or remote-UI compatibility.

v0.16 ships TUI, CLI, and Web as mandatory Client Families. Each owns a Family
Core, public Contract/extension point, Module surface, Client Runtime, and at
least one admitted production Presentation path. A missing or failing Family
blocks release; Android and Native remain future.

The official Module binding matrix is:

| SDK | Server | TUI | CLI | Web |
| --- | --- | --- | --- | --- |
| Rust | Required v0.16 | Required v0.16 | Required v0.16 | Excluded |
| Python | Required v0.16 | Non-blocking target | Non-blocking target | Excluded |
| TypeScript | Excluded | Excluded | Excluded | Required v0.16 |

On selection, each of the seven rows enters the registry with
`PlannedMissing` AdmissionStatus and may advance only through its own evidence
gate; Excluded cells have no AdmissionStatus. This initial-state law is not a
live implementation record. The implementation/admission order is Rust first,
then the service/transport reference, then Python and TypeScript independently.
Language-neutral contracts remain semantic authority.

Easy extension is a product acceptance concern. v0.16 defines a language-
neutral Extension Adapter, an Extension Engine Adapter specialization, an
Adapter Kit, a TestOnly model, and Quick Extension rules. It selects no
production interpreter/VM/engine, embedded Lua/Python, Haskell binding, native/
FFI profile, separate automation role, or Neovim compatibility facade.

## Architectural invariants

1. Foundation, Kernel Interface, Kernel Core, and Kernel Runtime contain no
   concrete OS/environment, Resource adapter, Domain policy, transport,
   presentation, Client, or extension implementation.
2. Strings, lines, columns, graphemes, text cursors, terminal cells, pixels,
   and another Domain's coordinates never shape Domain-neutral contracts.
3. Resource Identity, Address, Extent, Unit, Revision, capabilities, limits,
   and errors remain explicit; byte offsets are not universal.
4. A Resource Adapter connects data. A Domain supplies meaning. A
   Representation exposes interpreted state. A Projection synchronizes it. A
   Client-owned View presents it.
5. A Buffer is transactional working state for exactly one Resource and one
   editing context; it is not a string or mandatory full copy.
6. Views, Clients, and extensions submit public Intents/Transactions and never
   mutate storage or private Buffer state directly.
7. Assembly selects concrete components and builds inert ownership graphs.
   Lifecycle owners—not Assembly—activate, publish Ready, supervise, roll back,
   and close their subtrees.
8. Environment services cross small consumer-driven Host Capabilities.
   Resource, Host, Protocol, Presentation, Extension Adapter, and Extension
   Engine Adapter roles remain distinct even when one consumes another.
9. Every queue, payload, retained state, concurrency group, retry, callback,
   drain, and shutdown wait is bounded with visible exhaustion.
10. Performance claims use real Scenarios and retained Experiment Bundles.
    Input-to-present p99 is the primary reaction metric.
11. Editor diagnostics use one bounded, loss-visible structured Event Stream;
    causal traces/graphs and snapshots remain separate bounded evidence.
12. First-party Clients/extensions use the same public contracts offered to
    equivalent third parties; private reduction shortcuts are forbidden.

## Dependency policy

The Rust standard library is allowed. A third-party dependency is admitted only
for a concrete consumer after review of maintenance, ownership, license and
compatibility rationale, transitive graph/cost, unsafe/native surface, build and
runtime behavior, target support, and measured overhead. Reovim adds an
abstraction when it owns different semantics, needs deterministic substitution,
or must contain a volatile/unsafe boundary—not merely to hide a package name.

## Document map

1. [Concept](00-Concept.md) — routed North Star, Kernel/Client split, quality,
   and v0.16 proof.
2. [Vocabulary](00-Vocabulary.md) — canonical cross-contract terms.
3. [Architecture](01-Architecture/README.md) — role/dependency, Resource and
   Buffer, semantics and View, execution, and Client contract map.
4. [Host API](02-Host-API.md) — semantic environment capabilities and adapters.
5. [Extension contracts](03-Extensions/README.md) — SDK/API, Server/Client
   Modules, Contribution Points, binding matrix, and Extension Adapters.
6. [Performance](04-Performance/README.md) — real Scenarios, causal trace, IPC,
   instrumentation, locks, and evidence.
7. [Editor Event Stream](05-Editor-Event-Stream/README.md) — bounded logging,
   canonical rendering, loss, and causal-debug separation.
8. [Selective migration](05-Migration/01-Selective-Migration.md) — donor
   classification and slice cutover.
9. [Conformance](06-Conformance/README.md) — evidence taxonomy, authority,
   stable Rule Matrix, and release audit.
10. [Mission-critical quality](07-Mission-Critical-Quality/README.md) —
    integrity, lifecycle/isolation, admission, and exact qualification.

`heritage/` is non-normative historical material. Git history preserves the
superseded OS/no-std/system-kernel specification; removed chapters do not retain
authority merely because historical code still exists.

## Reading and change rules

Read Concept, Vocabulary, and Layer Model before changing an architectural
boundary. Then read the owning contract and its Rule Matrix rows. A boundary
change updates the contract, conformance mapping, implementation, tests,
migration evidence, and performance/Event evidence together where applicable.

Contract families use short indexes and focused chapters. Every active
normative Markdown file stays at or below 250 lines; split by one owning subject
before exceeding that bound. Link to the focused file rather than a fragile
local heading fragment, and never retain an old monolith as a redirecting second
authority.

Do not describe a planned contract as implemented. Do not put branch hashes,
issue order, live progress, measured results, concrete dependency admissions,
or waivers in normative Documentation. Do not defer a missing proof by leaving
a permanent compatibility shim; record a bounded owner/removal milestone in the
tracker and remove the duplicate path after the conforming path is proved.
