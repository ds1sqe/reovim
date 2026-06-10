# Extension Contracts (#584)

Cross-boundary dependency declarations between server modules and client
extensions. Part of the Extension Manager epic (#562).

## Overview

Server modules push state to clients via extension bridges. Before #584,
these pairings were implicit — a module registered a bridge with a kind
string, and a client extension consumed it by matching that same string.

Extension contracts formalize this relationship:

- **Server modules** declare `extension_kinds()` — the bridge kinds they
  push during `init()`.
- **Client extensions** declare `server_kinds()` (TUI) or `serverKinds()`
  (web) — the server kinds they expect to receive data from.
- **Startup validation** compares these declarations and logs warnings for
  mismatches (non-fatal, graceful degradation).

## Cross-Boundary Pair Map

| Server Module | Bridge Kind(s) | Client Extension(s) |
|---|---|---|
| `completion` | `completion` | `CompletionExtension` (TUI + Web) |
| `whichkey` | `whichkey` | `WhichKeyExtension` (TUI + Web) |
| `cmdline` | `cmdline` | `CmdlineExtension` (TUI + Web) |
| `notification` | `notification` | `NotificationExtension` (TUI + Web) |
| `microscope` | `microscope` | `MicroscopeExtension` (TUI + Web) |
| `explorer` | `explorer` | `ExplorerExtension` (TUI + Web) |
| `range-finder` | `range-finder-jump`, `range-finder-fold` | `RangeFinderJump` + `RangeFinderFold` (TUI + Web) |
| `lsp-navigation` | `hover`, `signature-help` | `HoverExtension` + `SignatureHelpExtension` (TUI) |
| `lsp` | `diagnostics` | `DiagnosticsExtension` (TUI) |
| `tetromino` | `polyblocks` | `TetrominoExtension` (TUI only) |

Note: `treesitter-markdown` pairs with `MarkdownRenderExtension` but uses
content analysis, not notification push. It does NOT register a bridge.

## Server-Side Validation

At server startup, `collect_bridges()` in `apps/bin/src/bootstrap.rs`:

1. Collects `extension_kinds()` from all running modules into a
   `BTreeSet` (sorted, deduplicated).
2. Compares against registered bridge kinds.
3. Logs warnings for orphaned bridges (bridge with no module declaring it)
   and orphaned module kinds (module declares kind with no bridge).
4. Sets `available_kinds` on `BridgeRegistry` for client consumption.

The `ListExtensions` RPC response includes `available_kinds` so clients
can validate their extensions against the server.

## Client-Side Validation

### TUI

`validate_extensions()` in `clients/tui/extensions/defaults/src/lib.rs`
compares each extension's `server_kinds()` against the server-provided
`available_kinds`. Logs warnings for unmatched kinds.

### Web

`validateExtensions()` in `clients/web/src/extensions/index.ts` performs
the same check. Uses `serverKinds()` override or falls back to
`[ext.kind()]`.

## Adding a New Cross-Boundary Pair

1. **Server module**: Override `extension_kinds()` returning kind string
   constants defined locally in the module.
2. **Bridge**: Register via `BridgeProvider` during `init()` with matching
   kind.
3. **Client extension**: Implement `TuiExtension` (TUI) or
   `WebExtension` (web) with matching `kind()`.
4. **Override `server_kinds()`** if the extension consumes from multiple
   bridges or has a different kind than its own.

The startup validation will automatically detect mismatches.

## Kernel Purity

The `Module` trait in `server/lib/kernel/` returns `&[&'static str]` —
the kernel has zero dependency on extension kind constants. Modules
define their own kind strings locally — no shared crate needed.
