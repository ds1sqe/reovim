# Clients

This is a target contract, not a claim of implemented clients or APIs. Operation
names describe semantics; the consumed public types must make them concrete.
[Goals and release scope](../README.md) own the client and language matrix.

## Client Layer Model (CLM)

The Client Layer Model (CLM) defines composition, extension, and presentation
within one Client Runtime. It owns neither cross-client synchronization nor
authoritative Buffer/History state.

Each client family has a Core, public Contract, Module surface, and Runtime.
The Core enforces local contracts and dispatches contributions; modules supply
feature policy. The Runtime owns one client instance, its module instances,
sessions, input, local state, and presentation subtree. Shared libraries do
not become a global client lifecycle or state owner.

The family Contract exposes typed input, keymap, view/layout, decoration,
output, and render contribution points as applicable. Modules use that public
surface and common semantic contracts, not Core internals, another family's
module, or concrete presentation/transport adapters. A CLI may have bounded
output without inventing a viewport or frames.

Clients own focus, mode, cursor, selection, scrolling, layout, viewports,
hit-testing, drawing, and scheduling. Such state stays local unless explicitly
promoted into a shared semantic schema. A buffer switch changes the client's
active binding; it does not replace authoritative content or discard History.

Raw keys, device events, and screen coordinates stop at the client. The client
resolver maps normalized input and registered keymaps into local actions or
semantic requests. Editing requests carry resource-qualified coordinates and
observed revisions, never terminal cells or viewport coordinates. Missing or
stale correspondence rejects the action; it never guesses an edit location.
See [events and input](events-and-input.md) for registration and resolution.

## Semantic Position and Selection

Position identifies a location or target under a Domain/Representation schema;
Selection identifies the chosen parts under that schema. Neither requires
text coordinates, a single scalar, a contiguous range, or two endpoints.
Examples include video time under a declared timebase, an image layer, or
several disjoint paths/subtrees in a JSON shape. These are examples of open
schemas, not a closed list of Kernel variants.

Shared or submitted values identify their schema, semantic coordinate space,
Resource/Buffer observation and applicable Representation generation, with
bounded payloads. Interpretation and mapping belong outside the Kernel.

Cross-domain/Representation translation requires explicit correspondence;
neutral values are not universally convertible. A valid source Position or
Selection may have no equivalent target meaning. Distinguish exact mapping,
partial/lossy mapping, unrepresentable, unsupported, ambiguous and stale
results. Partial mapping identifies omitted meaning and requires explicit
client-policy acceptance; never silently drop selected parts or replace the
original with a guessed location. Failed translation is a local capability
result, not failure or reversal of an already accepted Kernel transition.
Only a valid, current correspondence can supply a subsequent edit target.

## Observe and present

The common client contract exposes attach, submit, observe/resynchronize, and
detach through a negotiated session. It fixes identity/incarnation, compatible
versions, grants, resource scope, request identity, and queue/payload/work
bounds. Protocol adapters provide transport, not editing authority. Client and
Kernel use separate Host grants, even when cohosted.

The root client attachment precedes open and carries no fabricated Buffer.
An explicit independent-open or join-existing operation yields the authorized
Buffer-scoped endpoint. Representation attachment and its initial snapshot
come afterward. Closing this endpoint or detaching the client leaves shared
Buffer state and other clients alive.

An edit's terminal outcome and its Representation update are different values.
Acceptance names the authoritative resulting Buffer revision. Updates describe
semantic snapshots, deltas, invalidation, or a gap with exact source revisions
and Projection generations. Either delivery may arrive first. A delta applies
only to its declared predecessor; a gap stops dependent delta application and
requires an authoritative snapshot/checkpoint.

Admitted resynchronization retains its exact gap/stream receipt until one
terminal update transfers: a qualified snapshot or a payload-free typed
failure. Failure cannot fabricate recovered state. Session close cancels
unstarted recovery and retains its Closing terminal for draining; it does
not discard the request merely because shutdown was requested.
Gap/failure controls have separately bounded credit and cannot be stranded
behind exhausted data credit. Admitting the exact delivered gap retires its
obsolete data-credit prefix, never editing outcomes or control credit;
old acknowledgements cannot release new credit. A fresh gap/token is
required after failed recovery, and stale receipts cannot install a result.

The client validates those stamps before installing derived state. A local
cache, preview, or speculative view is discardable, never another Buffer or
History authority. Editing-dependent local changes are gated by the
correlated outcome and matching update, not commanded by them. Any later
speculative policy must define bounded rollback and reconciliation explicitly.

On accepted edit/undo/redo, Kernel reports the committed transition and state
through outcomes and semantic notifications. It does not command a cursor or
selection change, including for the requesting client. Each client decides
whether to follow, keep, restore from its own context, or use another policy.
No client acknowledgement or cursor update gates Kernel acceptance.

