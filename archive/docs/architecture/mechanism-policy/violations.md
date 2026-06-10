# Policy Violations: What NOT to Do

## The Test

**Question**: "Could a non-Vim editor use this code unchanged?"

- If NO -> Policy violation exists
- If YES -> Clean separation achieved

---

## Violation Example 1: Decision in Mechanism Layer

### BAD (Policy leaked into server)

```rust
// server/lib/server/src/registry/keymap.rs
impl KeymapRegistry {
    pub fn lookup(&self, mode: &ModeId, keys: &KeySequence) -> KeyLookupResult {
        let exact = self.get_binding(mode, keys);
        let has_longer = self.has_longer_bindings(mode, keys);

        // VIOLATION: This is VIM's decision, not the server's!
        if has_longer {
            return KeyLookupResult::Prefix;  // "wait for more keys"
        }

        match exact {
            Some(cmd) => KeyLookupResult::Found(cmd),
            None => KeyLookupResult::NotFound,
        }
    }
}
```

**Problem**: Server decides "if longer exists, wait" - that's Vim behavior!
Emacs would execute immediately. Games need instant response.

### GOOD (Pure facts in server, policy in resolver)

```rust
// server/lib/server/src/registry/keymap.rs
impl KeymapRegistry {
    /// Returns FACTS only. No decisions.
    pub fn query(&self, mode: &ModeId, keys: &KeySequence) -> KeyLookupState {
        let exact = self.get_binding(mode, keys);
        let has_longer = self.has_longer_bindings(mode, keys);

        // CORRECT: Just report what exists
        match (exact, has_longer) {
            (Some(cmd), true) => KeyLookupState::ExactWithLonger { exact: cmd },
            (Some(cmd), false) => KeyLookupState::ExactOnly(cmd),
            (None, true) => KeyLookupState::PrefixOnly,
            (None, false) => KeyLookupState::NotFound,
        }
    }
}

// modules/vim/src/resolvers/normal.rs
impl ModeKeyResolver for VimNormalResolver {
    fn resolve(&self, ctx: &ResolveContext) -> ResolveResult {
        let state = ctx.query_keymap();

        // CORRECT: Vim decides what facts MEAN
        match state {
            KeyLookupState::ExactWithLonger { .. } => {
                ResolveResult::NeedMoreKeys  // Vim: wait for dd
            }
            KeyLookupState::ExactOnly(cmd) => {
                ResolveResult::Execute(cmd)
            }
            // ...
        }
    }
}
```

---

## Violation Example 2: Keybindings in Mechanism Module

### BAD (Vim keybindings in keymap module)

```rust
// modules/keymap/src/bindings.rs
pub fn register_default_bindings(registry: &mut KeymapRegistry) {
    // VIOLATION: These are VIM keybindings!
    registry.register(&normal, keys("h"), cmd("cursor_left"));
    registry.register(&normal, keys("j"), cmd("cursor_down"));
    registry.register(&normal, keys("k"), cmd("cursor_up"));
    registry.register(&normal, keys("l"), cmd("cursor_right"));
    registry.register(&normal, keys("dd"), cmd("delete_line"));
}
```

**Problem**: `modules/keymap/` should be pure mechanism.
An Emacs user shouldn't have to modify this module.

### GOOD (Keybindings in vim module)

```rust
// modules/vim/src/bindings/normal.rs
pub fn vim_normal_bindings() -> Vec<Binding> {
    vec![
        // CORRECT: Vim module owns Vim keybindings
        binding("h", "editor:cursor-left"),
        binding("j", "editor:cursor-down"),
        binding("k", "editor:cursor-up"),
        binding("l", "editor:cursor-right"),
        binding("dd", "operators:delete-line"),
    ]
}

// modules/keymap/src/lib.rs
// CORRECT: Only provides registration mechanism
pub struct KeymapModule;
impl Module for KeymapModule {
    fn init(&self, _ctx: &mut ModuleContext) {
        // No bindings here! Pure mechanism.
    }
}
```

---

## Violation Example 3: Hardcoded Behavior

### BAD (Hardcoded timeout in driver)

```rust
// lib/drivers/input/src/resolver.rs
const KEY_TIMEOUT_MS: u64 = 1000;  // VIOLATION: Policy!

impl KeyResolver {
    fn wait_for_sequence(&self) {
        // Hardcoded 1 second timeout - what if user wants 500ms?
        sleep(Duration::from_millis(KEY_TIMEOUT_MS));
    }
}
```

### GOOD (Configurable in policy layer)

```rust
// modules/vim/src/config.rs
pub struct VimConfig {
    pub key_timeout_ms: u64,  // User-configurable
}

// User can override in keymap.toml:
// [vim]
// key_timeout = 500
```

---

## Quick Checklist

Before committing code, ask:

- [ ] Does `server/lib/server/` make any behavior decisions? (Should be NO)
- [ ] Does `modules/keymap/` have any default keybindings? (Should be NO)
- [ ] Could I swap `modules/vim/` for `modules/emacs/` without changing mechanism? (Should be YES)
- [ ] Are all user preferences in config, not hardcoded? (Should be YES)

---

## See Also

- [Mechanism vs Policy Philosophy](../../contributing/philosophy/mechanism-vs-policy.md) - The foundational Unix principle
- [Layer Architecture](layers.md) - Where each layer's responsibilities end
- [Vision](vision.md) - The Policy-Composable Editor promise
