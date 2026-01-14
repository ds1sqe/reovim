# Module Development Guide

This guide covers developing dynamic modules for reovim, including FFI workflow, hot reload support, and testing.

## Overview

Reovim follows a Linux kernel-inspired architecture where modules provide policy (HOW things behave) while the kernel provides mechanism (WHAT can be done). Dynamic modules are shared libraries (`.so` on Linux, `.dylib` on macOS, `.dll` on Windows) that implement the `Module` trait.

## FFI Workflow

### 1. Create Module Crate

Create a new crate with the `cdylib` crate type:

```toml
# modules/my-module/Cargo.toml
[package]
name = "reovim-module-my-module"
version = "0.9.0-dev"
edition = "2024"

[lib]
crate-type = ["cdylib", "rlib"]  # cdylib for dynamic loading, rlib for testing

[dependencies]
reovim-kernel = { workspace = true }
reovim-module-macros = { workspace = true }
```

### 2. Implement the Module Trait

```rust
// modules/my-module/src/lib.rs
use reovim_kernel::api::v1::{
    Module, ModuleContext, ModuleError, ModuleId, ProbeResult, Version,
};
use reovim_module_macros::declare_module;

pub struct MyModule {
    // Your module state
    initialized: bool,
}

impl MyModule {
    pub fn new() -> Self {
        Self { initialized: false }
    }
}

impl Module for MyModule {
    fn id(&self) -> ModuleId {
        ModuleId::new("my-module")
    }

    fn name(&self) -> &'static str {
        "My Module"
    }

    fn version(&self) -> Version {
        Version::new(1, 0, 0)
    }

    fn init(&mut self, _ctx: &ModuleContext) -> ProbeResult {
        self.initialized = true;
        ProbeResult::Success
    }

    fn exit(&mut self) -> Result<(), ModuleError> {
        self.initialized = false;
        Ok(())
    }
}

// Generate FFI entry points
declare_module!(MyModule);
```

### 3. Build the Module

```bash
cargo build -p reovim-module-my-module

# The .so file is created at:
# target/debug/libreovim_module_my_module.so (debug)
# target/release/libreovim_module_my_module.so (release)
```

### 4. Install the Module

Copy the shared library to a module search path:

```bash
# User modules directory
mkdir -p ~/.local/share/reovim/modules
cp target/release/libreovim_module_my_module.so ~/.local/share/reovim/modules/

# Or system-wide (requires root)
sudo cp target/release/libreovim_module_my_module.so /usr/lib/reovim/modules/
```

## FFI Entry Points

The `declare_module!` macro generates these FFI symbols:

| Symbol | Type | Purpose |
|--------|------|---------|
| `REOVIM_MODULE_API_VERSION` | `Version` | Pre-load version check |
| `reovim_module_probe()` | `fn() -> ModuleProbe` | Metadata query |
| `reovim_module_entry()` | `fn() -> *mut c_void` | Instance creation |
| `reovim_module_init()` | `fn(*mut c_void, *const c_void) -> i32` | Init trampoline |
| `reovim_module_exit()` | `fn(*mut c_void) -> i32` | Exit trampoline |
| `reovim_module_destroy()` | `fn(*mut c_void)` | Cleanup trampoline |

Hot reload symbols:
| Symbol | Type | Purpose |
|--------|------|---------|
| `reovim_module_supports_hot_reload()` | `fn(*const c_void) -> i32` | Check hot reload support |
| `reovim_module_save_state()` | `fn(*const c_void, *mut *mut u8, *mut usize) -> i32` | Save state |
| `reovim_module_restore_state()` | `fn(*mut c_void, *const u8, usize) -> i32` | Restore state |
| `reovim_module_free_state()` | `fn(*mut u8, usize)` | Free state buffer |

### Return Codes

Init and exit trampolines return:
- `0`: Success
- `1`: Defer (init only - try again later)
- `-1`: Failed/Error
- `-2`: Panic occurred

## Hot Reload Support

Hot reload allows updating module code without restarting the editor. The module's state is saved, the old code is unloaded, new code is loaded, and state is restored.

### Lifecycle

```
┌─────────────────────────────────────────────────────────────┐
│                     Hot Reload Sequence                      │
├─────────────────────────────────────────────────────────────┤
│                                                              │
│  1. User triggers reload                                     │
│     ↓                                                        │
│  2. save_state() → binary blob                              │
│     ↓                                                        │
│  3. exit() → cleanup                                         │
│     ↓                                                        │
│  4. Unload old .so                                           │
│     ↓                                                        │
│  5. Load new .so                                             │
│     ↓                                                        │
│  6. entry() → new instance                                   │
│     ↓                                                        │
│  7. restore_state(blob) → state restored                     │
│     ↓                                                        │
│  8. init() → module ready                                    │
│                                                              │
└─────────────────────────────────────────────────────────────┘
```

### Implementing Hot Reload

```rust
impl Module for MyModule {
    fn supports_hot_reload(&self) -> bool {
        true
    }

    fn save_state(&self) -> Option<Box<[u8]>> {
        // Binary format with version header for forward compatibility
        let mut state = Vec::with_capacity(16);

        // Version header (always first 4 bytes)
        state.extend_from_slice(&1u32.to_le_bytes());

        // Module state
        state.extend_from_slice(&self.counter.to_le_bytes());
        state.extend_from_slice(&self.some_value.to_le_bytes());

        Some(state.into_boxed_slice())
    }

    fn restore_state(&mut self, state: &[u8]) -> Result<(), ModuleError> {
        // Size check
        if state.len() < 4 {
            return Err(ModuleError::InitFailed("state too short".into()));
        }

        // Version check
        let version = u32::from_le_bytes(state[0..4].try_into().unwrap());
        if version > CURRENT_VERSION {
            return Err(ModuleError::InitFailed(
                format!("state version {} too new", version)
            ));
        }

        // Parse and restore state
        self.counter = i64::from_le_bytes(state[4..12].try_into().unwrap());
        self.some_value = u32::from_le_bytes(state[12..16].try_into().unwrap());

        Ok(())
    }
}
```

