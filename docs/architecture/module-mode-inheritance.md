# Module-Mode with Inheritance

This document describes Reovim's mode system architecture where modules define modes that can inherit behavior from other modes.

## Overview

Each module defines its own modes, but can **inherit** from other modes (typically core).

```
ModeId = ModuleId(u16) + LocalModeId(u16) = 32 bits

core/         → normal, insert, visual, command, operator (ROOT modes)
microscope/   → files, grep, input (inherit from core:normal, core:insert)
git/          → status, log, diff, commit (inherit from core:normal, core:insert)
explorer/     → tree, preview (inherit from core:normal)
```

---

## Mechanism vs Policy: Mode

| Aspect | Mechanism (Kernel) | Policy (Module) |
|--------|-------------------|-----------------|
| **ModeId** | `ModuleId + LocalModeId` (32 bits) | Which IDs mean what |
| **Syscalls** | `mode_set(ModeId)`, `mode_get() -> ModeId` | When to call them |
| **Trait** | `ModeProvider { modes(), handle_key(), cursor_style() }` | Keybinding implementations |
| **Inheritance** | Resolution algorithm + `inherits_from` field | Which modes inherit from which |
| **Registry** | `ModuleId → ModeProvider` mapping | Module registration |
| **Fallback** | Loop: handle_key → NotHandled → try parent | Define parent relationships |

---

## Kernel Types (Mechanism)

```rust
pub struct ModuleId(pub u16);
pub struct LocalModeId(pub u16);

pub struct ModeId {
    pub module: ModuleId,
    pub local: LocalModeId,
}

pub struct ModeDescriptor {
    pub local_id: LocalModeId,
    pub name: &'static str,
    pub cursor_style: CursorStyle,
    pub accepts_char_input: bool,
    pub inherits_from: Option<ModeId>,  // Inheritance link
}

pub enum KeyResult {
    Handled,              // Key processed
    NotHandled,           // Try parent mode
    SetMode(ModeId),      // Switch mode
    AcceptChar,           // Insert character
}
```

---

## Kernel Trait (Mechanism)

```rust
pub trait ModeProvider: Send + Sync {
    fn module_id(&self) -> ModuleId;
    fn modes(&self) -> Vec<ModeDescriptor>;
    fn handle_key(&self, mode: LocalModeId, key: KeyEvent, ctx: &mut KeyContext) -> KeyResult;
    fn cursor_style(&self, mode: LocalModeId) -> Option<CursorStyle>;
}
```

---

## Resolution Algorithm

The kernel provides the resolution mechanism:

```
1. module.handle_key(local_mode, key)
2. if NotHandled → check inherits_from
3. if has parent → parent_module.handle_key(parent_mode, key)
4. repeat until Handled or no parent
```

### Example: Microscope Files Mode

```
microscope:files inherits from core:normal

User presses 'G' in microscope:files:
  1. microscope.handle_key(files, 'G') → NotHandled
  2. inherits_from = core:normal
  3. core.handle_key(normal, 'G') → goto_last_line() → Handled
  → Microscope scrolls to last item using core behavior

User presses Enter in microscope:files:
  1. microscope.handle_key(files, Enter) → select() → Handled
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

1. Define `LocalModeId` constants in your module
2. Implement `ModeProvider` trait
3. Return `ModeDescriptor` with `inherits_from` set appropriately
4. Handle only the keys specific to your mode
5. Return `KeyResult::NotHandled` for everything else

### Best Practices

- **Inherit from the closest match** - If your mode is mostly navigation, inherit from `core:normal`
- **Don't override common keys** - Let `j/k/h/l` fall through unless you have good reason
- **Document overrides** - If you override `Enter`, document why
- **Test inheritance chain** - Verify fallback behavior works as expected

---

## Related Documents

- [Mechanism vs Policy](./mechanism-vs-policy.md) - Core architectural principle
- [Clean Architecture Proposal](./clean-architecture-proposal.md) - Overall architecture
- [Module System](../plugins/system.md) - Module development guide
