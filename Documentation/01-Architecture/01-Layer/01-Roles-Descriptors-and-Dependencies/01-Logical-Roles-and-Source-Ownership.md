# Logical Roles and Source Ownership

[Roles, Descriptors, and Dependencies](../01-Roles-Descriptors-and-Dependencies.md)

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

Resource Adapter, Host Adapter, External Protocol Binding, Protocol Adapter,
Presentation Adapter, Extension Adapter, and Extension Engine Adapter are
distinct roles. Selection or composition of one never merges it with another.
Host is one Kernel Interface family, Resource is another, and the private
Runtime ExecutionFacility is not a Host Capability.

## Product and non-product source ownership

The Logical roles table is the closed product-role namespace. A Source Role Map
classifies each in-scope path with a separate **Source Owner**:

- `Product(ArchitecturalRole)` names one role from that table; or
- `RepositoryTooling` names the closed non-product owner for repository checks,
  generation, configuration, their declarative policy/evidence inputs, and
  their non-shipping test inputs.

`RepositoryTooling` is not an ArchitecturalRole, Kernel or Client layer, or
Assembly-selectable component. Its dependency and shipping-closure consequences
are owned by the
[Source Dependency Graph and Edges](03-Source-Dependency-Graph-and-Edges.md).

Source Provenance (`Authored`, `Generated`, or `Vendored`) and Source Use
(`Production`, `Test`, `Example`, `Tool`, or `BuildOutput`) are orthogonal to
Source Owner and never change dependency direction. Product-owned test or
generated source remains product-owned. Generated source compiled or bundled
into a product is `Production`, not `BuildOutput`. The Conformance contract owns
the closed classification-policy and evidence schemas and exact use/closure
rules.