### State Format Best Practices

1. **Always include a version header** (first 4 bytes)
2. **Use fixed-size binary format** for predictable parsing
3. **Keep state minimal** - only save what's necessary
4. **Handle old versions gracefully** - migrate or use defaults
5. **Reject future versions** - return error if version > current
6. **Limit state size** - MAX_STATE_SIZE is 16 MiB

## RPC Commands

### module/load

Load a module from a file path.

**Request:**
```json
{
  "jsonrpc": "2.0",
  "id": 1,
  "method": "module/load",
  "params": {
    "path": "/path/to/libmodule.so"
  }
}
```

**Response:**
```json
{
  "jsonrpc": "2.0",
  "id": 1,
  "result": {
    "module": {
      "id": "my-module",
      "name": "My Module",
      "version": "1.0.0",
      "state": "Running",
      "path": "/path/to/libmodule.so",
      "is_static": false,
      "dependencies": []
    }
  }
}
```

### module/unload

Unload a module by ID.

**Request:**
```json
{
  "jsonrpc": "2.0",
  "id": 2,
  "method": "module/unload",
  "params": {
    "id": "my-module"
  }
}
```

### module/reload

Hot reload a module (preserves state).

**Request:**
```json
{
  "jsonrpc": "2.0",
  "id": 3,
  "method": "module/reload",
  "params": {
    "id": "my-module"
  }
}
```

### module/list

List all loaded modules.

**Request:**
```json
{
  "jsonrpc": "2.0",
  "id": 4,
  "method": "module/list"
}
```

**Response:**
```json
{
  "jsonrpc": "2.0",
  "id": 4,
  "result": {
    "modules": [
      {
        "id": "my-module",
        "name": "My Module",
        "version": "1.0.0",
        "state": "Running",
        "is_static": false
      }
    ]
  }
}
```

## Configuration

### Config File

Module configuration is in `~/.config/reovim/config.toml`:

```toml
[modules]
# Additional search paths for modules
search_paths = [
  "~/.local/share/reovim/modules",
  "/opt/reovim/modules"
]

# Modules to auto-load on startup
autoload = [
  "lang-rust",
  "feat-completion"
]
```

### Default Search Paths

Modules are searched in order:

1. `/usr/lib/reovim/modules` (system)
2. `/usr/local/lib/reovim/modules` (local install)
3. `~/.local/share/reovim/modules` (user)
4. Paths from config `search_paths`

### Path Expansion

The `~` character is expanded to the user's home directory:
- `~/.local/share` → `/home/user/.local/share`

## Dependencies

### Required Dependencies

Dependencies that must be loaded before your module:

```rust
fn dependencies(&self) -> Vec<ModuleId> {
    vec![
        ModuleId::new("core"),
        ModuleId::new("treesitter"),
    ]
}
```

### Optional Dependencies

Dependencies that enhance your module if available:

```rust
fn optional_dependencies(&self) -> Vec<ModuleId> {
    vec![
        ModuleId::new("lsp"),  // Better completions if available
    ]
}
```

## Deferred Probing

Like Linux's `-EPROBE_DEFER`, modules can request retry if dependencies aren't ready:

```rust
fn init(&mut self, ctx: &ModuleContext) -> ProbeResult {
    // Check if required service is available
    if !ctx.has_service("treesitter") {
        return ProbeResult::Defer("waiting for treesitter".into());
    }

    // Continue initialization
    ProbeResult::Success
}
```

## Testing

### Unit Tests

Test your module logic without FFI:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_module_id() {
        let module = MyModule::new();
        assert_eq!(module.id().as_str(), "my-module");
    }

    #[test]
    fn test_save_restore_state() {
        let mut original = MyModule::new();
        original.counter = 42;

        let state = original.save_state().unwrap();

        let mut restored = MyModule::new();
        restored.restore_state(&state).unwrap();

        assert_eq!(restored.counter, 42);
    }
}
```

### Integration Tests

Test FFI loading (see `runner/tests/module_loading.rs`):

```rust
#[test]
fn test_module_load_dynamic() {
    let path = demo_module_path();
    let library = unsafe { Library::new(&path).unwrap() };

    unsafe {
        let probe: Symbol<ProbeFn> = library.get(b"reovim_module_probe").unwrap();
        let metadata = probe();

        assert_eq!(metadata.id_str(), "my-module");
    }
}
```

## Error Handling

### Common Errors

| Error | Cause | Solution |
|-------|-------|----------|
| `LoadFailed` | .so file not found or invalid | Check path and file permissions |
| `NoEntryPoint` | Missing FFI symbol | Ensure `declare_module!` is used |
| `IncompatibleVersion` | API version mismatch | Rebuild module with compatible kernel |
| `InUse` | Module has dependents | Unload dependents first |

### Panic Safety

All FFI trampolines use `catch_unwind` to prevent panics from crossing the FFI boundary. Panics return `-2` to the caller.

## Example: Hot Reload Demo

See `modules/hot-reload-demo/` for a complete working example that demonstrates:

- Module lifecycle (init/exit)
- Hot reload with state preservation
- Command registration
- Version-compatible state format

```bash
# Build the demo module
cargo build -p reovim-module-hot-reload-demo

# The module is at:
target/debug/libreovim_module_hot_reload_demo.so
```
