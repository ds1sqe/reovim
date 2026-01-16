# reovim-driver-ffi-python

Python FFI bindings for reovim modules via PyO3.

## Overview

This crate provides Python bindings that allow writing reovim modules in Python.
It wraps kernel types (`ModuleId`, `Version`, `ProbeResult`) and provides the
foundation for Python modules to interact with the reovim module system.

## Architecture

```
Python Module (my_module.py)
        │
        ▼
┌─────────────────────────┐
│  ffi-python crate       │
│  PythonModule wrapper   │
│  impl Module trait      │
└─────────────────────────┘
        │
        ▼
┌─────────────────────────┐
│  ModuleLoader           │
│  .py detection          │
└─────────────────────────┘
```

## Python Types

The following types are exported to Python:

- `Module` - Base class for Python modules
- `ModuleId` - Unique module identifier
- `Version` - Semantic version (major, minor, patch)
- `ProbeResult` - Module initialization result

## Python Usage

```python
from reovim import Module, ModuleId, ProbeResult

class MyModule(Module):
    def id(self) -> ModuleId:
        return ModuleId("my-python-module")

    def name(self) -> str:
        return "My Python Module"

    def version(self) -> tuple[int, int, int]:
        return (1, 0, 0)

    def init(self, ctx) -> ProbeResult:
        print(f"Initializing with data_dir: {ctx['data_dir']}")
        return ProbeResult.Success

    def exit(self):
        print("Goodbye!")
```

## Requirements

- Python 3.11 or later
- PyO3 0.27+

## Building

```bash
cargo build -p reovim-driver-ffi-python
```

## Testing

```bash
cargo test -p reovim-driver-ffi-python
```

## License

AGPL-3.0-only
