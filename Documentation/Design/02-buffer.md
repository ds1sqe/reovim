# 02 — Buffer

A Buffer owns working state and its History. Opening a source creates a new
Buffer; joining an existing Buffer shares its state and History.

```rust
// Proposed Buffer wrapper; source is a granted file containing "# Draft".
let draft = buffers.open(source).await?;
let copy = buffers.open(source).await?; // Independent edits and History.
let joined = buffers.join(draft.state.buffer).await?; // Shares draft.
// Saving draft does not merge or synchronize copy.
```

For the Markdown example, these roles separate storage from interpretation:

| Role | Example responsibility |
| --- | --- |
| State Model | Store working source units and define valid initial state |
| Domain | Define Text/Markdown values, valid positions and edit meaning |
| Representation | Read that meaning from working state and map edits back |
| Client | Turn semantic data into its own source or preview view |

A Domain can reuse an existing State Model. A new storage structure needs a
model provider; it does not need a new kind of Kernel Buffer.

Models, Domains and Representations can be supplied by
[loaded providers](06.1-providers.md). Their public contracts must admit new
working-state types without rebuilding Kernel. Buffer/History retains the
data and provider code needed by accepted states; unload cannot invalidate it.

Continue with [History](02.1-history.md), [observation](03-observation.md),
[Representation](03.1-representation.md), [editing](03.2-editing.md) and
[publication](03.3-publication.md). Calls use ordinary
[Resource access](01.2-resource-access.md) and its request outcomes.

## Observed state

```rust
// Path: kernel/interface/src/buffer.rs; module: interface::buffer
pub struct BufferState {
    pub buffer: ResourceId,
    pub revision: u64, // Currentness within this Buffer lifetime.
}
pub struct BufferDescriptor {
    pub state: BufferState,
    pub history: ResourceId,
    pub backing: Option<ResourceId>, // Not an input link or export target.
}
```

Currentness identifies the exact state against which a request is valid.
BufferState is an observation, not mutation authority. Core advances its
revision with working state; a History node is a separate identity. Changing
backing invalidates prior observations without creating a History node.

## Access and initial input

The selected State Model defines its initial value and encoding. There is
no universal Initial type to import. This logical model supplies a boolean:

```rust
// Path: domain/logical/src/initial.rs; module: logical::initial
pub struct LogicalInitial {
    pub active: bool, // Initial logical working value, with no hardware effect.
}
```

```rust
// Path: kernel/interface/src/buffer/api.rs; module: interface::buffer::api
pub enum ClosePolicy {
    RejectModified, // Refuse loss of unpublished working state.
    DiscardLocal,   // Authorized local loss, never external-effect rollback.
}
```

A Buffer helper encodes the selected model input over Resource access.
Runtime validates compatibility and grants; Rust types alone establish
neither. Descriptors are SDK-owned metadata, not mutable state or leases.

| Call | Result |
| --- | --- |
| create(initial) | Independent state/History from model input; backing is absent |
| open(source) | Independent state/History from supported source state and its base |
| join(buffer) | Access to existing state/History and its retained base |
| describe(buffer) | Coherent current metadata; no active-Buffer switch or close |
| close(expected, policy) | Close this shared Buffer under owner authority and exact currentness |

Create needs no filename or mmap. Open preserves uninterpreted source units;
partial residency is not absence. Live endpoints without state access reject.
Settle startup races before publishing a descriptor; failure keeps unfinished
rollback owned and exposes no half-open Buffer. Joining neither reopens the
source nor merges independent Buffers. Lookup uses the
[namespace](01.1-resource-tree.md); file syntax belongs to its adapter.

## Caller

```rust
// Proposed model helper; buffers uses caller access and the logical contract.
let initial = LogicalInitial { active: false };
let buffer = buffers.create(&initial).await?;
// buffer is the actual descriptor. RequestError carries any unknown outcome.
```

Independent-Buffer synchronization still needs a defined sharing scope,
authority, conflict behavior and receiving History effect. Sharing Client
context does not supply that contract; join is the way to share one Buffer.

## Close and retained memory

Close checks shared-Buffer authority and exact currentness. RejectModified
protects unpublished state, including unbound/unexported data. DiscardLocal
authorizes local loss; it cannot roll back an external effect.

```text
save selected state -> await proved Applied -> describe current Buffer
  -> close(current.state, RejectModified)
// Later edits survive. Save completion never authorizes their discard.
```

Admission is not teardown completion. Runtime fences new work and drains or
transfers ownership; incomplete-shutdown stays progressable. Close is
idempotent for that lifetime; stale references never close replacement subjects.
Releasing a descriptor or caller view leaves shared state alive. Buffer close
deletes no backing and stops no independently owned source.

The initial `kernel.buffer.close` v1 profile writes little-endian revision:u64
followed by policy:u8 (0 = RejectModified, 1 = DiscardLocal), with one reference
to the expected Buffer. The creating caller scope owns shared close; ordinary
write permission alone does not transfer that authority. Retained work can
return busy before teardown. Closed identity metadata permits idempotent close
without retaining model code. Current unbound Buffers require DiscardLocal;
publication must establish the evidence for RejectModified success.

Selected data is owned or read-leased; a revision alone retains nothing.
Sparse/partial residency is normal. Borrowed inputs last for the call; queued
work captures needed input before handoff. Caller input stays unchanged;
SDK-owned results use that SDK's allocator. Kernel never retains/frees
caller-owned output.

Direct views stay pinned through their borrows. Checked reads may expire;
pointers cannot dangle. Close drains/transfers borrows and recovery ownership
while protecting unsaved/unexported data. See
[request lifetime](01.3-requests.md) for result ownership.
