# api/ - Public API

The ONLY public module. Everything else is `pub(crate)`.

## Source Location

`lib/kernel/src/api/`

## Module Structure

```rust
pub mod v1;        // Stable re-exports
pub mod module;    // Module trait
pub mod context;   // KernelContext, ModuleContext
pub mod version;   // Version types
```

## v1.rs - Stable API

```rust
// Memory Management (mm/)
pub use crate::mm::{Buffer, BufferId, Position, Edit, Cursor, WindowId};
pub use crate::mm::{Selection, SelectionMode, BufferSnapshot};

// IPC (ipc/)
pub use crate::ipc::{EventBus, EventScope, Event, DynEvent};
pub use crate::ipc::events;  // Access kernel events: events::ModeChanged, etc.

// Core Primitives (core/)
pub use crate::core::{Motion, TextObject, MotionEngine, TextObjectEngine};
pub use crate::core::{Mode, ModeId, ModeStack, CommandId};  // Mode/command identity
pub use crate::core::{RegisterBank, MarkBank, Jumplist};

// Block Operations (block/)
pub use crate::block::{UndoTree, Transaction, History};

// Module System (api/)
pub use crate::api::module::{Module, ModuleId, ModuleProbe};
pub use crate::api::context::{KernelContext, ModuleContext};

// Sync Primitives (from reovim-arch)
pub use reovim_arch::sync::{Mutex, RwLock, ArcSwap};
```

## Module Trait

```rust
pub trait Module: Send + Sync {
    fn id(&self) -> ModuleId;
    fn name(&self) -> &'static str;
    fn version(&self) -> Version;
    fn dependencies(&self) -> Vec<ModuleId>;

    fn init(&mut self, ctx: &ModuleContext) -> ProbeResult;
    fn exit(&mut self) -> Result<(), ModuleError>;

    // Declarative registrations
    fn commands(&self) -> Vec<CommandRegistration>;
    fn keybindings(&self) -> Vec<KeybindingRegistration>;
    fn event_handlers(&self) -> Vec<EventHandlerRegistration>;
}
```

## Event Access

**Important**: Access events via `reovim_kernel::api::v1::events`:

```rust
use reovim_kernel::api::v1::events::ModeChanged;

// Emit a mode change event
ctx.event_bus.emit(ModeChanged {
    from: "normal".to_string(),
    to: "insert".to_string(),
});
```

## Context Types

```rust
// Full kernel access (for runner)
pub struct KernelContext {
    pub event_bus: Arc<EventBus>,
    pub buffer_manager: Arc<dyn BufferManager>,
    // ...
}

// Limited access (for modules)
pub struct ModuleContext {
    pub module_id: ModuleId,
    pub event_bus: Arc<EventBus>,
    // No direct buffer access
}
```

## Related Documents

- [Kernel Overview](../overview.md) - Kernel architecture
- [Module System](../../modules/overview.md) - Module development
- [Module Development Guide](../../contributing/guides/module-development.md) - FFI workflow
