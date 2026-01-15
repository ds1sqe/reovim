# Buffer Provider System (DEFERRED)

**Status**: Deferred from Issue #211
**Reason**: Complex plugin buffer patterns need dedicated design issue
**Original Location**: `lib/core/src/content/provider.rs`

---

## Overview

The buffer provider system allows plugins to create **virtual buffers** with programmatically-generated content. This enables features like:

- File explorer (directory listings)
- LSP diagnostics panel
- Terminal emulator buffers
- Help/documentation viewers
- Search results panels

---

## Current Implementation (OLD)

### Key Types

```rust
// lib/core/src/content/provider.rs

/// Trait for plugins that provide buffer content
pub trait PluginBufferProvider: Send + Sync {
    /// Get current lines (called every render)
    fn get_lines(&self, ctx: &BufferContext) -> Vec<String>;

    /// Handle cursor movement
    fn on_cursor_move(&mut self, position: Position, ctx: &mut BufferContext);

    /// Handle user input (optional)
    fn on_input(&mut self, key: KeyEvent, ctx: &mut BufferContext) -> InputResult;

    /// Whether buffer allows editing
    fn is_editable(&self) -> bool;
}

/// Context passed to provider methods
pub struct BufferContext<'a> {
    pub buffer_id: usize,
    pub width: u16,
    pub height: u16,
    pub state: &'a PluginStateRegistry,
}

/// Result of input handling
pub enum InputResult {
    Handled,
    Unhandled,
    RequestClose,
}
```

### Window Content Source

```rust
// lib/core/src/content/mod.rs

pub enum WindowContentSource {
    /// Normal file buffer
    Buffer(BufferId),
    /// Plugin-provided virtual buffer
    Plugin {
        plugin_id: PluginId,
        provider: Arc<dyn PluginBufferProvider>,
    },
}
```

---

## Design Questions (To Resolve)

### 1. Async Buffer Loading

Current design calls `get_lines()` synchronously on every render. This is problematic for:
- Large directory listings
- Network-based content (LSP results)
- Expensive computations

**Question**: Should we use async streaming or double-buffered caching?

### 2. Buffer Lifecycle

**Question**: How should plugin buffers interact with:
- Buffer list (`:ls`)
- Window management
- Session persistence
- Undo/redo (for editable plugin buffers)

### 3. Input Routing

**Question**: Should plugin input handling be part of:
- The keymap module (consistent with other key handling)
- The plugin system directly
- A hybrid approach

### 4. Memory Management

**Question**: How to handle plugins that generate large content:
- Lazy loading / virtualization
- Memory limits per plugin
- Garbage collection of old content

---

## Proposed Architecture (Future)

### Option A: Driver-Based

```
drivers/display/
├── buffer_source.rs      # WindowContentSource enum
└── plugin_buffer.rs      # PluginBufferProvider trait

modules/plugins/
└── buffer_providers/     # Concrete implementations
```

### Option B: Kernel Extension

```
kernel/block/
├── virtual_buffer.rs     # Virtual buffer abstraction
└── plugin_source.rs      # Plugin buffer source

drivers/display/
└── Uses kernel virtual buffer API
```

### Option C: Dedicated Plugin Subsystem

```
lib/plugins/
├── buffer_provider.rs    # Trait and types
├── registry.rs           # Provider registration
└── lifecycle.rs          # Buffer lifecycle management
```

---

## Migration Path

1. **Phase 1**: Keep current implementation in `lib/core/`
2. **Phase 2**: Design new architecture in dedicated issue
3. **Phase 3**: Implement new design
4. **Phase 4**: Migrate plugins to new API
5. **Phase 5**: Remove old implementation

---

## Related Issues

- Issue #211: Remaining Core Directories Assessment (current)
- Future Issue: Buffer Provider Architecture Design (TBD)

---

## References

- `lib/core/src/content/mod.rs` - Current window content source
- `lib/core/src/content/provider.rs` - Current provider trait
- `plugins/features/explorer/` - File explorer using provider
