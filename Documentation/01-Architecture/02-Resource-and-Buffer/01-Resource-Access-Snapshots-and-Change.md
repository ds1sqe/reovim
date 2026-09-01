# Resource Access, Snapshots, and Change

> **Normative target contract.** This chapter is part of the
> [Architecture contract](../README.md). Present-tense requirements define the
> v0.16 target and do not assert current implementation or qualification.

## Resource and Resource Adapter

A **Resource** is an external or synthetic addressable subject that Reovim can
observe and may be able to change. A **Resource Adapter** translates between
that subject and the Resource contract; it does not interpret content or own
presentation.

Each Resource family defines explicit native types for:

- identity;
- Address and Extent;
- Unit or bounded transfer value;
- Revision; and
- capability, bound, and error outcomes.

These types are never silently reduced to byte offsets, text positions, wall
time, or presentation coordinates.

## Composable capability dimensions

A Resource descriptor negotiates these independent dimensions:

1. **Source access authority:** observe only, or publish selected mutation
   families to the same Resource.
2. **Source stability:** stable under one Resource Identity for its declared
   lifetime, or externally evolving.
3. **Observation and freshness:** Snapshot consistency scope, capture stamps,
   historical availability, freshness, expiry, and indeterminate outcomes.
4. **Mutation grammar:** none, replacement, Add-Only, resize, or another
   Resource-family operation set.
5. **Destination creation authority:** an independent grant over a destination
   family or namespace for create-another.
6. **Change discovery:** none, invalidation notification, or an ordered
   resumable change feed.

No dimension is inferred from another. Different Resource Access Incarnations
may receive different authority over the same Resource. The descriptor rejects
incoherent combinations with a typed outcome.

Three concepts remain distinct:

- **Read-Only Access** means one Access Incarnation cannot mutate the same
  Resource. The subject may still change externally.
- **Stable Resource** means the family guarantees that state cannot change
  under the same Resource Identity for the declared lifetime.
- **Snapshot immutability** means one published observation and its stamps do
  not change. An evolving Resource can produce many immutable Snapshots.

## Adapter descriptor, bounds, and selection

Each Resource Adapter family owns a contract identity/version and descriptor
covering capabilities, side-effect-free applicability, authority requirements,
hard bounds, Snapshot schemas, publication and creation proofs, reconcile
status, readiness, and discard accounting.

Every bound declares:

- its Unit;
- per-object and aggregate maxima;
- owned-byte and shared-payload accounting;
- concurrency, queue, retention, and completion scope;
- checked arithmetic and overflow behavior;
- zero-limit meaning;
- validation order before traversal/allocation; and
- typed exhaustion behavior.

“Bounded” without these fields is not a capability claim.

Assembly selects an adapter through explicit configuration or one unambiguous
descriptor match before activation. Missing or ambiguous selection is typed;
there is no hidden fallback or revealing probe. Authority is validated before
lookup or observation.

Opening and bounded access do not require whole-Resource materialization by
default. Reads identify the exact Snapshot observed and never mix Revisions.
Unavailable historical state, stale state, indeterminate freshness, invalid
ranges, cancellation, closure, denial, conflict, pressure, and transient
failure remain distinct where applicable.

## Transactional Resource open

Opening is provisional. Kernel Runtime authorizes revealing lookup, negotiates
the adapter contract and bounds, and allocates a private **Provisional Access
Token** before observation. It establishes Resource Identity, Revision, and a
bounded provisional Snapshot stamped with the token.

The family descriptor exposes open-readiness predicates:

- granted authority;
- selected contract/version;
- validated Bound Descriptor;
- live adapter state;
- established Resource Identity and Revision;
- validated initial Snapshot; and
- required initial change-subscription or synchronization state.

Ready atomically promotes the token to a Resource Access Incarnation and
publishes it with the Snapshot only after every predicate passes. Failure
retires the token, reverses provisional state with dependants before
dependencies, closes the handle, and exposes neither token nor Snapshot. A
Provisional Access Token never authorizes public work.

## Snapshot consistency and freshness

A **Snapshot** is an immutable observation identified by Resource Identity,
Resource Access Incarnation, and Revision. It provides stable semantics for the
native regions exposed but may be sparse, lazy, shared, partial, or adapter-
managed. Consumers cannot manufacture a current Snapshot from a Revision token.

Every Resource family owns a versioned Snapshot Descriptor and validator. The
descriptor declares:

- required identity, incarnation, Revision, and structural fields;
- native region, Address, and Extent invariants;
- capture and consistency scope;
- family-owned capture epoch or per-region stamps;
- capability and bound provenance;
- compatible unknown fields or variants;
- structural metadata, region, handle, payload, and owned-capacity bounds; and
- a freshness predicate with typed `Fresh`, `Stale`, and `Indeterminate`
  outcomes.

The adapter validates the complete descriptor before publication. Invalid or
unsupported descriptors are definitely not published and cannot become
Snapshot authority.

A time-dependent family declares its time authority and comparison domain,
capture point or interval, uncertainty, discontinuity behavior, supported
maximum age/expiry, and the typed outcome of incomparable time. Revision is not
time. A Resource Adapter consumes MonotonicClock only for a declared
process-elapsed freshness contract.

A lazy read joins an older Snapshot only when the adapter preserves that
Snapshot's capture semantics. It never mixes later live state into the older
observation. `Unavailable` means requested historical state cannot be served;
`Stale` means a once-valid Snapshot fails freshness or compatibility;
`Indeterminate` means neither can be proved.

External change or freshness expiry invalidates the Buffer compatibility proof
until rechecked; it does not itself prove conflict and never mutates accepted
Buffer state behind History. Re-observe, rebase, resynchronize, or open-another
are explicit operations.

## Change notification and feed

Change-discovery strengths remain separate:

- **Resource Change Notification** is bounded invalidation evidence. A new
  Snapshot is required before new source state becomes authoritative.
- **Resource Change Feed** is a stronger Resource-native stream with declared
  ordering, completeness, cursor/resume, scope, retention, coalescing,
  duplicates, backpressure, gaps, and resynchronization.

Neither is the Editor Event Stream. A correlated diagnostic Event may describe
a Resource change but cannot become source-state authority.

Open readiness closes the initial Snapshot/subscription race using a proved
Snapshot-plus-checkpoint or an explicit gap/resynchronization state. Every
delivery is Access- and subscription-incarnation-fenced. Pressure or retention
loss produces a typed gap.
