# input/ - Input Driver

Keyboard, mouse, and clipboard abstractions.

## Source Location

`lib/drivers/input/src/`

## Key Traits

```rust
pub trait KeyboardDriver: Send + Sync {
    fn read_key(&mut self) -> Option<KeyEvent>;
    fn pending(&self) -> bool;
}

pub trait MouseDriver: Send + Sync {
    fn read_mouse(&mut self) -> Option<MouseEvent>;
    fn enable(&mut self);
    fn disable(&mut self);
}

pub trait ClipboardDriver: Send + Sync {
    fn get(&self) -> Result<String>;
    fn set(&mut self, content: &str) -> Result<()>;
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

## Mode Behavior

```rust
// Mode behavior trait
pub trait ModeInput {
    fn accepts_char_input(&self) -> bool;  // Does this mode accept character input?
}
```

## Related Documents

- [Driver Overview](../overview.md) - Driver layer architecture
- [Display Driver](../display/overview.md) - Cursor styles per mode
