# First Visible Edit Interface Chain

[Interaction, Updates, Protocols, and View](../02-Interaction-Updates-Protocols-and-View.md)

## Scope

This chapter specializes the general Inter-Layer Contract law for the
[first visible edit](06-First-Visible-Edit-Oracle.md). It fixes every boundary
that must type-check before an inner editing mechanism begins.

Public contracts expose semantic authority and observable outcomes. They do
not expose a lock, queue, executor, task, state-lane guard, allocator, codec,
socket, concrete adapter, View, or presentation handle. Intra-tree helper
interfaces remain private and unstable.

## Contract and stamp vocabulary

Every public family has an authority-assigned contract identity, selected
version, compatible range, and carrier schema versions. Role identities are
distinct typed stamps even when their scalar representation is shared.

The first chain uses exact stamps for:

- Resource, Resource Access, Revision, and Snapshot;
- Buffer, Buffer incarnation, version, Transaction, and History transition;
- Domain, Representation, Projection generation, and coordinate space;
- Client, Contract Session, authority generation, request, cancellation, and
  root correlation;
- TUI Family, Module, View, layout, render, and frame generations; and
- Rendering Capability, Presentation Adapter, target, and handoff.

An opaque coordinate, value, Address, Extent, Unit, Operation, or Precondition
carrier always names its type and schema, ownership/lifetime contract, exact
byte and structural bounds, and charge owner. An unqualified integer, string,
slice, or serialized object grants no semantic authority.

## Resource Access port

Kernel Interface owns this public port. One synthetic Resource Adapter
provides it; Kernel Runtime consumes it for the selected Access incarnation.

The port declares descriptor inspection, provisional open, initial Snapshot,
bounded observation, freshness, invalidation/change delivery when selected,
cancellation, and close. The first fixture selects stable observe-only byte
access, but byte Address and Unit schemas remain adapter-qualified values.

Open publishes nothing until authority, compatibility, bounds, Resource
Identity, Revision, initial Snapshot, and applicable change checkpoint are one
coherent Ready result. Failures distinguish denied, unsupported, invalid,
stale, unavailable, indeterminate, pressure, cancelled, replaced, closing,
closed, and adapter failure.

## Domain and Representation policy ports

Kernel Interface owns both policy ports. A public Text policy provides them;
Kernel Runtime invokes them outside Buffer State Lane authority.

The Domain operation validates one bounded schema-qualified semantic Intent.
The Representation operation validates its location and correspondence, then
returns one complete Transaction plan containing Buffer-valid Operations and
Preconditions. A valid plan retains origin and all observed Resource, Buffer,
Representation, and Projection stamps.

Projection computation observes immutable stamped inputs and returns a bounded
semantic Snapshot or predecessor-bound delta plus correspondence and affected
source dependencies. Policy receives no mutation authority, private Buffer
borrow, Resource/Host adapter, Client state, transport endpoint, or executor.

Policy invocation may call admitted module code and therefore has explicit
callback, panic-containment, work, deadline, cancellation, concurrency,
temporary-byte, result-byte, replacement, and shutdown terms. Invalid semantic
data, no correspondence, ambiguous mapping, stale state, unsupported schema,
pressure, cancellation, and failure remain distinct.

## Buffer Transition port

Kernel Core owns this private unstable port; Kernel Runtime is its only
consumer. Domain and Representation policy never call it.

The transition accepts one already validated and fully reserved Transaction
plan. Under exact Buffer and Resource stamps it revalidates Operations and
Preconditions, then either rejects without state change or atomically advances
Buffer version and History and records Projection invalidation.

The operation is synchronous, bounded, non-suspending, non-reentrant, and
callback-free. It performs no adapter, Host, Protocol, Event-consumer, Client,
module, or presentation work under Buffer State Lane authority. Conflict,
stale state, pressure, closing, and invariant failure have typed results.

## Interaction Session port

Kernel Interface owns the common Client semantic port. Kernel Runtime provides
the direct endpoint; TUI Client Runtime consumes it. A later transport binds
the same semantics without changing them.

The descriptor declares negotiation, grants, readiness, pending request and
payload bounds, terminal retention, Update-stream retention, credits, fairness,
cancellation, reconciliation, replacement, and close. Its public operations
are:

