# Debugging

The operational log, semantic notifications and causal evidence serve
different purposes. None owns editing state or may change an edit's result.

## One dmesg-like diagnostic stream

Kernel owns one bounded structured Editor Event Stream. Clients, adapters and
extensions submit through its public admission contract. Records contain
family/schema, severity, monotonic time, stamped producer, stream incarnation,
sequence, correlation, and bounded typed fields. Admission derives producer
identity and redacts before any subscriber or output sees a record.

Producer admission does bounded, non-waiting work. Pressure returns a typed
drop/rejection; no user callback, subscriber, file or stderr I/O executes
synchronously on the editing path. Sequence allocation and complete-record
admission are atomic. Failed admission does not consume a sequence.

The retained ring has a hard 1 MiB allocation budget, including record payload,
cached rendering, indexes and metadata capacity. Evict oldest complete records;
never truncate a record or retain uncharged external payloads. Independently
bound producer/subscriber registries, queued bytes/records, rates and workers.

Return distinct loss causes for rejection, rate limiting, pressure, eviction,
allocation, sink failure and closure. Pre-admission loss has no sequence.
Readers use stream-incarnation/sequence cursors; a retained snapshot followed
by updates has no subscription race. Missing admitted records produce a gap;
a cursor from another incarnation requires resynchronization.

One canonical renderer fixes field order, escaping, redaction and termination.
Ring replay, file, stderr, text export and reovim log consume identical stored
bytes. Machine export preserves the typed record, not a different meaning.
Each sink has independent bounded pressure/drain behavior. A slow sink cannot
block other consumers or the producer.

Unsubscribe is idempotent. Closing fences admission, captures a final sequence,
and returns each consumer's drain/discard outcome. Fixed, saturating health
state remains inspectable when the stream or a sink fails; failure reporting
never recurses through the failing path. Crash persistence is not implied.

The kernel.startup schema-v1 family and reovim-event-canonical/v1 encoding
remain unchanged. Its checked proposal, enum tokens, field order, final LF,
and 495-byte maximum are defined by the public event types and semantic
encoding fixtures in kernel/interface. That bound is a wire-format limit,
not a hash or checkout-shape expectation. Additional diagnostic families must
declare their schema, safe fields and actionability before use.

The executing service's `kernel.editing/v2` extends its earlier v1 records
with observer/update identity, a second causal parent, received and installed.
Other stages are opened, admitted, prepared, committed, update-ready,
update-failed, rejected, closing and closed. Startup v1 remains unchanged.
Update failure never reclassifies an accepted edit as rejected. Runtime
stamps producer/stream/sequence and clock identity; safe fields are the exact
origin session/request, observing session, exact update, Buffer-plan
correlation, state identity, typed rejection and optional causal parents.
Native content and selections are not logged.
Canonical fields have fixed order and bounded ASCII encoding. This family
shares the diagnostic stream and output contracts, not the semantic update
feed. A missing diagnostic or failed sink never changes editing acceptance.

## Backtrace an event and inspect related state

Record actual causal edges, not guessed timestamp relationships:

```text
input -> resolver/command -> request -> transaction/History
      -> representation update -> Client layout/render -> presentation
```

Fan-out, joins, coalescing, supersession, module work, Host calls, IPC
send/receive and CCM updates have explicit edges. Nodes carry applicable
session/request, Buffer incarnation/version, representation/projection,
Client/module/view/render and transport generations. Client presentation
identities do not enter Kernel editing state.

Cause/effect edges form a DAG. Lock held-by/waits-for relations are a separate
overlay that may contain cycles. Correlation alone proves neither causality
nor deadlock.

A reverse query starts at an event, request, transaction, state version or
presentation. Specify permitted edges, depth, nodes, bytes, work and deadline.
Return the bounded incoming subgraph and known roots/frontiers, with gaps.
No inference bridges lost IPC segments or replaced component generations.

State inspection returns an immutable captured version or a typed
not-captured, evicted, redacted, incomparable or gap result. Never present
today's state as the state at an earlier event. Captured state leases charge
all transitive retained memory and have bounded retention/lifetime.

The local reference captures produced semantic snapshots under explicit debug
grants and separate metadata/payload limits. Each capture names the actual
schema, Buffer, Projection and update identity. It is not native-source
capture. Requests for uncaptured/evicted evidence remain explicit; disabled
instrumentation and incomplete ancestry never become inferred success.

Keep full graphs and snapshots outside the diagnostic ring in separately
bounded, access-controlled stores. The log contains only safe summaries and
references. Trace sampling propagates from roots; gaps invalidate claims that
need the missing edge, not an otherwise valid endpoint timing sample.

## Locks, stalls and fail-fast

Record acquisition attempt/result, owner/waiter, order, wait/hold duration,
queue and cancellation state where observable. Capture must not acquire the
lock or wait on the queue it diagnoses.

A stall means a pending operation missed a declared progress point. A freeze
means its visible Client outcome missed a threshold. Neither proves deadlock.
Starvation and livelock are distinct from a complete wait-cycle proof in which
no permitted timeout, cancellation, runnable owner or external completion can
advance the obligations. Missing observations cannot prove such a cycle.

The watchdog has an observation/clock path independent of the suspected owner,
lock, queue and sink. Declare detection scope, thresholds, exclusions such as
debugger pauses, and uncertainty. Otherwise report detection as unavailable.

Development compositions explicitly select fail-fast policy. Proved deadlock,
lock-order violations and internal ownership defects fail loudly. A declared
freeze policy may also fail, but its result remains a freeze, not a deadlock.
Expected stale/unsupported/cancelled/pressure results are not defects.

On a fatal trigger, one observer performs bounded best-effort capture, seals
or records seal failure, then requests process/test termination. A failing
sink never prevents termination. Panic is allowed as final disposition, not
as an unobserved worker panic or unwinding across an unsafe callback boundary.

Production watchdogs observe only; they do not unlock, retry, restart, kill
or rewrite state as recovery. A separate lifecycle/effect contract must
authorize recovery. Internal invariant corruption still fails closed at its
smallest safe owner and escalates when safe continuation cannot be proved.

Abort promises no Drop, flush or rollback. External mutation uncertainty
survives a crash: an independently resumable owner needs its reconciliation
witness. A local crash dump alone does not prove that evidence survived.

## Required exercises

Test canonical bytes across sinks, provenance/redaction, snapshot/follow
races, eviction, saturation, slow/failed outputs, recursion and shutdown.
Test reverse traversal across edits, modules, Clients and IPC, including
missing edges, stale generations and evicted state.

Use deterministic schedules for lock order, complete/gapped cycles, stalled
owners and shutdown. Separately run real hosted scenarios for wait/hold,
freeze and capture latency. Compare instrumentation estimates with measured
overhead as specified in [performance](performance.md).
