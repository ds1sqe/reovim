# Project Kernel: Phase History

> **Epic #150**: Linux-Inspired Architecture Overhaul

This document chronicles Reovim's transformation from a monolithic editor to a **microkernel-inspired architecture** following Linux kernel design principles.

## Overview

```
Phase 0-4 ──> Phase 5 ──> Phase 6 ──> Phase 7 ──> Phase 8
Foundation    Implement   Decouple    Features    Extract
   ✅            ✅          ✅          🚧
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

### Sprint 0: Infrastructure (Blocking)

| Issue | Title | Status |
|-------|-------|--------|
| #229 | [7.1] Test Infrastructure | Pending |
| #230 | [7.2] Session/Viewport Architecture | **Current** |

### Sprint 1: Foundation

| Issue | Title | Dependencies |
|-------|-------|--------------|
| #231 | [7.3] Cursor Movement | #230 |
| #232 | [7.4] Undo/Redo | #230 |

### Sprint 2: Basic Editing

| Issue | Title | Dependencies |
|-------|-------|--------------|
| #233 | [7.5] Insert Mode | #231, #232 |
| #234 | [7.6] Delete Operations | #232 |
| #235 | [7.7] File Save | #230 (VFS) |
| #236 | [7.8] File Open | #235 |

### Sprint 3: Yank & Motions

| Issue | Title | Dependencies |
|-------|-------|--------------|
| #237 | [7.9] Yank/Paste | #234, #230 |
| #238 | [7.10] Word Motions | #231 |
| #239 | [7.11] Line Motions | #231 |
| #240 | [7.12] Find-Char Motions | #231 |

### Sprint 4: Advanced

| Issue | Title | Dependencies |
|-------|-------|--------------|
| #241 | [7.13] Text Objects | #238, #237 |
| #242 | [7.14] Visual Mode | #234, #237, #230 |
| #243 | [7.15] Change Operator | #233, #234 |
| #244 | [7.16] Replace Operations | #233 |
| #245 | [7.17] Repeat Command | #234 |

### Sprint 5: Search

| Issue | Title | Dependencies |
|-------|-------|--------------|
| #246 | [7.18] Search | #231 |

### Phase 7 Metrics

- **Issues:** 18
- **Estimated LOC:** ~4,600
- **Estimated Tests:** ~425+

---

## Phase 8: Feature Extraction (Future)

**Goal:** Extract and re-implement features from `archive/` using new architecture.

Following the same **concept-extraction** methodology as Phase 5:

```
archive/plugins/features/    →    modules/
archive/plugins/languages/   →    modules/ + lib/drivers/
archive/lib/lsp/             →    lib/drivers/lsp/ (enhanced)
```

| Category | Examples | Target |
|----------|----------|--------|
| **UI Components** | statusline, explorer, which-key | Modules |
| **Editing Features** | completion, pair, snippets | Modules |
| **Navigation** | microscope, pickers, range-finder | Modules |
| **Language Support** | LSP, treesitter, formatters | Drivers + Modules |
| **Rendering** | markdown decoration, table rendering | Drivers + Modules |

Study archived code → Design for new architecture → Implement fresh

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

### Remaining (Phase 7-8)

- [ ] Basic editing: open, edit, save files
- [ ] Cursor movement, visual mode, operators
- [ ] Multi-client with independent viewports
- [ ] ~425+ tests passing
- [ ] Performance benchmarks

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
