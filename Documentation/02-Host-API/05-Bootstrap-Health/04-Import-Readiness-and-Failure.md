# Bootstrap-Health Import, Readiness, and Failure

[Bootstrap Health index](../05-Bootstrap-Health.md)

## Frozen bounded import

The first import attempt freezes the health high-water mark and loss counters.
Its input is one nonzero loss summary, when loss exists, followed by retained
anomalies through that mark in health-sequence order.

Only accepted Event admission advances the cursor. Another result leaves that
entry pending. Accepted entries are never replayed, later health records are
excluded, and one fixed aggregate attempt limit halts further work as pending.
Import never erases or acknowledges health state.

Imported Events preserve health provenance but receive new Event Stream
sequences. A live importer requires its own separately ratified Event family,
schema, redaction rule, canonical renderer, and admission relation. No generic
or recursive diagnostic Event is implied by the OutputHealth contract.

Import is:

```text
NonBlockingInvocation + InlineTerminal + Single + LocalOnly
+ NoUserCallback + NonReentrant
```

## Startup summary projection

The startup proposal carries only one `BootstrapHealth` summary marker:

- `Clear` when no frozen startup-health input remains to import;
- `Imported` when the frozen input was completely admitted; or
- `Pending` when loss or an anomaly through the frozen mark remains.

The marker is derived by the Event Stream Lifecycle Owner from the fixed state
and import result. It is not a state handle, lease, snapshot, health sequence,
or authority to alter pending work.

## Readiness and failure

Failure to admit the startup summary changes no editing state and does not
silently block readiness. A required Event Stream that fails its readiness
predicate prevents Kernel readiness.

If the Event Stream is Ready but summary admission fails, the Lifecycle Owner
may publish only the descriptor's explicit diagnostics-degraded Ready result.
That result names the fixed health cause and remaining safe operations. It
cannot claim full diagnostic health or discard pending evidence.

Clock, Event Stream, and DiagnosticOutput failures first use their fixed
non-recursive health partitions. They never call the failed capability, open a
second logging path, or synchronously re-enter Event admission. If the health
record operation itself cannot accept an anomaly, its exact result counter and
loss state remain the only bounded evidence available on that path.
