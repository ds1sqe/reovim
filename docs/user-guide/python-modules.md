# Writing Python Modules for Reovim

This guide explains how to write reovim modules in Python.

## Prerequisites

### Enable Python Support

The Python FFI driver (`reovim-driver-ffi-python`) is an unconditional workspace member — no feature flag is needed. Build it directly:

```bash
cargo build -p reovim-driver-ffi-python
```

### Python Version

Python 3.11 or later is required.

## Quick Start

### 1. Create a Module File

Create a `.py` file in your modules directory:

```bash
mkdir -p ~/.local/share/reovim/modules
touch ~/.local/share/reovim/modules/my_module.py
```

### 2. Write the Module

```python
from reovim import Module, ModuleId, ProbeResult

class MyModule(Module):
    """My custom Python module."""

    def id(self) -> ModuleId:
        return ModuleId("my-module")

    def name(self) -> str:
        return "My Module"

    def version(self) -> tuple[int, int, int]:
        return (1, 0, 0)

    def init(self, ctx) -> ProbeResult:
        print(f"Module initialized! Data dir: {ctx['data_dir']}")
        return ProbeResult.Success()

    def exit(self):
        print("Module exiting")
```

### 3. Start Reovim

The module will be discovered and loaded automatically.

## Module Structure

### Required Methods

Every module must implement these methods:

```python
class MyModule(Module):
    def id(self) -> ModuleId:
        """Return unique module identifier (kebab-case recommended)."""
        return ModuleId("my-module")

    def name(self) -> str:
        """Return human-readable display name."""
        return "My Module"

    def version(self) -> tuple[int, int, int]:
        """Return semantic version as (major, minor, patch)."""
        return (1, 0, 0)

    def init(self, ctx) -> ProbeResult:
        """Initialize the module. Called when module is loaded."""
        return ProbeResult.Success()

    def exit(self):
        """Clean up the module. Called when module is unloaded."""
        pass
```

### Init Context

The `ctx` parameter in `init()` is a dictionary with:

| Key | Type | Description |
|-----|------|-------------|
| `data_dir` | `str` | Module's data directory path |
| `cache_dir` | `str` | Module's cache directory path |
| `optional_deps` | `list[str]` | Loaded optional dependencies |

### ProbeResult

Return one of these from `init()`:

```python
# Success - module initialized
return ProbeResult.Success()

# Defer - try again later (dependency not ready)
return ProbeResult.Defer("waiting for treesitter module")

# Failed - permanent failure
return ProbeResult.Failed("configuration file not found")
```

## Dependencies

### Required Dependencies

Declare modules that must be loaded before yours:

```python
def dependencies(self) -> list[ModuleId]:
    return [
        ModuleId("editor"),
        ModuleId("keymap"),
    ]
```

### Optional Dependencies

Declare modules that enhance yours if available:

```python
def optional_dependencies(self) -> list[ModuleId]:
    return [
        ModuleId("lsp"),
        ModuleId("treesitter"),
    ]
```

Check if optional dependencies are loaded:

```python
def init(self, ctx) -> ProbeResult:
    if "lsp" in ctx['optional_deps']:
        self._setup_lsp_integration()
    return ProbeResult.Success()
```

## Registrations

### Commands

Register commands that can be invoked:

```python
from reovim import CommandRegistration

def commands(self) -> list[CommandRegistration]:
    return [
        CommandRegistration("my-command")
            .with_name("My Command")
            .with_description("Does something useful")
            .with_category("custom"),

        CommandRegistration("my-operator")
            .with_name("My Operator")
            .with_count()      # Accepts count prefix (e.g., 5x)
            .with_operand()    # Accepts operand (e.g., xw for word)
            .with_content_modifying(),
    ]
```

### Keybindings

Register keybindings that invoke commands:

```python
from reovim import KeybindingRegistration

def keybindings(self) -> list[KeybindingRegistration]:
    return [
        KeybindingRegistration("<C-m>", "my-command")
            .with_modes(["normal"])
            .with_description("Run my command")
            .with_category("custom"),

        KeybindingRegistration("gx", "my-operator")
            .with_modes(["normal", "visual"])
            .with_priority(50),  # Lower = higher priority
    ]
```

### Event Handlers

Register handlers for editor events:

```python
from reovim import EventHandlerRegistration

def event_handlers(self) -> list[EventHandlerRegistration]:
    return [
        EventHandlerRegistration("BufferChanged")
            .with_description("Update on buffer change")
            .with_priority(100),

        EventHandlerRegistration("CursorMoved")
            .with_once()  # Auto-unsubscribe after first event
            .with_target("editor"),
    ]
```

## Hot Reload

### Enable Hot Reload

Support preserving state when the module is reloaded:

```python
def supports_hot_reload(self) -> bool:
    return True
```

### Save State

Serialize your module's state:

```python
import pickle

def save_state(self) -> bytes | None:
    return pickle.dumps({
        "counter": self._counter,
        "settings": self._settings,
    })
```

### Restore State

Restore state after reload:

```python
def restore_state(self, state: bytes):
    data = pickle.loads(state)
    self._counter = data.get("counter", 0)
    self._settings = data.get("settings", {})
```

### Complete Example

