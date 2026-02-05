# ffi-python/ - Python FFI Bindings Driver

Python bindings via PyO3 for writing reovim modules in Python.

## Source Location

`server/lib/drivers/ffi-python/src/`

## Purpose

Provides Python bindings that allow writing reovim modules in Python. Wraps kernel
types (`ModuleId`, `Version`, `ProbeResult`) and provides a `PythonModule` wrapper
that implements the `Module` trait.

## Architecture

```
Python Module (my_module.py)
        |
        v
+-------------------------+
|  ffi-python crate       |
|  PythonModule wrapper   |
|  impl Module trait      |
+-------------------------+
        |
        v
+-------------------------+
|  ModuleLoader           |
|  .py detection          |
+-------------------------+
```

## Python Usage

```python
from reovim import Module, ModuleId, Version, ProbeResult

class MyModule(Module):
    def id(self) -> ModuleId:
        return ModuleId("my-python-module")

    def name(self) -> str:
        return "My Python Module"

    def version(self) -> tuple[int, int, int]:
        return (1, 0, 0)

    def init(self, ctx) -> ProbeResult:
        return ProbeResult.Success

    def exit(self):
        pass
```

## Key Types

### PyModuleBase

Base class for Python modules:

```rust
#[pyclass(name = "Module", module = "reovim", subclass)]
pub struct PyModuleBase;

#[pymethods]
impl PyModuleBase {
    fn id(&self) -> PyModuleId;
    fn name(&self) -> &'static str;
    fn version(&self) -> (u32, u32, u32);
    fn api_version(&self) -> PyVersion;
    fn dependencies(&self) -> Vec<PyModuleId>;
    fn init(&mut self, ctx: &Bound<'_, PyAny>) -> PyProbeResult;
    fn exit(&mut self);
    fn supports_hot_reload(&self) -> bool;
    fn commands(&self) -> Vec<PyCommandRegistration>;
    fn keybindings(&self) -> Vec<PyKeybindingRegistration>;
    fn event_handlers(&self) -> Vec<PyEventHandlerRegistration>;
}
```

### PythonModule Wrapper

Rust wrapper that implements `Module` trait:

```rust
pub struct PythonModule {
    py_instance: Py<PyAny>,
}

impl Module for PythonModule {
    // Delegates all calls to Python instance
}
```

### Python Types

| Python Type | Rust Type | Purpose |
|-------------|-----------|---------|
| `ModuleId` | `PyModuleId` | Module identifier |
| `Version` | `PyVersion` | Semantic version |
| `ProbeResult` | `PyProbeResult` | Init result |
| `ModuleContext` | `PyModuleContext` | Init context |
| `CommandRegistration` | `PyCommandRegistration` | Command registration |
| `KeybindingRegistration` | `PyKeybindingRegistration` | Keybinding registration |
| `EventHandlerRegistration` | `PyEventHandlerRegistration` | Event handler registration |

## Initialization

```rust
use reovim_driver_ffi_python::init_python;

// Initialize Python with reovim module
// MUST be called before any other Python operations
init_python();

// Now Python code can: from reovim import Module
```

## Dependencies

- `pyo3` - Python bindings for Rust
- `reovim_kernel::api::v1` - Module trait, Version, etc.

## Related Documents

- [Driver Overview](../overview.md)
- [ffi Driver](../ffi/overview.md)
- [Module System](../../modules/overview.md)
