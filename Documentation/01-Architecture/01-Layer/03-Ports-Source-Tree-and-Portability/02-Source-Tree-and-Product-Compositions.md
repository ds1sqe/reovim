# Source Tree and Product Compositions

[Ports, source tree, and portability](../03-Ports-Source-Tree-and-Portability.md)

## Placement law

Canonical top-level roots expose logical ownership. The exact Source Role Map,
package metadata, imports, public types, and build graph remain dependency
truth. A path name never grants an edge or merges two roles.

```text
kernel/          Foundation, Kernel Interface, Core, and Runtime
host/            Standard and deterministic Host Adapters
resource/        Resource models and concrete/synthetic adapters
domain/          concrete Domain policy
representation/  concrete Representation policy
sdk/             Rust, language-neutral service, Python, and TypeScript
clients/         common contract carriers plus TUI, CLI, and Web families
protocol/        in-memory oracle and selected Protocol Adapters
presentation/    deterministic and production Presentation Adapters
extensions/      public-only Server and Family Client Modules/adapters
apps/            Assembly roots and product lifecycle triggers
tests/           Product-owned public consumers and composed journeys
tools/           Repository Tooling only
```

No package is created to make the tree symmetric. A package requires one owner,
an immediate journey consumer, an inward dependency set, and a boundary needed
for independent dependency, lifecycle, trust, target, or release treatment.
Otherwise source remains in the smallest conforming owned package.

## Kernel placement

The Kernel root uses these roles in dependency order:

```text
kernel/runtime -> kernel/core -> kernel/interface -> kernel/foundation
                         \----> kernel/interface
```

`kernel/foundation` owns portable language primitives.
`kernel/interface` owns public contract families.
`kernel/core` owns authoritative editing mechanisms.
`kernel/runtime` owns Kernel-side admission, execution, sessions, lifecycle,
fencing, Events, supervision, and shutdown.

Kernel packages contain no concrete Domain, Resource/Host/Protocol/
Presentation/Extension Adapter, Client, renderer, environment, or Assembly
implementation.

## Server product placement

There is no architectural `Server Core` or second server state layer. “Server”
names product placement and the Server Module role:

- `apps/server` is an Assembly root and root lifecycle trigger;
- Kernel Runtime is the server-side semantic execution owner;
- `extensions/server` contains public-only Server Modules; and
- a selected server-side Protocol endpoint is an injected adapter/binding.

`apps/server` may select Kernel Runtime/Core, Domain/Representation policies,
Resource and Host Adapters, Server Modules, public SDKs, and a Protocol endpoint.
It constructs an inert graph and then triggers the Kernel Lifecycle Owner. It
does not own another Buffer, session, registry, readiness, or shutdown truth.

## Client placement

Each mandatory Family under `clients/` has logically distinct Contract, Core,
Runtime, and Module surfaces. Physical package splitting follows a demonstrated
dependency or release boundary rather than directory symmetry.

The optional `clients/common` root contains only common contract carriers,
generated schema values, fixtures, and neutral helpers. It is not a global
Common Client Core and owns no Client lifecycle, state lane, View, or
presentation state.

Client source dependencies point inward:

```text
Client Runtime -> Family Core -> common/Family/SDK contracts
       |               |
       |               +-> family policy libraries
       +-> exact Host and Presentation ports consumed
```

Every Client Runtime receives concrete endpoints and adapters from its Assembly
without importing implementation packages. TUI, CLI, and Web retain separate
owners and evidence.

## Adapter and policy placement

- `host/standard` contains `std`-backed Host Adapter mechanisms.
- `host/test` contains Product-owned Test adapters and private controllers.
- `resource/model-*` owns Resource-native coordinate and value models.
- concrete and synthetic Resource Adapters stay under `resource/`.
- Domain and Representation policy stay under their named roots.
- `protocol/in-memory` is the deterministic semantic service oracle;
  production adapter names are selected only after review.
- `presentation/test` contains deterministic Family Presentation adapters;
  target mechanisms use explicit names such as terminal or web.
- first-party extensions depend only on supported public SDK/Module/Family
  contracts and never on private Kernel or Client packages.

Grouping by top-level role does not merge Resource, Host, Protocol,
Presentation, External Protocol Binding, Extension Adapter, or Engine Adapter
semantics.

## Application compositions

`apps/server`, `apps/tui`, `apps/cli`, and `apps/web` are independently
buildable product Assembly roots. `apps/reovim` may compose Server and one
Client in one process, but it constructs the same independent Runtime owners,
Contract Sessions, grants, generations, and shutdown transactions used by
separate products.

An embedded composition cannot call a private Buffer or Client state path.
Direct and transported endpoints implement the same public semantic contracts.

A custom-host Assembly may live outside this repository. Its source is not
silently classified as a Reovim product, and its support claim follows the
[portable Host profile](03-Portable-Host-and-No-Std-Profiles.md).

## Tests and tooling

Product-owned contract consumers, deterministic adapters, examples, and
integration journeys live under `tests/` or the owning Product package and
retain their Product role. They are not Repository Tooling.

`tools/` and conformance orchestration contain no Product source or shipping
asset. Tooling may observe exact Product inputs/artifacts only through admitted
evidence boundaries.
