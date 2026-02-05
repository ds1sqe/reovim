# clipboard/ - Clipboard Provider Driver

System clipboard access and yank history interface.

## Source Location

`server/lib/drivers/clipboard/src/`

## Purpose

Defines the interface for system clipboard access and yank history. Complements
the kernel's `RegisterBank` with system integration.

Following the mechanism/policy separation:
- **Mechanism** (this driver): `ClipboardProvider` trait, `ClipboardKey`, registry
- **Policy** (modules): Implementations with actual OS clipboard integration

## Architecture

```
+-------------------------------------------------------------+
|  Kernel RegisterBank                                         |
|  - Unnamed register ("")                                     |
|  - Named registers (a-z)                                     |
+-------------------------------------------------------------+
|  Clipboard Driver (this crate)                               |
|  - System clipboard (+)                                      |
|  - Selection clipboard (*)                                   |
|  - History registers (0-9)                                   |
+-------------------------------------------------------------+
```

## Usage Flow

**Yank/Delete Operation:**
1. Check register name
2. If `+`/`*` -> `ClipboardProvider.copy_to_clipboard/selection()`
3. If a-z/unnamed -> `RegisterBank.set_by_name()`
4. Always -> `ClipboardProvider.push_history()` (for 0-9 access)

**Paste Operation:**
1. Check register name
2. If `+`/`*` -> `ClipboardProvider.paste_from_clipboard/selection()`
3. If 0-9 -> `ClipboardProvider.history_entry()`
4. If a-z/unnamed -> `RegisterBank.get_by_name()`

## Key Types

```rust
/// Error type for clipboard operations
pub enum ClipboardError {
    NotAvailable,
    AccessDenied,
    InvalidData,
}

/// Typed key for clipboard provider lookup
pub enum ClipboardKey {
    Default,
    // Future: X11, Wayland, macOS, etc.
}

/// Clipboard provider trait
pub trait ClipboardProvider: Send + Sync {
    fn copy_to_clipboard(&self, content: &str) -> Result<(), ClipboardError>;
    fn paste_from_clipboard(&self) -> Result<String, ClipboardError>;
    fn copy_to_selection(&self, content: &str) -> Result<(), ClipboardError>;
    fn paste_from_selection(&self) -> Result<String, ClipboardError>;
    fn push_history(&self, content: RegisterContent);
    fn history_entry(&self, index: usize) -> Option<RegisterContent>;
}

/// Registry for clipboard providers
pub struct ClipboardProviderRegistry { /* ... */ }
```

## Example Usage

```rust
use reovim_driver_clipboard::{ClipboardKey, ClipboardProviderRegistry};

// In module init():
let clipboard = Arc::new(MyClipboardService::new());
let registry = ctx.services.get_or_create::<ClipboardProviderRegistry>();
registry.register(ClipboardKey::Default, clipboard);

// In operator execute():
if let Some(registry) = ctx.services.get::<ClipboardProviderRegistry>() {
    if let Some(provider) = registry.get(&ClipboardKey::Default) {
        provider.push_history(content.clone());
        if register == Some('+') {
            let _ = provider.copy_to_clipboard(&content.text);
        }
    }
}
```

## Related Documents

- [Driver Overview](../overview.md)
- [Kernel RegisterBank](../../kernel/core/overview.md)
