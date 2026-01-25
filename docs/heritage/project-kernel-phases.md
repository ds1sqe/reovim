# Project Kernel: Phase History

> **Epic #150**: Linux-Inspired Architecture Overhaul

This document chronicles Reovim's transformation from a monolithic editor to a **microkernel-inspired architecture** following Linux kernel design principles.

## Overview

```
Phase 0-4 ──> Phase 5 ──> Phase 6 ──> Phase 7 ──> Phase 8 ──> Phase 9 ──> Phase 10 ──> Phase 11
Foundation    Implement   Decouple    Features    Foundation  Features    Languages    Release
   ✅            ✅          ✅          🚧         (future)    (future)    (future)    (future)
```

---

## Phase 0-4: Foundation ✅

**Goal:** Build the architectural layers from scratch.

### Phase 0-1: Preparation (#152-#153)

- Initial planning and design documentation
- Established Linux kernel mapping strategy

### Phase 2: Architecture Layer (#156)

- Created `lib/arch/` for platform abstraction
- Unix and Windows platform support

### Phase 3: Kernel Core (#159-#168, #173)

Built `lib/kernel/` with Linux-inspired subsystems:

| Subsystem | Purpose | Linux Equivalent |
|-----------|---------|------------------|
| `mm/` | Memory management (Buffer, Position) | mm/ |
| `ipc/` | Inter-process communication (EventBus) | ipc/ |
| `core/` | Core types (Motion, TextObject, Mode) | kernel/ |
| `block/` | Block devices (UndoTree, transactions) | block/ |
| `sched/` | Scheduler (Runtime, WorkQueue) | kernel/sched/ |
| `api/` | Public API boundary | syscalls |

### Phase 4: Driver Layer (#174-#180, #190-#197)

Built `lib/drivers/` as service layer:

| Driver | Purpose |
|--------|---------|
| `syntax/` | Tree-sitter integration |
| `input/` | Keyboard and mouse events |
| `display/` | Terminal rendering, compositor |
| `lsp/` | Language Server Protocol client |
| `net/` | Network transport (TCP, Unix socket) |
| `vfs/` | Virtual filesystem operations |
| `log/` | Logging infrastructure (tracing) |

Created module system:
- Module trait definition
- `declare_module!` proc-macro
- Dynamic module loader
- Module registry

**Result:** Complete layer separation - Kernel (mechanism) / Drivers (services) / Modules (policy)

---

## Phase 5: Implementation ✅

**Goal:** Wire everything together and archive legacy code.

| Issue | Title | Accomplishment |
|-------|-------|----------------|
| #200-#213 | Core Implementation | Connected kernel + drivers + modules |
| #214 | [5.15] New Runner + Editor Module | Created `runner/` with event loop |
| #215 | [5.16] Archive Legacy Code | Moved legacy → `archive/` |

### Key Decisions

- **Concept-Extraction, Not Migration**: Studied `lib/core/`, implemented fresh in new architecture
- **No Backward Compatibility Hacks**: Clean slate for proper layering
- **API Boundary Enforcement**: Only `api/` is public, internals are `pub(crate)`

See [Phase 5 Extraction Strategy](./phase5-extraction.md) for methodology.

**Result:** Working editor on new architecture, legacy code preserved for reference

---

## Phase 6: Client-Server Decoupling ✅

**Goal:** Split into headless server and multiple clients.

| Issue | Title | Accomplishment |
|-------|-------|----------------|
| #223 | [6.0] Protocol | `lib/protocol/` - Shared JSON-RPC 2.0 types |
| #220 | [6.1] Server | Headless editor engine with RPC |
| #221 | [6.2] TUI Client | Terminal UI connecting via RPC |
| #222 | [6.3] CLI Client | Command-line interface for scripting |
| #226 | [6.4] Integration Fixes | CLI/Server fixes + TUI notification listening |
| #227 | [6.5] Debug RPC | Debug endpoints + tracing integration |

