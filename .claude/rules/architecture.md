# Workspace Structure

The project follows a server/client architecture with clear separation:

```
reovim/
├── apps/bin/              # Main binary entry point (reovim-app)
├── server/                # Server-side components
│   ├── lib/kernel/        # Core mechanisms (mm/, ipc/, core/, block/, sched/, api/)
│   ├── lib/server/        # Server runtime (gRPC handlers, session management)
│   └── lib/drivers/       # Server drivers (14 crates)
│       ├── command/       # Command trait and registry
│       ├── command-types/ # Command type definitions
│       ├── input/         # Key events and input parsing
│       ├── syntax/        # Syntax highlighting abstraction
│       ├── syntax-treesitter/ # Tree-sitter implementation
│       ├── lsp/           # Language server protocol client
│       ├── vfs/           # Virtual filesystem operations
│       ├── session/       # Session management traits
│       ├── undo/          # Undo/redo system
│       ├── buffer/        # Buffer operations
│       ├── search/        # Search and replace
│       ├── clipboard/     # Clipboard operations
│       ├── ffi/           # Foreign function interface
│       └── ffi-python/    # Python FFI bindings
├── server/modules/        # Policy modules (21 loadable modules)
│   ├── vim/               # Core Vim-like behavior (includes operators)
│   ├── editor/            # Editor core operations
│   ├── motions/           # Movement commands
│   ├── textobjects/       # Text object definitions
│   ├── commands/          # Ex-commands (:w, :q, :e, :wq)
│   ├── keymap/            # Keymap definitions
│   ├── mode-manager/      # Mode state management
│   ├── options/           # Editor settings (virtualedit)
│   ├── buffer-ops/        # Buffer lifecycle events
│   ├── buffer-simple/     # SimpleBufferManager implementation
│   ├── scratch-buffer/    # Empty buffer on session start
│   ├── defaults/          # Meta-module aggregating 14 modules
│   ├── clipboard/         # Clipboard operations
│   ├── search/            # Search provider
│   ├── undo/              # Undo provider
│   ├── vfs-local/         # Local filesystem VFS
│   ├── window-ops/        # Window operations (<C-w> commands)
│   ├── cmdline/           # Command-line mode input (#468)
│   ├── whichkey/          # Which-key hints (#468)
│   ├── treesitter-rust/   # Rust syntax highlighting
│   └── treesitter-markdown/ # Markdown syntax highlighting
├── clients/               # Client applications
│   ├── cli/               # CLI client (gRPC v2)
│   ├── tui/               # TUI client (gRPC v2)
│   │   └── lib/drivers/   # TUI-specific drivers (tui, display)
│   └── web/               # Web client (gRPC-Web, WASM)
├── shared/                # Shared libraries
│   ├── protocol/          # gRPC v2 protocol definitions (.proto files)
│   ├── arch/              # Platform abstraction (unix/, windows/)
│   ├── net/               # Network transport layer
│   ├── log/               # Logging infrastructure
│   ├── trace/             # Tracing/diagnostics
│   ├── module-macros/     # `declare_module!` proc-macro for FFI
│   └── testing/           # Integration test utilities
├── tools/                 # Development tools
│   ├── bench/             # Benchmarking suite
│   └── perf-report/       # Performance report generator
├── perf/                  # Versioned performance reports (PERF-{version}.md)
└── archive/               # Legacy code (reference only)
    └── pre_kernel/        # v0.8.x code (lib/core, lib/sys, plugins)
```

## Key Crate Names

- `reovim-app` → `apps/bin/` (main binary)
- `reovim-kernel` → `server/lib/kernel/`
- `reovim-server` → `server/lib/server/`
- `reovim-driver-*` → `server/lib/drivers/*/`
- `reovim-module-*` → `server/modules/*/`
- `reovim-client-tui` → `clients/tui/`
- `reovim-client-cli` → `clients/cli/`
- `reovim-client-web` → `clients/web/`
- `reovim-protocol` → `shared/protocol/`
- `reovim-arch` → `shared/arch/`
- `reovim-testing` → `shared/testing/`
- `reovim-log` → `shared/log/`
- `reovim-net` → `shared/net/`
- `reovim-trace` → `shared/trace/`
- `reovim-module-macros` → `shared/module-macros/`

## Key Dependencies

- `tokio` - Async runtime
- `crossterm` (via reovim-arch) - Terminal I/O
- `futures`/`futures-timer` - Async utilities
- `tracing` - Logging and diagnostics

## Minimum Rust Version

1.92 (Rust 2024 edition)
