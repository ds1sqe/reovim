# Semantic Descriptors, Stamps, and Intent

[Domains, Representations, and Projections](../01-Domains-Representations-and-Projections.md)

## Scope and authority

This chapter owns the portable values with which Kernel mechanism identifies
and structurally validates semantic edit-policy work. Concrete Domain and
Representation policy owns encoded meaning. Client-owned View and presentation
state never enters these values.

The Rust projection remains `no_std + alloc`. Its public values are
Experimental in v0.16. A Rust type layout, trait object, or in-process type
identity is not the language-neutral contract.

## Identity and stamp chain

Domain, Representation, Projection, coordinate-space, and semantic-request
identities are distinct typed values even when their scalar representation is
the same. A selected contract identity/version is distinct from a live policy
instance or Projection generation.

One **Domain Policy Stamp** binds:

- Domain identity;
- selected Domain contract and compatible range; and
- exact policy instance, incarnation, and lifecycle generation.

One **Representation Policy Stamp** binds its own identity, contract, and
lifecycle generation to exactly one Domain Policy Stamp. One **Projection
Stamp** binds a Projection identity and lifecycle to that exact Representation
plus the predecessor Projection generation observed by the caller.

A **Coordinate Space Stamp** binds its identity, selected contract/version, and
incarnation. Replacement creates another incarnation even when schema choices
are equal. A stale Domain, Representation, Projection, or coordinate-space
stamp is never silently mapped to a current one.

## Coordinate-space descriptor

One Semantic Coordinate Space Descriptor independently selects coordinate,
region, and value schema identities and versions. It also declares canonical
encoding, normalization, origin/axis/Unit meaning, stability/remap behavior,
and its applicable item, nesting, and byte bounds.

Kernel mechanism compares the selections and enforces their structural bounds.
It does not infer equality, ordering, adjacency, range, slicing, or hashing.
Concrete policy implements only operations explicitly selected by the
descriptor and returns typed unsupported behavior for the rest.

## Domain and Representation descriptors

A Domain descriptor names its policy stamp and the exact validation call
contract it supplies. The presence of that port is the selected capability;
there is no universal capability registry or semantic operation inventory.

The first in-process policy call contract fixes execution to a Runtime-owned
worker with any-worker affinity for the selected thread-safe provider. It
grants no outward Runtime, queue, I/O, or service call. Runtime observes
cancellation and deadline state at callback boundaries; bounded-to-completion
provider code does not claim mid-callback interruption.

A Representation descriptor names:

- its policy stamp and exact owning Domain stamp;
- its coordinate-space descriptor;
- its Interaction Intent carrier contract;
- fidelity, editability, and correspondence classes;
- the exact Buffer-plan contract plus origin, Operation, and Precondition
  carrier selections it may produce; and
- its lowering call contract and bounds.

The following combinations fail descriptor construction:

- Derived read-only fidelity with Transactional editability;
- Constrained editable fidelity without Transactional editability;
- Transactional editability without reverse/bidirectional correspondence; and
- Faithful fidelity without source correspondence.

Read-only faithful representations remain valid. A lossy or derived form does
not gain editability merely because a concrete provider can manufacture bytes.

## Semantic Intent owner and stamp

One **Semantic Intent** is an immutable move-only owner of exactly one
schema-qualified encoded carrier. It is transferable with immutable shared
borrowing, releases its allocation on Drop, and exposes no payload through
Debug. Type or version mismatch rejects without reinterpretation.

Its public stamp binds:

- one unique semantic Intent identity;
- caller authority lifecycle and selected caller-role contract;
- semantic request identity;
- exact Domain, Representation, Projection predecessor, and coordinate-space
  stamps;
- exact `BufferStateStamp`, including Resource Snapshot and History state; and
- the future `BufferPlanStamp`, including Transaction, History transition,
  cancellation, correlation, deadline, and plan-owner generation.

The Intent identity identifies one immutable carrier owner. The semantic
request identity groups it under caller authority; neither replaces the
Interaction Session request identity. The later Interaction contract maps
them explicitly and retains every identity whose scope differs.

The encoded payload may contain Text, structured, intrinsic image, register,
or another selected semantic value. Kernel Interface never exposes `Any`,
`TypeId`, downcast, a universal opcode, or a concrete Domain type.

## Bounds and validation order

Intent item count, nesting depth, and encoded bytes use exact Count or Byte
Bound Descriptors. Policy calls separately bound work units, temporary bytes,
result bytes, and concurrent calls. All bounds in one selected policy call name
one explicit charge owner and use checked arithmetic.

Structural validation occurs in this order:

1. caller authority and selected contract families;
2. policy and coordinate lifecycle generations;
3. Resource, Buffer, History, Projection, and plan stamps;
4. carrier type and selected schema version;
5. item/depth/byte units, zero rules, checked arithmetic, and hard limits; and
6. cancellation/deadline state before policy dispatch.

No allocation, traversal, decoding, policy call, Resource lookup, or Buffer
work occurs before its applicable preceding check. The large complete stamp is
move-owned once and borrowed at policy cuts; candidates echo compact typed
identities and proofs rather than copying it. Structural acceptance says
nothing about encoded semantic validity or current Buffer Preconditions.

## Exclusions

These values contain no raw input, key, mode, View, viewport, layout, terminal
cell, target pixel, frame, Protocol carrier, Resource/Host Adapter, storage
handle, lock, task, executor, callback closure, or concrete Buffer borrow.

See [Domain Validation and Representation Lowering](02-Domain-Validation-and-Representation-Lowering.md)
for policy ownership, candidates, and completion cuts.
