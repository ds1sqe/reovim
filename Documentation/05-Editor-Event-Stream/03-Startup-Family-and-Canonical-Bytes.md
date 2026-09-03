# Startup Event Family and Canonical Bytes

[Editor Event Stream index](README.md)

## Bootstrap scope

The initial v0.16 startup family is `kernel.startup`, schema version 1. It is a
public consumer of the [MonotonicClock][clock] and
[lifecycle contract][lifecycle]. It does not add follow, subscribers,
`reovim log`, output workers, persistence, or another logging pipeline.

The Contract Authority assigns the stable family, schema, remediation-catalog,
and contract identities. Assembly and the Lifecycle Owner explicitly supply
component, clock, and Stream instance/incarnation/generation identities. The
live admission authority derives producer and correlation identity from the
accepted dispatch context.

Portable identities are opaque fixed-width carriers, not strings, paths, or
ambient generators. The canonical tokens `kernel.startup` and
`kernel.bootstrap` are versioned schema labels; they are not those identity
carriers.

## Proposal and editor stamping

An unadmitted startup proposal contains only:

- lifecycle owner instance, incarnation, and generation;
- readiness outcome: `ready`, `diagnostics-degraded`, or
  `activation-failed`;
- rollback disposition: `none`, `reverted`, or `incomplete-shutdown`, where
  only `activation-failed` may select a non-`none` value;
- checked active, degraded, and unavailable component counts;
- bootstrap-health state: `clear`, `imported`, or `pending`; and
- the outcome's closed Actionability/remediation selection.

The proposal cannot assign producer identity, Event Stream incarnation,
sequence, monotonic timestamp, or canonical bytes. It contains no free-form
text or raw environment value and therefore has no field-specific secret to
preserve. A future field family with protected or textual data requires an
explicit redaction rule before admission.

Admission validates authority and structural bounds, derives editor-owned
producer/correlation provenance, redacts fields, then observes the injected
MonotonicClock. A caller-supplied instant is not a startup timestamp. Clock
failure produces the typed pre-admission outcome defined below and a fixed
bootstrap-health charge without sequence assignment.

One Event Stream incarnation binds to exactly one clock instance,
incarnation, and generation in its descriptor. Activation verifies that clock
stamp before Stream readiness. Clock replacement or close fences and closes
the dependent Stream first; admission against a replacement cannot continue
under the old Stream incarnation. A replacement clock therefore requires a
new Stream incarnation. Timestamps are comparable within one Stream without
placing the clock stamp in every canonical record.

The admitted immutable record contains the stamped producer, Stream
incarnation, total sequence, monotonic instant, family/schema, severity,
correlation, Actionability, fixed fields, and canonical bytes. Severity is
`info` for `ready`, `warning` for `diagnostics-degraded`, and `error` for
`activation-failed`.

`ready` has Actionability `none`. The other outcomes are `operator` and carry
catalog `kernel.bootstrap`, catalog version 1, plus the closed remediation code
`diagnostic-path-degraded` or `activation-failed` respectively.

Component counts and readiness are one coherent result. `ready` requires zero
degraded and unavailable components. `diagnostics-degraded` requires at least
one degraded or unavailable component. `activation-failed` requires at least
one unavailable component. Contradictory combinations are invalid structure,
not a degraded mode inferred by a consumer.

## Sequence and atomic admission

The sequence carrier is unsigned 64-bit. Zero is reserved as unassigned; the
first admitted record is 1. The Stream computes the next value with checked
arithmetic while holding its non-waiting admission authority.

Admission performs this order:

1. validate authority, family/schema, and proposal bounds;
2. derive producer and correlation provenance;
3. apply redaction;
4. observe the injected clock and validate its current incarnation;
5. compute exact charge and reserve every required capacity;
6. compute the candidate sequence without publishing it;
7. render the complete candidate record into reserved storage; and
8. atomically commit the sequence and complete record before later fan-out.

No fallible work remains after the commit cut. Validation, clock, allocation,
charge, or rendering failure returns without consuming a sequence or changing
retained state. The exact outcome mapping is:

- Stream not ready, invalid authority, structure, bounds, actionability, or a
  clock observation that cannot produce a valid current in-domain instant is
  `rejected`, with a distinct cause;
- rate, non-waiting synchronization pressure, allocation, charge preparation,
  or rendering failure is `dropped`, with a distinct cause;
- Stream or bound clock Closing/Closed/replacement is `closed`, with a distinct
  cause and a fenced Stream; and
- sequence exhaustion is `closed(sequence-exhausted)`, fences the Stream,
  begins Closing, and increments the fixed exhaustion health counter.

No outcome wraps, reuses a number, or silently maps one cause to another.

Admission is exactly:

```text
NonBlockingInvocation + InlineTerminal + Single + ExternalObservation
+ NoUserCallback + NonReentrant
```

The Effect Class covers the injected clock observation. Ring mutation remains
local, and admission performs no external mutation. Every synchronization
attempt is non-waiting. Contention is a typed pressure drop. Admission invokes
no user code, subscriber, output, callback, I/O, or Runtime facility.

## Canonical startup bytes

