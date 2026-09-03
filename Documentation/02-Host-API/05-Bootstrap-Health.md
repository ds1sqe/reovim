# Bootstrap Health

> **Normative contract index.** The linked chapters jointly define the one
> non-recursive bootstrap-health state and its OutputHealth partition. This
> index is navigation, not a second authority.

[Host API index](../02-Host-API.md)

## Reading order

1. [Ownership](05-Bootstrap-Health/01-Ownership-Schema-and-Counters.md)
   defines the one Event Stream-owned state, partition, descriptor, bounds,
   and exact counter bank.
2. [OutputHealth proposals](05-Bootstrap-Health/02-Output-Health-Proposals.md)
   defines the complete pre-sequence write vocabulary and its typed
   relations.
3. [Retention](05-Bootstrap-Health/03-Retention-Lease-and-Snapshot.md)
   defines anomaly sequencing, capacity behavior, write authority, and owner
   observation.
4. [Import](05-Bootstrap-Health/04-Import-Readiness-and-Failure.md)
   defines frozen import, startup summary projection, and degraded-readiness
   behavior.

Clock and Event Stream projections require separately versioned schemas. Every
partition remains inside the same aggregate record and resident-byte bounds.
