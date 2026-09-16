# Module-owned data and executable interfaces

This chapter owns domain API examples and the Parse signature. These are target
contracts; examples do not establish implemented SDK support.

[Composition](05.1-composition.md) owns dependencies: domain API packages belong
to provider and interested Client modules. The common Kernel/server/SDK/base
Clients use generic declarations, values and access. These examples show
extension-owned meanings and typed use of that shared boundary.

## Parse is an executable handle, not a data type

The parser API package exports a concrete SDK handle. Its private ResourceHandle
contains caller access and a checked binding to the declared parser operations.
[Resource access](01.2-operations.md) owns binding and Request outcomes.

```rust
// In the proposed parser API package, shared by callers and implementations.
pub struct Parse {
    access: ResourceHandle, // Private, constructed only after checked binding.
}
pub struct ParseResult {
    pub input: BufferState, // Buffer identity and selected revision.
    pub tree: TreeSnapshot, // Owned, immutable result access.
}
impl Parse {
    pub fn exec(&self, input: &TextSnapshot) -> Request<ParseResult>;
}
```

Signatures here omit implementation bodies. Parse.exec captures READ access to
an immutable Text snapshot; its return contains the same input provenance and
owned Tree snapshot access. It does not accept arbitrary bytes, ResourceId or a
mutable Buffer disguised as a snapshot. It does not edit the source Buffer.
Parse declares Factory-dispatched Exec: the created Resource is the Tree
snapshot, and its auxiliary output is the input BufferState. The wrapper
combines those fields into ParseResult; the host never needs a parser-specific
result tag.

```rust
// Inside an optional Client module, not the base client/kernel/server.
use parser_api::{Parse, ParseResult}; // Proposed extension API package.
use text_api::TextSnapshot;

let buffer = k.buffers.find(buffer_id).await?;
let target: TextSnapshot = buffer.snapshot().await?;
let parse: Parse = k.resolve(b"/modules/treesitter/parse").await?;
let result: ParseResult = parse.exec(&target).await?;
```

The Rust type of variable parse is Parse, not ResourceHandle<Parse>. Parse is
not a zero-sized tag, the parser implementation, or the returned syntax tree.
The bound provider can change without recompiling this caller if it implements
the same API. A provider cannot change the signature merely by using this path.

## Data interfaces and their values

Bytes/Text/Tree/Mesh name APIs, not Kernel storage kinds. Each API package owns
its value types, methods, reference rights and representation rules. Provider
state is private and can use different storage structures. Snapshot wrappers
retain one immutable subject/state, with operations defined by the selected API.

| Interface | Value/coordinate types | Snapshot operations |
| --- | --- | --- |
| Bytes | ByteRange, Vec<u8> | len, read_at |
| Text | ScalarRange, TextUnit | len, read_units |
| Tree | NodeId, Node, Subtree | root, node, children |
| Mesh | VertexId, FaceId, Vertex, Face | vertex, face, enumerate IDs |

```rust
// In the respective Bytes, Text, Tree and Mesh extension API packages.
pub struct ByteRange { pub start: u64, pub end: u64 }
pub struct ScalarRange { pub start: u64, pub end: u64 }
pub enum TextUnit {
    Scalar(char),
    Opaque(u8), // One preserved uninterpreted source byte.
}
#[derive(Clone, Copy, Eq, PartialEq, Ord, PartialOrd)]
pub struct NodeId(pub u64);
pub struct NodeKind(pub String); // Tree API's declared kind vocabulary.
pub struct Node {
    pub id: NodeId,
    pub kind: NodeKind,
    pub children: Vec<NodeId>,
}
pub struct Subtree { pub root: NodeId, pub nodes: Vec<Node> }
#[derive(Clone, Copy, Eq, PartialEq, Ord, PartialOrd)]
pub struct VertexId(pub u64);
#[derive(Clone, Copy, Eq, PartialEq, Ord, PartialOrd)]
pub struct FaceId(pub u64);
#[derive(Clone)]
pub struct Vertex { pub position: [f64; 3] }
#[derive(Clone)]
pub struct Face { pub vertices: Vec<VertexId> }
```

Ranges are half-open; separate types prevent mixing byte and Text-unit
positions. These IDs address parts of the selected Tree/Mesh state, not Kernel
Resources. Enumerations are bounded/paged by those APIs; they do not require
materializing an unbounded tree/mesh in one result. NodeKind and allowed subtree
content are declared by the Tree API; parser results may supply a syntax-tree
specialization. Mesh positions use the Mesh API's explicitly declared coordinate
frame/unit; replacing geometry never infers meters or a transform from three
floats alone.