### Architecture After Phase 6

```
┌─────────────────────────────────────────────────────────────┐
│                        CLIENTS                              │
├─────────────────┬─────────────────┬─────────────────────────┤
│   TUI Client    │   CLI Client    │   (Future: GUI, Web)    │
└────────┬────────┴────────┬────────┴─────────────────────────┘
         │    JSON-RPC 2.0 (TCP / Unix Socket / Stdio)
┌────────▼─────────────────▼──────────────────────────────────┐
│                   SERVER (Headless)                         │
│  Kernel + Drivers + Modules                                 │
└─────────────────────────────────────────────────────────────┘
```

**Result:** True client-server separation, multiple concurrent clients supported

---

## Phase 7: Feature Completion 🚧

**Goal:** Basic editor functionality - open, edit, save files with Vim-like operations.

### Completed Foundation

| Issue | Title | Status | Key Files |
|-------|-------|--------|-----------|
| #229 | [7.1] Test Infrastructure | ✅ Done | `runner/tests/common/` |
| #230 | [7.2] Session/Viewport | ✅ Done | `runner/src/server/session/` |
| #231 | [7.3] Cursor Movement | ✅ Done | `modules/editor/src/command.rs` |
| #232 | [7.4] Undo/Redo | ✅ Done | `lib/kernel/src/block/undo.rs` |
| #233 | [7.5] Insert Mode | ✅ Done | `modules/editor/src/command.rs` |
| #234 | [7.6] Delete Operations | ✅ Done | `modules/editor/src/command.rs` |

### Parallel Streams ✅ (Merged in PR #272)

All four streams developed in parallel and merged via PR #272.

#### Stream A: Module System ✅

| Issue | Title | Status | Key Files |
|-------|-------|--------|-----------|
| #262 | [7.19] Module Loading Mechanism | ✅ Done | `runner/src/server/module/` |
| #264 | [7.20] Default Autoload Policy | 🚧 Pending | `runner/src/server/defaults.rs` |
| #265 | [7.21] Runnable Milestone | 🚧 Pending | `runner/tests/e2e_smoke.rs` |

#### Stream B: File Operations ✅

| Issue | Title | Status | Key Files |
|-------|-------|--------|-----------|
| #235 | [7.7] File Save | ✅ Done | `lib/drivers/vfs/`, `modules/editor/` |
| #236 | [7.8] File Open | ✅ Done | VFS integration |

#### Stream C: Motions ✅

| Issue | Title | Status | Key Files |
|-------|-------|--------|-----------|
| #238 | [7.10] Word Motions | ✅ Done | `modules/editor/src/` |
| #239 | [7.11] Line Motions | ✅ Done | `modules/editor/src/` |
| #240 | [7.12] Find-Char Motions | ✅ Done | `runner/src/server/app.rs` |
| #246 | [7.18] Search | ✅ Done | `runner/src/search/` |

#### Stream D: Operators ✅

| Issue | Title | Status | Key Files |
|-------|-------|--------|-----------|
| #237 | [7.9] Yank/Paste | ✅ Done | `modules/operators/src/` |
| #243 | [7.15] Change Operator | ✅ Done | `modules/editor/src/command.rs` |
| #267 | is_linewise Flag | ✅ Done | `modules/operators/src/` |
| #271 | Unify Phase-7-D | ✅ Done | Unified infrastructure |

### Blocked (Need Streams Above)

| Issue | Title | Blocked By | Key Files |
|-------|-------|------------|-----------|
| #241 | [7.13] Text Objects | #238, #237 | `modules/operators/src/` |
| #242 | [7.14] Visual Mode | #237 | `modules/editor/src/mode.rs` |

### Phase 7 Dependency Graph

