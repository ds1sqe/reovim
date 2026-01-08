# Runtime Events

Internal events passed to the runtime via mpsc channel.

## Location

`lib/core/src/event/inner/mod.rs`

## RuntimeEvent Wrapper

```rust
/// Wrapper with optional scope tracking
pub struct RuntimeEvent {
    payload: RuntimeEventPayload,
    scope: Option<EventScope>,
}
```

## RuntimeEventPayload

The actual event data, grouped into logical categories:

```rust
pub enum RuntimeEventPayload {
    Buffer(BufferEvent),       // Buffer operations
    Window(WindowEvent),       // Window management
    Command(CommandEvent),     // Command execution
    Editing(EditingEvent),     // Text editing operations
    Mode(ModeEvent),           // Mode changes
    Render(RenderEvent),       // Visual updates
    Settings(SettingsEvent),   // Settings/options
    Input(InputEvent),         // External input (mouse, resize)
    File(FileEvent),           // File operations
    Rpc(RpcEvent),             // RPC requests (server mode)
    Plugin(PluginEventData),   // Plugin-defined events
    Kill,                      // Terminate runtime
}
```

## Sub-Enums

### EditingEvent

```rust
pub enum EditingEvent {
    TextInput(TextInputEvent),
    OperatorMotion(OperatorMotionAction),
    VisualTextObject(VisualTextObjectAction),
    MoveCursor { buffer_id: usize, line: u32, column: u32 },
    SetRegister { register: Option<char>, text: String },
}
```

### ModeEvent

```rust
pub enum ModeEvent {
    Change(ModeState),
    PendingKeys(String),
}
```

### RenderEvent

```rust
pub enum RenderEvent {
    Signal,
    Highlight(HighlightEvent),
    Syntax(SyntaxEvent),
}
```

### SettingsEvent

```rust
pub enum SettingsEvent {
    LineNumbers { enabled: bool },
    RelativeLineNumbers { enabled: bool },
    Theme { name: String },
    Scrollbar { enabled: bool },
    IndentGuide { enabled: bool },
    SignColumn { mode: SignColumnMode },
    ApplyCmdlineCompletion { text: String, replace_start: usize },
}
```

### InputEvent

```rust
pub enum InputEvent {
    Mouse(MouseEvent),
    ScreenResize { width: u16, height: u16 },
}
```

### FileEvent

```rust
pub enum FileEvent {
    Open { path: PathBuf },
    OpenAt { path: PathBuf, line: usize, column: usize },
}
```

## Convenience Constructors

```rust
RuntimeEvent::render_signal()
RuntimeEvent::kill()
RuntimeEvent::mode_change(mode)
RuntimeEvent::pending_keys(keys)
RuntimeEvent::buffer(event)
RuntimeEvent::window(event)
RuntimeEvent::command(event)
RuntimeEvent::open_file(path)
RuntimeEvent::plugin(plugin_id, event)
// ... and more
```

## BufferEvent

Buffer management operations:

```rust
pub enum BufferEvent {
    SetContent { buffer_id: usize, content: String },
    LoadFile { buffer_id: usize, path: PathBuf },
    Create { buffer_id: usize },
    Close { buffer_id: usize },
    Switch { buffer_id: usize },
}
```

## CommandEvent

Command execution request:

```rust
pub struct CommandEvent {
    pub command: CommandRef,
    pub context: CommandContext,
}

pub struct CommandContext {
    pub buffer_id: usize,
    pub window_id: usize,
    pub count: Option<usize>,
}
```

## WindowEvent

Window management:

```rust
pub enum WindowEvent {
    // Focus
    FocusPlugin { id: ComponentId },
    FocusEditor,

    // Splits
    SplitHorizontal { filename: Option<String> },
    SplitVertical { filename: Option<String> },
    Close { force: bool },
    CloseOthers,

    // Navigation
    FocusDirection { direction: NavigateDirection },
    MoveWindow { direction: NavigateDirection },

    // Resize
    Resize { direction: SplitDirection, delta: i16 },
    Equalize,

    // Tabs
    TabNew { filename: Option<String> },
    TabClose,
    TabNext,
    TabPrev,
    TabGoto { index: usize },
}
```

