# Mechanism vs Policy Architecture

> **Unix Philosophy**: *"Provide mechanism, not policy."*

This document describes Reovim's core architectural principle: the separation of **mechanism** (kernel) from **policy** (modules).

## Overview

| Layer | Responsibility | Example |
|-------|---------------|---------|
| **Kernel (Mechanism)** | WHAT can be done | `sys.motion.calculate(...)` |
| **Module (Policy)** | HOW it should be done | `'j' → sys.motion.calculate(Down, 1)` |

The kernel provides primitives via **service objects**. Modules decide how to use them.

---

## Kernel Design

### Service Objects (KernelContext)

The kernel provides **concise, minimal, policy-free primitives** via service objects:

```rust
/// Kernel context - provides all kernel services
pub struct KernelContext {
    pub buffers: BufferManager,        // Buffer lifecycle
    pub windows: WindowManager,        // Window lifecycle
    pub motion: MotionEngine,          // Position calculation
    pub text_objects: TextObjectEngine,
    pub registers: RegisterBank,
    pub undo: UndoManager,
    pub event_bus: EventBus,
}

// Module receives `sys: &mut KernelContext` for syscalls
fn on_key(&mut self, sys: &mut KernelContext, key: KeyEvent) {
    let pos = sys.motion.calculate(&buffer, from, Motion::Down, 1);
    sys.windows.get_mut(id).viewport.top_line = pos.line;
}
```

**Syscalls via Service Objects:**

```
sys.buffers.create()              sys.windows.create(buffer_id)
sys.buffers.open(path)            sys.windows.close(id)
sys.buffers.get(id)               sys.windows.get(id)
sys.buffers.close(id)             sys.windows.list()

sys.motion.calculate(buf, from, motion, count)
sys.text_objects.range(buf, pos, object, around)

sys.registers.set(name, content)  sys.undo.checkpoint()
sys.registers.get(name)           sys.undo.revert()

sys.event_bus.publish(event)
sys.event_bus.subscribe::<T>(handler)
```

**Note**: `sys.windows` manages lifecycle only. Window **positions** are decided by `LayoutPolicy` in the layout module (policy), not the kernel (mechanism).

### Traits (Mechanism Contracts)

The kernel defines traits that modules implement:

```rust
trait Motion { fn destination(&self, buf, from, count) -> Position; }
trait Operator { fn execute(&self, sys: &mut KernelContext, range) -> Result<()>; }
trait KeymapProvider { fn resolve(&self, mode, keys) -> KeymapResult; }
trait TextObject { fn range(&self, buf, pos, around) -> Option<Range>; }
```

**Note**: `LayoutPolicy` and `FocusPolicy` are defined in the **display driver**, not the kernel (see Screen & Window section below).

---

## Policy Modules

Modules implement the policy decisions:

| Module | Policy Decision |
|--------|-----------------|
| `keymap` | `j` = Down, `k` = Up, `dd` = delete line |
| `motions` | Word = alphanumeric + underscore |
| `operators` | Delete, Yank, Change behavior |
| `textobjects` | `iw` = inner word, `ap` = around paragraph |
| `layout` | Window arrangement policy |
| `options` | theme: dark, tabwidth: 4 |

### Example: Motion Policy

The kernel provides `sys.motion.calculate(buf, from, motion, count)`. The `motions` module decides:

- What constitutes a "word" (alphanumeric + underscore? include punctuation?)
- How `w` behaves at end of line (wrap to next line? stop at EOL?)
- Whether `e` includes the last character

These are policy decisions, not mechanisms.

---

## Screen & Window Architecture

Following the Wayland/wlroots/River model, Screen and Window span multiple layers:

