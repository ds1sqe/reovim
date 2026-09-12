# Editing context model

This native Model retains an optional active Buffer and an ordered set of known
Buffers through the common Session operations. It uses public
`kernel.buffer.describe` reads to validate references and captures READ access;
it knows no Runtime types, text coordinates, viewport or renderer state.

Build the `context` example library and select `editing.context.model` v1.
Create a Session with `editing.context.initial`, update with
`editing.context.update`, and observe `editing.context`, all schema version 1.
The `api` module supplies `EditingSessionState` and codecs for Rust consumers.

Semantic data is one little-endian u64 active index. u64::MAX means no active
Buffer. Buffer identities live only in the ordered Resource slots. Duplicate
identities, an absent active member, non-Buffer subjects and closed Buffers
reject before installing context. This native profile allows at most 256
semantic reference slots; Runtime capacity may impose a smaller bound.

Session replies prepend Session and independently owned snapshot references.
Semantic references follow those two slots; snapshot reads return only the
semantic value. Updates prepend the same two currentness references to the new
known Buffer slots. Stale snapshots cannot authorize an update.

Focus changes do not edit Buffers or advance their History. Observers receive
READ rights to known Buffers, not incidental mutation or close authority.
Closing a Session leaves retained context snapshots and shared Buffers alive;
release each snapshot separately. Snapshots copy their values and retain the
referenced subjects, so they can outlive the context model's executable code.

`kernel.session.observe` acquires a snapshot and independent watch at one cut.
The common `session::Watching` helper decodes revision, snapshot, watch and the
semantic context value. `kernel.session.next` reports accepted revision edges,
Gap, End or passive Pending; empty input waits and byte zero polls. Each Session
allows 32 readers and 32 queued events per reader. A slow reader gets sticky
Gap and renews observation; it does not delay another reader. Release snapshots
and close watches independently.

This supplies passive follow data. Client policy chooses whose context to
follow and how to present it; observing grants no mutation authority. Full
participation/seek, open/backing ownership, browser transport and frontend
commands remain separate consumers.
