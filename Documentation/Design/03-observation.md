# Observing changes

A snapshot is a coherent read of state at one boundary. A watch is a Resource
owning a subscription to later changes; Runtime retains its delivery position.
Coherent observation creates both together so an edit cannot slip between the
initial read and subscription. External notifications promise no coherent
baseline unless their provider supplies that authority. Event is a typed payload
carrying subject, relevant state/lifetime and known request cause.

```text
watch(subject)   -> future events from the registered boundary
observe(state)   -> snapshot + watch at one coherent boundary
// State consumers use observe; separate read-then-watch can race.

delta(before, after) -> apply only to the exact predecessor/interpretation/watch
invalidation        -> discard currentness; supplies no replacement content
gap                 -> stop dependent updates, recover, fence old delivery
// UDP/GPIO may offer no replay/snapshot; keep loss visible under live policy.
```

## Independent subscriptions

```rust
enum WatchSelection { Subject, Children, Subtree }
struct WatchOptions {
    selection: WatchSelection,
    mask: u64,       // Bits defined by the chosen event API, not grants.
    capacity: u32,   // Requested bounded per-reader queue.
}
struct Observation<E> {
    watch: Watch<E>,
    baseline: Option<Snapshot>,
    watermark: u64,
}
enum Next<E> {
    Items(Vec<(u64, E)>),
    Pending,
    Gap { first: Option<u64>, last: Option<u64> },
    End { lost: bool, error: Option<Error> },
}
```

The selected feed owns Event's schema. A declared NEXT ioctl returns its value;
an optional typed helper decodes Next<Event>. A blocking helper awaits
readiness; a nonblocking read may return Pending. Both preserve gap and end.

```rust
// Proposed feed helper using caller access; source is an authorized Resource.
let first = feed.watch(source).await?;
let second = feed.watch(source).await?; // A separate delivery position.
let next = first.next().await?;
first.close().await?; // Second and source remain alive.
```

next/close use the particular subscription and grants. Close is idempotent
and fences late delivery before reclaim. Registration may complete
remotely; a lost reply uses [request recovery](01.3-requests.md), so retrying
cannot create an extra watch. No callback runs under Core commit authority.

## State, gaps and close

[Representation](03.1-representation.md) owns concrete state observations.
Clients install/render them; rendering does not gate Kernel acceptance.
Acceptance and semantic updates may arrive in either order; correlate actual
request/state.

A watch is owned qualified Resource access; Watch<E> is its typed SDK wrapper.
Acquisition may return an auxiliary baseline snapshot. Items carry per-watch
sequence numbers, not Buffer revisions or counts of physical events. Buffer
and Session payloads may identify known accepted state and
[request causes](01.3-requests.md); external sources cannot fabricate them.

Baseline policy is part of the observation declaration: Coherent pairs a
snapshot and cursor at one authority boundary; Notifications supplies no
snapshot guarantee. Buffer/Session observation supports Coherent. External
inotify-style observation does not claim it merely because read precedes watch.
A coherent factory can stage the watch and auxiliary snapshot in one batch,
retaining events that arrive while the immutable snapshot provider is prepared.

Slow readers have independent queues. Overflow latches Gap and suppresses
further items until a new observation establishes a delivery boundary. Stateful
clients need a coherent baseline. After fresh observation succeeds, fence old
delivery, switch observations and release the old watch. Failure preserves old
access and loss evidence. A new baseline cannot reconstruct missing transitions.

Notification-only sources retain visible loss under their declared live-resumption
policy. Source loss follows that repair rule; lost service replies instead
recover the original delivered batch. Recovery cannot reconstruct events the
source never observed.

Ending a source drains queued items then returns End; an unresolved gap sets
lost=true. Closing a watch fences delivery/cancels waits and does not close its
source. Revocation follows access fencing; undisclosed events/references cannot
grant access outside the watcher's authority.

Subject watches retain identity semantics through rename/unlink; Children/
Subtree observe declared naming coverage. Replacement is explicit, not silent
retargeting. Providers state recursive coverage and rename correlation. Backing
watch generations identify their Buffer source association so detached sources'
queued events cannot apply to a replacement binding. No watch automatically
refreshes, writes, recursively saves or starts an unbounded reactive cycle.

Owners bound per-reader and aggregate retention and reserve independent gap/end
capacity. Slow readers cannot block acceptance or erase outcomes. The
[native encoding](06.4-io-providers.md) implements these rules using ordinary
factory/control operations; it does not add a separate watcher syscall.

Buffer, Resource effects, Representation, Session, module and Client
input/context have separately owned event contracts. Editing events carry
actual prior/new state and History edge; metadata changes invalidate. Do not
invent external causes, accepted events for rejection, or a universal
domain/viewport enum.

Request results, semantic events and [diagnostics](../debugging.md) have
distinct duties: an event cannot settle an unknown external effect. The
diagnostic stream stays singular; a log view cannot generate itself
recursively.

[Service examples](07.4-service-workflows.md) apply these contracts to language
support and peer focus after introducing Sessions and modules.
