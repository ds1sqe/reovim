# Python Module Examples

This directory contains example Python modules for reovim.

## Prerequisites

Reovim must be built with the `python` feature enabled:

```bash
cargo build --features python
```

## Installation

Copy modules to your reovim modules directory:

```bash
cp *.py ~/.local/share/reovim/modules/
```

## Examples

### hello.py - Hello World

The simplest possible Python module demonstrating:
- Module identity (id, name, version)
- Lifecycle (init, exit)

```python
from reovim import Module, ModuleId, ProbeResult

class HelloModule(Module):
    def id(self) -> ModuleId:
        return ModuleId("hello-python")

    def name(self) -> str:
        return "Hello Python Module"

    def version(self) -> tuple[int, int, int]:
        return (1, 0, 0)

    def init(self, ctx) -> ProbeResult:
        print(f"Hello from Python! Data dir: {ctx['data_dir']}")
        return ProbeResult.Success()

    def exit(self):
        print("Goodbye from Python!")
```

### counter.py - Hot Reload Support

Demonstrates state preservation across hot reloads using pickle:

```python
from reovim import Module, ModuleId, ProbeResult
import pickle

class CounterModule(Module):
    def __init__(self):
        super().__init__()
        self._count = 0

    def supports_hot_reload(self) -> bool:
        return True

    def save_state(self) -> bytes | None:
        return pickle.dumps({"count": self._count})

    def restore_state(self, state: bytes):
        data = pickle.loads(state)
        self._count = data.get("count", 0)
```

## Module API Reference

### Required Methods

| Method | Return Type | Description |
|--------|-------------|-------------|
| `id()` | `ModuleId` | Unique module identifier |
| `name()` | `str` | Human-readable module name |
| `version()` | `tuple[int, int, int]` | Semantic version (major, minor, patch) |
| `init(ctx)` | `ProbeResult` | Initialize the module |
| `exit()` | `None` | Clean up before unload |

### Optional Methods

| Method | Return Type | Default | Description |
|--------|-------------|---------|-------------|
| `api_version()` | `Version` | Current API | Required kernel API version |
| `dependencies()` | `list[ModuleId]` | `[]` | Required dependencies |
| `optional_dependencies()` | `list[ModuleId]` | `[]` | Optional dependencies |
| `supports_hot_reload()` | `bool` | `False` | Whether hot reload is supported |
| `save_state()` | `bytes \| None` | `None` | Serialize state for hot reload |
| `restore_state(state)` | `None` | No-op | Restore state after hot reload |

### Registration Methods

| Method | Return Type | Description |
|--------|-------------|-------------|
| `commands()` | `list[CommandRegistration]` | Commands to register |
| `keybindings()` | `list[KeybindingRegistration]` | Keybindings to register |
| `event_handlers()` | `list[EventHandlerRegistration]` | Event handlers to register |

### Available Types

Import from the `reovim` module:

```python
from reovim import (
    Module,              # Base class for modules
    ModuleId,            # Module identifier
    Version,             # Semantic version
    ProbeResult,         # Init result (Success, Defer, Failed)
    ModuleContext,       # Context passed to init()
    CommandRegistration,       # Command registration builder
    KeybindingRegistration,    # Keybinding registration builder
    EventHandlerRegistration,  # Event handler registration builder
)
```

## Tips

1. **Keep it simple**: Python modules should be lightweight. Heavy computation should be in Rust.

2. **Handle errors gracefully**: Return `ProbeResult.Defer("reason")` if dependencies aren't ready.

3. **Use pickle carefully**: Only pickle trusted data. The state comes from your own module.

4. **Test locally**: Copy to `~/.local/share/reovim/modules/` and restart reovim.

## More Information

- [Python Modules User Guide](../../docs/user-guide/python-modules.md)
- [Python FFI Architecture](../../docs/architecture/ffi/python.md)
