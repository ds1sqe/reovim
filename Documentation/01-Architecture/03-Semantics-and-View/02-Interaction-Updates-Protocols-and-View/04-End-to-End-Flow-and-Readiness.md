# End-to-End Edit Flow and Readiness

[Interaction, Updates, Protocols, and View](../02-Interaction-Updates-Protocols-and-View.md)

## End-to-end edit flow

~~~text
device event
  -> Client input/keybinding mapping
  -> presentation coordinate mapped to stamped Semantic Location
  -> versioned Interaction Intent against observed Representation state
  -> Kernel Runtime authority, version, and bound admission
  -> concrete Domain/Representation validation and lowering
  -> typed Transaction / Operations / Preconditions
  -> Buffer State Lane atomic acceptance, History, Projection invalidation
  -> one correlated terminal Interaction Outcome
  -> generation-fenced Projection computation
  -> bounded Representation Update
  -> Client View layout/render policy
  -> Presentation Adapter handoff
  -> optional, separate Resource publication
~~~

This flow permits faithful low-level and Domain-aware high-level editing without
creating two content authorities or placing View mechanics in the Kernel.

## Readiness and lifecycle

A Domain/Representation/Projection attachment is provisional until contract
version, capability, bounds, authority, initial source Revision, semantic
coordinate descriptor, and initial Projection generation validate. A Client
becomes ready only after initial bounded Representation state or an explicit
resynchronization state is available.

Failure before Ready fences provisional generations, cancels owned work,
removes provisional registrations without partial visibility, and reverses
dependants before dependencies. A multi-grant or multi-subscription Contract
Session is all-or-nothing before Ready.

Closing stops new Intents, advances fences, resolves or exposes every admitted
Outcome, boundedly drains/discards only declared non-authoritative queues,
detaches the Client, then releases Projection and policy state in reverse
readiness order. Repeated close is idempotent and no detached work survives.

Kernel Runtime owns Kernel-side attachment and semantic state. Client Runtime
independently owns its Client Family, View, client-side Protocol endpoint,
Presentation Port/Adapter, and lifecycle. Neither Runtime shares lifecycle state
or reaches through a Contract Session to control the other's private subtree.
