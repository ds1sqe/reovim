# Diagnostic Output and Bootstrap Health

[Host API index](../02-Host-API.md)

## DiagnosticOutput

**DiagnosticOutput** accepts already-redacted, canonically rendered bytes from
the [Editor Event Stream](../05-Editor-Event-Stream/README.md). It owns only the
private mechanics needed to route those bytes to configured diagnostic
destinations, such as a file or standard error.

Its descriptor declares at least contract identity/version, maximum record
size, queue record and charged-byte capacity, invocation behavior, lifecycle
owner, and shutdown policy. One bounded submission returns exactly one of:

- **accepted:** the adapter took ownership within its declared bounds;
- **dropped:** it did not accept the record, with a typed pressure or output
  failure cause; or
- **closed:** its lifecycle no longer accepts records.

Accepted transfers ownership of the exact byte carrier to DiagnosticOutput.
Dropped and closed return ownership to the caller without retaining a borrow,
pointer, or partial copy.

Accepted does not imply crash durability. Output failure never changes an
editing result and must not synchronously call back into the Event Stream.
Every drop is visible through non-recursive bounded counters owned by the
stream. Closing stops new admission; shutdown has a bounded drain-or-discard
outcome and is idempotent.

DiagnosticOutput must not assign Event sequence or timestamp, redact, render,
retain the Event Ring, or create another logging pipeline.

## Operation classifications

Submission is exactly:

```text
NonBlockingInvocation + InlineTerminal + Single + LocalOnly
+ NoUserCallback + NonReentrant
```

Its terminal effect is a bounded local ownership transfer. A private
destination attempt is a separately classified `ExternalMutation` that
preserves Effect Certainty and Outcome Cause. Submission never claims that the
destination wrote, flushed, or durably retained the record.

Activation and shutdown are:

```text
CallerMayWait + DeferredTerminal + Single + LocalOnly
+ NoUserCallback + NonReentrant
```

Shutdown relays per-record destination certainty and cause but grants no new
destination-mutation authority. Output-health record and snapshot operations
are bounded LocalOnly operations with all six axes declared.

## Non-recursive bootstrap health

Clock, Event Stream, and DiagnosticOutput bootstrap and failure reporting must
not recursively require the failed capability. One fixed bounded bootstrap
health state records failure before the Event Stream is ready or while output
is unavailable.

Its descriptor names one Event Stream owner, schema/version, maximum record
and resident-byte counts, counter widths and saturation behavior, record-order
rule, lifecycle, and one-time import order relative to ordinary Event
admission. Assembly wires a write-only health lease; the lease has no
backreference and is not a third Host Capability.

If the stream later becomes ready, it imports bootstrap health as
provenance-stamped Events without erasing or taking ownership of the counters.
Failure to import remains visible in the fixed state and never recursively
uses the failed output.

Failure to admit the one startup summary does not change an editing result or
silently block readiness. A required Event Stream that fails its own readiness
predicate prevents Kernel readiness. If the stream is Ready but startup
summary admission fails, the Lifecycle Owner may publish only the descriptor's
explicit diagnostics-degraded Ready result, naming the fixed health cause and
remaining safe operations.
