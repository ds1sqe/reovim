# Update Delivery, Gaps, and Resynchronization

[Interaction Intent, Outcome, and Representation Update](../01-Intent-Outcome-and-Representation-Updates.md)

## Representation attachment

One attachment descriptor binds an exact Session and Resource Scope to one
Representation policy generation, Projection series, semantic coordinate-space
incarnation, Update stream incarnation, selected carrier contract, bounds, and
initial credit. An attachment identity does not replace any of those stamps.

Attachment validates authority, Session and policy lifecycle, Buffer scope,
carrier compatibility, initial credit, and retained initial-state bounds. Ready
transfers either one full initial semantic snapshot or an explicit
resynchronizing state. Client Runtime installs initial state before publishing
Client Ready. Missing initial state, a timer, or transport readiness is not a
successful attachment.

## Update identity and lineage

Every Representation Update directly carries its exact stream incarnation,
Update identity and cursor, Resource Revision/Snapshot, Buffer version/History,
Projection predecessor, and typed lineage.

The immutable attachment descriptor retained for the stream lifetime supplies
the exact Session/attachment authority, Resource Access, Buffer incarnation,
Representation/Projection series, and coordinate-space qualifiers. Descriptor
plus Update is the complete stamp; an identity alone or lookup of mutable
current state is never a substitute. Any change to one supplied qualifier
rotates the attachment and stream generation before another Update can publish.
A stream identity/lifecycle pair names one immutable descriptor across all
Sessions. Runtime must never reuse or rebind that pair to another descriptor.

Its lineage has exactly one typed shape:

- **Full snapshot:** one result generation and the prior generation it
  explicitly supersedes;
- **Delta:** exact predecessor and result generations;
- **Invalidation:** the exact generation no longer current;
- **Gap:** expected predecessor, available frontier, gap cause, and one
  resynchronization identity; or
- **Resynchronization:** one complete result generation.

Delta result follows its predecessor under the Projection contract. A full or
resynchronization snapshot explicitly replaces prior semantic state.
Invalidation grants no replacement state. Gap grants no missing ancestry.
Duplicate or older delivery never advances Client state.

## Qualified Update carrier

Full snapshot, delta, and resynchronization shapes move-own exactly one
schema-qualified immutable carrier. Its contract fixes type and selected schema
version, ownership, borrow/copy/drop, lifetime, transfer/affinity,
sharing/concurrency, unwind, and unsafe-boundary behavior.

Item count, nesting depth, and bytes validate before allocation. Binding
rechecks actual allocation length and returns unchanged bytes on mismatch.
Debug exposes only stamp, shape, and carrier qualification, never payload.

The exact Session descriptor is the public Update-preparation authority. It
selects its own attachment binding, carrier contract, capabilities, and bounds;
a caller cannot combine one Session's contract with another bound family.
Construction validates expanded input stamps, then retains only dynamic state;
resolution always requires the same descriptor. Delivery borrows the exact
stream request and takes the candidate Update only after cursor validation.
Failure leaves both the request and provider-owned candidate unchanged and
returns a typed delivery error.

Invalidation and Gap have no invented empty carrier. Their typed control fields
are the complete payload. A transient semantic value, when a Representation
defines one, still receives a stream cursor and visible loss behavior; there is
no untracked fire-and-forget Update class.

An Update may carry Text, structured values, samples, or intrinsic source
pixels under its selected schema. It never contains a Viewport, terminal cell,
target-screen pixel, glyph, layout tree, drawing/GPU command, frame, or
presentation schedule.

## Stream ordering, credit, and acknowledgement

The Update stream has one exact incarnation and monotonically ordered cursor.
Data delivery consumes one granted data credit; control delivery consumes its
separate reserved slot. Acknowledgement returns only that delivery's capacity
and reports one Client observation:

- installed atomically on the exact predecessor;
- duplicate of already installed state;
- older than installed state;
- missing predecessor; or
- rejected because attachment or Client fences changed.

Acknowledgement does not grant Buffer, Projection, or Client State Lane
authority and does not prove presentation. Unknown Update, duplicate
acknowledgement, wrong cursor, stale stream, Closing, and Closed are distinct
results.

