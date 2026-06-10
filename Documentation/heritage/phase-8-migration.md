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
│   ├── lib/drivers/      # Server-side drivers (13 crates at migration; 27 crates as of v0.14.5)
│   └── modules/          # Policy modules (17+ crates at migration; 73 as of v0.14.5)
│       └── window-ops/   # Window operations (Phase 11) ← NEW
├── clients/
│   ├── cli/              # CLI client (gRPC v2)
│   ├── tui/              # TUI client (gRPC v2)
│   │   ├── lib/drivers/  # TUI-specific drivers
│   │   └── src/adapter/  # Common model adapters (Phase 10.1) ← NEW
│   └── web/              # Web client (TypeScript + WASM) ← NEW
├── shared/
│   ├── clients/
│   │   └── model/        # Common Client Model (Phase 10) ← NEW
│   ├── protocol/         # gRPC v2 protocol definitions
│   ├── arch/             # Platform abstraction
│   ├── net/              # Network transport
│   ├── log/              # Logging infrastructure
│   ├── module-macros/    # FFI macros
│   └── testing/          # Integration test utilities
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
| `modules/layout/` | Archived (replaced by window-ops and LayoutService) |
| — | `server/modules/window-ops/` (new in Phase 11) |

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
| `reovim-client-model` | `shared/clients/model/` | Common client abstraction (Phase 10) |
| `reovim-protocol` | `shared/protocol/` | gRPC v2 definitions |
| `reovim-net` | `shared/net/` | Network transport |
| `reovim-testing` | `shared/testing/` | Test utilities |
| `reovim-module-window-ops` | `server/modules/window-ops/` | Window operations (Phase 11) |

### Web Client (Non-Rust)

| Location | Stack | Purpose |
|----------|-------|---------|
| `clients/web/` | TypeScript + WASM | Web client using Common Client Model |

### Renamed/Moved Crates

| Old Name | New Name | Notes |
|----------|----------|-------|
| `reovim` (runner) | `reovim-app` | Binary crate |
| `reovim-arch` | `reovim-arch` | Moved to `shared/arch/` |
| `reovim-module-macros` | `reovim-module-macros` | Moved to `shared/module-macros/` |

## Archive (Removed)

The `archive/` directory was removed from the tree in v0.14.3. Historical code remains accessible via git history:

- [`archive/pre_kernel/`](https://github.com/ds1sqe/reovim/tree/81806439/archive/pre_kernel) (v0.8.x) — Old core library, system abstraction, plugin system, runner
- `archive/post_kernel/` (v0.9.0 pre-Phase 8B) — Removed in January 2026, superseded by gRPC v2

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

Modules were consolidated from 26 to 17 active modules at migration time (grown to 73 as of v0.14.5):
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

- [Architecture Overview](../../archive/docs/architecture/overview.md) - Current architecture
- [Module System](../../archive/docs/architecture/modules/overview.md) - Module loading and registry
- [Driver Layer](../../archive/docs/architecture/drivers/overview.md) - Driver architecture
