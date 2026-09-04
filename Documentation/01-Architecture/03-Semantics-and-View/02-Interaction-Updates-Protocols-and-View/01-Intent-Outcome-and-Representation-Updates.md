# Interaction Intent, Outcome, and Representation Update

[Interaction, Updates, Protocols, and View](../02-Interaction-Updates-Protocols-and-View.md)

## Interaction Intent and Outcome

**Interaction Intent** is the versioned transport-neutral Client-to-Kernel
request. Its descriptor declares contract/version, semantic intent families,
capabilities, value schemas, payload/stream/work/concurrency bounds,
backpressure, cancellation, and failures.

Each Intent identifies:

- Contract Session incarnation and authority generation;
- Client role;
- Resource Identity, Access Incarnation, and Revision;
- Buffer identity/incarnation/version;
- Representation identity and Projection generation;
- semantic coordinate-space identity/incarnation and a bounded semantic value;
- request/correlation and cancellation identities; and
- deadline and authority required for admission.

Authority and shape bounds are checked before Resource lookup, expansion,
policy invocation, or mutation. Raw keyboard, pointer, terminal, or device input
is not an Intent; Client-owned policy translates it. A stale Intent is rejected
or enters an explicit reconciliation path and is never silently retargeted.

Kernel Runtime retains the owned Intent while
[Domain validation and Representation lowering](../01-Domains-Representations-and-Projections/02-Domain-Validation-and-Representation-Lowering.md)
receive bounded immutable invocation borrows. A policy failure or caught panic
cannot consume, retarget, or silently convert that retained request.

Every submitted Intent receives exactly one bounded correlated **Interaction
Outcome**. It distinguishes local acceptance from non-acceptance, preserves
Effect Certainty and Outcome Cause where a response can be lost, and names the
resulting Buffer version. Cancellation races have a declared winner or retained
reconciliation. Representation Update never proves that an Intent was accepted.

## Representation Update

**Representation Update** is the versioned transport-neutral Kernel-to-Client
state result. It names exact Resource, Access, Revision, Buffer,
Representation, Projection, and semantic-coordinate-space state and carries a
bounded:

- full semantic snapshot;
- predecessor-bound delta;
- invalidation;
- Representation Gap; or
- resynchronization result.

A delta names exact predecessor and result generations. The Client applies it
atomically only to the named predecessor. Duplicate or older delivery is a typed
no-op/stale result. Missing ancestry produces a Gap and resynchronization. A full
snapshot declares the prior state superseded. Bounded delivery never guesses
after loss.

An Update describes semantic Representation state. It may carry schema-qualified
intrinsic source pixels, samples, structured values, or Text values. It never
prescribes a Viewport, target grid, target-screen pixel, glyph, layout tree,
drawing command, GPU command, or frame schedule.

In-process and Protocol Adapter compositions share Update semantics. Protocol
Adapter owns framing, serialization, carrier, connection, and queue mechanics
only.
