# Source Dependency Graph and Edges

[Roles, Descriptors, and Dependencies](../01-Roles-Descriptors-and-Dependencies.md)

Product dependencies point inward without cycles. `RepositoryTooling` grants
no product edge and enters no product shipping closure. It is absent from the
product-role DAG and grants no source, invocation, packaging, public-type, or
implementation edge. Product source does not depend on, import, include,
invoke, re-export, expose, or bundle repository tooling. Repository tooling
does not import product crates or private product types; it may consume exact
source or artifact data only through a bounded evidence-input boundary.

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

Foundation and Kernel contracts contain no concrete environment, adapter,
Domain, transport, Client, presentation, or extension mechanism.

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
6. Resource, Host, Protocol, Presentation, and Extension Adapter dependencies
   gain no editing-state authority side door.
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

Repository Tooling may invoke an exact Product compiler/package validation
closure and inspect its source/artifacts only as bounded evidence data. The
tooling implementation never imports, links, re-exports, or packages Product
code; the separately compiled validation consumer remains Product-owned.

Classification and projection are path- and target-aware. One package may
contain several owners only under exact non-overlapping rules; a package-level
label cannot hide a forbidden source edge. Cross-partition product/tooling edges
fail before projection. The product projection contains ArchitecturalRoles only;
Repository Tooling remains outside it while its concrete packages and edges are
still checked for cycles, dependency review, and exact ownership.
