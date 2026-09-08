# Resources and editing state

Target contract. Signatures below describe language-neutral interactions, not
implemented Rust APIs. Clients and SDK modules use the public session; only
Kernel state mechanisms accept changes. Concrete adapters and semantic policy
are selected by assembly, never discovered inside a Buffer.

## Ownership and identity

A Resource Adapter connects an external or synthetic subject. Its family
defines identity, locator, address, extent, unit, revision, access rights,
bounds, and errors. These are not universally paths, bytes, or text offsets.
Read-only access does not mean the subject is immutable. Snapshot consistency
does not mean the source cannot change afterward.

A Buffer owns working state, accepted edits, History, base Resource snapshot,
and publication/conflict state for exactly one Resource. Its local version is
not the Resource revision. Several independent editing contexts may open that
Resource, but clients sharing edits identify the same Buffer and History.

Resource access, Buffer, session, and policy lifetimes carry generations.
Reopen/replacement never lets old work install into a new lifetime. Immutable
reads identify the exact observed state; a revision token alone is not data.
Partial residency is normal. Missing, nonresident, stale, and failed reads are
different outcomes; eviction cannot discard edits or History reconstruction.

## Open and select

```text
session.open(locator, resource_grant, open_policy) -> operation
operation.result -> Opened(buffer, state, capabilities) | failure
client.select_buffer(local_slot, buffer) -> Selected | failure
```

The session coordinates open; the selected Resource Adapter performs access
using its own Host grants. Check authority before revealing lookup. Reserve
bounded access/state ownership, obtain a coherent initial snapshot, and close
the snapshot/change-subscription race before publishing the Buffer as Ready.
Failure reverses provisional ownership; no half-open Buffer becomes public.

Open policy explicitly selects an independent context or an existing context;
matching a path alone never silently shares mutable state. Capabilities state
whether local edits, source publication, and destination creation are allowed.
A read-only source may allow a local branch without same-source publication.

Selection changes Client-local focus/binding only. The Client first attaches
the requested Representation and initial state, then replaces its local slot.
Failure leaves the previous selection intact. Switching neither saves nor
closes another Buffer, and does not change another Client's selection.

## Edit, undo, and redo

```text
session.edit(buffer, observed_state, representation, intent) -> operation
session.undo(buffer, observed_state) -> operation
session.redo(buffer, observed_state, selected_child?) -> operation
operation.result -> Accepted(new_state, transition) | Rejected(cause)
```

An edit carries qualified semantic coordinates and values. Domain policy
validates meaning; Representation policy lowers it to resource-native
operations and preconditions. The Kernel validates identity, grants, bounds,
and currentness without interpreting lines, graphemes, pixels, or native keys.
A Resource-native State Model prepares the immutable state/History candidate
from those operations outside Core commit ownership, as specified in
[architecture](../architecture.md). Neither model nor adapter is a second
Buffer writer; Core installs only a fully prepared, current candidate.
The Rust preparation port uses a model-owned associated state type, not a
Kernel byte container or downcast registry. Immutable working-state views
pair actual data with exact Buffer and model/schema stamps. Initial
preparation consumes a bounded Resource observation; it does not fabricate
an accepted edit. Edit preparation consumes the lowered plan. Restore
preparation receives the current state and the History-selected retained
state; the model neither selects eligibility nor advances Buffer versions.

Domain validation and Representation lowering borrow that same immutable
working-state view alongside the semantic intent. Each selected policy binds
its supported state model/schema; mismatched state rejects before invocation.
A validation receipt names both the exact observed Buffer state and model,
so it cannot authorize lowering against another observation.

Read-only snapshot production is independent of edit lowering. Representation
policy interprets immutable working state and returns bounded, schema-qualified
semantic data, or an explicit unavailable/unrepresentable result. It receives
no client delivery handles. Projection/Interaction owners stamp and bind that
data to their selected generations and update lineage, rechecking currentness
before installation. Neither snapshot production nor its failure changes an
accepted Buffer transition or dictates a client's Position/Selection.

Preparation validates model/schema and structural plans before native work.
Models report bounded retained memory and work, with typed malformed,
precondition, unavailable, pressure and cancellation failures. These are
trusted provider obligations, not a sandbox or proof of arbitrary heap usage.
Core rechecks candidate identity, selected History relation, current target
and installation fences under the same serialization that installs it.
Scalar checks may run there; model calls and candidate destruction may not.
Rejection retains candidate ownership for release outside that authority.
Retained data may predate the current source snapshot; restore publishes a
fresh Buffer version against the current base, never the retained old stamp.

Ambiguous, unsupported, or stale correspondence rejects without guessing.
Derived Markdown preview is not a second authoritative document. An editable
Representation needs a faithful reverse mapping; otherwise it is read-only.
Units not expressible in a high-level view must survive unrelated edits.

