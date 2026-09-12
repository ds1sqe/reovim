# Source byte model

This native State Model stores exact bytes under one Buffer/History authority.
Text/Markdown interpretation belongs to consumers of its public byte API.
Load the `source` example library as an ordinary native module; the server
needs no compiled dependency on this implementation.

| Contract v1 | Request | Reply |
| --- | --- | --- |
| source.initial | Exact initial bytes; empty creates an empty source | Buffer observation with byte length as u64 descriptor |
| source.open | One READ reference to an immutable resource.snapshot; empty data | Independent Buffer observation with logical byte length |
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
Segment traversal is linear. The `source` module starts resident; the
`source_open` example starts with ranges backed by an immutable snapshot.
Only inserted bytes become resident model allocations. Runtime
message/history limits bound operations and retained states; initial input must
fit one message, while subsequent insertions may grow beyond that frame size.
State cost conservatively charges reachable byte allocations and segment
metadata per retained state, including storage behind slices.

Native retained reads use the optional Model `inspect` pending callback. The
resident implementation completes when polled; backed ranges await bounded
`resource.snapshot` reads. Acceptance descriptors remain finite and synchronous.
Opening and splice preparation do not read untouched backing ranges. Captured
backing access survives through every state that still references those ranges.

`buffer::observe` acquires a retained source snapshot and change watch at one
boundary. `buffer::next` reports accepted Edit/Undo/Redo edges, Pending, Gap or
End through `kernel.buffer.next`; slow readers renew observation after Gap.
The current profile allows 32 watches per Buffer and 32 queued edges per watch.
Snapshot and watch close separately. Events qualify the current finite History
node ordinals and execution correlation token; they grant no mutation rights.

The Linux Host can supply atomic file snapshots with server boot option
`--snapshot snapshots/input SOURCE SCRATCH_DIRECTORY`. Scratch must support
anonymous temporary files and atomic cloning on the source filesystem.
Load `source_open` and call `source.open` with `modules/snapshots/input` as its
reference. Each call creates independent working state/History from that
captured snapshot. A new file capture requires a new snapshot grant; this boot
profile does not yet provide interactive file selection or save association.

`resource.snapshot` v1 returns length:u64 for empty reads, or exact bytes for
a start:u64/end:u64 request of at most 32768 bytes. Meaning remains immutable
through access lifetime; unavailable windows fail explicitly. There is no
mtime-based assumption that an externally mutable file is a stable snapshot.