1. negotiate one compatible Session and authority generation;
2. attach Representation delivery and obtain initial Snapshot/checkpoint;
3. submit a bounded Intent without waiting for policy or Buffer work;
4. receive exactly one terminal Outcome for each admitted request;
5. receive ordered predecessor-bound Representation Updates;
6. cancel or reconcile one exact request;
7. report a Gap and request a bounded full resynchronization; and
8. close admission and reach a bounded terminal state.

Submission distinguishes immediate definite non-admission from accepted
ownership. Deferred completion never makes a missing response success. A
terminal Outcome carries local effect certainty, cause, result stamps, and any
reconcile identity. Update delivery has its own stream incarnation and cursor.

Duplicate and older Updates cannot install. Missing ancestry stops delta
application and requires resynchronization. Slow-Client pressure cannot block
Buffer acceptance or unrelated Clients. Close exposes unresolved ownership
rather than dropping it.

## TUI Family and Module ports

The public TUI Family Contract owns normalized TUI input contributions,
Client-local View and cell contribution schemas, Rendering Capability
requirements, and its Module Contribution Points. TUI Family Core provides
the family behavior; TUI Client Runtime owns attachment and lifecycle.

A TUI Client Module may contribute bounded input mapping, local mode, View,
layout, decoration, cell render, or output policy. It depends only on common
Client semantics, the TUI Family Contract, and admitted Rust SDK surfaces. It
cannot import Family Core internals or submit a private edit.

Input mapping consumes Presentation-originated observations and current
Client-local state. It emits either local state effects or a public semantic
Intent. Presentation coordinates are mapped to a qualified stamped Semantic
Location before submission. Raw keys, device codes, and local modes do not
cross the Interaction Session.

## Presentation ports

The TUI Family Contract separately owns input-observation and presentation-
handoff ports because they have independent pressure and lifecycle.
The deterministic Presentation Adapter provides both to TUI Runtime/Family
Core and receives no editing authority.

The input port reports bounded ordered observations, pressure/loss, target
replacement, and close. Concrete controller and device values remain adapter-
private until normalized by TUI policy.

The handoff port accepts a generation-stamped cell contribution only after the
cell Rendering Capability is negotiated. It reports accepted completion,
pressure, supersession, capability or target loss, cancellation, closing,
closed, and failure. Every request binds capability, adapter, target, Client,
View, layout, render, and frame generations.

Rendering is an explicit cell capability for this fixture, not a property of
the TUI name. Pixel, raster, image, video, or GPU paths require independent
descriptors, ports, and evidence.

## Rust SDK and Server Module port

The Rust SDK projects deliberately exported Kernel and Client contracts. The
first public Text Server Module declares its Domain/Representation
Contribution Points, compatible schemas, grants, bounds, callback behavior,
failure containment, readiness, replacement, and close.

Kernel Runtime owns installation and invocation of the contribution. The
module receives no Core or Runtime state and returns complete public policy
results. The corresponding public TUI Module uses only its Family Contribution
Points. Both remain Experimental APIs for v0.16.

## Causal and measurement relations

Every boundary accepts or derives an exact causal context without making the
Event Stream state authority. Typed edges record map, send, receive, admit,
lower, accept/reject, fan-out, deliver, install, render, and handoff relations.
They bind before/result generations and never infer cross-process ancestry from
timestamps.

Each operation exposes applicable start, completion, queue, payload, work,
allocation/copy, lock/wait, cancellation, pressure, and loss observation
points. `Unavailable` remains distinct from observed zero. Instrumentation has
bounded non-recursive failure and later paired overhead measurement.

## Public fixture and source order

Before mechanism source, Product-owned external fixtures must compile both
providers and consumers for every public boundary above. They exercise normal,
stale, pressure, cancellation, gap, replacement, failure, and close values and
compose the complete first-edit sequence without private placeholders.

Source proceeds in this order:

1. Kernel Interface Resource, Buffer-plan, semantic, and Interaction contracts;
2. the TUI Family Contract and its public consumer;
3. Rust Server/TUI Module contracts and public consumers;
4. one public-only end-to-end compile fixture;
5. Kernel Core/Runtime and TUI Core/Runtime mechanisms;
6. synthetic Resource, Text policy, deterministic Presentation Adapter, and
   Assembly; and
7. composed correctness, fault, causal, and performance evidence.

No package is created until the current fixture reaches it. A contract-only
package is valid only when its real external provider and consumer compile in
the same slice. First-party code gains no privileged path.
