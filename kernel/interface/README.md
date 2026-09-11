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
