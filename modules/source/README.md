# Source byte model

This native State Model stores exact bytes under one Buffer/History authority.
Text/Markdown interpretation belongs to consumers of its public byte API.
Load the `source` example library as an ordinary native module; the server
needs no compiled dependency on this implementation.

| Contract v1 | Request | Reply |
| --- | --- | --- |
| source.initial | Exact initial bytes; empty creates an empty source | Buffer observation with byte length as u64 descriptor |
| source.bytes | Empty for descriptor, or start:u64/end:u64 for a window | Length:u64, or start:u64/end:u64 followed by exact bytes |
| source.splice | Start:u64/end:u64 followed by replacement bytes | Buffer observation with new length descriptor |

Integers are little endian; ranges include start and exclude end. Byte ranges
may select any source unit, including invalid UTF-8. There is no lossy decoding.
Use `kernel.buffer.retain` to acquire a snapshot, then read its `source.bytes`
windows. Splice uses the common Buffer expected-revision/reference envelope;
invalid ranges and stale input reject without changing state or History.
The Rust `api` helpers preserve the requested window's state/range association.

Immutable states share unchanged byte segments. Preparation copies segment
metadata and replacement bytes; selected reads copy only the requested bytes.
Segment traversal is linear, and the first profile is fully resident. Runtime
message/history limits bound operations and retained states; initial input must
fit one message, while subsequent insertions may grow beyond that frame size.
State cost conservatively charges reachable byte allocations and segment
metadata per retained state, including storage behind slices.

`buffer::observe` acquires a retained source snapshot and change watch at one
boundary. `buffer::next` reports accepted Edit/Undo/Redo edges, Pending, Gap or
End through `kernel.buffer.next`; slow readers renew observation after Gap.
The current profile allows 32 watches per Buffer and 32 queued edges per watch.
Snapshot and watch close separately. Events qualify the current finite History
node ordinals and execution correlation token; they grant no mutation rights.

This is working-source creation, not file open/backing association, a partially
resident backend, or a Text Representation with coherent snapshot/watch. Those
consumers reuse this byte contract and the existing acceptance authority.