The descriptor reserves at least one bounded control-delivery slot outside data
credit. Pressure may coalesce or retire data only under the declared retention
rule and must publish a Gap. It cannot hide Gap, replacement, or close state
behind exhausted data credit. One slow Session cannot consume another
Session's terminal or Update reservation and cannot block Buffer acceptance.

## Gap and resynchronization

Missing ancestry, retention eviction, declared coalescing, Protocol loss,
pressure overflow, source discontinuity, and replacement are distinct Gap
causes. A Gap retains its known cursor and Projection frontiers. It never
guesses missing state from the latest payload or from timestamp order.
A Gap remains mandatory visible loss when Update delivery is selected, even
if resynchronization is unsupported; only the follow-up request and result
lineage require that optional capability.

A resynchronization request names the exact Session, attachment, stream, Gap,
and resynchronization identity. It move-owns one prepared charge containing one
outstanding-request count, bounded abstract work, and maximum result-carrier
bytes. Each dimension has an independent per-request and aggregate bound,
checked arithmetic, and the same Session charge owner. The prepared values are
the Session descriptor's maxima, not caller-selected accounting. Revalidation
requires exact work and byte reservations even when the supplied charge has
the same owner. Result bytes must also pass per-Update and retained-Update
limits before ownership transfer;
data-delivery credit remains independent.

Submission is a serialized, non-waiting admission for deferred local work.
Definite non-admission returns the unchanged request and charge. Admission
transfers both to Kernel Runtime and returns only a copyable receipt carrying
the exact request stamps and prepared charge. The receipt witnesses ownership
transfer; it owns neither request nor reservation.

Kernel Runtime retains request and reservation until the matching ordered
resynchronization Update enters bounded retention. At that cut, result bytes
transfer to normal Update accounting and request/work reservations release.
Before that cut, callback failure, fencing, pressure, or caller loss cannot
silently release or retarget them; incomplete close exposes their count, work,
and bytes. The Update still obeys stream credit, carrier validation, and Client
installation fencing.
The first direct profile resumes at the Gap cursor's checked successor. The
result must name that stream and resynchronization identity and fit the
prepared byte charge before the retained request can release.

After reporting missing predecessor, Client Runtime stops applying deltas until
a full resynchronization installs. Duplicate and older control items remain
typed no-ops. A new Session, attachment, or stream incarnation cannot bridge an
old Gap by scalar equality.

## Delivery and close

Non-waiting Update retrieval names the exact expected cursor and returns one
owned Update or typed pending, no-credit, cursor-behind, cursor-ahead, unknown
stream, already delivered, replaced, Closing, or Closed state. Retention loss
returns an explicit Gap Update when control capacity remains current.

Closing first rejects new Intents, resynchronization requests, and data credit;
advances Session, attachment, and stream fences; requests cancellation; and
boundedly resolves admitted Intents and resynchronizations, retained Outcomes,
retained Updates, outstanding credit, and Outcome reconciliation. Dependants
close before the Session.

Close reports exactly one of:

- `Closed`, proving execution and retained delivery/release obligations are
  terminal;
- stale Session, which never closes a different generation; or
- incomplete shutdown, with independent execution disposition and bounded
  pending Intent, retained Outcome, retained Update, unacknowledged Update,
  Outcome-reconciliation count, and admitted-resynchronization count/work/byte
  totals.

A cancellation request, deadline, caller timeout, stream fence, or discarded
delivery does not prove execution stopped. Repeated close returns the same
terminal state. Unresolved same-process work remains failed-owned.

## Operation shape and public proof

Attachment of already retained initial state, non-waiting retrieval, and
acknowledgement are bounded local operations without callbacks. The Update
contract is a deferred local stream. Resynchronization admission may lead to a
selected Representation-policy callback outside every State Lane. Close may
wait only under its declared bound. All operations are non-reentrant.

Public conformance covers initial snapshot, full/delta/invalidation/Gap/resync
lineage, carrier type/schema/shape/length faults, cursor order, credit and
control reserve, every acknowledgement result, slow-Client pressure,
independent resynchronization count/work/result-byte pressure and owner faults,
admitted receipt versus returned request ownership, retained-result cut,
replacement, idempotent close, and incomplete-close ownership. The fixture
uses opaque non-Text bytes and no View or transport type.
