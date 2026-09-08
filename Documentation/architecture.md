# Architecture

The Kernel provides editing mechanisms, not editing policy or presentation.
A contract specifies responsibility and observable behavior; it does not
require an extra package, generic message bus, or wrapper around every value.

## Owners and source tree

```text
kernel/runtime -> kernel/core -> kernel/interface -> kernel/foundation
                         \----> kernel/interface
```

| Owner | Provides | Does not own |
| --- | --- | --- |
| Foundation | Core-only identities, versions, bounds | Allocators or policy |
| Interface | Resource, editing, Host and interaction contracts | Adapters |
| Core | Atomic Buffer transitions and History | Domain or Client policy |
| Runtime | Admission, sessions, lifecycle and scheduling | Concrete modules |
| Resource | Resource-native models and access adapters | Editing authority |
| Domain | Meaning and validation | Storage mutation |
| Representation | Interpretation and lowering | Concrete viewports |
| Client | Input, keymaps, views and presentation policy | Buffer mutation |

A Resource is an addressable subject. A Buffer is transactional working state
over one Resource in one editing context. A Representation interprets that
state; a Projection maintains stamped correspondence and updates; a View
presents it. History belongs to the Buffer, not a Representation or View.

Place source by its owner:

```text
kernel/          foundation, interface, core, runtime
host/            standard and deterministic Host adapters
resource/        native models and concrete access adapters
domain/          meaning and edit validation
representation/  interpretation, lowering and correspondence
sdk/             public Rust and language-neutral service projections
clients/         family contracts, core and runtime
protocol/        semantic binding, codec and transport adapters
presentation/    test and production presentation adapters
extensions/      server and client modules using public APIs
apps/            assembly roots and lifecycle triggers
tests/           public consumers and composed journeys
scripts/         contributor checks, never product dependencies
```

Create a package only for an immediate consumer and a genuine dependency,
lifecycle, trust, target or release boundary. Logical roles do not require
symmetric directories or empty packages.

## Dependency rules

Dependencies point toward contracts and inward owners. Assembly selects and
constructs concrete policies/adapters, then hands them to separate Kernel and
Client lifecycle owners. Runtime code does not import those implementations.
apps/server composes Kernel Runtime; it is not another editing-state owner.

Domains and representations depend on their semantic contracts. Resource
adapters depend on Resource contracts and exact consumed Host capabilities.
Clients depend on public interaction/SDK/family contracts, never private
Kernel state. Presentation and protocol adapters depend on the ports they
implement. First-party extensions use the same public APIs as third parties.

Foundation, Interface, Core and Runtime contain no concrete resource,
environment, domain, transport, client, renderer or extension implementation.
They do not assume strings, lines, text cursors, terminal cells, pixels,
file paths, byte addressing, random access, mutation or whole-resource copies.
Native addresses, units and semantic values carry explicit schema and state
identity; their meaning belongs to their defining resource/domain contract.

Within one owner, use ordinary private types and coherent breaking changes.
Across owners, define a small typed interface; do not leak private state,
locks, executor tasks or native handles. No hidden globals, service locator,
cyclic callbacks or universal platform object.

## Contract shape

For every used interface, name provider, consumer and mutation authority;
requests/results; capabilities and limits; identity/revision fences; errors;
cancellation and shutdown; observation; and one public behavior example.
Operation names in these documents describe semantics, not an implemented
function list. Rust API documentation defines concrete public projections.

Validate permission and structural bounds before lookup or policy work.
Return explicit unsupported, denied, stale, conflict, pressure, cancelled,
closed, timeout and failure outcomes where applicable. Never erase whether
an external mutation happened when simplifying an error.

Nonlocal exposure fails closed until authentication, authorization, a threat
model, limits and shutdown behavior are defined and tested. Local placement
alone is not proof of trust; a transport never grants editing authority.

Use a complete interface chain for one accepted journey:

```text
Client command -> Interaction -> Domain validation -> Representation lowering
               -> Buffer transaction/History -> semantic update -> Client
```

Open and publication go through Resource ports. Boot and close go through
separate lifecycle owners. A preview uses the resulting state; it cannot be
a second writer. The first execution proof covers boot, attach, open, edit,
observe, undo/redo, detach and shutdown, including failures.

Agree the full journey before inner mechanisms, compile public providers and
consumers for every boundary it uses, then execute it. Later-only operations
need their public chain before their own mechanisms, not before this edit.
A scripted provider proves interface use, not an executing editor.

## State and work

Core serializes each Buffer transition at one atomic commit point. Validation
and complete lowering occur outside commit ownership; recheck the Buffer
incarnation, version and preconditions at commit. Publish state, History and
invalidation coherently or leave all unchanged.

A Resource-native State Model provides the Interface-owned preparation port.
Runtime supplies immutable observed working state, lowered operations and
bounded Resource observations. The model evaluates native operations outside
commit ownership and returns an immutable prepared state plus History and
precondition evidence, or rejection. This is pure preparation, not mutation
of the authoritative Buffer. It need not copy the whole Resource.

Core validates the candidate's exact target, model/schema, bounds and fences,
then installs its prepared state and History relation atomically. Undo/redo
uses retained state/reconstruction evidence through the same preparation cut.
Core does not decode native operations or invoke a model at commit. Retired
state is released outside commit ownership so destruction cannot call policy
or introduce unbounded work under that authority.

No I/O, waiting, allocation that can fail after commit, domain callback,
extension, event fan-out or Client code runs under Buffer commit ownership.
Clients separately serialize their own state and fence stale computations.
A stale result never installs merely because its numeric revision is newer.

Each Runtime owns its private bounded execution facility. Work has a named
owner, limits, cancellation and a terminal outcome; it is not detached.
Background work computes from owned or immutable stamped data, then returns
a proposal for the owning state boundary to revalidate and install.
A module never receives a general executor or private state handle.

## Host and portability

Foundation uses core only. The portable Kernel, selected pure policies,
public Rust contracts and applicable Rust Client path use no_std + alloc.
Hosted adapters, process supervisors and assembly may use std.
Compilation for a target proves neither runtime support nor qualification.

Kernel, Client and Resource consumers receive separate grants from the same
Host contract family. The initial capabilities are MonotonicClock and
DiagnosticOutput. Resource access, presentation, transport and execution
remain distinct responsibilities, even when they use the same environment.

MonotonicClock returns an instant in an explicit clock incarnation or a typed
unavailable/stale/closed result. Instants from unrelated clocks cannot be
subtracted. DiagnosticOutput accepts bounded canonical records with explicit
pressure, partial-write and shutdown outcomes; it never renders them again.

Add a Host capability only with a named immediate consumer, typed limits and
errors, standard and deterministic adapters, shared behavior tests, lifecycle
and cancellation, structured observations and measured cost. No fallback to
ambient std calls inside portable code when a capability is unavailable.

Third-party dependencies need a concrete use and review of maintenance,
license, transitive/unsafe cost, targets, runtime behavior and overhead.
Abstract them only for Reovim semantics, deterministic substitution, or
containment of a volatile/unsafe boundary.

See [state](interfaces/resources-and-state.md),
[lifecycle](interfaces/lifecycle-and-modules.md), and
[debugging](debugging.md) for the operation and failure contracts.
