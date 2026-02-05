# buffer/ - Buffer Manager Driver

Typed key and registry for buffer manager lookup.

## Source Location

`server/lib/drivers/buffer/src/`

## Purpose

Defines the typed key and registry for buffer manager implementations. The
`BufferManager` trait itself is defined in the kernel; this driver provides
the registration mechanism.

Following the mechanism/policy separation:
- **Mechanism** (kernel): `BufferManager` trait
- **Mechanism** (this driver): `BufferManagerKey`, `BufferManagerRegistry`
- **Policy** (modules): Implementations like `SimpleBufferManager`

## Architecture

```
lib/kernel/           -> BufferManager trait (MECHANISM)
lib/drivers/buffer/   -> Key + Registry (MECHANISM)
modules/buffer-simple/-> SimpleBufferManager implementation (POLICY)
```

## Key Types

```rust
/// Typed key for buffer manager lookup
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum BufferManagerKey {
    Simple,
    // Future: Rope, Gap, etc.
}

/// Registry for buffer manager implementations
pub struct BufferManagerRegistry {
    managers: RwLock<HashMap<BufferManagerKey, Arc<dyn BufferManager>>>,
}

impl BufferManagerRegistry {
    pub fn register(&self, key: BufferManagerKey, manager: Arc<dyn BufferManager>);
    pub fn get(&self, key: &BufferManagerKey) -> Option<Arc<dyn BufferManager>>;
}
```

## Test Utilities

```rust
/// Test buffer manager for unit testing
pub struct TestBufferManager {
    // Simplified implementation for tests
}
```

## Dependencies

- `reovim_kernel::api::v1::BufferManager` - The trait being registered
- `reovim_arch::sync::RwLock` - Thread-safe locking

## Related Documents

- [Driver Overview](../overview.md)
- [buffer-simple Module](../../modules/builtin/buffer-simple.md)
