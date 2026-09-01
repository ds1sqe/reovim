# Event Records, Admission, Retention, and Consumers

[Editor Event Stream index](README.md)

## Purpose

The **Editor Event Stream** is the Kernel-owned structured path for
editor diagnostics. The retained in-memory ring, Clients, extensions,
adapters, subscribers, diagnostic outputs, replay, export, and `reovim log`
observe or submit through this stream's public contracts; none creates a
parallel logger or independently interpreted record.

The stream is bounded, loss-visible, non-blocking with respect to editing, and
measured as part of the product's reaction cost.

Its versioned descriptor states the maximum producer-side synchronous steps or
work units, temporary allocated bytes, and lock-acquisition behavior for one
admission attempt. Producer admission is a `NonBlockingInvocation`: every lock
attempt is non-waiting, and contention returns typed pressure/drop rather than
waiting. Any separately admitted management operation that may wait is not
producer admission, cannot run on an editing State Lane, and declares its own
Invocation Class, deadline, and cancellation. The descriptor also owns the
rate-limit policy and the global-state dimensions defined below.

## Event and admission model

An event is an immutable structured record with a stable family and family
schema version, severity, monotonic timestamp, editor-stamped producer
identity, Event Stream incarnation, optional correlation identity, and bounded
typed fields. The editor derives provenance from the active dispatch context;
an extension cannot spoof it. Mutation events preserve Effect Certainty and
Outcome Cause as separate fields.

Client, extension, and adapter submissions enter through bounded admission
contracts. They cannot assign the Kernel sequence, impersonate another
producer, attach unbounded rendering payloads, or expose a concrete viewport,
frame, or raw platform handle as structured event state.

Validation and field-level redaction occur before any subscriber or output can
observe the event. A single total sequence number is assigned at admission,
before fan-out. Sequence numbers are never reused within one Event Stream
incarnation. Closing and recreating a stream creates a new opaque incarnation,
even within the same process.

Every event family declares an **Actionability** value:

- `none`; or
- `operator`, carrying a bounded **RemediationRef** with versioned catalog
  family, catalog version, and remediation code.

Error-severity families use `operator` when their contract defines a safe
operator action. Human prose may add context but is not the only machine-
readable distinction between missing, stale, malformed, incompatible, denied,
pressured, or failed states, and it does not replace the remediation catalog.

Producer admission performs bounded work and returns an explicit result:

- accepted with its sequence number;
- rejected because the record is invalid or exceeds a declared bound;
- dropped because of rate, pressure, or allocation limits; or
- closed because stream shutdown has begun.

Pre-admission loss has no sequence number and increments a cause-specific
counter; it never fabricates an Event Gap. Accepted records retain their
sequence even after ring eviction. Counters are fixed-size, saturating, and
expose saturation explicitly.

## Exact ring bound

The Kernel-owned ring has a hard resident allocation limit of exactly
1 MiB (1,048,576 bytes). That budget includes every allocation owned on behalf
of retained records: structured record data, cached canonical rendering,
payload storage, index entries, and per-record metadata. Capacity, not merely
current length, is charged. A retained record cannot reference uncharged
off-ring heap payload.

The ring admits only records whose declared maximum size can fit within the
budget. Admission evicts oldest complete records until the new complete record
fits. It never truncates a structured record to satisfy the ring. The ring
exposes earliest and latest retained sequences plus total record and byte
eviction counters.

The executable charge function validates allocated capacity for the structured
record, canonical bytes, payload, index, and per-record metadata before mutating
ring state. Allocation or rendering failure leaves no partially admitted
record. Shared storage has one declared charge owner and cannot be counted by
neither or both subsystems. Allocator bookkeeping and process RSS remain
separately measured rather than guessed into the structural oracle.

The descriptor independently bounds global state outside retained records. It
names at least counter slots and owned bytes; event-family and remediation-
catalog entries/bytes; producer and rate-limiter entries/bytes; subscriber and
output registry entries/bytes; synchronization objects, waiters, and owned
capacity; and worker/task count. Dynamic keys cannot bypass the aggregate
entry/byte ceilings. These dimensions are included in memory measurements.
Allocator bookkeeping not owned by the ring is measured; it cannot be used to
hide retained payload or metadata.

## Loss visibility and cursors

No loss mode is silent. Validation rejection, rate limiting, ring eviction,
subscriber pressure, output pressure, allocation failure, and closed-state
rejection have distinct counters or outcomes. Reporting a sink failure never
recursively depends on that same failing sink; fixed health state remains
queryable even when no event can be admitted.

Readers use cursors containing Event Stream incarnation plus sequence. A
snapshot returns the incarnation, retained sequence range, and cursor for the
next event. One follow operation yields every available sequence at most once,
in order, and returns the next cursor; it does not consume or mutate the input
cursor. Reusing the same cursor deterministically replays records that remain
retained and reports the same leading Event Gap for the same retained range.
The Stream keeps no implicit per-reader acknowledgment state unless a separate
future contract defines it. A cursor for another incarnation returns typed
stale/resynchronize. If previously admitted records are no longer retained or
a subscriber queue loses them, the reader receives an explicit Event Gap before
later records.
Snapshot-plus-follow has one atomic handoff boundary within one incarnation so
events cannot disappear in a replay/subscription race.

## Canonical rendering

One renderer defines deterministic field order, escaping, redaction markers,
timestamp representation, and line termination. For the same admitted event,
human output is byte-identical across ring replay, file, stderr, human-text
export, and `reovim log`. Human destinations receive the canonical bytes; they
do not render the event again.

Machine-readable export preserves the structured schema and sequence semantics
and is also bounded. It does not redefine event meaning.

## Subscribers and diagnostic outputs

Every subscriber and `DiagnosticOutput` owns a declared queue bound in both
record count and charged bytes. The subsystem also declares hard maxima for
active subscribers/outputs and total queued records/charged bytes across them;
arbitrarily many individually bounded consumers are not admitted. Subscription
or output admission beyond those bounds returns typed pressure. Slow consumers
are isolated from one another as well as from the producer path.
Admission never invokes subscriber code or performs file/stderr I/O
synchronously on the editing path. A slow or failed consumer is isolated;
pressure produces a consumer-specific Event Gap or drop count rather than blocking
or growing memory without bound.

Subscriptions support cancellation and idempotent unsubscribe. Stream closing
stops new producer admission, establishes a final sequence high-water mark,
stamped with the Stream incarnation, and performs the declared bounded drain or
discard policy for each consumer. Shutdown returns observable per-consumer
outcomes. Recreating a Stream never accepts a cursor from the closed
incarnation as ordinary replay state.