History requests need the authorized Buffer observation, not an editable
Representation or a fabricated semantic intent. State notifications retain
prior/new state, the originating request and exact transaction/History edge.
The receiving observer is separately qualified; origin is not delivery scope.
Notification recovery restores a checkpoint, not fictitious lost outcomes.

Keeping a local Position/Selection does not make its old correspondence
current: resolve or explicitly invalidate it before submitting another edit.
Cross-client Position/Selection sharing is a separate, explicit CCM scope;
receiving another client's state does not impose a follow policy.

Example: the GUI submits a Markdown edit observed at Buffer revision R. An
accepted result names R+1; source and preview updates may arrive separately.
The GUI labels each installed result by its source revision and never presents
an older preview as current. A TUI can observe R+1 with unrelated local scroll
and cursor state. Both submit undo through the same History authority.

## Rendering is a capability

Neither `TUI` nor `GUI` implies a rendering backend. A presentation adapter
advertises supported contribution schemas: cells, image/raster/pixels, vector,
video, canvas, or structured output as applicable. CPU and GPU paths are
explicit capabilities, not inferred from image or video support.

The negotiated descriptor states:

- capability/schema version and adapter, target, and device incarnation;
- formats, coordinate rules, extents, damage and stream/frame-rate limits;
- payload, queue, in-flight, memory, upload/readback, and copy bounds;
- ordering, completion/fences, presentation handoff, drop and pressure rules;
- required versus optional behavior, fallback, cancellation, and shutdown.

Contributions bind that descriptor plus client, view/layout, and frame
generations. The family validates them and passes accepted work through its
presentation port. Terminal escape sequences, DOM/window handles, codecs,
rasterizers, GPU commands/queues, and device objects stay adapter-private.
They never enter the Kernel or ordinary module API. The Kernel describes what
semantic state changed, not how a concrete viewport draws it.

Capability or device loss stops affected admission, advances generation fences,
resolves or visibly discards pending presentation, and requires renegotiation.
Old work cannot install into the replacement. Required capability loss removes
affected readiness; optional loss exposes the declared degraded result.
Unsupported rendering is typed. Fidelity-changing fallback requires explicit
acceptance rather than silent substitution.

The first product target uses a TypeScript-ecosystem GUI, with SSR as the
starting presentation direction. SSR output is not authoritative editing state
or proof of interactive readiness. Framework, deployment, and render stack
remain unselected. The TUI target includes a negotiated Kitty image path and
an explicit unsupported/fallback result. That path alone proves neither video
nor GPU support. Each claimed path needs its own working scenario.

## Client-Client Model (CCM): scoped synchronization

CLM describes composition inside a client; CCM synchronizes explicitly shared
semantic or module state between clients. CCM owns no Buffer, History,
transport, viewport, or rendered-frame authority.

Each synchronization scope names one authority, schema/version, members and
their incarnations, grants/privacy, state revision, ordering/predecessors,
retention/expiry, conflict handling, and per-client plus aggregate bounds.
The reference composition is logically server-mediated. A carrier topology
does not grant a peer authority to apply editing deltas.

Join requires an authoritative snapshot and checkpoint before shared-state
readiness. Submissions go to the named authority; accepted revisions fan out
through bounded independent delivery. Declared coalescing names its covered
range. Missing ancestry, pressure loss, or restart produces a gap and requires
resynchronization. Reconnect negotiates new session/membership incarnations;
resume is allowed only with a compatible retained checkpoint.

Presence, shared selections, and paired module state need explicit schemas.
Local focus/scroll are not shared by default. A module maps shared semantic
state into its own client's view; cells, pixels, video frames, and GPU state
never become cross-family truth. Media content may be shared as a Resource
with independently rendered Representations.

## Behavior checks

Prove independent GUI/TUI views over one committed revision; input resolution
without presentation types in the Kernel; stale update/frame rejection;
snapshot recovery after a gap; and isolation of a slow or failed client.
Exercise render capability denial/loss and bounded teardown through the real
selected adapters. Record causal input-to-present paths and overhead under
[performance](../performance.md), including a typed unavailable observation
when presentation cannot be measured.
Exercise clients choosing different Position/Selection responses to the same
undo/redo notification. Include non-text and disjoint selection schemas;
stale retained positions cannot bypass normal edit-target validation.
Test a valid source selection that another Domain/Representation cannot
express, including a partially representable multipart selection. Both clients
still observe the same accepted Buffer revision without forced translation.

Full v0.16 release proof additionally joins TUI, CLI and Web in one mixed-family
CCM scope, exercising gap/reconnect and slow-member isolation. A GUI/TUI
milestone alone does not discharge that existing release obligation.