```
            COMPLETED FOUNDATION ✅
┌─────────────────────────────────────────────────────┐
│  #229  #230  #231  #232  #233  #234                 │
└─────────────────────────────────────────────────────┘
                      │
══════════════════════╧═══════════════════════════════
         ALL STREAMS COMPLETED ✅ (PR #272)
┌─────────────────────────────────────────────────────┐
│                                                     │
│  STREAM A ✅        STREAM B ✅      STREAM C ✅    │
│  Module System     File Ops        Motions          │
│  ────────────      ────────        ───────          │
│  #262 Mechanism✅   #235 Save✅     #238 Word✅     │
│    ↓               ↓               #239 Line✅      │
│  #264 Policy🚧     #236 Open✅     #240 Find-Char✅ │
│    ↓                               #246 Search✅    │
│  #265 Runnable🚧                                    │
│                                                     │
│  STREAM D ✅                                        │
│  Operators                                          │
│  ─────────                                          │
│  #237 Yank✅  #243 Change✅  #267 Linewise✅  #271✅ │
│                                                     │
└─────────────────────────────────────────────────────┘
                      │
══════════════════════╧═══════════════════════════════
              NOW UNBLOCKED (streams done)
┌─────────────────────────────────────────────────────┐
│  #241 Text Objects    ← CAN START                   │
│  #242 Visual Mode     ← CAN START                   │
│  #264 Default Policy  ← CAN START                   │
│  #265 Full E2E        ← needs #264                  │
└─────────────────────────────────────────────────────┘
```

### Phase 7 Metrics

- **Issues:** 21 (17 done, 4 remaining)
- **Lines Changed:** ~8,500+ (streams A-D)
- **Tests:** 407+ passing

---

## Phase 8: Foundation (Future)

**Goal:** Complete deferred Phase 7 items + basic plugins + TUI infrastructure.

This phase builds the foundation that Phase 9 features will depend on.

### Deferred Phase 7 Items

