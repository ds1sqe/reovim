# buffer-simple Module

SimpleBufferManager implementation of the BufferManager trait.

## Source Location

`server/modules/buffer-simple/src/`

## Purpose

Provides the `SimpleBufferManager` implementation of the `BufferManager` trait
from the kernel. Manages buffer storage with thread-safe access.

Following the mechanism/policy separation:
- **Mechanism**: `BufferManager` trait (in kernel)
- **Policy**: `SimpleBufferManager` (this module) provides implementation

## Design Philosophy

- **No I/O operations**: File loading/saving is VFS driver's job
- **No syntax attachment**: Syntax is handled by modules (policy)
- **Thread-safe**: All operations are safe for concurrent access
- **Simple**: Just manages buffer storage, nothing more

## Key Types

```rust
/// Simple buffer manager implementation
pub struct SimpleBufferManager {
    /// Buffer storage with outer RwLock protecting the HashMap
    /// Inner Arc<RwLock<Buffer>> allows multiple references
    buffers: RwLock<HashMap<BufferId, Arc<RwLock<Buffer>>>>,
}

impl BufferManager for SimpleBufferManager {
    fn get(&self, id: BufferId) -> Option<Arc<RwLock<Buffer>>>;
    fn create(&self) -> BufferId;
    fn register(&self, buffer: Buffer) -> BufferId;
    fn unregister(&self, id: BufferId) -> Result<Buffer, BufferError>;
    fn list(&self) -> Vec<BufferId>;
    fn count(&self) -> usize;
}

/// Module instance
pub struct BufferSimpleModule;

impl Module for BufferSimpleModule {
    fn id(&self) -> ModuleId { ModuleId::new("buffer-simple") }
    fn name(&self) -> &'static str { "Simple Buffer Manager" }
}
```

## Registration

During `init()`, the module registers itself with the `BufferManagerRegistry`:

```rust
fn init(&mut self, ctx: &ModuleContext) -> ProbeResult {
    let buffer_registry = ctx.services.get_or_create::<BufferManagerRegistry>();
    buffer_registry.register(
        BufferManagerKey::Simple,
        Arc::new(SimpleBufferManager::new())
    );
    ProbeResult::Success
}
```

## Dependencies

- `reovim_kernel::api::v1` - Module trait, Buffer, BufferId, BufferManager
- `reovim_driver_buffer` - BufferManagerKey, BufferManagerRegistry
- `reovim_arch::sync::RwLock` - Thread-safe locking

## Example Usage

```rust
let mgr = SimpleBufferManager::new();

// Create a buffer
let id = mgr.create();

// Get the buffer
if let Some(buffer) = mgr.get(id) {
    let content = buffer.read().to_string();
}

// Register an existing buffer
let buffer = Buffer::from_string("hello");
let id = mgr.register(buffer);

// List all buffers
for id in mgr.list() {
    println!("Buffer: {:?}", id);
}
```

## Related Documents

- [Module System Overview](../overview.md)
- [buffer Driver](../../drivers/buffer/overview.md)
