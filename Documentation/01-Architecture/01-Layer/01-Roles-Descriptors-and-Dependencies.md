# Roles, Descriptors, and Dependencies

> **Normative target contract.** This chapter is part of the
> [Architecture contract](../README.md). Present-tense requirements define the
> v0.16 target and do not assert current implementation or qualification.

## Purpose

Reovim is an Editing System built around a portable, Domain-neutral **Reovim
Kernel**. Kernel means an execution and lowering substrate in the limited
managed-runtime/compiler-substrate sense. It is not a boot environment,
hardware abstraction, driver stack, or system-services product.

The Kernel owns stable editing contracts and generic mechanism. Concrete
Resource, Host, Protocol, Extension, and Presentation Adapters; External
Protocol Bindings; concrete Domain policy; Clients; and product selection remain
outside it. Assembly constructs and injects those components through inward-
facing contracts.

Logical roles do not require one package per row. Source ownership, build
metadata, imports, and public types nevertheless make every allowed and
forbidden dependency mechanically checkable.

## Logical roles

| Role | Owns | Allowed inward dependencies |
| --- | --- | --- |
| Foundation | Environment-, transport-, presentation-, and Domain-neutral language primitives and opaque carriers governed by Carrier Safety Descriptors | Language/runtime primitives and reviewed dependencies preserving neutrality |
| Kernel Interface | Identity, revision, address, capability, bound, error, cancellation, lifecycle, Resource, editing, semantic, interaction, Host, extension, Event, and measurement contracts | Foundation |
| Kernel Core | Buffer transition, Transaction/History, conflict, residency, publication/reconcile state, generic correspondence, and Projection invalidation/generation mechanism | Kernel Interface |
| Kernel Runtime | Bounded Kernel work and Contract Session execution, negotiation, readiness, generation fencing, Event admission, rollback, supervision, and ordered shutdown | Kernel Core and Kernel Interface |
| Public SDKs | Registry-listed language projections and language-neutral service schemas | Deliberately exported Kernel/Client contracts and generated carriers, never private Core or Runtime types |
| Concrete policy | Domain and Representation policy selected for a product or extension | Public SDK or the exact semantic Interface family implemented |
| Resource Adapter | Resource-native observation, revision, publication, creation, change, and reconcile mechanism | Resource Interface and only explicitly consumed Host capabilities |
| Host Adapter | Concrete process-environment implementation of named Host Capabilities | Host Interface plus private reviewed mechanism dependencies |
| External Protocol Binding | Semantic translation between one external protocol and public Reovim contracts | Public SDK/service and applicable Resource/Domain/Client contracts |
| Protocol Adapter | Codec, framing, carrier, connection, queue, and reconnect mechanism | Versioned service/interaction contract plus private transport mechanism |
| Extension Adapter | Projection of public SDK/Module/Family contracts into one language toolchain or execution environment | Public extension contracts, generated carriers, and reviewed language mechanism |
| Extension Engine Adapter | Conditional engine-hosting specialization of Extension Adapter | Extension Adapter contract plus one exact reviewed engine/runtime mechanism |
| Client Family | Family Core, public Family Contract, Module surface, applicable View/presentation/output policy, and negotiated Rendering Capabilities | Common Client contracts, public SDK, family policy libraries, and only exact Host Interface contracts it consumes |
| Client Runtime | Contract Session attachment and readiness/rollback/shutdown for one Family Core/Module/View/Presentation subtree | Client Family, public Client contracts, neutral lifecycle/presentation ports, and only exact Host Interface contracts it consumes |
| Presentation Adapter | Target-specific terminal/window/display/raster/GPU/output mechanism | Client-owned Presentation Port plus private target mechanism |
| Assembly | Configuration, exact component selection, inert dependency/ownership graph construction, and root lifecycle trigger | Kernel Runtime, Client Runtimes, selected SDKs, policies, adapters, and product components |

TUI, CLI, and Web are mandatory v0.16 Client Families. Each owns an independent
Core, Contract, Module surface, and production presentation/output path. Android
and Native are future families and create no current support edge.

Official v0.16 Module binding scope and delivery are:

| Language | Server | TUI | CLI | Web |
| --- | --- | --- | --- | --- |
| Rust | `RequiredV0.16` | `RequiredV0.16` | `RequiredV0.16` | Excluded |
| Python | `RequiredV0.16` | `TargetNonBlocking` | `TargetNonBlocking` | Excluded |
| TypeScript | Excluded | Excluded | Excluded | `RequiredV0.16` |

Every newly selected row starts `PlannedMissing`. BindingScope, DeliveryGate, and
AdmissionStatus remain independent. Another language, role, binding form,
placement, or engine requires its own owner scope decision and evidence.

## Descriptor and dependency discipline

Every Foundation opaque-carrier family owns a versioned Carrier Safety
Descriptor. Before construction, copy, transfer, downcast, sharing, drop, or
invocation, it makes ownership/lifetime, type/schema mismatch, affinity and
concurrency, panic/unwind behavior, and the unsafe implementation/audit boundary
inspectable. An undeclared property is unsupported.

Every capability or operation family claiming a bound owns a Bound Descriptor.
It declares units and hard maxima for per-object and aggregate counts, owned
bytes, payload, retained state, concurrency, queues, and completion waits as
applicable. It also declares checked arithmetic, overflow, zero-limit behavior,
accounting ownership, validation order before traversal/allocation, and typed
exhaustion.

