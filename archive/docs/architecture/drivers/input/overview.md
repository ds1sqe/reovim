# input/ - Input Driver

Keyboard, mouse, and clipboard abstractions.

## Source Location

`lib/drivers/input/src/`

## Key Traits

```rust
pub trait ClipboardProvider: Send + Sync {
    fn get_clipboard(&self) -> Result<String>;
    fn set_clipboard(&self, content: &str) -> Result<()>;
    fn get_selection(&self) -> Result<String>;
    fn set_selection(&self, content: &str) -> Result<()>;
}

pub trait ModeKeyResolver: Send + Sync {
    fn mode_id(&self) -> &ModeId;
    fn inherits_from(&self) -> Option<&ModeId>;

    fn resolve(&self, key: &KeyEvent, state: &mut ModeState) -> ResolveResult;
    fn resolve_with_keymap(&self, ...) -> ResolveResult;
    fn resolve_with_extensions(&self, ...) -> ResolveResult;
    fn resolve_with_session(&self, ...) -> ResolveResult;
}
```

## Key Events

```rust
pub struct KeyEvent {
    pub code: KeyCode,
    pub modifiers: Modifiers,
    pub kind: KeyEventKind,  // Press, Release, Repeat
}

impl KeyEvent {
    pub fn new(code: KeyCode) -> Self;
    pub fn with_modifiers(code: KeyCode, modifiers: Modifiers) -> Self;
    pub fn is_press(&self) -> bool;  // Check if this is a key press
}
```

## Mouse Events

```rust
pub struct MouseEvent {
    pub kind: MouseEventKind,
    pub position: (u16, u16),
    pub modifiers: Modifiers,
}
```

## Key Sequences

```rust
// Key sequence for multi-key bindings (e.g., "gg", "<C-w>h")
pub struct KeySequence {
    keys: Vec<KeyEvent>,
}

impl KeySequence {
    pub fn parse(s: &str) -> Option<Self>;  // Parse "gg", "<C-w>h", etc.
    pub fn push(&mut self, key: KeyEvent);
    pub fn starts_with(&self, other: &Self) -> bool;
}
```

## Fallback Handling

When a key sequence doesn't match any keymap binding, the event loop delegates to a fallback handler. The input driver provides the trait interface and basic implementations.

```rust
// Context provided to fallback handlers
pub trait FallbackContext: Send {
    fn current_mode(&self) -> &ModeId;
    fn active_buffer(&self) -> Option<BufferId>;
    fn get_buffer(&self, id: BufferId) -> Option<Arc<RwLock<Buffer>>>;
    fn record_edit(&mut self, buffer_id: BufferId, edits: Vec<Edit>,
                   cursor_before: Position, cursor_after: Position);
}

// Result of fallback handling
pub enum FallbackResult {
    Handled,  // Key was processed (e.g., character inserted)
    Ignored,  // Key was ignored
    Beep,     // Key was invalid (show warning)
}

// Trait for handling unmatched keys
pub trait InputFallbackHandler<C: FallbackContext>: Send + Sync {
    fn handle_unmatched(&self, key: KeyEvent, ctx: &mut C) -> FallbackResult;
}
```

Built-in implementations:
- `NoOpFallback` - Ignores all unmatched keys
- `BeepFallback` - Beeps on all unmatched keys

Modules provide policy implementations (e.g., `EditorFallbackHandler` inserts characters in Insert mode).

## Mode Key Resolution

The input driver provides a **resolver chain** for mode-specific key handling via `ModeKeyResolver` (see "Key Traits" above). See [Resolver Chain Architecture](./resolver-chain.md) for the complete documentation.

The resolution methods follow a layered override pattern:
- `resolve()` — base resolution
- `resolve_with_keymap()` — adds keymap access
- `resolve_with_extensions()` — adds extension data
- `resolve_with_session()` — adds session access

Modules register resolvers for their modes:

```rust
// In vim module init()
resolver_registry.register(VimNormalResolver::new());
resolver_registry.register(VimInsertResolver::new());
resolver_registry.register(VimVisualResolver::new());
```

## Related Documents

- [Resolver Chain Architecture](./resolver-chain.md) - Full resolver documentation
- [Driver Overview](../overview.md) - Driver layer architecture
- [Display Driver](../display/overview.md) - Cursor styles per mode
