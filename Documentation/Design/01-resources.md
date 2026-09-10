# 01 — Resources

A Resource is an addressable subject with typed operations. Files, memory,
Buffers, History, Sessions, connections, clipboard, settings and logs share
this identity model. Scalar fields and payloads need no separate Resource.

```text
files/notes.md       // Source/destination with its own access interfaces.
buffers/draft       // Working state, independent of the source file.
buffers/draft/history // The Buffer's accepted editing states.
settings            // A service-owned subject; inspection needs no Buffer.
// Names resolve to ResourceIds. A Resource need not be a byte file.
```

Use the [tree](01.1-resource-tree.md) to find a Resource, then
[read or write](01.2-resource-access.md) through caller access. A connection
is that access to one Runtime; it may be local or remote. It owns caller
grants and request tracking. Ordinary calls return their results directly.

## Identity and access

```rust
// Path: kernel/interface/src/resource.rs; module: interface::resource
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ResourceId(pub u64); // One namespace, including Buffer and Session.
// Scoped to this Runtime connection; copying retains/grants nothing.
// Old IDs never name replacement subjects. Cross-connection use must resolve.
```

An interface declares a versioned operation/value contract. A capability means
support for it; a grant authorizes its use. Readiness and request validity are
separate checks. Calls check the requested contract and access; unsupported or
denied use rejects. There is no global ResourceFlag inventory or parallel
BufferId/SessionId.

A Resource need not offer bytes, paths, snapshots, revisions, seek, save or
Undo. Locators find subjects; samples, Buffer revisions and receive cursors
have different meanings. Native details stay in adapters.

Sequential read, random access, message receive and semantic commands belong
to their typed access interfaces. One Resource may support several of them;
module kind or tree position implies none. [Providers](06.1-providers.md) owns
extension/registration; user-view input/output remains Client responsibility.

## Relationships and lifetime

```text
input source -- capture/import --> Buffer -- save --> backing subject
                                    |
                                    +-- export --> chosen destination
// Each relationship is optional and explicit.
// Another Buffer from the same source has independent state/History.
// Join shares an existing Buffer; grouping by source does not synchronize it.
```

Sharing and independent state are explained with [Buffers](02-buffer.md).

References may form cycles, including self-reference. Inspection transfers no
ownership or downstream grants and triggers no History recording or
publication. Owners bound traversal/retention; reference cycles cannot imply
retention cycles.

Metadata copies pin nothing. A lease retains declared data for an access
lifetime; references alone do not. Access/leases retain only declared backing.
Releasing access, closing a shared subject and deleting persistent data are
separate operations; close accounts for pending work and direct borrows.

## Errors

```rust
// Path: kernel/interface/src/error.rs; module: interface::error
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ErrorCode(pub u64); // Versioned codes; unknown values stay valid.

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Error {
    pub code: ErrorCode, // Cause; does not prove whether an effect occurred.
    pub id: Option<ResourceId>, // Optional diagnostic Resource; pins nothing.
}
// Allocation-free cause. Details may expire or be denied. RequestError owns
// the separate, required recovery reference when the outcome is unknown.
```

Interface owns codes for denied, unsupported, invalid, unavailable, stale,
conflict, ambiguous, unrepresentable, pressure, cancelled, replaced,
not-ready, closing, closed, no-history, unbound, dirty, snapshot-expired,
busy, incomplete-shutdown and provider-error. Distinguish connection loss,
timeout and unavailable evidence. Unknown codes never authorize retry.

The [request API](01.2-resource-access.md) distinguishes rejected, failed and
unknown requests. Provider-specific causes belong in scoped details.
An `error/{id}` view may expose redacted [diagnostics](../debugging.md); lookup
cannot require another detail record recursively. These are same-build Rust
values; C layout and wire encoding are not selected.
