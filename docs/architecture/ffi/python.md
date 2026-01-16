# Python FFI Bindings

This document describes the Python FFI bindings for reovim modules via PyO3.

## Overview

Reovim supports writing modules in Python through the `reovim-driver-ffi-python` crate.
Python modules use the same `Module` trait as Rust modules, with automatic conversion
between Python and Rust types.

## Architecture

```
┌─────────────────────────────────────────────────────────────┐
│  PYTHON MODULES (~/.local/share/reovim/modules/*.py)        │
│  class MyModule(Module):                                    │
│      def init(self, ctx): ...                               │
│  User-written Python code                                   │
├─────────────────────────────────────────────────────────────┤
│  PYTHON BINDINGS (lib/drivers/ffi-python/)                  │
│  reovim Python package                                      │
│  #[pyclass] ModuleId, Version, ProbeResult                  │
│  PyO3 bindings for kernel types                             │
├─────────────────────────────────────────────────────────────┤
│  PYTHON MODULE WRAPPER                                      │
│  PythonModule struct implementing Module trait              │
│  Delegates to Python methods via PyO3 GIL                   │
├─────────────────────────────────────────────────────────────┤
│  MODULE LOADER (runner/src/server/module/loading/)          │
│  load_python() - Python module loading                      │
│  discover_python_modules() - .py file detection             │
│  Unified interface: static/dynamic/python                   │
├─────────────────────────────────────────────────────────────┤
│  LIFECYCLE MANAGER (existing)                               │
│  Same dependency resolution, deferred probing, hot reload   │
│  No changes needed!                                         │
└─────────────────────────────────────────────────────────────┘
```

## Design Decisions

### PyO3 over ctypes

We use PyO3 for Rust-Python interop because:
- **Type safety**: Automatic conversions with compile-time checks
- **Memory safety**: Rust's ownership model extends to Python objects
- **Performance**: Direct function calls, no marshalling overhead
- **Ergonomics**: Idiomatic Python API with decorators and type hints

### Embedded Python

Python runs in the same process as reovim:
- **Shared memory**: Direct access to kernel types
- **Low latency**: No IPC overhead
- **Full API**: Python modules can use all kernel services

### Unified Module Trait

Python modules implement the same 12-method `Module` trait:
- **Consistency**: Same lifecycle as Rust modules
- **Interoperability**: Python modules can depend on Rust modules
- **Hot reload**: State preservation works across languages

## Module Interface

### Base Class

Python modules inherit from `Module`:

```python
from reovim import Module, ModuleId, ProbeResult

class MyModule(Module):
    def id(self) -> ModuleId:
        return ModuleId("my-module")

    def name(self) -> str:
        return "My Module"

    def version(self) -> tuple[int, int, int]:
        return (1, 0, 0)

    def init(self, ctx) -> ProbeResult:
        return ProbeResult.Success()

    def exit(self):
        pass
```

### Available Types

The `reovim` Python module exports:

| Type | Purpose |
|------|---------|
| `Module` | Base class for modules |
| `ModuleId` | Unique module identifier |
| `Version` | Semantic version (major, minor, patch) |
| `ProbeResult` | Init result: `Success`, `Defer(reason)`, `Failed(reason)` |
| `ModuleContext` | Context passed to `init()` |
| `CommandRegistration` | Command registration builder |
| `KeybindingRegistration` | Keybinding registration builder |
| `EventHandlerRegistration` | Event handler registration builder |

### Method Signatures

| Method | Signature | Required |
|--------|-----------|----------|
| `id()` | `() -> ModuleId` | Yes |
| `name()` | `() -> str` | Yes |
| `version()` | `() -> tuple[int, int, int]` | Yes |
| `api_version()` | `() -> Version` | No |
| `dependencies()` | `() -> list[ModuleId]` | No |
| `optional_dependencies()` | `() -> list[ModuleId]` | No |
| `init(ctx)` | `(dict) -> ProbeResult` | Yes |
| `exit()` | `() -> None` | Yes |
| `commands()` | `() -> list[CommandRegistration]` | No |
| `keybindings()` | `() -> list[KeybindingRegistration]` | No |
| `event_handlers()` | `() -> list[EventHandlerRegistration]` | No |
| `supports_hot_reload()` | `() -> bool` | No |
| `save_state()` | `() -> bytes \| None` | No |
| `restore_state(state)` | `(bytes) -> None` | No |

