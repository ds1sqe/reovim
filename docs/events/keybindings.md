# Keybinding System

Key bindings and mode-specific behavior.

## Location

`lib/core/src/bind/mod.rs`

## KeyMap Structure

The keymap uses a scope-based architecture with `KeymapScope` for flexible component/mode binding:

```rust
pub struct KeyMapInner {
    pub command: Option<CommandRef>,
    pub description: Option<String>,
    pub category: Option<&'static str>,
    pub next: HashMap<KeySequence, Self>,
}

pub struct KeyMap {
    /// All keymaps indexed by scope
    maps: HashMap<KeymapScope, HashMap<KeySequence, KeyMapInner>>,
}
```

## KeymapScope

Identifies which keymap a binding belongs to:

```rust
pub enum KeymapScope {
    /// Component-specific: ComponentId + EditModeKind
    Component { id: ComponentId, mode: EditModeKind },
    /// Global sub-mode (Command, OperatorPending, Interactor)
    SubMode(SubModeKind),
    /// Fallback for all components in Normal mode
    DefaultNormal,
}

pub enum EditModeKind { Normal, Insert, Visual }
pub enum SubModeKind { Command, OperatorPending, Interactor(ComponentId) }
```

## Scope-Based Keymaps

| Scope | Example | Purpose |
|-------|---------|---------|
| `Component { EDITOR, Normal }` | `KeymapScope::editor_normal()` | Editor normal mode |
| `Component { EDITOR, Insert }` | `KeymapScope::editor_insert()` | Editor insert mode |
| `Component { EDITOR, Visual }` | `KeymapScope::editor_visual()` | Editor visual mode |
| `SubMode(Command)` | Ex-command input | Command line bindings |
| `SubMode(OperatorPending)` | After d/y/c | Motion bindings |
| `SubMode(Interactor(id))` | Plugin text input | Plugin-specific input mode |
| `Component { MICROSCOPE, Normal }` | Plugin registers | Microscope j/k navigation |
| `Component { EXPLORER, Normal }` | Plugin registers | Explorer navigation |
| `DefaultNormal` | Ctrl-W | Fallback for all Normal modes |

## Keymap Resolution

```rust
impl KeyMap {
    /// Convert ModeState to KeymapScope
    pub fn mode_to_scope(mode: &ModeState) -> KeymapScope {
        // SubMode takes precedence
        match &mode.sub_mode {
            SubMode::Command => KeymapScope::SubMode(SubModeKind::Command),
            SubMode::OperatorPending { .. } => KeymapScope::SubMode(SubModeKind::OperatorPending),
            SubMode::Interactor(id) => KeymapScope::SubMode(SubModeKind::Interactor(*id)),
            SubMode::None => {}
        }
        // Otherwise: ComponentId + EditMode
        KeymapScope::Component { id: mode.interactor_id, mode: mode.edit_mode.into() }
    }

    /// Lookup with fallback to DefaultNormal
    pub fn lookup_binding(&self, mode: &ModeState, keys: &KeySequence) -> Option<&KeyMapInner>;
}
```

## Plugin Keybinding Registration

Plugins register keybindings in `build()` phase:

```rust
fn build(&self, ctx: &mut PluginContext) {
    // Component-specific scope
    let my_normal = KeymapScope::Component {
        id: COMPONENT_ID,
        mode: EditModeKind::Normal,
    };

    ctx.bind_key_scoped(my_normal, keys!['j'], CommandRef::Registered(MY_NEXT));
    ctx.bind_key_scoped(my_normal, keys!['k'], CommandRef::Registered(MY_PREV));
}
```

## DefaultNormal Scope

The `DefaultNormal` scope provides fallback keybindings that work in all plugin windows:
- `Ctrl-W` - Enter window mode (works from Editor, Explorer, etc.)

This eliminates the need for each plugin to register common bindings.

```rust
// Example: Ctrl-W from Explorer
// 1. Check explorer_normal scope -> not found
// 2. Check DefaultNormal scope -> found! (enter_window_mode)
```

## Window Mode Scope

Window mode uses `SubMode::Interactor(ComponentId::WINDOW)`:

| Key | Command |
|-----|---------|
| `h/j/k/l` | Focus window in direction |
| `H/J/K/L` | Move window in direction |
| `x` + `h/j/k/l` | Swap with adjacent window |
| `s/v` | Split horizontal/vertical |
| `c/o/=` | Close/only/equalize |
| `Escape` | Exit window mode |

## Default Bindings

### Normal Mode

| Key | Command |
|-----|---------|
| h/j/k/l | Cursor movement |
| 0/$ | Line start/end |
| w/b | Word forward/backward |
| gg/G | Document start/end |
| i/a/A | Insert modes |
| o/O | Open line |
| v/Ctrl-v | Visual modes |
| : | Command mode |
| x | Delete char |
| p/P | Paste after/before |
| u/Ctrl-r | Undo/redo |
| d/y/c | Operators |
| s | Multi-char jump search |
| f/F/t/T | Single-char jump find/till |
| Ctrl-o/Ctrl-i | Jump list |
| Space e | Toggle explorer |
| Space ff/fb/fg/fr | Microscope pickers |

### Insert Mode

| Key | Command |
|-----|---------|
| Escape | Normal mode |
| Backspace | Delete backward |
| Enter | Newline |
| Alt-Space | Trigger completion |
| Ctrl-n/Ctrl-p | Next/prev completion |
| Ctrl-y | Confirm completion |
| Ctrl-e | Dismiss completion |
| (any char) | Insert character |

### Visual Mode

| Key | Command |
|-----|---------|
| Escape | Normal mode |
| h/j/k/l | Extend selection |
| d | Delete selection |
| y | Yank selection |

### Command Mode

| Key | Command |
|-----|---------|
| Escape | Normal mode |
| Enter | Execute command |
| Backspace | Delete char |
| (any char) | Append to command |

## ModeState Methods

Key methods on `ModeState`:

- `is_normal()` - Check if in normal editing mode
- `is_insert()` - Check if in insert mode
- `is_visual()` - Check if in visual selection mode
- `is_command()` - Check if in command-line mode (`:` prefix)
- `is_operator_pending()` - Check if waiting for motion after operator
- `is_editor_focus()` - Check if editor component has focus

## Related Documentation

- [Handlers](./handlers.md) - CommandHandler details
- [Patterns](./patterns.md) - Event patterns
- [Plugin Tutorial](../plugins/tutorial.md) - Registering keybindings