```python
import pickle
from reovim import Module, ModuleId, ProbeResult

class CounterModule(Module):
    def __init__(self):
        super().__init__()
        self._count = 0

    def id(self) -> ModuleId:
        return ModuleId("counter")

    def name(self) -> str:
        return "Counter Module"

    def version(self) -> tuple[int, int, int]:
        return (1, 0, 0)

    def init(self, ctx) -> ProbeResult:
        return ProbeResult.Success()

    def exit(self):
        pass

    def supports_hot_reload(self) -> bool:
        return True

    def save_state(self) -> bytes | None:
        return pickle.dumps({"count": self._count})

    def restore_state(self, state: bytes):
        data = pickle.loads(state)
        self._count = data.get("count", 0)

    # Public API
    def increment(self) -> int:
        self._count += 1
        return self._count
```

## Best Practices

### Keep It Lightweight

Python modules should be thin wrappers. Heavy computation belongs in Rust.

```python
# Good: Simple coordination
def init(self, ctx) -> ProbeResult:
    self._config = load_config(ctx['data_dir'])
    return ProbeResult.Success()

# Bad: Heavy computation in Python
def init(self, ctx) -> ProbeResult:
    self._index = build_massive_index()  # Do this in Rust
    return ProbeResult.Success()
```

### Handle Errors Gracefully

Return appropriate `ProbeResult` values:

```python
def init(self, ctx) -> ProbeResult:
    try:
        self._config = load_config()
    except FileNotFoundError:
        # Permanent failure - config required
        return ProbeResult.Failed("config.toml not found")
    except ConnectionError:
        # Temporary failure - retry later
        return ProbeResult.Defer("network unavailable")

    return ProbeResult.Success()
```

### Use Type Hints

Type hints improve readability and catch errors:

```python
from reovim import Module, ModuleId, ProbeResult, Version

class MyModule(Module):
    def id(self) -> ModuleId:
        return ModuleId("my-module")

    def version(self) -> tuple[int, int, int]:
        return (1, 0, 0)

    def api_version(self) -> Version:
        return Version(0, 9, 0)
```

### Document Your Module

Add docstrings for clarity:

```python
class MyModule(Module):
    """
    My custom module for reovim.

    Features:
    - Feature A
    - Feature B

    Configuration:
        Set MY_OPTION in config.toml
    """

    def init(self, ctx) -> ProbeResult:
        """Initialize with optional LSP integration."""
        ...
```

## Debugging

### Print Debugging

Use print statements (output goes to reovim's log):

```python
def init(self, ctx) -> ProbeResult:
    print(f"MyModule: data_dir = {ctx['data_dir']}")
    print(f"MyModule: optional_deps = {ctx['optional_deps']}")
    return ProbeResult.Success()
```

### Check Logs

View reovim logs:

```bash
tail -f ~/.local/share/reovim/reovim-*.log
```

### Common Errors

| Error | Cause | Fix |
|-------|-------|-----|
| "no Module subclass found" | Class doesn't inherit from `Module` | Add `class MyModule(Module):` |
| "Python syntax error" | Invalid Python code | Check syntax, run `python -m py_compile file.py` |
| "cannot instantiate" | Error in `__init__` | Check `__init__` for exceptions |

## Module Locations

Modules are discovered in these directories:

| Path | Description |
|------|-------------|
| `/usr/lib/reovim/modules/` | System modules (Linux) |
| `/usr/local/lib/reovim/modules/` | Local system modules |
| `~/.local/share/reovim/modules/` | User modules (XDG) |

## API Reference

### Types

| Type | Description |
|------|-------------|
| `Module` | Base class for Python modules |
| `ModuleId` | Module identifier (`ModuleId("name")`) |
| `Version` | Semantic version (`Version(1, 2, 3)`) |
| `ProbeResult` | Init result (`Success()`, `Defer(reason)`, `Failed(reason)`) |
| `ModuleContext` | Context with `data_dir`, `cache_dir`, `optional_deps` |

### Registration Types

| Type | Description |
|------|-------------|
| `CommandRegistration` | Command registration builder |
| `KeybindingRegistration` | Keybinding registration builder |
| `EventHandlerRegistration` | Event handler registration builder |

### CommandRegistration Methods

| Method | Description |
|--------|-------------|
| `.with_name(str)` | Set display name |
| `.with_description(str)` | Set description |
| `.with_category(str)` | Set category |
| `.with_count()` | Accept count prefix |
| `.with_operand()` | Accept operand |
| `.with_navigation()` | Record in navigation history |
| `.with_content_modifying()` | Marks content as modified |

### KeybindingRegistration Methods

| Method | Description |
|--------|-------------|
| `.with_modes(list[str])` | Set active modes |
| `.with_description(str)` | Set description |
| `.with_category(str)` | Set category |
| `.with_priority(int)` | Set priority (lower = higher) |
| `.with_disabled()` | Disable by default |

### EventHandlerRegistration Methods

| Method | Description |
|--------|-------------|
| `.with_priority(int)` | Set priority |
| `.with_description(str)` | Set description |
| `.with_once()` | Auto-unsubscribe after first event |
| `.with_target(str)` | Set target component |

## Examples

Complete example modules (a minimal hello-world module and a hot-reload counter module)
can be found in the integration test fixtures under `shared/testing/`.

## Related Documentation

- [Python FFI Architecture](../architecture/ffi/python.md)
- [Module Overview](../architecture/modules/overview.md)
- [Commands Reference](./commands.md)
