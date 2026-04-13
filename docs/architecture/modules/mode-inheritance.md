# Module-Mode with Inheritance

This document describes Reovim's mode system architecture where modules define modes that can inherit behavior from other modes.

## Overview

Each module defines its own modes, but can **inherit** from other modes (typically core).

```
ModeId = ModuleId(string) + discriminant(u16)

core/         → normal, insert, visual, command, operator (ROOT modes)
microscope/   → files, grep, input (inherit from core:normal, core:insert)
git/          → status, log, diff, commit (inherit from core:normal, core:insert)
explorer/     → tree, preview (inherit from core:normal)
```

---

## Mechanism vs Policy: Mode

| Aspect | Mechanism (Kernel) | Policy (Module) |
|--------|-------------------|-----------------|
| **ModeId** | `ModuleId(string) + discriminant(u16)` | Which IDs mean what |
| **Syscalls** | `mode_set(ModeId)`, `mode_get() -> ModeId` | When to call them |
| **Kernel traits** | `Mode { id(), cursor_style() }`, `ModeStack` | Mode state management |
| **Driver trait** | `ModeKeyResolver { resolve(), inherits_from() }` | Keybinding resolution |
| **Resolution** | Input driver: resolver chain + `inherits_from` parent fallback | Which modes inherit from which |
| **Fallback** | Loop: resolve() → try parent via `inherits_from()` | Define parent relationships |

---

## Kernel Types (Mechanism)

```rust
// String-based module identity
pub struct ModuleId(Cow<'static, str>);

// Composite mode identity: which module + which mode within that module
pub struct ModeId {
    module: ModuleId,
    name: &'static str,
    discriminant: u16,
}

// Kernel mode trait: identity and cursor appearance only
pub trait Mode: Send + Sync {
    fn id(&self) -> ModeId;
    fn cursor_style(&self) -> CursorStyle;
}

// Kernel stack of active modes
pub struct ModeStack { /* ... */ }
```

---

## Input Driver Trait (Mechanism)

Key resolution lives in the input driver (`reovim-driver-input`), not the kernel. The
kernel provides the `Mode` trait and `ModeStack`; the input driver provides the resolver
contract.

```rust
// Defined in reovim-driver-input
pub trait ModeKeyResolver: Send + Sync {
    fn mode_id(&self) -> &ModeId;
    fn inherits_from(&self) -> Option<&ModeId>;  // Inheritance link

    fn resolve(&self, key: &KeyEvent, state: &mut ModeState) -> ResolveResult;
}

pub enum ResolveResult {
    Handled,
    NotHandled,  // Caller should try parent via inherits_from()
    // ...
}
```

---

## Resolution Algorithm

The input driver provides the resolution mechanism via `ModeKeyResolver`:

```
1. resolver.resolve(key)
2. if NotHandled → check resolver.inherits_from()
3. if has parent → look up parent resolver, call resolve(key)
4. repeat until Handled or no parent
```

### Example: Microscope Files Mode

```
microscope:files inherits from core:normal

User presses 'G' in microscope:files:
  1. MicroscopeFilesResolver.resolve('G') → NotHandled
  2. inherits_from() → core:normal
  3. VimNormalResolver.resolve('G') → goto_last_line() → Handled
  → Microscope scrolls to last item using core behavior

User presses Enter in microscope:files:
  1. MicroscopeFilesResolver.resolve(Enter) → select() → Handled
  → Module-specific behavior, no fallback needed
```

---

## Inherited Behavior

### What Modules Get for Free

**Inherit from core:normal:**
```
j/k/h/l, gg/G, Ctrl-U/Ctrl-D, w/b/e, /, :, v/V, Ctrl-W
```

**Inherit from core:insert:**
```
Esc → normal, Ctrl-C → normal, char input
```

**Inherit from core:visual:**
```
Selection extend on j/k/h/l, d/y/c on selection, Esc → normal
```

---

## Module Examples (Policy)

### Core Module (Root Modes)

```rust
mode::NORMAL  → inherits_from: None (ROOT)
mode::INSERT  → inherits_from: None (ROOT)
mode::VISUAL  → inherits_from: Some(core:normal)
```

### Microscope Module

```rust
mode::FILES → inherits_from: Some(core:normal)  // Gets j/k/gg/G
mode::INPUT → inherits_from: Some(core:insert)  // Gets char input
```

### Git Module

```rust
mode::STATUS → inherits_from: Some(core:normal)
mode::COMMIT → inherits_from: Some(core:insert)  // Typing commit msg
mode::DIFF   → inherits_from: Some(core:normal)  // + core:visual for partial staging
```

### Explorer Module

```rust
mode::TREE    → inherits_from: Some(core:normal)  // Navigation
mode::PREVIEW → inherits_from: Some(core:normal)  // Read-only viewing
```

---

## Benefits

1. **Code reuse** - Modules don't reimplement basic navigation
2. **Consistent UX** - `j/k` works the same everywhere
3. **Override when needed** - Module handles key first, falls back if not handled
4. **Composable** - Git diff mode can inherit from both normal and visual
5. **Discoverable** - Users can predict behavior based on inheritance

---

## Implementation Notes

### Adding a New Mode

1. Define `ModeId` constants in your module (using `ModuleId` + a `u16` discriminant)
2. Implement `ModeKeyResolver` (from `reovim-driver-input`)
3. Set `inherits_from()` to return the parent `ModeId` when applicable
4. Handle only the keys specific to your mode in `resolve()`
5. Return `ResolveResult::NotHandled` for everything else so the driver falls back to the parent

### Best Practices

- **Inherit from the closest match** — If your mode is mostly navigation, inherit from `core:normal`
- **Don't override common keys** — Let `j/k/h/l` fall through unless you have good reason
- **Document overrides** — If you override `Enter`, document why
- **Test inheritance chain** — Verify `ResolveResult::NotHandled` fallback behavior works as expected

---

## Related Documents

- [Mechanism vs Policy](../../contributing/philosophy/mechanism-vs-policy.md) - Core architectural principle
- [Clean Architecture Proposal](../../heritage/clean-architecture-proposal.md) - Overall architecture (historical)
- [Module System](./overview.md) - Module development guide
