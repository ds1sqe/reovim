# Kernel Subsystems

The kernel (`lib/kernel`) provides core mechanisms without policy. All subsystems are internal (`pub(crate)`) except the `api/` module.

## Subsystem Overview

| Subsystem | Linux Equivalent | Purpose |
|-----------|------------------|---------|
| `mm/` | `mm/` | Memory management (Buffer, Position) |
| `ipc/` | `ipc/` | Inter-process communication (EventBus) |
| `core/` | `kernel/` | Core primitives (Motion, TextObject) |
| `block/` | `block/` | Block operations (UndoTree, Transaction) |
| `sched/` | `kernel/sched/` | Scheduler (Runtime, WorkQueue) |
| `api/` | `include/linux/` | Public API surface |
| `printk/` | `kernel/printk/` | Kernel logging |

---

## mm/ - Memory Management

Buffer storage and position tracking.

```rust
// lib/kernel/src/mm/

pub struct Buffer {
    lines: Vec<Line>,
    cursor: Position,
    // ...
}

pub struct Position {
    pub line: usize,
    pub column: usize,
}

pub struct Edit {
    pub range: Range,
    pub text: String,
}
```

### Key Types

| Type | Purpose |
|------|---------|
| `Buffer` | Text storage with lines |
| `Position` | Line + column coordinate |
| `Range` | Start + end positions |
| `Edit` | Text modification operation |
| `Line` | Single line of text |

---

## ipc/ - Inter-Process Communication

Event-based communication between components.

```rust
// lib/kernel/src/ipc/

pub struct EventBus {
    subscribers: HashMap<TypeId, Vec<Handler>>,
}

pub struct EventScope {
    counter: AtomicUsize,
    // For synchronization
}
```

### EventBus

Publish-subscribe system for decoupled communication:

```rust
// Subscribe to events
event_bus.subscribe::<BufferChanged>(|event| {
    // Handle buffer change
});

// Publish events
event_bus.publish(BufferChanged { buffer_id });
```

### EventScope

Tracks event lifecycles for synchronization:

```rust
let scope = EventScope::new();
scope.increment();  // Event started
// ... process event ...
scope.decrement();  // Event completed
scope.wait().await; // Wait for all events
```

---

## core/ - Core Primitives

Editor-specific types that don't fit elsewhere.

```rust
// lib/kernel/src/core/

pub enum Motion {
    Left, Right, Up, Down,
    WordStart, WordEnd,
    LineStart, LineEnd,
    // ...
}

pub enum TextObject {
    Word, WORD,
    Paragraph,
    Block(char, char),  // e.g., '(', ')'
    // ...
}
```

### Motion

Cursor movement types (mechanism only, no keybindings):

| Motion | Description |
|--------|-------------|
| `Left/Right/Up/Down` | Character/line movement |
| `WordStart/WordEnd` | Word boundaries |
| `LineStart/LineEnd` | Line boundaries |
| `DocumentStart/End` | Document boundaries |

### TextObject

Selection regions (mechanism only, no keybindings):

| TextObject | Description |
|------------|-------------|
| `Word` | Word under cursor |
| `Paragraph` | Paragraph block |
| `Block(c1, c2)` | Between delimiters |

---

## block/ - Block Operations

Undo/redo and transactional editing.

```rust
// lib/kernel/src/block/

pub struct UndoTree {
    nodes: Vec<UndoNode>,
    current: NodeId,
}

pub struct Transaction {
    edits: Vec<Edit>,
    // Atomic group of edits
}
```

### UndoTree

Non-linear undo history (like Vim's undotree):

```
       [1]
       / \
     [2] [3]
     /     \
   [4]     [5] ← current
```

### Transaction

Atomic edit groups:

```rust
let txn = Transaction::new();
txn.add(Edit::insert(pos, "hello"));
txn.add(Edit::delete(range));
buffer.apply(txn);  // All or nothing
```

---

## sched/ - Scheduler

Event loop and async task management.

```rust
// lib/kernel/src/sched/

pub struct Runtime {
    event_rx: Receiver<Event>,
    work_queue: WorkQueue,
}

pub struct WorkQueue {
    tasks: VecDeque<Task>,
}
```

### Runtime

Main event loop:

```rust
loop {
    // 1. Process events
    while let Ok(event) = event_rx.try_recv() {
        handle_event(event);
    }

    // 2. Run scheduled tasks
    work_queue.run_pending();

    // 3. Render if needed
    if needs_render {
        render();
    }
}
```

### WorkQueue

Deferred task execution:

```rust
work_queue.schedule(|| {
    // Run later in event loop
});

work_queue.schedule_delayed(Duration::from_ms(100), || {
    // Run after delay
});
```

---

## api/ - Public API

The ONLY public module. Everything else is `pub(crate)`.

```rust
// lib/kernel/src/api/

pub mod v1;        // Stable re-exports
pub mod module;    // Module trait
pub mod context;   // KernelContext, ModuleContext
pub mod version;   // Version types
```

### v1.rs - Stable API

```rust
// Re-exports for modules to use
pub use crate::mm::{Buffer, Position, Range, Edit};
pub use crate::ipc::{EventBus, EventScope};
pub use crate::core::{Motion, TextObject};
pub use crate::block::{UndoTree, Transaction};

// Module system
pub use crate::api::module::{Module, ModuleId, ModuleProbe};
pub use crate::api::context::{KernelContext, ModuleContext};
```

### Module Trait

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

---

## printk/ - Kernel Logging

Internal logging without external dependencies.

```rust
// lib/kernel/src/printk/

pub trait Logger: Send + Sync {
    fn log(&self, level: Level, message: &str);
}

// Macros
pr_err!("Error: {}", msg);
pr_warn!("Warning: {}", msg);
pr_info!("Info: {}", msg);
pr_debug!("Debug: {}", msg);
```

Logger implementations live in `lib/drivers/log/`.

---

## Visibility Rules

```rust
// lib/kernel/src/lib.rs

pub mod api;           // PUBLIC - the interface

pub(crate) mod mm;     // PRIVATE - internal only
pub(crate) mod ipc;    // PRIVATE
pub(crate) mod core;   // PRIVATE
pub(crate) mod block;  // PRIVATE
pub(crate) mod sched;  // PRIVATE
pub(crate) mod printk; // PRIVATE
```

Modules can only import from `api::*`:

```rust
// In a module
use reovim_kernel::api::v1::*;  // OK
use reovim_kernel::mm::*;       // ERROR: private
```

---

## Related Documents

- [Overview](./overview.md) - Architecture overview
- [Mechanism vs Policy](./mechanism-vs-policy.md) - Design principle
- [Driver Layer](./drivers.md) - Driver implementations
- [Module System](./modules.md) - Dynamic modules
