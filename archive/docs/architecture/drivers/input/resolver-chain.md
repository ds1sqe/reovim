# Resolver Chain Architecture

The input driver provides a **resolver chain** that enables mode-specific key handling with progressive customization levels. This is the core mechanism for translating keypresses into editor actions.

## Overview

```
┌─────────────────────────────────────────────────────────────────┐
│                     Key Event Processing                        │
├─────────────────────────────────────────────────────────────────┤
│                                                                 │
│   KeyEvent ──► ResolverRegistry.resolve_with_session()          │
│                        │                                        │
│                        ▼                                        │
│              ┌──────────────────────┐                           │
│              │ resolve_with_session │  ◄── Override for session │
│              │   (level 4)          │      access (undo, etc.)  │
│              └──────────┬───────────┘                           │
│                         │ default delegates to                  │
│                         ▼                                       │
│              ┌───────────────────────┐                          │
│              │resolve_with_extensions│ ◄── Override for         │
│              │   (level 3)           │      extension data      │
│              └──────────┬────────────┘                          │
│                         │ default delegates to                  │
│                         ▼                                       │
│              ┌───────────────────────┐                          │
│              │ resolve_with_keymap   │  ◄── Override for keymap │
│              │   (level 2)           │      lookup access       │
│              └──────────┬────────────┘                          │
│                         │ default delegates to                  │
│                         ▼                                       │
│              ┌───────────────────────┐                          │
│              │      resolve          │  ◄── Basic key handling  │
│              │   (level 1)           │      (legacy/simple)     │
│              └───────────────────────┘                          │
│                                                                 │
└─────────────────────────────────────────────────────────────────┘
```

## The ModeKeyResolver Trait

```rust
pub trait ModeKeyResolver: Send + Sync {
    /// The mode ID this resolver handles
    fn mode_id(&self) -> &ModeId;

    /// Parent mode for inheritance (e.g., visual inherits from normal)
    fn inherits_from(&self) -> Option<&ModeId>;

    /// Level 1: Basic resolution (legacy interface)
    fn resolve(&self, key: &KeyEvent, state: &mut ModeState) -> ResolveResult;

    /// Level 2: Resolution with keymap access
    fn resolve_with_keymap(
        &self,
        key: &KeyEvent,
        state: &mut ModeState,
        input: &ResolveInput<'_>,  // Contains keymap query
    ) -> ResolveResult {
        // Default: delegate to resolve()
        self.resolve(key, state)
    }

    /// Level 3: Resolution with extension data
    fn resolve_with_extensions(
        &self,
        key: &KeyEvent,
        state: &mut ModeState,
        input: &ResolveInput<'_>,
        extensions: &mut ExtensionMap,
    ) -> ResolveResult {
        // Default: delegate to resolve_with_keymap()
        self.resolve_with_keymap(key, state, input)
    }

    /// Level 4: Resolution with full session access
    fn resolve_with_session(
        &self,
        key: &KeyEvent,
        state: &mut ModeState,
        input: &ResolveInput<'_>,
        session: &mut dyn SessionApiDyn,
        extensions: &mut ExtensionMap,
    ) -> ResolveResult {
        // Default: delegate to resolve_with_extensions()
        self.resolve_with_extensions(key, state, input, extensions)
    }
}
```

## Resolution Levels

### Level 1: `resolve()` - Basic Key Handling

The simplest interface. Use when you only need the key event and mode state.

```rust
impl ModeKeyResolver for SimpleResolver {
    fn resolve(&self, key: &KeyEvent, state: &mut ModeState) -> ResolveResult {
        match key.code {
            KeyCode::Escape => ResolveResult::ModeTransition(/* ... */),
            _ => ResolveResult::NotHandled,
        }
    }
}
```

**Use cases:**
- Simple mode transitions
- Static key mappings
- No external dependencies needed

### Level 2: `resolve_with_keymap()` - Keymap Lookup

Override when you need to query the keymap for bindings.

```rust
impl ModeKeyResolver for KeymapAwareResolver {
    fn resolve_with_keymap(
        &self,
        key: &KeyEvent,
        state: &mut ModeState,
        input: &ResolveInput<'_>,
    ) -> ResolveResult {
        // Query keymap for binding
        let keys = KeySequence::from_keys(&[*key]);
        match input.keymap.query(input.mode, &keys) {
            KeyLookupState::ExactOnly(cmd) => ResolveResult::Execute(cmd, ctx),
            KeyLookupState::PrefixOnly => ResolveResult::Pending,
            KeyLookupState::NotFound => self.resolve(key, state),
        }
    }
}
```

**Use cases:**
- Dynamic key bindings
- Multi-key sequence handling
- Prefix detection (operator-pending)

### Level 3: `resolve_with_extensions()` - Extension Data

Override when you need to store/retrieve mode-specific state.

```rust
impl ModeKeyResolver for StatefulResolver {
    fn resolve_with_extensions(
        &self,
        key: &KeyEvent,
        state: &mut ModeState,
        input: &ResolveInput<'_>,
        extensions: &mut ExtensionMap,
    ) -> ResolveResult {
        // Store pending operator in extensions
        let pending = extensions.get_or_insert::<PendingOperator>();
        pending.set_operator(Operator::Delete);
        ResolveResult::ModeTransition(/* operator-pending mode */)
    }
}
```

