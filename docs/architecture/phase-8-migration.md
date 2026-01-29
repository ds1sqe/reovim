# Phase 8 Migration Guide

This document explains the structural changes made in Phase 8 and maps old paths to new locations.

## Overview

Phase 8 restructured the project from a monolithic runner with separate lib directories to a clean server/client architecture with clear separation of concerns.

## Structural Changes

### Before Phase 8 (v0.9.0 pre-8B)

```
reovim/
├── runner/                # Monolithic binary with embedded server + TUI
├── lib/
│   ├── kernel/           # Core mechanisms
│   ├── drivers/          # All drivers at root level
│   ├── arch/             # Platform abstraction
│   └── module-macros/    # FFI macros
└── modules/              # Policy modules at root level
```

### After Phase 8

```
reovim/
├── apps/bin/             # Thin binary entry point
├── server/
│   ├── lib/kernel/       # Core mechanisms
│   ├── lib/server/       # Server runtime (gRPC handlers)
│   ├── lib/drivers/      # Server-side drivers (13 crates)
│   └── modules/          # Policy modules (17 crates)
├── clients/
│   ├── cli/              # CLI client (gRPC v2)
│   └── tui/              # TUI client (gRPC v2)
│       └── lib/drivers/  # TUI-specific drivers
├── shared/
│   ├── protocol/         # gRPC v2 protocol definitions
│   ├── arch/             # Platform abstraction
│   ├── net/              # Network transport
│   ├── log/              # Logging infrastructure
│   ├── module-macros/    # FFI macros
│   └── testing/          # Integration test utilities
└── archive/
    ├── pre_kernel/       # v0.8.x code
    └── post_kernel/      # Pre-Phase 8B code
```

## Path Mapping

### Kernel & Core

| Old Path | New Path |
|----------|----------|
| `lib/kernel/` | `server/lib/kernel/` |
| `lib/arch/` | `shared/arch/` |
| `lib/module-macros/` | `shared/module-macros/` |

### Drivers

| Old Path | New Path |
|----------|----------|
| `lib/drivers/command/` | `server/lib/drivers/command/` |
| `lib/drivers/input/` | `server/lib/drivers/input/` |
| `lib/drivers/syntax/` | `server/lib/drivers/syntax/` |
| `lib/drivers/lsp/` | `server/lib/drivers/lsp/` |
| `lib/drivers/vfs/` | `server/lib/drivers/vfs/` |
| `lib/drivers/display/` | `clients/tui/lib/drivers/display/` |
| `lib/drivers/net/` | `shared/net/` |
| `lib/drivers/log/` | `shared/log/` |

### Modules

| Old Path | New Path |
|----------|----------|
| `modules/` | `server/modules/` |
| `modules/keymap/` | `server/modules/keymap/` |
| `modules/motions/` | `server/modules/motions/` |
| `modules/layout/` | `archive/post_kernel/modules/layout/` (archived) |

### Runner & Application

| Old Path | New Path |
|----------|----------|
| `runner/` | Split into multiple locations |
| `runner/src/main.rs` | `apps/bin/src/main.rs` |
| `runner/src/server/` | `server/lib/server/src/` |
| `runner/src/client/tui/` | `clients/tui/src/` |
| `runner/src/module/` | `server/lib/server/src/registry/` |
| `runner/src/testing/` | `shared/testing/src/` |

### Tests

| Old Path | New Path |
|----------|----------|
| `lib/core/tests/` | `server/lib/kernel/tests/` |
| `runner/tests/` | Module-specific: `server/modules/*/tests/` |
| Test harness | `shared/testing/` |

## Crate Names

### New Crates

| Crate | Location | Purpose |
|-------|----------|---------|
| `reovim-app` | `apps/bin/` | Main binary entry point |
| `reovim-server` | `server/lib/server/` | Server runtime |
| `reovim-client-tui` | `clients/tui/` | TUI client |
| `reovim-client-cli` | `clients/cli/` | CLI client |
| `reovim-protocol` | `shared/protocol/` | gRPC v2 definitions |
| `reovim-net` | `shared/net/` | Network transport |
| `reovim-testing` | `shared/testing/` | Test utilities |

### Renamed/Moved Crates

| Old Name | New Name | Notes |
|----------|----------|-------|
| `reovim` (runner) | `reovim-app` | Binary crate |
| `reovim-arch` | `reovim-arch` | Moved to `shared/arch/` |
| `reovim-module-macros` | `reovim-module-macros` | Moved to `shared/module-macros/` |

## Archive Structure

The `archive/` directory contains legacy code for reference:

### `archive/pre_kernel/` (v0.8.x)

- `lib/core/` - Old core library (monolithic)
- `lib/sys/` - Old system abstraction
- `plugins/` - Old plugin system (19 plugins)
- `runner/` - Old monolithic runner

### `archive/post_kernel/` (v0.9.0 pre-Phase 8B)

- `lib/` - Intermediate architecture (7 crates)
- `modules/` - Old module locations (26 modules)
- `runner/` - Pre-split runner with embedded server/TUI

## Key Architectural Changes

### 1. Server/Client Split

The monolithic runner was split into:
- **Server** (`server/lib/server/`): Handles all editing logic via gRPC
- **TUI Client** (`clients/tui/`): Terminal UI connecting via gRPC
- **CLI Client** (`clients/cli/`): Command-line interface for scripting

### 2. gRPC v2 Protocol

Communication between server and clients uses gRPC v2:
- Protocol definitions: `shared/protocol/proto/`
- Services: InputService, StateService, NotificationService, ServerService

### 3. Shared Libraries

Common code extracted to `shared/`:
- `protocol/` - gRPC definitions
- `arch/` - Platform abstraction
- `net/` - Transport layer
- `log/` - Logging
- `testing/` - Test utilities

### 4. Module Consolidation

Modules were consolidated from 26 to 17 active modules:
- Core modules (vim, editor, motions, etc.) moved to `server/modules/`
- UI modules (layout, cmdline, etc.) archived pending client-side implementation

## Migration Checklist

When updating code or documentation:

- [ ] Replace `lib/kernel/` with `server/lib/kernel/`
- [ ] Replace `lib/drivers/` with `server/lib/drivers/`
- [ ] Replace `modules/` with `server/modules/`
- [ ] Replace `runner/` references appropriately:
  - Binary entry: `apps/bin/`
  - Server logic: `server/lib/server/`
  - TUI: `clients/tui/`
- [ ] Replace `lib/arch/` with `shared/arch/`
- [ ] Replace `lib/module-macros/` with `shared/module-macros/`
- [ ] Update crate names in `cargo test -p` commands
- [ ] Verify paths exist before documenting

## Related Documents

- [Architecture Overview](./overview.md) - Current architecture
- [Module System](./modules/overview.md) - Module loading and registry
- [Driver Layer](./drivers/overview.md) - Driver architecture
