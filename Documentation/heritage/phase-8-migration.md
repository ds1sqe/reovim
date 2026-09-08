# Phase 8 migration map

Historical path reference for the split of the v0.9 runner into server,
clients, and shared libraries. The original map also included later Phase 10
and 11 additions, identified below. Paths describe that historical tree.

## Workspace split

```text
Before                         After
runner/                        apps/bin/ + server/ + clients/
lib/kernel/                    server/lib/kernel/
lib/drivers/                   server, TUI, or shared owner
modules/                       server/modules/
lib/arch/ + lib/module-macros/   shared/
```

The headless server owned editing logic; TUI and CLI connected through gRPC
v2. `shared/protocol/proto/` defined InputService, StateService,
NotificationService, and ServerService. Display moved to the TUI; protocol,
platform, transport, logging, and testing support moved to shared libraries.

## Source mapping

| Before | After |
| --- | --- |
| `lib/kernel/` | `server/lib/kernel/` |
| `lib/arch/` | `shared/arch/` |
| `lib/module-macros/` | `shared/module-macros/` |
| `lib/drivers/command/` | `server/lib/drivers/command/` |
| `lib/drivers/input/` | `server/lib/drivers/input/` |
| `lib/drivers/syntax/` | `server/lib/drivers/syntax/` |
| `lib/drivers/lsp/` | `server/lib/drivers/lsp/` |
| `lib/drivers/vfs/` | `server/lib/drivers/vfs/` |
| `lib/drivers/display/` | `clients/tui/lib/drivers/display/` |
| `lib/drivers/net/` | `shared/net/` |
| `lib/drivers/log/` | `shared/log/` |
| `modules/` (including keymap and motions) | `server/modules/` |
| `runner/src/main.rs` | `apps/bin/src/main.rs` |
| `runner/src/server/` | `server/lib/server/src/` |
| `runner/src/client/tui/` | `clients/tui/src/` |
| `runner/src/client/cli/` | `clients/cli/src/` |
| `runner/src/module/` | `server/lib/server/src/registry/` |
| `runner/src/testing/` | `shared/testing/src/` |
| `lib/core/tests/` | `server/lib/kernel/tests/` |
| `runner/tests/` | Module-specific `server/modules/*/tests/` |

The test harness lived in `shared/testing/`. Blanket replacement of
`lib/drivers/` would have been wrong: display, network, and logging had
different destinations.

Core policy modules such as vim, editor, and motions moved server-side.
UI modules such as layout and cmdline were archived pending client-side
implementations.

## Crate names

| Crate | Historical location |
| --- | --- |
| `reovim-app` (formerly the `reovim` runner crate) | `apps/bin/` |
| `reovim-server` | `server/lib/server/` |
| `reovim-client-tui` | `clients/tui/` |
| `reovim-client-cli` | `clients/cli/` |
| `reovim-protocol` | `shared/protocol/` |
| `reovim-net` | `shared/net/` |
| `reovim-testing` | `shared/testing/` |
| `reovim-arch` (name unchanged) | `shared/arch/` |
| `reovim-module-macros` (name unchanged) | `shared/module-macros/` |

## Later annotations

- Phase 10 added the Common Client Model (`reovim-client-model`) in
  `shared/clients/model/`; Phase 10.1 added TUI adapters in
  `clients/tui/src/adapter/`.
- The map recorded a TypeScript/WASM web client in `clients/web/` using that
  common model.
- Phase 11 added `reovim-module-window-ops` in `server/modules/window-ops/`.
  The old `modules/layout/` was recorded as replaced by window-ops and
  `LayoutService`.

These annotations explain paths in later revisions without treating every
addition as part of Phase 8.

## Archived sources

The archive directory was removed from the tree in v0.14.3. The
[pre-kernel source][archive] contains the v0.8.x core, plugins, and runner.
`archive/post_kernel/` held the pre-Phase 8B v0.9 runner and was recorded as
removed in January 2026 after gRPC v2 superseded it.

See the [runner overview](runner-overview.md) for its earlier role and the
[heritage index](README.md) to read the full original migration note.

[archive]: https://github.com/ds1sqe/reovim/tree/81806439/archive/pre_kernel