Canonical startup rendering is `reovim-event-canonical/v1`. It is one ASCII
line terminated by exactly one LF and no other framing byte. Fields occur in
this order, separated by one ASCII space:

```text
reovim-event/v1 stream=<hex128> sequence=<u64> timestamp_ns=<u64>
producer=<hex128> family=kernel.startup schema=1 severity=<severity>
correlation=<hex128> actionability=<actionability>
remediation=<remediation> outcome=<outcome> rollback=<rollback>
active=<u64> degraded=<u64> unavailable=<u64>
bootstrap_health=<health>\n
```

The displayed block wraps for documentation only; encoded output contains no
line break before the final LF. `<hex128>` is exactly 32 lowercase hexadecimal
digits without a prefix. Unsigned integers are shortest decimal with no sign or
leading zero. Closed enum tokens are the lowercase hyphenated spellings above.
`remediation` is `none` or
`kernel.bootstrap/1/diagnostic-path-degraded` or
`kernel.bootstrap/1/activation-failed`.
`rollback` is `none` for `ready` and `diagnostics-degraded`, `reverted` for a
fully reversed activation failure, or `incomplete-shutdown` when failed-owned
work remains.

The maximum v1 line is exactly 495 bytes including its final LF. It uses
`u64::MAX` for sequence and timestamp, the longest coherent three-count split
whose checked sum is `u64::MAX`, the coherent `activation-failed` plus
`incomplete-shutdown` tokens, and three 32-digit identities. It is part of the
record-size bound and canonical-rendering golden rather than an estimated
allocation size.

A borrowed canonical-record reference exposes only validated Stream, family,
schema, candidate-sequence, canonical-byte, and checked-size values. It proves
their structural coherence without reparsing in a consumer; possession alone
does not prove the record crossed the atomic admission cut. Live Runtime fan-
out obtains this reference only from the admitted record after commit.

Contract Authority supplies the versioned reference-safety descriptor, and the
startup constructor rejects any nonexact axis. The required declaration is
`Borrowed + Shared + Copy + Trivial + IncarnationBound`,
`Transferable + Immutable`, `DataOnly + NotInvocable`, mismatch Reject, and no
unsafe boundary. It can be retained only while its Rust borrow and source
Stream incarnation remain valid.

`IncarnationBound` is a logical identity scope, not proof of current liveness,
readiness, or admission. The Rust lifetime protects the borrowed bytes; every
use against a live capability must also compare the complete current Stream
identity. Same-incarnation data from another generation is not current.

The v1 family has no escaping case because it accepts no free-form field.
Adding one requires a new family schema that fixes escaping and redaction while
preserving this record's bytes. Ring replay, DiagnosticOutput, human export,
and `reovim log` use these exact retained bytes without re-rendering.

## Counters, snapshot, and readiness

Cause-specific counter values are unsigned 64-bit, saturating, and paired with
an explicit saturation bit. Validation, clock, rate, pressure, allocation,
rendering, sequence exhaustion, ring eviction, output pressure, and closed
rejection remain distinct. Pre-admission failures have no Event Gap.

One snapshot request declares maximum returned records and aggregate copied
canonical bytes within the Stream descriptor. It grants a preallocated caller-
owned output carrier with those exact logical capacities. The Stream allocates
no output, retains no snapshot lease, and transfers no ring ownership. On
success, the caller continues to own an independent bounded copy.

Snapshot copies complete records oldest-to-newest. Its result contains Stream
incarnation, earliest/latest retained sequence or typed empty, complete copied
records, eviction totals, and a next cursor of Stream incarnation plus next
sequence. Coverage is `complete` or `partial`; partial coverage identifies the
first uncopied sequence. A single record whose canonical bytes cannot fit the
granted bound returns typed `snapshot-bound-exceeded` without a partial record.
The returned canonical-byte count is exactly the checked sum of
`CanonicalEventBytes::size()` for the complete returned records. A lower or
higher declared total is invalid snapshot metadata.

Record count and canonical-byte count are portable content quantities. They do
not claim an allocation-byte total for the Rust record carrier, slice or index
capacity, metadata layout, allocator bookkeeping, or process RSS. A concrete
Runtime snapshot carrier separately declares and validates its owned capacity
charge before copying, using executable carrier and allocator facts. That
mechanism charge and its allocator overhead are measured rather than inferred
from a language-level record layout.

This contract defines the cursor carrier but no follow operation and makes no
`EVT-005` claim. Snapshot is a bounded non-waiting LocalOnly operation with
`InlineTerminal`, `Single`, `NoUserCallback`, and `NonReentrant`; contention
returns typed pressure without changing the caller's carrier.

A required Event Stream that fails activation prevents Kernel readiness. Once
the Stream is Ready, failure to admit the startup summary does not change an
editing outcome or silently block readiness. The Lifecycle Owner may publish
only the declared `diagnostics-degraded` Ready result, and fixed bootstrap
health retains the exact cause for later one-time import. Shutdown closes the
Stream before the clock dependency and returns one idempotent bounded outcome.

[clock]: ../02-Host-API/02-Monotonic-Clock.md
[lifecycle]:
  ../01-Architecture/01-Layer/02-Interaction-Assembly-and-Lifecycle.md