```
┌─────────────────────────────────────────────────────────────────┐
│  KERNEL (lib/kernel/)                           MECHANISM       │
│  ┌───────────────────────────────────────────────────────────┐  │
│  │  ABSTRACT DATA + LIFECYCLE (no display concepts)          │  │
│  │                                                           │  │
│  │  Window { id, buffer_id, viewport }  ← NO position!       │  │
│  │  WindowManager { create(), get(), close(), list() }       │  │
│  └───────────────────────────────────────────────────────────┘  │
└─────────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────────┐
│  DISPLAY DRIVER (lib/drivers/display/)          MECHANISM       │
│  ┌───────────────────────────────────────────────────────────┐  │
│  │  RENDERING + TRAIT CONTRACTS                              │  │
│  │                                                           │  │
│  │  Screen { width, height }            ← terminal surface   │  │
│  │  WindowView { window_id, bounds }    ← positioned window  │  │
│  │  render_window(window, buffer, bounds) → cells            │  │
│  │  Compositor, FrameBuffer, Cell                            │  │
│  │                                                           │  │
│  │  trait LayoutPolicy { arrange(windows) → Vec<WindowView> }│  │
│  │  trait FocusPolicy { next(dir, current) → WindowId }      │  │
│  └───────────────────────────────────────────────────────────┘  │
└─────────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────────┐
│  LAYOUT MODULE (modules/layout/)                POLICY          │
│  ┌───────────────────────────────────────────────────────────┐  │
│  │  LAYOUT DECISIONS                                         │  │
│  │                                                           │  │
│  │  impl LayoutPolicy for TilingLayout {                     │  │
│  │      fn arrange(...) { /* hsplit, vsplit, tabs */ }       │  │
│  │  }                                                        │  │
│  │  impl FocusPolicy for VimFocus {                          │  │
│  │      fn next(...) { /* C-w hjkl logic */ }                │  │
│  │  }                                                        │  │
│  └───────────────────────────────────────────────────────────┘  │
└─────────────────────────────────────────────────────────────────┘
```

| Component | Location | Type | Responsibility |
|-----------|----------|------|----------------|
| `Window` struct | Kernel | Data | buffer_id, viewport (NO position) |
| `WindowManager` | Kernel | Service | Lifecycle: create/get/close |
| `Screen` | Display Driver | Mechanism | Terminal surface |
| `WindowView` | Display Driver | Data | Window + screen position |
| `LayoutPolicy` trait | Display Driver | Contract | Interface for layout |
| `FocusPolicy` trait | Display Driver | Contract | Interface for focus |
| `TilingLayout` | Layout Module | **Policy** | WHERE windows go |
| `VimFocus` | Layout Module | **Policy** | HOW focus moves |

**Key insight**: The kernel's `Window` has **no position**. Position is a **policy decision** made by `LayoutPolicy` in the layout module. This follows River's non-monolithic approach where compositor (mechanism) and window manager (policy) are separate.

---

## API Boundary

**Critical Rule:** Modules interact with kernel ONLY via `api::*` - no direct imports.

```
┌─────────────────────────────────────────────────────────────┐
│  KERNEL SPACE (lib/kernel/)                                 │
│  ┌────────────────────────────────────────────────────────┐ │
│  │  sched/ │ mm/ │ ipc/ │ block/ │ core/                  │ │
│  │  (internal implementation - pub(crate))                │ │
│  └────────────────────────────────────────────────────────┘ │
│─────────────────────────────────────────────────────────────│
│  ░░░░░░░░░░░░░░░░ SYSCALL INTERFACE ░░░░░░░░░░░░░░░░░░░░░░  │
│  api::v1 { KernelContext, traits::*, types::* }             │
└═════════════════════════════════════════════════════════════┘
                            │
                            ▼
┌─────────────────────────────────────────────────────────────┐
│  USER SPACE (modules/)                                      │
│  use reovim_kernel::api::v1::*;                             │
│  fn on_key(&mut self, ctx: &mut KernelContext, key: Key)    │
│  // Cannot import kernel internals - won't compile          │
└─────────────────────────────────────────────────────────────┘
```

### Enforcement

- `lib/kernel/src/lib.rs`: Only `pub mod api;`, rest is `pub(crate)`
- Compile-time guarantee: modules cannot depend on internals
- Breaking internal changes don't break modules

---

## Migration Impact

### Before (Mixed)

| Component | Lines | Problem |
|-----------|-------|---------|
| `CorePlugin` | 969 | Mechanism + policy mixed |
| `WindowPlugin` | 83 | Mechanism + policy mixed |
| Keybindings | hardcoded | Policy in kernel |
| Motion rules | hardcoded | Policy in kernel |

### After (Separated)

| Component | Location | Type |
|-----------|----------|------|
| Service Objects | `lib/kernel/src/api/` (KernelContext) | Mechanism |
| Keybindings | `modules/keymap/` | Policy |
| Operators | `modules/operators/` | Policy |
| Layout logic | `modules/layout/` | Policy |

---

## Benefits

1. **Swappable behavior** - Replace vim keybindings with emacs by loading different module
2. **Testable kernel** - Test mechanisms without policy interference
3. **Stable API** - Internal refactoring doesn't break modules
4. **Clear boundaries** - Easy to understand what goes where
5. **Hot reload** - Policy changes without kernel restart

---

## Related Documents

- [Module-Mode Inheritance](../modules/mode-inheritance.md) - Mode system architecture
- [Clean Architecture Proposal](../../heritage/clean-architecture-proposal.md) - Overall architecture (historical)
- [Module System](../modules/overview.md) - Module development guide
