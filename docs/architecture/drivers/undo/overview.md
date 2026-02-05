# undo/ - Undo Provider Driver

Per-buffer undo/redo operations interface.

## Source Location

`server/lib/drivers/undo/src/`

## Purpose

Defines the interface for per-buffer undo/redo operations. Provides the trait
and types for undo functionality with optional persistence.

Following the mechanism/policy separation:
- **Mechanism** (this driver): `UndoProvider` trait, registry, error types
- **Policy** (modules): Implementations like `UndoRegistry` with persistence

## Architecture

```
lib/drivers/undo/     -> Trait + Key + Registry + Error (MECHANISM)
modules/undo/         -> UndoRegistry implementation (POLICY)
```

## Key Types

### UndoProvider Trait

```rust
pub trait UndoProvider: Send + Sync {
    /// Record an edit for a buffer
    fn record(&self, buffer_id: BufferId, edit: Edit);

    /// Undo the last edit for a buffer
    fn undo(&self, buffer_id: BufferId) -> Option<Edit>;

    /// Redo the last undone edit for a buffer
    fn redo(&self, buffer_id: BufferId) -> Option<Edit>;

    /// Check if undo is available
    fn can_undo(&self, buffer_id: BufferId) -> bool;

    /// Check if redo is available
    fn can_redo(&self, buffer_id: BufferId) -> bool;

    /// Clear undo history for a buffer
    fn clear(&self, buffer_id: BufferId);

    /// Save undo history to disk (optional)
    fn persist(&self, buffer_id: BufferId, path: &Path) -> Result<(), UndoPersistError>;

    /// Load undo history from disk (optional)
    fn restore(&self, buffer_id: BufferId, path: &Path) -> Result<(), UndoPersistError>;
}
```

### UndoKey

```rust
pub enum UndoKey {
    Default,
    // Future: Branching, DAG, etc.
}
```

### UndoProviderRegistry

```rust
pub struct UndoProviderRegistry {
    providers: RwLock<HashMap<UndoKey, Arc<dyn UndoProvider>>>,
}

impl UndoProviderRegistry {
    pub fn register(&self, key: UndoKey, provider: Arc<dyn UndoProvider>);
    pub fn get(&self, key: &UndoKey) -> Option<Arc<dyn UndoProvider>>;
}
```

### UndoPersistError

```rust
pub enum UndoPersistError {
    IoError(std::io::Error),
    SerializationError(String),
    VersionMismatch { expected: u32, found: u32 },
    CorruptedData,
}
```

## Dependencies

- `reovim_kernel::api::v1` - BufferId, Edit

## Persistence

Undo history can optionally be persisted to disk alongside buffer files.
The persistence format and location are determined by the implementing module.

Typical persistence location:
```
~/.local/share/reovim/undo/{buffer_hash}.undo
```

## Related Documents

- [Driver Overview](../overview.md)
- [Kernel Block Operations](../../kernel/block/overview.md)
