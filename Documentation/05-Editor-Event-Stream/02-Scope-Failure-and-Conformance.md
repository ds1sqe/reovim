# Event Scope, Failure, and Conformance

[Editor Event Stream index](README.md)

## Scope boundary

Normal operation uses one volatile stream. Crash persistence is outside this
v0.16 contract and cannot introduce a second normal logging pipeline.
If separately justified later, persistence would consume the same admitted
records and canonical rendering through its own evidenced Host capability.

`reovim log`, stderr, file output, replay, and Client diagnostics are consumers
of this one stream and canonical renderer. They do not define another severity,
schema, ordering, retention, or redaction authority. The stream is the editor's
dmesg-like operational surface, not authoritative editing state.

## Relationship to trace, causal debugging, and snapshots

The [Execution and Concurrency contract](../01-Architecture/04-Execution/05-Causal-Debugging-and-Evidence.md)
owns Causal Debug Graph relations, reverse queries, state-reference semantics,
and development capture. The [Performance contract](../04-Performance/02-Scenarios-Bundles-and-Traces.md) owns
Causal Interaction Trace, sampling, completeness, clock domains, overhead, and
claim evidence. This contract owns only Event admission, retained ring, canonical
rendering, loss, and their separation from those evidence systems.

Events, trace nodes, graph nodes, and snapshots may share correlation identity,
but Event presence never proves causal completeness and graph retention never
satisfies Event delivery. Full traces, graphs, state references, and snapshots
remain in separately bounded access-controlled evidence stores and never enter
the 1 MiB Event Ring.

The Event Stream receives only a bounded actionable summary containing the
applicable trigger/proof, policy/disposition, safe state stamps, graph/snapshot
reference or typed unavailable, gap counts, supervisor handoff, and remediation
reference. Panic hooks and fatal paths do not synchronously render or fan out
Events. Missing Event delivery is visible but never blocks evidence sealing or
fatal disposition.

## Failure and recursion rules

Event, ring, renderer, subscriber, DiagnosticOutput, graph, trace, or snapshot
failure never changes an editing result, becomes a state authority, waits on
the failing path, or recursively reports through itself. Each subsystem keeps
fixed bounded non-recursive health state sufficient to make pressure, loss,
failure, saturation, and shutdown outcome inspectable.

Development fatal capture attempts one best-effort correlated Event summary,
but Event delivery is never a prerequisite for evidence sealing or process
termination. Production observation never panics, aborts, kills, unlocks,
retries, restarts, or rewrites state because of a diagnostic classification.

## Conformance evidence

Conformance requires schema and bounds tests; provenance and actionable-
category tests; total-order concurrency tests; Stream-incarnation stale cursor
tests; exact pre-mutation byte-accounting and wrap tests; redaction-before-fan-
out tests; snapshot/follow race and Event Gap tests; deterministic rendering
goldens across all destinations; concurrent subscribe/unsubscribe at per-consumer and
subsystem-wide record/byte limits; slow, failed, and closed consumer tests;
recursion and allocation failure tests; cancellation and shutdown tests; and
the overhead workloads in [Performance](../04-Performance/03-Path-Overhead-and-Regression-Evidence.md). It also covers
trace/graph/snapshot separation, bounded summaries, fatal-path recursion
failure, and production observation-only behavior. Target-contract prose is
not evidence that any of these obligations has passed.