| Item | Description | Dependency |
|------|-------------|------------|
| Undo E2E (#311) | Wire `UndoApi` to SessionRuntime | None |
| Search E2E (#310) | Implement search via command-line mode | Command-line |
| Named registers | `"a-z` register prefix parsing | None |
| Text objects | `iw`, `a"`, `i(`, etc. | Operators |
| Visual mode polish | Edge cases, block mode | Selection API |

### Command-Line Mode (Critical Foundation)

| Feature | Keys | Enables |
|---------|------|---------|
| Ex-commands | `:` | `:w`, `:q`, `:e`, `:set` |
| Forward search | `/` | Search patterns |
| Backward search | `?` | Reverse search |
| Input handling | `<CR>`, `<Esc>`, `<BS>` | All above |

### Basic Plugins (Category A)

Extracted from `archive/plugins/features/`:

| Plugin | Purpose | Priority |
|--------|---------|----------|
| **statusline** | Mode, file, position display | High |
| **which-key** | Key hint popups | High |
| **notification** | User feedback messages | Medium |
| **pair** | Auto-close brackets | Medium |

### TUI Infrastructure

| Component | Epic/Issue | Purpose |
|-----------|------------|---------|
| Window system | #403 Phase 1 | Splits, focus, `<C-w>` commands |
| Overlay system | #403 Phase 3 | Popups, menus, hints |
| Color/theme system | TBD | Syntax colors, UI styling |

---

## Phase 9: Feature Plugins (Future)

**Goal:** Extract feature plugins from archive. Battle-tests Phase 8 foundation.

### Code Intelligence

| Plugin | Purpose | Depends On |
|--------|---------|------------|
| **treesitter** | Syntax parsing, highlighting | Syntax driver |
| **lsp** | Language servers, diagnostics | Async, popups |
| **completion** | Autocomplete menu | Overlay, LSP |
| **cmdline-completion** | `:` command completion | Command-line mode |

### Navigation

| Plugin | Purpose | Depends On |
|--------|---------|------------|
| **microscope** | Fuzzy finder (files, symbols) | Overlay, async |
| **pickers** | Buffer/file/help pickers | Microscope |
| **explorer** | File tree sidebar | Window system |
| **range-finder** | Jump labels (like hop.nvim) | Overlay |

### Context

| Plugin | Purpose | Depends On |
|--------|---------|------------|
| **context** | Cursor context tracking | Treesitter |
| **sticky-context** | Sticky function headers | Context, treesitter |

### Phase 9 Feedback Loop

Phase 9 will discover Phase 8 gaps. Budget ~20% of Phase 9 time for fixing
foundation issues exposed by feature plugin development.

---

## Phase 10: Language Plugins (Future)

**Goal:** Extract language plugins from archive. Battle-tests Phase 8+9.

### Language Support

Extracted from `archive/plugins/languages/`:

| Plugin | Language(s) |
|--------|-------------|
| **rust** | Rust (rust-analyzer) |
| **python** | Python (pyright/pylsp) |
| **javascript** | JS/TS (tsserver) |
| **c** | C/C++ (clangd) |
| **markdown** | Markdown (preview, rendering) |
| **json** | JSON (schema validation) |
| **toml** | TOML (config files) |
| **bash** | Shell scripts |

### Customization & Polish

| Plugin | Purpose |
|--------|---------|
| **profiles** | User config profiles |
| **settings-menu** | GUI settings editor |
| **health-check** | `:checkhealth` diagnostics |
| **landing** | Startup screen |

### Advanced Window Features

| Component | Epic/Issue | Purpose |
|-----------|------------|---------|
| Floating windows | #403 Phase 2 | Toggle float, move/resize |
| Transparency | #403 Phase 4 | Layer opacity |
| Tab pages | #403 Phase 5 | Workspace organization |

### Phase 10 Feedback Loop

Language plugins are the heaviest consumers of the feature stack. They will
expose edge cases in LSP, treesitter, completion, and window management.
Budget ~20% for fixing Phase 8+9 issues discovered during language plugin work.

---

## Phase 11: Release (Future)

**Goal:** Feature parity with v0.8.1, close Epic #150.

### Verification

- [ ] All archive plugins extracted or explicitly deferred
- [ ] Feature parity checklist against v0.8.1
- [ ] Performance benchmarks meet targets
- [ ] Documentation complete

### Polish

- [ ] Bug fixes from Phase 8-10 feedback
- [ ] UX refinements from dogfooding
- [ ] Error messages and edge cases

### Release Criteria

- [ ] All E2E tests passing
- [ ] Zero known critical bugs
- [ ] README and user guide updated
- [ ] CHANGELOG complete

### Scope Boundary

**Epic #150 ends here.** New features, new language support, or broader
expansion targets are out of scope and should be tracked in new epics.

---

## Beyond #150: Future Directions

After Epic #150 achieves v0.8.1 feature parity, new epics may explore:

| Area | Examples | Epic |
|------|----------|------|
| **New Languages** | Go, Zig, Lua, Ruby | TBD |
| **GUI Client** | Native desktop app | TBD |
| **Web Client** | Browser-based editor | TBD |
| **Remote Editing** | SSH, containers, cloud | TBD |
| **Plugin API** | Third-party plugin system | TBD |
| **Collaboration** | Multi-user editing | TBD |

These are explicitly **out of scope** for #150 to keep the epic bounded and measurable.

---

## Battle-Testing Philosophy

Each phase validates the previous through actual usage:

```
Phase 8 (Foundation)     Phase 9 (Features)      Phase 10 (Languages)
─────────────────────    ──────────────────      ────────────────────
Build base               Build on Phase 8        Stress-test 8+9
                         Discover 8's gaps       Discover 8+9's gaps
                         Fix foundation          Fix discovered issues
                                ↓                        ↓
                         ~20% backflow           ~20% backflow
```

You cannot design perfect abstractions without consumers using them.
Phase 10 language plugins are the real validation of the architecture.

---

## Lessons Learned

Patterns and insights discovered during the architecture overhaul:

### Concurrency Patterns

| Pattern | Problem | Solution |
|---------|---------|----------|
| **Double-Swap** | Borrow checker blocks `&mut self` with callbacks | Swap state out → operate → swap back |
| **Message Passing** | Shared mutable state across async boundaries | Channels instead of shared references |
| **Lock Ordering** | Deadlocks from inconsistent lock acquisition | Document and enforce lock hierarchy |

### Rendering Patterns

| Pattern | Problem | Solution |
|---------|---------|----------|
| **Double-Buffer** | Terminal flickering on screen updates | Front/back buffer with O(1) swap |
| **Cell Diff** | Slow full-screen redraws | Only send changed cells to terminal |
| **Cursor Hiding** | Cursor blinking during render | Hide cursor before flush, show after |
| **Reset-Before-Set** | Style bleed (underline, bold artifacts) | Always reset style before setting new |

### Architecture Insights

| Insight | Description |
|---------|-------------|
| **Mechanism vs Policy** | Kernel provides primitives, modules decide behavior |
| **API Boundary** | `pub(crate)` internals + `pub mod api` prevents coupling |
| **Trait Contracts** | Drivers define traits, modules implement them |
| **Event-Driven** | Decouple components via EventBus, not direct calls |

### Anti-Patterns Avoided

| Anti-Pattern | Why It Failed | What We Do Instead |
|--------------|---------------|---------------------|
| God objects | `CorePlugin` grew to 969 lines | Single-responsibility modules |
| Mixed concerns | Policy in kernel code | Strict layer separation |
| Tight coupling | Plugins importing kernel internals | API-only access |
| Callback hell | Nested closures with lifetime issues | Message passing + channels |

---

## Acceptance Criteria

### Completed ✅

- [x] `lib/kernel/` has ZERO external syntax dependencies
- [x] Clean client-server architecture
- [x] Legacy code archived in `archive/`
- [x] Server runs headless with RPC
- [x] TUI client connects and renders
- [x] CLI client enables scripting
- [x] Debug endpoints for introspection

### Phase 7 (In Progress)

- [x] Basic editing: open, edit, save files
- [x] Cursor movement, operators
- [ ] Undo/redo E2E tests (#311)
- [ ] Search E2E tests (#310)
- [ ] Visual mode polish
- [ ] Text objects

### Phase 8 (Foundation)

- [ ] Command-line mode (`:`, `/`, `?`)
- [ ] Named registers (`"a-z`)
- [ ] Basic plugins: statusline, which-key, notification, pair
- [ ] Window system (#403 core)
- [ ] Overlay system for popups

### Phase 9 (Feature Plugins)

- [ ] treesitter, lsp, completion
- [ ] microscope, pickers, explorer
- [ ] context, sticky-context, range-finder

### Phase 10 (Language Plugins)

- [ ] Language plugins from archive/
- [ ] profiles, settings-menu, health-check
- [ ] Advanced window features (#403 Phase 2-5)

### Phase 11 (Release)

- [ ] Feature parity with v0.8.1
- [ ] Performance benchmarks
- [ ] Documentation complete
- [ ] Epic #150 closed

---

## Key Principles Applied

| Principle | Application |
|-----------|-------------|
| **Mechanism vs Policy** | Kernel provides WHAT, modules decide HOW |
| **Do one thing well** | Each component has single responsibility |
| **Composability** | Small, focused modules that combine |
| **Separation of concerns** | Clear boundaries: kernel → drivers → modules |
| **API purity** | Kernel has zero external syntax dependencies |
| **Simplicity** | No over-engineering, minimal complexity |

---

## Related Documents

- [Clean Architecture Proposal](./clean-architecture-proposal.md) - Original 119KB design document
- [Phase 5 Extraction Strategy](./phase5-extraction.md) - Concept-extraction methodology
- [Mechanism vs Policy](../contributing/philosophy/mechanism-vs-policy.md) - Core design principle
- [Architecture Overview](../architecture/overview.md) - Current system design