## TextInputEvent

Character input event routed to the currently focused component:

```rust
pub enum TextInputEvent {
    InsertChar(char),       // Character to insert
    DeleteCharBackward,     // Backspace/delete
}
```

### Direct Dispatch (Runtime Fast Path)

```rust
fn handle_interactor_input(&mut self, event: TextInputEvent) {
    let interactor_id = self.mode_state.interactor_id;

    // Fast path: Built-in components with direct Runtime access
    match interactor_id {
        ComponentId::EDITOR => {
            match event {
                TextInputEvent::InsertChar(c) => {
                    handle_editor_input(self, Some(c), false, false);
                }
                TextInputEvent::DeleteCharBackward => {
                    handle_editor_input(self, None, true, false);
                }
            }
            self.request_render();
            return;
        }
        ComponentId::COMMAND_LINE => {
            // Similar direct dispatch
        }
        _ => {
            // Plugin component path via UIComponent trait
            component.handle_insert_char(c, &mode_state, &plugin_state)
        }
    }
}
```

### Design Rationale

- **Built-in components** (Editor, CommandLine) need direct Runtime access (buffers, command_line state)
- **Plugin components** use `PluginStateRegistry` with interior mutability via `UIComponent` trait
- **Synchronous execution** for built-ins eliminates async bounce and race conditions
- **Clear separation** between core and plugin input handling

### Emitted By

- `CommandHandler` when processing single printable characters in insert/command mode
- `CommandHandler` when processing Backspace key in input-accepting modes
- Single printable characters → `TextInputEvent::InsertChar(c)`
- Backspace key → `TextInputEvent::DeleteCharBackward`

## SyntaxEvent

Syntax/treesitter operations (part of `RenderEvent`):

```rust
pub enum SyntaxEvent {
    /// Attach a syntax provider to a buffer
    Attach { buffer_id: usize, syntax: Box<dyn SyntaxProvider> },
    /// Detach syntax provider from a buffer
    Detach { buffer_id: usize },
    /// Request a reparse (after buffer modification)
    Reparse { buffer_id: usize },
}
```

> **Note:** Emitted via `RuntimeEvent::syntax(event)` or `RuntimeEventPayload::Render(RenderEvent::Syntax(event))`.

## Plugin-Specific Events

### Completion Events

**Location:** `plugins/features/completion/src/commands.rs`

```rust
// Unified command-event types (via declare_event_command! macro)
pub struct CompletionTrigger;      // Trigger completion popup
pub struct CompletionSelectNext;   // Select next item
pub struct CompletionSelectPrev;   // Select previous item
pub struct CompletionConfirm;      // Confirm selection
pub struct CompletionDismiss;      // Close popup

// Event with data
pub struct CompletionReady {
    pub items: Vec<CompletionItem>,
    pub prefix: String,
}
```

### Microscope Events

**Location:** `plugins/features/microscope/src/commands.rs`

```rust
pub struct MicroscopeOpen { pub picker: String }
pub struct MicroscopeSelectNext;  // via declare_event_command!
pub struct MicroscopeSelectPrev;
pub struct MicroscopePageDown;
pub struct MicroscopePageUp;
pub struct MicroscopeConfirm;
pub struct MicroscopeClose;
pub struct MicroscopeInsertChar { pub c: char }
```

### Explorer Events

**Location:** `plugins/features/explorer/src/command.rs`

```rust
// Unified command-event types (via declare_event_command! macro)
pub struct ExplorerToggle;         // Toggle file explorer
pub struct ExplorerRefresh;        // Refresh explorer view
pub struct ExplorerCursorUp;       // Move cursor up
pub struct ExplorerCursorDown;     // Move cursor down
pub struct ExplorerToggleNode;     // Expand/collapse directory
pub struct ExplorerOpenNode;       // Open file or expand directory
pub struct ExplorerClose;          // Close explorer
```

## Related Documentation

- [Event Bus](./eventbus.md) - Type-erased events
- [Handlers](./handlers.md) - Event processing
- [Patterns](./patterns.md) - Event design patterns