A third-party dependency enters or remains in a role only with a retained
Dependency Review Record for its concrete consumer. Package name, popularity,
or a permissive-looking license is not blanket approval.

Host is one Kernel Interface family, not a universal object or service locator.
Resource is another Interface family. A Resource Adapter may consume an
explicitly injected Host Capability without merging the roles. The private
Runtime ExecutionFacility is not a Host Capability.

The Kernel owns Domain, Representation, Projection, and semantic-coordinate
contracts plus generic coordination. It does not own concrete Text, byte, ELF,
QEMU, structured-data, or media policy merely because that policy implements a
Kernel contract.

## Source-dependency graph

~~~text
source dependency: consumer -> contract or inward owner

Assembly -> Kernel Runtime -> Kernel Core -> Kernel Interface -> Foundation
    |                                  \-> Kernel Interface
    +-> Domain/Representation policy -> semantic Interface
    +-> Resource Adapter -> Resource Interface
    |                         \-> exact Host capabilities consumed
    +-> Host Adapter -> Host Interface
    +-> External Protocol Binding -> public semantic/service contracts
    +-> Protocol Adapter -> versioned operation contract
    |                         \-> private carrier mechanism
    +-> Extension Adapter -> public SDK/Module/Family contracts
    +-> Extension Engine Adapter -> Extension Adapter contract
    |                                  \-> reviewed engine mechanism
    +-> Client Runtime -> Client Family -> common/family Client contracts
    |       +-> exact Host capabilities consumed
    +-> Presentation Adapter -> Client Presentation Port
    |                              \-> private target mechanism
    +-> selected public SDK projections
~~~

The picture compresses several Interface families. Every concrete component
depends inward on the contract it implements. Kernel Runtime and Client Runtime
receive concrete instances from Assembly without source dependencies on their
implementation packages.

Runtime observations, Intents, Updates, Outcomes, invalidations, and Events may
flow in either direction only through an inward-owned contract. Runtime data
flow never grants a reverse source dependency.

## Explicitly allowed edges

1. Kernel Interface depends on Foundation only.
2. Kernel Core depends on Kernel Interface.
3. Kernel Runtime depends on Kernel Core and Kernel Interface.
4. A public SDK depends on deliberately exported contract surfaces and
   contract-owned generated carriers.
5. Concrete Domain or Representation policy depends on its public SDK or exact
   semantic Interface contracts.
6. A Resource Adapter depends on Resource contracts and only the Host
   Capability contracts its mechanism actually consumes.
7. A Host Adapter depends on its Host contracts and private reviewed mechanism
   dependencies.
8. An External Protocol Binding depends on public semantic/service contracts.
   Its Protocol Adapter depends on the selected versioned operation contract and
   private codec/carrier mechanism.
9. An Extension Adapter depends on public SDK/Module/Family contracts. An Engine
   Adapter additionally depends on its exact reviewed engine. Neither imports
   private Kernel or Client implementation types.
10. A Client Family depends on common Client contracts, its Family Contract,
    public SDK, family policy libraries, and only exact Host Capability
    contracts it consumes. A Presentation Adapter depends on the Presentation
    Port it implements and private target mechanism.
11. Client Runtime depends on its Client Family and neutral lifecycle and
    presentation contracts plus only exact Host Capability contracts it
    consumes. It receives concrete endpoints/adapters from Assembly without
    importing their packages. Client Runtime owns those capability instances'
    lifecycle within its subtree.
12. Assembly may depend on and construct all components selected for one product
    graph.

## Explicitly forbidden edges

1. Foundation and Kernel Interface do not depend on Core, Runtime, Clients,
   concrete policy, adapters, transports, environment or presentation
   mechanisms, extension engines, or Assembly.
2. Kernel Core does not depend on Runtime, Clients, concrete Domains, adapters,
   transports, extension engines, or Assembly.
3. Kernel Runtime does not import a concrete adapter, Domain policy, Client,
   transport, codec, renderer, extension engine, or environment package.
4. A concrete Domain does not reach through a Resource Adapter, Host Adapter, or
   private Buffer mechanism. It uses public Kernel contracts.
5. Clients and Client Runtimes do not import private Kernel Core/Runtime types,
   mutate Buffer or Resource state directly, or make View/presentation state
   authoritative editing state.
6. Resource and Host Adapter roles do not merge. Protocol, Presentation, and
   Extension Adapters gain no editing-state side door.
7. Backward callbacks use an Interface owned by the inward layer. Dependency
   cycles, hidden globals, and implementation discovery inside the Kernel are
   forbidden.
8. Domain-neutral contracts do not expose strings, lines, columns, grapheme
   positions, terminal cells, presentation pixels, key codes, or another
   Domain's coordinate policy.
9. Resource contracts do not assume byte addressing, stable random access,
   mutability, resize, or change notification.
10. A system-services runtime, bare-environment product path, universal runtime
    floor, or global platform vtable is not a Reovim Kernel layer.

Build metadata is dependency truth. Architecture prose names the law but grants
no edge. Automated checks compare registered packages, transitive closure,
source imports, and public types with the allowed graph. Source-level checks
reject concrete environment, filesystem, socket, protocol/codec, input,
terminal, renderer, device, and extension-engine types inside portable
Foundation and Kernel packages.
