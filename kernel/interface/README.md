# Kernel Interface

Public Resource access and provider contracts, independent of the
[Runtime implementation](../runtime/README.md).

- `Connection`, `ResourceId` and `Message`: granted Resource requests.
- `ResourceProvider` and `ModelProvider`: supplied implementations.
- `Module`, `Factory` and `DriverOps`: construction and driven lifecycle.
- `abi`: C-compatible caller tables used by language adapters.

```rust
// The module's API package defines parse_contract; the common SDK does not.
let input = Message::reference(parse_contract, source);
let parsed = kernel.write(parse_resource, &input).await?;
```

Module-specific schemas and helpers belong with their module. Adding a service
must not add fields to the common Interface. Names and concrete storage remain
private to the implementation.

This crate uses `no_std + alloc` and has no dependency on Runtime or Clients.

## Backed opening

`buffer::open` writes `kernel.buffer.open` v1 to the selected Model. Data uses
the Model's initialization encoding; one READ reference names the source. The
current profile requires exactly one READ initialization slot and no variable
reference policy. The Model initializes from the selected source state;
transformations belong to subsequent edits.

Prepare any source I/O through its backend before this call. Runtime acquires
an inert independent read view that declares `resource.base` v1 alongside the
Model's data surfaces. That view owns one coherent immutable data/base version;
the base's meaning stays with the backend. Runtime retains the actual source
separately and translates the view into the Model's receiving namespace.

The reply is a Buffer descriptor: revision:u64 little endian and Buffer,
History, source references. Decode it with `buffer::descriptor`. Buffer/History
receive normal owner access; the source slot conveys identity only. Describing
a backed Buffer likewise grants no extra source operations to observers.

Buffer/History and association install together after initialization succeeds.
Failed or cancelled initialization exposes no Buffer. Ordinary Model creation
remains unbound even when it reads a source. RejectModified accepts a backed
Buffer at its original source state, including Undo to that state; later edits
remain unpublished. Closing one Buffer releases its source/base leases, not
the shared source or independently retained working state.

This is the initial association mechanism under the
[Buffer contract](../../Documentation/Design/02-buffer.md). It does not implement
file saving, Save As, rebinding, or filesystem publication guarantees.
