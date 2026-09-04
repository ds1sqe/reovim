# Layer Interface Contracts

[Ports, source tree, and portability](../03-Ports-Source-Tree-and-Portability.md)

## Interface ownership law

An **Inter-Layer Contract** is the only supported interaction boundary between
logical roles. It has exactly one inward Contract Authority. Outward providers
and consumers depend on that contract; data flow in either direction never
reverses the source dependency.

A contract exposes Reovim-owned semantics, not implementation representation.
Public or cross-package types contain no concrete lock, executor, task, queue,
allocator, codec, transport, environment handle, adapter, registry, Runtime
state, Buffer borrow, View, or presentation mechanism unless that exact
contract owns the meaning.

There is no generic service locator, type-erased message bus, `Any` value bag,
unbounded callback surface, or global context between layers. Assembly passes
explicitly selected implementations or inward-owned ports to constructors.

## Internal interface stability

An Intra-Tree Interface wholly inside one owner is private and unstable. Its
owner may change or remove it at any commit without compatibility support when
all internal consumers, behavior, and evidence are updated coherently.

An Intra-Tree Interface is absent from public SDKs, service schemas, Extension
Packages, public examples, cross-package third-party use, and compatibility
claims. Re-export, path access, first-party convenience, or use by a sibling
owner promotes the boundary to an Inter-Layer Contract and requires its full
contract before continued use.

Unstable does not waive safety, bounds, lifecycle, dependency, style, or test
requirements. It removes only external compatibility commitment.

## Complete contract shape

Before an implementing mechanism begins, every used Inter-Layer Contract
defines:

1. owner, provider, consumer, and mutation or installation authority;
2. contract identity/version and compatibility;
3. instance/incarnation, generation, revision, and coordinate stamps;
4. request, stream, result, and exactly applicable terminal outcomes;
5. capabilities, grants, bounds, validation order, and charge ownership;
6. invocation, completion, delivery, effect, callback, and reentrancy classes;
7. pressure, cancellation, stale, gap, reconcile, and failure behavior;
8. readiness, replacement, rollback, supervision, and shutdown;
9. Event and causal relations plus measurement points; and
10. public provider and consumer conformance fixtures.

Rust traits, enums, owned carriers, and borrows are projections of this shape.
They do not define language-neutral or wire meaning by accident.

## Editing interface chain

The first visible-edit vertical uses the following existing authorities.

### Resource access

Kernel Interface owns Resource observation, Snapshot, change, publication, and
close contracts. A Resource Adapter provides them to Kernel Core/Runtime for
one exact Resource Access Incarnation.

The boundary carries Resource-native Address, Extent, Unit, Revision,
capabilities, bounds, Effect Certainty, and typed outcomes. It contains no path,
file descriptor, byte assumption, text, Host implementation, or View state.

### Domain and Representation policy

Kernel Interface owns semantic policy contracts. Selected Domain and
Representation policies provide validation, lowering, semantic state, and
correspondence operations to Kernel Runtime/Core.

Domain policy validates bounded semantic Intent meaning. Representation policy
then validates its coordinate/correspondence and completely lowers it into
Buffer-valid Operations and Preconditions before Buffer mutation. Neither
policy receives storage authority nor Client, Protocol, Presentation, or
concrete adapter state.

### Buffer transition

Kernel Core owns the private transition boundary consumed only by Kernel
Runtime. It accepts already validated Operations and Preconditions under exact
Buffer and Transaction stamps. It returns atomic acceptance or typed
non-acceptance, resulting version, History relation, and Projection
invalidation.

No Domain, extension, adapter, Event consumer, presentation, protocol, I/O,
wait, or callback executes under Buffer State Lane authority. The transition
boundary is not an SDK surface.

### Client interaction

Kernel Interface owns Contract Session negotiation, Interaction Intent,
terminal Interaction Outcome, Representation Snapshot/Update, gap,
resynchronization, reconciliation, and close semantics.

Kernel Runtime or an External Protocol Binding provides that boundary to a
Client Runtime or public service projection. Every admitted Intent gets one
bounded correlated terminal Outcome. Updates remain independently delivered
and predecessor-bound.

Raw input, key notation, Viewport, cells, pixels, frames, serialization,
sockets, callback closures, and private Buffer handles do not cross it.

### Server and Client Modules

Public Rust SDK and language-neutral Module contracts own Server Module
Contribution Points. Kernel Runtime owns attachment, bounded policy dispatch,
result installation, and lifecycle. A Server Module receives no private Core
or Runtime state, Resource Adapter, View, or transport.

Common Client and exact Family Contracts own Client Module Contribution Points.
Client Runtime owns attachment and lifecycle. Family policy maps input and
semantic state into Client-local View, layout, rendering, or no-View output.
Editing still returns through Interaction Intent.

### Presentation

Each Family Contract owns its Presentation Ports and Rendering Capability
descriptors. Selected Presentation Adapters provide device-input observation
and presentation handoff to Client Runtime/Family Core through separately
bounded ports when their lifecycle or pressure differs.

Concrete terminal, browser, window, GPU, device, and encoded-command types stay
inside the adapter. The adapter receives no editing authority.

### Protocol and service

The language-neutral service/interaction contract owns semantic envelopes,
credits, cancellation, reconciliation, streams, and terminals. A Protocol
Adapter owns codec, framing, connection, carrier I/O, queues, and reconnect.
An External Protocol Binding owns semantic translation when needed.

Direct in-process and production transports implement the same semantic
fixture. Co-location grants no private shortcut.

### Host and execution

Kernel Interface owns each Host Capability. Standard, deterministic, or
external host adapters provide the same contract to exact Kernel, Client, or
Resource consumers. Host applicability is detailed in the
[Host boundary](../../../02-Host-API/01-Boundary-and-Capability-Admission.md).

Kernel Runtime and each Client Runtime separately own their private
ExecutionFacility contract and instance. Assembly may supply an inert
realization through an inward-owned construction interface. No SDK, Module,
adapter, Client, or Host consumer receives a general executor.

### CCM

A Synchronization Scope owns its CCM contract. The selected Synchronization
Authority provides admitted shared semantic or Module state, checkpoints,
deltas, gaps, and resynchronization to joined Clients through distinct Contract
Sessions. Peer Buffer edits, View state, rendered output, and transport
mechanics never cross this boundary.

## Interface-first vertical rule

For one accepted user or author journey:

1. trace the complete provider/consumer interface chain;
2. ratify missing contract semantics and source edges;
3. compile public provider and consumer fixtures for every used boundary;
4. implement the necessary inner mechanisms;
5. implement concrete adapters and Assembly; and
6. run the composed journey and its fault/performance evidence.

Inward dependency direction is not an implementation schedule. A lower package
is not built merely to complete a layer, and an empty future-role package is
forbidden. Every new source package and port must be reachable from the current
journey's composed smoke.

Fixtures that only return success do not accept a contract. Applicable normal,
stale, pressure, cancellation, gap, replacement, failure, and close outcomes
must be observable through public types before the inner mechanism is accepted.