**Use cases:**
- Operator-pending state
- Count accumulation
- Register selection

### Level 4: `resolve_with_session()` - Full Session Access

Override when you need session-level operations (undo, buffers, etc.).

```rust
impl ModeKeyResolver for VimInsertResolver {
    fn resolve_with_session(
        &self,
        key: &KeyEvent,
        state: &mut ModeState,
        input: &ResolveInput<'_>,
        session: &mut dyn SessionApiDyn,
        extensions: &mut ExtensionMap,
    ) -> ResolveResult {
        // Insertable characters: return InsertChar
        if let Some(c) = Self::is_insertable(key) {
            return ResolveResult::InsertChar(c);
        }

        // Non-insertable keys: query keymap for commands
        // This allows Escape to trigger vim:exit-insert command
        // which properly ends undo batching
        let keys = KeySequence::from_keys(&[*key]);
        match input.keymap.query(input.mode, &keys) {
            KeyLookupState::ExactOnly(cmd) => {
                ResolveResult::Execute(cmd, ResolveContext::new())
            }
            KeyLookupState::NotFound => ResolveResult::NotHandled,
            _ => ResolveResult::Pending,
        }
    }
}
```

**Use cases:**
- Insert mode character handling with keymap fallback
- Operations needing buffer access
- Undo batching coordination

## Case Study: Insert Mode Escape Handling

A practical example showing why different resolution levels exist.

### The Problem

In insert mode:
- Typed characters (`a`, `b`, `1`, etc.) should insert text
- Escape should exit to normal mode AND properly end undo batching

### Naive Approach (Broken)

```rust
// Level 1 only - doesn't work properly
fn resolve(&self, key: &KeyEvent, _state: &mut ModeState) -> ResolveResult {
    if key.code == KeyCode::Escape {
        // This exits insert mode but DOESN'T end undo batching!
        return ResolveResult::ModeTransition(normal_mode);
    }
    if let KeyCode::Char(c) = key.code {
        return ResolveResult::InsertChar(c);
    }
    ResolveResult::NotHandled
}
```

The problem: Undo batching is managed by the `vim:exit-insert` command. Direct mode transitions bypass it.

### Correct Approach

```rust
// Level 4 - proper keymap lookup for non-insertable keys
fn resolve_with_session(
    &self,
    key: &KeyEvent,
    _state: &mut ModeState,
    input: &ResolveInput<'_>,
    _session: &mut dyn SessionApiDyn,
    _extensions: &mut ExtensionMap,
) -> ResolveResult {
    // Insertable characters: insert directly
    if let Some(c) = Self::is_insertable(key) {
        return ResolveResult::InsertChar(c);
    }

    // Non-insertable keys (Escape, Backspace, etc.): query keymap
    // Keymap has: <Esc> -> vim:exit-insert
    // The command properly ends undo batching
    let keys = KeySequence::from_keys(&[*key]);
    match input.keymap.query(input.mode, &keys) {
        KeyLookupState::ExactOnly(cmd) => {
            ResolveResult::Execute(cmd, ResolveContext::new())
        }
        KeyLookupState::NotFound => ResolveResult::NotHandled,
        _ => ResolveResult::Pending,
    }
}
```

The keymap binds `<Esc>` to `vim:exit-insert`, which:
1. Ends undo batching (groups all insert edits together)
2. Transitions to normal mode
3. Moves cursor back one position (vim behavior)

## ResolveResult Variants

```rust
pub enum ResolveResult {
    /// Execute a command with context
    Execute(CommandId, ResolveContext),

    /// Insert a character at cursor
    InsertChar(char),

    /// Transition to a different mode
    ModeTransition(ModeTransition),

    /// Key sequence is incomplete, wait for more keys
    Pending,

    /// Key was not handled, try fallback or parent resolver
    NotHandled,

    /// Pop pending mode with result (for operator-pending)
    PopResult(PopResult),
}
```

## Mode Inheritance

Resolvers can inherit from parent modes:

```rust
impl ModeKeyResolver for VimVisualResolver {
    fn inherits_from(&self) -> Option<&ModeId> {
        Some(&VimMode::NORMAL_ID)  // Visual inherits from normal
    }
}
```

When a key is `NotHandled`, the registry tries the parent mode's resolver.

## Best Practices

1. **Start with `resolve()`** - Use the simplest level that works
2. **Override only what you need** - Each level adds complexity
3. **Use keymap for configurable bindings** - Don't hardcode keys
4. **Commands for side effects** - Mode transitions with side effects (undo, etc.) should go through commands
5. **Return `NotHandled` for unknown keys** - Let the fallback system handle them

## Related Documents

- [Input Driver Overview](./overview.md) - Key events, mouse, clipboard
- [Module System](../../modules/overview.md) - How modules register resolvers
- [Command System](../command/overview.md) - Command execution
