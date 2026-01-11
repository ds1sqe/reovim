# Kernel Migration Guide

This guide documents the migration from `lib/core` to the new Linux-inspired kernel architecture.

## Overview

Reovim is transitioning from a monolithic `lib/core` crate to a microkernel-inspired architecture:

```
OLD                          NEW
───                          ───
lib/core/                    lib/arch/      (platform abstraction)
lib/sys/                     lib/kernel/    (core mechanisms)
                             lib/drivers/   (service adapters)
```

## Deprecation Timeline

| Version | Status | Notes |
|---------|--------|-------|
| v0.7.x | Add new crates | New architecture crates added alongside old |
| v0.8.x | Deprecation warnings | `lib/core` marked deprecated, migration encouraged |
| v0.9.x | Remove old crates | `lib/core` and `lib/sys` removed |
| v1.0.0 | Stable release | New architecture is the only option |

## Feature Flags

### lib/core: `kernel-migration`

Enable to use new kernel re-exports:

```toml
[dependencies]
reovim-core = { version = "0.8", features = ["kernel-migration"] }
```

This feature will become the default in v0.9.x.

### runner: `use-new-drivers`

Enable to use new driver architecture:

```toml
[dependencies]
reovim = { version = "0.8", features = ["use-new-drivers"] }
```

This feature will become the default in v0.9.x.

## API Mapping Table

### Core → Kernel

| Old Path | New Path | Notes |
|----------|----------|-------|
| `lib/core/src/runtime/core.rs` | `lib/kernel/src/sched/runtime.rs` | Main runtime loop |
| `lib/core/src/runtime/event_loop.rs` | `lib/kernel/src/sched/executor.rs` | Async executor |
| `lib/core/src/buffer/mod.rs` | `lib/kernel/src/mm/buffer.rs` | Buffer storage |
| `lib/core/src/buffer/rope.rs` | `lib/kernel/src/mm/rope.rs` | Rope data structure |
| `lib/core/src/event_bus/bus.rs` | `lib/kernel/src/ipc/event_bus.rs` | Event bus |
| `lib/core/src/event_bus/scope.rs` | `lib/kernel/src/ipc/scope.rs` | EventScope |
| `lib/core/src/motion/mod.rs` | `lib/kernel/src/core/motion.rs` | Motion calculations |
| `lib/core/src/textobj/mod.rs` | `lib/kernel/src/core/textobj.rs` | Text objects |
| `lib/core/src/command/mod.rs` | `lib/kernel/src/core/command.rs` | Command dispatch |
| `lib/core/src/undo/mod.rs` | `lib/kernel/src/block/undo.rs` | Undo tree |
| `lib/core/src/undo/history.rs` | `lib/kernel/src/block/history.rs` | Change history |

### Core → Drivers

| Old Path | New Path | Notes |
|----------|----------|-------|
| `lib/core/src/screen/window.rs` | `lib/drivers/display/src/window/` | Window management |
| `lib/core/src/screen/render_*.rs` | `lib/drivers/display/src/frame/` | Frame rendering |
| `lib/core/src/frame/mod.rs` | `lib/drivers/display/src/frame/` | Frame buffer |
| `lib/core/src/overlay/mod.rs` | `lib/drivers/display/src/compositor/` | Layer compositing |
| `lib/core/src/event/key/mod.rs` | `lib/drivers/input/src/keyboard/` | Keyboard handling |
| `lib/core/src/event/mouse/mod.rs` | `lib/drivers/input/src/mouse/` | Mouse handling |
| `lib/core/src/rpc/` | `lib/drivers/net/src/rpc/` | RPC server |
| `lib/core/src/rpc/transport.rs` | `lib/drivers/net/src/transport/` | Transport layer |

### Sys → Arch

| Old Path | New Path | Notes |
|----------|----------|-------|
| `lib/sys/` | `lib/arch/` | Platform abstraction |
| crossterm re-exports | `lib/arch/src/traits.rs` | Platform traits |

### LSP

| Old Path | New Path | Notes |
|----------|----------|-------|
| `lib/lsp/` | `lib/drivers/lsp/` | LSP client |

## Migration Examples

### Example 1: Using Buffer from Kernel

**Before (lib/core):**
```rust
use reovim_core::buffer::Buffer;

let buffer = Buffer::new();
```

**After (lib/kernel):**
```rust
use reovim_kernel::mm::Buffer;

let buffer = Buffer::new();
```

### Example 2: Using EventBus from Kernel

**Before (lib/core):**
```rust
use reovim_core::event_bus::EventBus;

let bus = EventBus::new();
bus.emit(MyEvent);
```

**After (lib/kernel):**
```rust
use reovim_kernel::ipc::EventBus;

let bus = EventBus::new();
bus.emit(MyEvent);
```

### Example 3: Using Motion from Kernel

**Before (lib/core):**
```rust
use reovim_core::motion::{Motion, MotionEngine};

let engine = MotionEngine::new();
let new_pos = engine.apply(Motion::Word, &buffer, cursor);
```

**After (lib/kernel):**
```rust
use reovim_kernel::core::{Motion, MotionEngine};

let engine = MotionEngine::new();
let new_pos = engine.calculate(Motion::Word, &buffer, cursor, 1);
```

## Plugin Migration

### Language Plugins

Language plugins will need to implement the `SyntaxDriver` trait from `lib/drivers/syntax`:

**Before:**
```rust
// plugins/languages/rust/src/lib.rs
use reovim_plugin_treesitter::LanguageSupport;

impl LanguageSupport for RustLanguage {
    fn language(&self) -> tree_sitter::Language { ... }
    fn highlights_query(&self) -> &str { ... }
}
```

**After:**
```rust
// plugins/languages/rust/src/lib.rs
use reovim_driver_syntax::SyntaxDriver;

impl SyntaxDriver for RustSyntaxDriver {
    fn language(&self) -> &str { "rust" }
    fn parse(&mut self, content: &str) { ... }
    fn highlights(&self, range: Range<usize>) -> Vec<HighlightSpan> { ... }
}
```

### Feature Plugins

Feature plugins will use the kernel API:

**Before:**
```rust
use reovim_core::{Buffer, EventBus, Motion};
```

**After:**
```rust
use reovim_kernel::api::v1::{Buffer, EventBus, Motion};
```

## Dependency Changes

### For Plugin Authors

Update your `Cargo.toml`:

**Before:**
```toml
[dependencies]
reovim-core = "0.8"
```

**After:**
```toml
[dependencies]
reovim-kernel = "0.9"
# Or for driver access:
reovim-driver-display = "0.9"
reovim-driver-syntax = "0.9"
```

## Breaking Changes

### v0.9.0

1. `lib/core` removed - use `lib/kernel` instead
2. `lib/sys` removed - use `lib/arch` instead
3. Tree-sitter no longer in core - language plugins provide implementations
4. `SyntaxDriver` trait required for language support

### API Stability

The kernel provides stable API versioning:

```rust
use reovim_kernel::api::v1::*;  // Stable API
use reovim_kernel::api::unstable::*;  // Unstable, may change
```

## Getting Help

- [Architecture Proposal](../architecture/clean-architecture-proposal.md)
- [GitHub Issues](https://github.com/ds1sqe/reovim/issues)
- [Epic #150](https://github.com/ds1sqe/reovim/issues/150)