For Text, positions count Unicode scalars plus explicitly preserved opaque units
as the existing Text requirement specifies; byte offsets are a different API.
Grapheme motion stays in Client/domain logic. Read-only Text interpretation
over Bytes must preserve the selected source and report incompatible input;
binding a view cannot silently transcode or modify it.

## Domain edit values

```rust
// Each edit type is declared in its domain's extension API package.
pub enum BytesEdit {
    Replace { range: ByteRange, data: Vec<u8> },
}
pub enum TextEdit {
    Replace { range: ScalarRange, units: Vec<TextUnit> },
}
pub enum TreeEdit {
    ReplaceSubtree { node: NodeId, replacement: Subtree },
}
pub enum MeshEdit {
    MoveVertex { vertex: VertexId, position: [f64; 3] },
}
```

Each enum belongs to its API package; adding a domain does not extend one
Kernel-wide edit enum. Subtree is an owned validated tree value with explicit
node references; providers validate ranges, references and domain invariants
before returning a candidate. These are useful witnesses, not complete lists
of all future domain operations.

```rust
// Inside a Client module that depends on the Mesh API package.
let original: MeshSnapshot = buffer.snapshot().await?;
let changed = buffer.edit(&original, MeshEdit::MoveVertex {
    vertex: VertexId(7),
    position: [1.0, 2.0, 3.0], // In this Mesh's declared coordinate frame.
}).await?;
```

The snapshot identifies the target Buffer/revision and required Mesh interface.
changed reports the accepted BufferState. [Buffer](02-buffer.md) owns atomic
acceptance; [state providers](06.3-state-providers.md) own preparation callbacks.
These domain values use that shared mechanism.

Two supported interfaces on one Buffer still use one identity and History.
Bytes/Text views can share source state and coordinate conversion rules. A
derived syntax tree or converted mesh is a separate result unless the provider
explicitly declares an invariant-preserving view of the same state. Resolving
an interface alone does not parse, convert or create a second Buffer.

## Mapping an extension API to provider interfaces

An ordinary Mesh Kernel module supplies MeshInit creation, immutable
vertex/face reads and MeshEdit preparation. It uses the
[state-provider records](06.3-state-providers.md)
for private state, factories and candidates. That chapter owns staging,
publication, currentness and cleanup; this API defines the geometry meaning.
For example, MoveVertex rejects a missing vertex or non-finite coordinates.
Storage and preparation algorithms remain provider-private.

The Mesh API package owns its declarations; the module implementation supplies
private context and handlers; Kernel Interface declares the common provider
contracts. Runtime binds and drives those handlers. A Buffer uses the common
[Resource identity model](01-resources.md) with editable state: these provider
interfaces do not introduce another extension layer between Resource and
Buffer. [Kernel modules](05.2-modules.md) owns contribution terminology.

BytesSnapshot.read_at uses the standard positional operation described by
[Resource I/O](01.4-io.md). Its native callback and transfer result use
[opened I/O records](06.4-io-providers.md). Text, Tree and Mesh reads use their own
declared control operations; adding them does not extend the native I/O table.
Domain event APIs also use that chapter's common watch delivery protocol.

A separate triangulate executable may declare MeshSnapshot -> MeshSnapshot at
/modules/mesh/triangulate. As with Parse, its API specifies the transformation
and result meaning; the shared factory protocol owns creation and transfer.

The [map journey](07.6-map-journey.md) follows a .map file into a Buffer,
separates camera movement/zoom from object edits, and traces Save across owners.

## Aggregate creation

The repository API owns RepositoryFactory, RepositoryInit and CapturePolicy.
Its initializer contains [source entries](02.2-sources.md) and a capture policy:
PerSource selects individually coherent inputs; Joint requires joint authority.

```rust
// Inside a module using the repository API; entries carry granted source access.
let factory: RepositoryFactory = k.resolve(b"/modules/git/workspace").await?;
let repo = factory.exec(&RepositoryInit {
    sources: entries,
    capture: CapturePolicy::PerSource,
}).await?;
```

The result is one Buffer with one History, as in the [Buffer graph](02-buffer.md).
Factory selection and initialization use this ordinary declared operation.