Policy, I/O, allocation needed for commit, and callbacks finish before state
acceptance. The Buffer's single state authority rechecks the observed version
and preconditions, then commits the complete transaction and History change
atomically. This section is bounded, synchronous, and non-reentrant; no waits
or user callbacks run inside it. Failure changes neither state nor History.

History is a tree, not a linear version counter. Each retained state has an
opaque node identity and explicit parent/child edges. The root has no parent;
a node may have several children. Node identity implies neither ordering nor
depth. Core owns the tree, its current node and selected redo child.

Undo follows the current node's parent. Redo follows an explicitly requested
child or the current node's selected child. An edit after undo creates and
selects a new child; existing siblings remain available as branches. Retention
and explicit truncation follow the History descriptor, not an implicit
delete-on-edit rule. A missing eligible edge returns `NoHistory` unchanged.

Undo and redo produce fresh Buffer versions while selecting existing History
nodes. Only the Buffer version advances monotonically; History traversal uses
edges, never node-ID arithmetic. Acceptance identifies the exact traversed or
created edge. Core selects the edge before native preparation and rechecks it
at commit; changing the preferred child cannot redirect pending redo work.
The first profile uses one accepted transaction per new node. An atomic group
submits one transaction; implicit timing-based merging is outside this
profile. This is shared Buffer history, not per-client undo.

Acceptance notifies clients of the resulting state and History edge; it does
not restore or synchronize client cursors. Position/Selection interpretation
and each client's response are owned by the [client contract](clients.md).

Example: edit version 4 -> version 5; undo at 5 -> version 6 with the prior
units; redo at 6 -> version 7. A concurrent edit still naming version 4
rejects as stale. Both a Text edit and a keyed non-byte-addressed edit use
this same acceptance/history mechanism with their own native operations.
For branching, A -> B, undo to A, then edit C retains both edges A -> B and
A -> C. Redo can select either child after undoing to A; B and C are not
consecutive positions in a history list.

Each edit has a session-scoped request identity. An identical retry returns
its pending/retained result, never applies the same edit again; changed payload
or target under that ID rejects. Lost or expired results require explicit
reconciliation against retained request/state evidence. Do not turn missing
evidence into permission to resubmit under a new ID. Session replacement
fences old requests rather than resetting their effect to not-applied.

## Save and rename

```text
session.save(buffer, selected_state, expected_source) -> operation
session.rename(buffer, expected_location, destination) -> operation
operation.result -> Effect(certainty, cause, proof_or_reconcile)
```

Save is explicit publication through the Resource Adapter, never a side effect
of accepting an edit. It captures one immutable selected Buffer version and
the expected Resource revision. Edits accepted later are not included or
marked saved. Conflicts preserve local intent for explicit reject or rebase;
save must not overwrite external change merely because a path still exists.

Rename is capability-gated, family-owned locator change. It preserves the
Buffer and History only when the adapter proves the same Resource identity.
The first file profile is a same-namespace, no-overwrite rename; destination
collision, stale source, or unsupported identity preservation rejects.
Only proved application changes the known locator. Cross-resource moves,
copies, and Save As use explicit destination creation/publication under a
separate grant; they never silently rebind the source Buffer.

Effect certainty and cause are independent:

- `Applied`: validated proof identifies the selected operation and resulting
  Resource identity/revision or family recovery witness.
- `DefinitelyNotApplied`: proof establishes that no external effect occurred.
- `OutcomeUnknown`: handoff occurred but certainty is unavailable; a bounded
  reconciliation owner retains the operation identity and recovery witness.

Cancellation before handoff may prove no effect; cancellation, timeout, or
disconnect after handoff cannot promise rollback. Unknown effects are not
retried blindly. Reconciliation/duplicate rules belong to the adapter family.
An atomic capability applies all or none under its declared failure model;
known partial application is a violation, not a disguised unknown outcome.

Publication visibility and crash durability are separate claims. Each adapter
declares supported durability levels and their completion evidence. Successful
save/reopen does not prove crash durability. A rename does not imply a content
save. Undo changes local state, never reverses a completed external effect;
an Add-Only Resource requires a new compensating effect where supported.

## Failure, cancellation, and close

Admission is non-waiting: it transfers ownership and reserves bounded terminal
capacity, or returns the unchanged request with a typed failure. Accepted work
has one terminal outcome or explicit reconciliation ownership. Queue/payload,
resident state, History, and unresolved-effect bounds have visible pressure.

Relevant failures distinguish denied, unsupported, invalid region/intent,
unavailable, stale, conflict, pressure, cancelled, replaced, closing, closed,
and provider failure. Cancellation fences installation before acceptance; once
the atomic commit starts it finishes and reports its actual result.

`close(buffer, policy)` explicitly rejects dirty state, saves it, or discards
local edits according to the caller's authorized choice. It stops admission,
fences late work, and drains within bounds. Unresolved external effects must
finish, transfer to a named live owner, or return `IncompleteShutdown`; local
discard never means their effects were undone. Repeated close is idempotent.

State observations and diagnostic correlation use [events](events-and-input.md).
