# Monotonic Clock

[Host API index](../02-Host-API.md)

## Capability and descriptor

**MonotonicClock** supplies opaque instants for duration measurement,
deadlines, Event timestamps, and benchmark boundaries. Its descriptor fixes:

- contract identity, selected version, and compatible version range;
- explicitly supplied capability instance, incarnation, and generation;
- nanosecond duration unit, observable resolution, and maximum representable
  elapsed duration;
- lifecycle owner and readiness predicate; and
- the exact operation classifications below.

The descriptor and its values are Kernel Interface types. It contains no
hosted clock, wall-clock epoch, platform handle, path, scheduler, or adapter
type. Contract compatibility is independent of package version.

## Instant semantics

An instant is opaque and meaningful only within the exact instance,
incarnation, and generation that created it. The adapter internally selects
one monotonic origin for each incarnation. The portable carrier represents the
whole number of nanoseconds elapsed from that private origin; it does not expose
the origin or give the scalar wall-time meaning.

A later successful observation never precedes an earlier successful
observation from the same incarnation. Equal consecutive instants are valid.
The descriptor's resolution is evidence about distinguishable observations,
not permission to synthesize progress or round a result upward.

Same-incarnation comparison and duration derivation are local checked
operations. They reject a different instance, incarnation, or generation
before arithmetic. A later-minus-earlier duration rejects observed regression
before subtracting. Because both elapsed scalars and the checked duration are
unsigned 64-bit nanoseconds, a validated later-minus-earlier result cannot
overflow. Overflow remains applicable to hosted-source conversion and to
checked Deterministic Test Adapter advancement.

Historical instants from one closed incarnation remain comparable with each
other. They cannot be installed as current timestamps after Closing, Closed,
or replacement. Replacement always receives another incarnation and generation
even when the stable capability instance identity is unchanged.

Wall time is absent. It cannot order Transactions, Events, lifecycle, or work.
Wall-clock presentation metadata is a separate future capability only if a
named consumer satisfies full admission.

## Operations and outcomes

Descriptor inspection and negotiation are:

```text
NonBlockingInvocation + InlineTerminal + Single + LocalOnly
+ NoUserCallback + NonReentrant
```

One clock observation is:

```text
NonBlockingInvocation + InlineTerminal + Single + ExternalObservation
+ NoUserCallback + NonReentrant
```

Instant comparison and duration derivation are:

```text
NonBlockingInvocation + InlineTerminal + Single + LocalOnly
+ NoUserCallback + NonReentrant
```

Activation and shutdown use the lifecycle operation descriptors selected by
the owning Runtime. They are never hidden inside observation.

The closed outcome categories are:

- incompatible or unsupported contract;
- not ready;
- invalid cross-instance or cross-incarnation use;
- stale or replaced generation;
- observed regression;
- source-conversion, deterministic-advancement, or descriptor exhaustion;
- Closing; and
- Closed.

An immediate observation retains no work and justifies no per-call deadline or
cancellation token. This does not remove instance ownership, generation
fencing, close behavior, or the requirement to retain evidence for the
immediate/no-wait classification.

## Adapter obligations

The Standard Host Adapter uses the hosted implementation's monotonic clock and
keeps its concrete instant private. Conversion to whole elapsed nanoseconds is
checked; exceeding the declared maximum is a typed observation failure rather
than saturation or wrap.

The Deterministic Test Adapter implements the same contract and advances only
through a separate test controller. It requires no real sleep, cannot move
backward, and exposes exact control of equal observations, checked advancement,
close, replacement, and overflow faults. The controller is not part of the
production Host capability.

Both adapters run one shared suite covering negotiation, pre-readiness use,
nondecreasing observation, equal instants, comparison, duration, cross-clock
rejection, overflow, activation rollback, read/close cuts, replacement,
post-close use, and idempotent shutdown.
