# Interaction Intent, Outcome, and Representation Update

[Interaction, Updates, Protocols, and View](../02-Interaction-Updates-Protocols-and-View.md)

## Reading order

1. This chapter owns the separation among Interaction Intent, terminal Outcome,
   and independently delivered Representation Update.
2. [Session Negotiation, Intent, and Outcome](01-Intent-Outcome-and-Representation-Updates/01-Session-Negotiation-Intent-and-Outcome.md)
   owns exact Session authority, identity mapping, admission, ownership, and
   reconciliation.
3. [Update Delivery, Gaps, and Resynchronization](01-Intent-Outcome-and-Representation-Updates/02-Update-Delivery-Gaps-and-Resynchronization.md)
   owns attachment, lineage, carrier, cursor, credit, acknowledgement, gap,
   resynchronization, and close semantics.

## Ownership split

**Interaction Intent** is the versioned transport-neutral Client-to-Kernel
request. Its outer envelope owns exact Contract Session and Client authority,
request and causal identity, and one already validated move-only Semantic
Intent. Raw device input and presentation coordinates never enter it.

**Interaction Outcome** is the one correlated terminal semantic disposition of
an admitted Intent. Definite non-admission returns the unchanged Intent with an
immediate terminal non-acceptance. Outcome records local acceptance, explicit
non-acceptance, or retained reconciliation; it never substitutes for current
Representation state.

**Representation Update** independently carries bounded semantic state. It is
a full snapshot, predecessor-bound delta, invalidation, explicit Gap, or
resynchronization result. It never proves Intent acceptance and never contains
View, viewport, layout, rendering, or presentation commands.

Direct in-process and later Protocol Adapter compositions implement the same
semantics. Protocol Adapter owns only encoding, framing, carrier, connection,
and queue mechanics; it cannot redefine authority, Effect Certainty, ordering,
gap, or reconciliation.
