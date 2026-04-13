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
// Version
pub use crate::api::version::{API_VERSION, Version, VersionError, check_api_version, is_compatible};

// Context
pub use crate::api::context::{KernelContext, ModuleContext};

// Memory Management (mm/)
pub use crate::mm::{BufferId, TabId, WindowId};
pub use crate::mm::{SaturatorHandle, SaturatorConfig, spawn_saturator,
                    RequestPriority, SaturationRequest};

// Core Primitives (core/)
pub use crate::core::{Mode, ModeId, ModeStack, CommandId, CursorStyle};
pub use crate::core::{Config, ConfigValue, OptionSpec, OptionValue,
                      OptionConstraint, OptionScope, OptionScopeId,
                      OptionError, OptionRegistry, SetResult, ConstraintError};

// Block Operations (block/)
pub use crate::block::{ByteEdit, ByteUndoLog, ByteUndoEntry,
                       StorageOps, StorageCapabilities, StorageError,
                       BufferMeta, KernelBuffer};

// IPC (ipc/)
pub use crate::ipc::{EventBus, EventScope, Event, DynEvent, EventResult};
pub use crate::ipc::events;  // Access kernel events: events::ModeChanged, etc.

// Module System (api/)
pub use crate::api::module::{Module, ModuleId, ModuleProbe, ProbeResult, ModuleError};

// Service Registry
pub use crate::api::service::{ServiceRegistry, MultiServiceRegistry, Service, ServiceKey};

// Sync Primitives (from reovim-arch)
pub use reovim_arch::sync::*;

// Debug / Profiler
pub use crate::debug::{KernelStateSnapshot, Profiler, ProfileScope,
                       Counter, Histogram, MetricsRegistry};

// Panic / Recovery
pub use crate::panic::{CrashReport, install_panic_handler, save_buffer_for_recovery};
```

Note: `Buffer`, `Position`, `Edit`, `Cursor`, `Motion`, `TextObject`, `UndoTree`,
`Transaction`, `RegisterBank`, `MarkBank`, and `Jumplist` are **not** part of the kernel
API. They were moved to `reovim-domain-text` and `reovim-provider-text` in #739/#740.

## Module Trait

```rust
pub trait Module: Send + Sync {
    fn id(&self) -> ModuleId;
    fn name(&self) -> &'static str;
    fn version(&self) -> Version;
    fn dependencies(&self) -> Vec<ModuleId>;
    fn optional_dependencies(&self) -> Vec<ModuleId>;

    fn init(&mut self, ctx: &ModuleContext) -> ProbeResult;
    fn exit(&mut self) -> Result<(), ModuleError>;

    fn supports_hot_reload(&self) -> bool;
    fn save_state(&self) -> Option<Vec<u8>>;
    fn restore_state(&mut self, state: Vec<u8>) -> Result<(), ModuleError>;

    fn on_all_loaded(&mut self, ctx: &ModuleContext);
}
```

## Event Access

Access events via `reovim_kernel::api::v1::events`:

```rust
use reovim_kernel::api::v1::events::ModeChanged;

// Emit a mode change event
ctx.event_bus.emit(ModeChanged {
    from: "normal".into(),
    to: "insert".into(),
});
```

## Context Types

```rust
// Full kernel access (for server)
pub struct KernelContext {
    pub event_bus: Arc<EventBus>,
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
- [Module Development Guide](../../../contributing/guides/module-development.md) - FFI workflow
