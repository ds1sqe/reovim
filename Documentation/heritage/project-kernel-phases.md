# Project Kernel: phase history

Historical record of Epic #150, the Linux-inspired architecture overhaul from
v0.8.x toward v0.9. The record reports the phases below as completed; those
reports describe that era, not verification of today's implementation.

## Milestones

| Phase | Result recorded |
| --- | --- |
| 0-4 | Platform abstraction, kernel subsystems, drivers, and module loading. |
| 5 | New implementation connected; legacy code archived. |
| 6 | Headless server, TUI, and CLI separated through RPC. |
| 7 | File editing, motions, operators, undo, and search. |
| 8 | Compositor foundation, basic plugins, and themes. |
| 9 | Feature plugins and remaining window capabilities. |
| 10 | Language plugins, customization, and polish. |
| 11 | v0.8.1 parity review and epic closure. |

## Foundation and replacement: phases 0-5

- #152-#153 established the plan and Linux subsystem mapping.
- #156 introduced `lib/arch/` for Unix/Windows abstraction.
- #159-#168 and #173 built `lib/kernel/`: `mm`, `ipc`, `core`, `block`,
  `sched`, and `api`.
- #174-#180 and #190-#197 established syntax, input, display, LSP, network,
  VFS, and logging drivers, plus the Module trait, loader, registry, and
  `declare_module!`.
- #200-#213 connected the new layers. #214 added the runner/editor module;
  #215 archived the legacy code.

The [architecture proposal](clean-architecture-proposal.md) explains the
layer choices. The [Phase 5 strategy](phase-5-extraction.md) describes how
legacy behavior informed fresh implementations with an explicit public API.

## Client/server separation: phase 6

| Issue | Result |
| --- | --- |
| #223 | Shared JSON-RPC 2.0 protocol types in `lib/protocol/`. |
| #220 | Headless editor server with RPC. |
| #221 | TUI client. |
| #222 | CLI client for scripting. |
| #226 | Integration fixes and TUI notification listening. |
| #227 | Debug RPC and tracing integration. |

```text
TUI client ----+
               +-- JSON-RPC 2.0 --> headless server
CLI client ----+                   kernel + drivers + modules
```

Transports included TCP, Unix sockets, and stdio. Later gRPC/workspace changes
are mapped in the [Phase 8 migration note](phase-8-migration.md).

## Editing behavior: phase 7

The foundation covered test infrastructure (#229), session/viewport state
(#230), cursor movement (#231), undo/redo (#232), insert mode (#233), and
delete operations (#234).

PR #272 brought together four work streams:

| Stream | Issues and behavior |
| --- | --- |
| Module system | #262 loading mechanism, #264 autoload policy, #265 runnable milestone. |
| Files | #235 save and #236 open through VFS. |
| Motions | #238 words, #239 lines, #240 find-character, #246 search. |
| Operators | #237 yank/paste, #243 change, #267 linewise flag, #271 integration. |

Text objects (#241) and visual mode (#242) depended on those streams.
The later completion record also listed search E2E (#310), undo E2E (#311),
named registers (#333), and command-line mode (#338, #435).

## Windows and basic plugins: phase 8

Epic #403 provided the window foundation:

| Issue | Capability |
| --- | --- |
| #438 | Core compositor infrastructure. |
| #398 | Floating windows. |
| #399 | Overlays and popups. |
| #439 | Color and theme system. |
| #440 | Auto-pairing. |
| #441 | Statusline. |
| #442 | Which-key hints. |
| #443 | Notifications. |

The display contract supplied `RootCompositor` and `LayerCompositor`;
`HybridCompositor` in `modules/layout/` implemented layout policy. A binary
split tree supported tiled windows and `<C-w>` navigation, splits, close/only,
and resize/equalize commands. Floating windows and overlays supplied popup
placement. Transparency/passthrough (#400) and tab pages (#401) followed
in phase 9.

The separate migration note uses Phase 8 for the workspace split. It is a
source-location record with later annotations, not a second feature checklist.

## Feature consumers: phase 9

| Area | Plugins and dependencies |
| --- | --- |
| Intelligence | Treesitter, LSP, completion, and command-line completion used syntax, async work, and overlays. |
| Navigation | Microscope, pickers, explorer, and range-finder used search, previews, windows, and popup placement. |
| Context | Context and sticky-context used syntax queries and scroll synchronization. |

These consumers exposed gaps in the phase 8 foundation and drove fixes there.

## Languages and polish: phase 10

| Language | Recorded integration |
| --- | --- |
| Rust | rust-analyzer. |
| Python | pyright/pylsp. |
| JavaScript/TypeScript | tsserver. |
| C/C++ | clangd. |
| Markdown | Preview and rendering. |
| JSON | Schema validation. |
| TOML | Configuration-file syntax. |
| Bash | Shell-script support. |

Profiles, settings-menu, health-check, and landing supplied customization and
startup UI. Language consumers exposed further edge cases in LSP, treesitter,
completion, and window management. The useful sequencing lesson was to
validate abstractions through real features and feed failures back to their
owners. Specific failure lessons are in the [legacy notes](legacy-memorial.md).

## Closure: phase 11

The recorded release review covered v0.8.1 parity, disposition of archived
plugins, E2E tests, critical bugs, performance targets, documentation, and
user-facing polish. Epic #150 ended at that boundary.

New languages, GUI/web clients, remote editing, third-party plugins, and
collaboration were listed as possible later epics. Those historical ideas
are not an active backlog; current scope is in the
[documentation index](../README.md).
