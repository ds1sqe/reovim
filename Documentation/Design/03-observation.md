# 03 — Observing changes

A snapshot is a coherent read of state at one boundary. A watch is a Resource
owning a subscription to later changes; Runtime retains its delivery position.
Observe creates both together so an edit cannot slip between the initial read
and subscription. Event is the selected interface's typed payload, carrying
subject, relevant state/lifetime and known request cause.

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
// Path: kernel/interface/src/notify.rs; module: interface::notify
pub enum Next<Event> {
    Item(Event),
    Pending, // A nonblocking read found no event; await readiness.
    Gap,     // Renew coherent observation before using dependent updates.
    End { lost: bool, error: Option<Error> },
}
```

The selected feed owns Event's schema. A common read returns its message;
an optional typed helper decodes Next<Event>. A blocking helper awaits
readiness; a nonblocking read may return Pending. Both preserve gap and end.

```rust
// Proposed feed helper using caller access; source is an authorized Resource.
let first = feed.watch(source).await?;
let second = feed.watch(source).await?; // A separate delivery position.
let next = feed.read(first).await?;
feed.unwatch(first).await?; // Second and source remain alive.
```

Read/unwatch validate the particular subscription and grants. Unwatch is
idempotent and fences late delivery before reclaim. Registration may complete
remotely; a lost reply uses [request recovery](01.3-requests.md), so retrying
cannot create an extra watch. No callback runs under Core commit authority.

## State, gaps and close

[Representation](03.1-representation.md) owns concrete state observations.
Clients install/render them; rendering does not gate Kernel acceptance.
Acceptance and semantic updates may arrive in either order; correlate actual
request/state.

Owners bound per-reader and aggregate retention and reserve independent
gap/end capacity. Slow readers cannot block acceptance or erase outcomes.
Normal close drains final records or sets lost; revocation cannot reveal
queued protected data. Connection-owned retry bookkeeping prevents repeated
reads from consuming another event; it never replaces visible gap/closure
semantics.

Buffer, Resource effects, Representation, Session, module and Client
input/context have separately owned event contracts. Editing events carry
actual prior/new state and History edge; metadata changes invalidate. Do not
invent external causes, accepted events for rejection, or a universal
domain/viewport enum.

Request results, semantic events and [diagnostics](../debugging.md) have
distinct duties: an event cannot settle an unknown external effect. The
diagnostic stream stays singular; a log view cannot generate itself
recursively.

[Service examples](07.2-service-workflows.md) apply these contracts to language
support and peer focus after introducing Sessions and modules.