## Implementation Details

### PythonModule Wrapper

The `PythonModule` struct wraps a Python object and implements `Module`:

```rust
pub struct PythonModule {
    /// Python module class instance (Py<PyAny> in Arc for thread safety)
    py_object: Arc<Py<PyAny>>,

    /// Path to source file (for debugging/hot reload)
    source_path: Option<String>,

    /// Cached metadata (id, name, version, api_version)
    metadata: OnceLock<CachedMetadata>,
}
```

### GIL Acquisition

All Python calls acquire the Global Interpreter Lock (GIL):

```rust
fn id(&self) -> ModuleId {
    self.get_metadata().id.clone()
}

fn init(&mut self, ctx: &ModuleContext) -> ProbeResult {
    Python::attach(|py| {
        let obj = self.py_object.bind(py);
        // ... call Python method
    })
}
```

### Thread Safety

`PythonModule` is `Send + Sync` because:
- `Py<PyAny>` is wrapped in `Arc` for shared ownership
- All Python calls acquire the GIL before accessing the object
- Cached metadata uses `OnceLock` for thread-safe initialization

### Module Loading

The loader finds Python modules by:

1. Scanning search paths for `.py` files
2. Filtering out files starting with `_` (private)
3. Executing the Python code in a new module namespace
4. Finding a class that inherits from `Module`
5. Instantiating the class
6. Wrapping in `PythonModule`

```rust
#[cfg(feature = "python")]
pub fn load_python(&mut self, path: &Path) -> Result<ModuleId, ModuleError> {
    // ... load Python file, find Module subclass, instantiate
}
```

### Registration Types

Registration builders use `Box::leak()` to convert dynamic strings to `&'static str`:

```rust
impl PyCommandRegistration {
    pub fn to_kernel(&self) -> v1::CommandRegistration {
        v1::CommandRegistration {
            id: Box::leak(self.id.clone().into_boxed_str()),
            // ...
        }
    }
}
```

This is acceptable because registrations are typically created once at module load
and live for the program's lifetime.

## Feature Flag

Python support is optional and controlled by the `python` feature:

```toml
# runner/Cargo.toml
[features]
default = []
python = ["dep:reovim-driver-ffi-python", "dep:pyo3"]
```

Build with Python support:

```bash
cargo build --features python
```

## Search Paths

Python modules are discovered in the same paths as native modules:

1. `/usr/lib/reovim/modules/` (system, Linux)
2. `/usr/local/lib/reovim/modules/` (local, Linux)
3. `~/.local/share/reovim/modules/` (user, XDG)

## Hot Reload

Python modules can support hot reload by implementing:

```python
def supports_hot_reload(self) -> bool:
    return True

def save_state(self) -> bytes | None:
    import pickle
    return pickle.dumps(self.__dict__)

def restore_state(self, state: bytes):
    import pickle
    self.__dict__.update(pickle.loads(state))
```

State is limited to 16 MiB to prevent memory issues.

## Error Handling

Python exceptions are converted to `ModuleError`:

| Python Exception | ModuleError |
|------------------|-------------|
| Syntax error | `LoadFailed` |
| Import error | `LoadFailed` |
| Exception in `init()` | `InitFailed` |
| Exception in `exit()` | `InitFailed` |
| No Module subclass | `NoEntryPoint` |

## Performance Considerations

### GIL Contention

The GIL is acquired for each method call. For performance:
- Keep Python code fast
- Avoid blocking operations in Python
- Heavy computation should be in Rust modules

### Startup Cost

Python modules have higher startup cost than Rust:
- Python interpreter initialization
- Module compilation
- Object creation

For latency-critical paths, prefer Rust modules.

## Security

### Pickle Deserialization

Hot reload state uses pickle, which can execute arbitrary code.
Mitigations:
- State comes from the module itself (not user input)
- Size limit (16 MiB) prevents memory attacks
- Document risks in user guide

### Module Sandboxing

Python modules run with full Python capabilities.
Future work may add:
- Resource limits
- Filesystem restrictions
- Network restrictions

## Related Documentation

- [FFI Overview](./overview.md) - C FFI interface
- [Module Overview](../modules/overview.md) - Module system architecture
- [Python Modules User Guide](../../user-guide/python-modules.md) - Writing Python modules
