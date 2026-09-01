# Interaction, Updates, Protocols, and View

> **Normative target contract.** This chapter is part of the
> [Architecture contract](../README.md). Present-tense requirements define the
> v0.16 target and do not assert current implementation or qualification.

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

## Client and View

A Client owns every View it instantiates. View state includes Viewport and
visible-range policy, layout, clipping, focus, scroll, Client-local cursors/
selections, decoration, hit testing, input mapping, drawing, and frame
scheduling. A typed no-View Client path owns its declared output state and does
not invent View geometry.

A device or presentation coordinate is mapped to a stamped Semantic Location
before Intent submission. Carrying that location in one request does not promote
persistent View state into Kernel authority.

Shared, persistent, collaborative, or command-semantic state moves inward only
through a separately accepted contract defining authority, identity/version,
bounds, lifecycle, fencing, privacy, failure, and conformance. The CCM may
synchronize only such explicitly admitted state; it owns no View, rendered
frame, Buffer, or transport state.

A Client does not write storage, mutate Buffer/Projection internals, or retain a
second authoritative content buffer. It submits Intents against observed state.

A Presentation Adapter is a concrete Client-side target mechanism behind a
Presentation Port. It owns terminal/window/display/rasterizer/GPU/device I/O and
presentation handoff. For every instantiated View, the Client owns Viewport,
layout, input mapping, drawing/theme policy, frame scheduling, and negotiated
Rendering Capabilities. The adapter receives no editing authority.

Rendering is negotiated, not inferred from family name. A TUI may negotiate
cells, raster/pixels, video, or GPU acceleration. CLI, Web, and future families
negotiate their own output sets. Concrete device handles and encoded target
commands remain Presentation Adapter-private.

## External semantic protocols

**External Protocol Binding** is outside the Kernel and owns semantic
translation. **Protocol Adapter** separately owns encoding, framing, connection,
and carrier I/O.

For example, an LSP binding:

- resolves `DocumentUri` under authority to a Resource/Buffer/Text
  Representation binding without treating URI as Resource Identity;
- maps LSP document version and negotiated position encoding to exact Resource,
  Buffer, Projection, and Text coordinate-space stamps;
- translates line/character locations and edits through the Text
  Representation contract;
- keeps workspace and other protocol values binding-owned or uses separately
  negotiated orchestration contracts;
- uses public SDK/service APIs only; and
- returns explicit stale, encoding, mapping, gap, and resynchronization outcomes.

LSP diagnostics are LSP/Text semantic data, not Editor Event Stream records
merely because the protocol calls them diagnostics.

The v0.16 foundation proves translation shape with mutually incompatible Text,
structured, and intrinsic-image coordinate fixtures. A full external protocol
implementation requires separate release admission. Neovim parity admits only
the subset it actually requires; it does not imply full LSP.

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

## Fidelity and conformance proof

The v0.16 production proof attaches a faithful byte/hex Representation and a
Text Representation to one partially resident byte Resource and one Buffer.
Client-owned Views present both. A hex Intent updates Text where decoding
permits; a Text Intent produces exact source-Unit Operations; invalid byte
sequences remain visible and unchanged unless explicitly edited. An edit that
cannot preserve unexpressed state rejects or the form remains read-only.

The proof exercises Intent, Outcome, and Update seams through every required
in-process or supported Protocol composition. The release matrix names exact
contract/adapter versions, fixture digest or generator, and fidelity,
differential, outcome, coordinate, and resynchronization oracles. Adjacent
transport evidence does not qualify a protocol composition.

Each Presentation Adapter family supplies a deterministic sink or golden harness
for bounded writes, short/failed handoff, input decoding, close/restore, and
frame/result ordering. Client journeys run against that harness and every
supported production adapter.

See [Layer and Dependency Model](../01-Layer/01-Roles-Descriptors-and-Dependencies.md)
and [Resource, Snapshot, Buffer, and Transaction](../02-Resource-and-Buffer/01-Resource-Access-Snapshots-and-Change.md).
