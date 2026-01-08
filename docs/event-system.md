# Event System

The event system handles all input and internal communication in reovim.

## Overview

```
lib/core/src/
├── event_bus/          # Type-erased event system
│   ├── mod.rs          # Event trait, DynEvent, exports
│   ├── bus.rs          # EventBus implementation
│   ├── core_events.rs  # Core event definitions
│   └── scope.rs        # EventScope for lifecycle tracking
│
├── event/              # Legacy event system
│   ├── mod.rs          # Trait definitions, exports
│   ├── input.rs        # InputEventBroker
│   ├── key/
│   │   └── mod.rs      # KeyEventBroker
│   ├── handler/
│   │   ├── mod.rs      # TerminateHandler
│   │   ├── command/    # CommandHandler
│   │   │   ├── mod.rs
│   │   │   ├── dispatcher.rs
│   │   │   └── count_parser.rs
│   │   └── completion.rs # CompletionHandler
│   └── inner/
│       └── mod.rs      # RuntimeEventPayload enum
```

## Event Types

### Event Bus (Type-Erased Events)

The event bus provides a type-erased event system for plugin communication. Unlike `RuntimeEventPayload`, the event bus allows plugins to define their own event types without modifying core enums.

**Location:** `lib/core/src/event_bus/`

```rust
// Event trait - implemented by all plugin events
pub trait Event: Send + Sync + 'static {
    fn priority(&self) -> u32 { 100 }  // Lower = higher priority
}

// EventBus - type-erased event dispatch
pub struct EventBus {
    handlers: RwLock<HashMap<TypeId, Vec<EventHandler>>>,
}

impl EventBus {
    // Subscribe to events of type E
    pub fn subscribe<E: Event, F>(&self, priority: u32, handler: F)
    where
        F: Fn(&E, &mut HandlerContext) -> EventResult + Send + Sync + 'static;

    // Emit an event to all subscribers
    pub fn emit<E: Event>(&self, event: E);
}
```

**Event Result:**
```rust
pub enum EventResult {
    Handled,      // Event was handled, continue processing other handlers
    Consumed,     // Event was consumed, stop propagation to other handlers
    NotHandled,   // Event was not handled by this handler
    NeedsRender,  // Request a render after all handlers complete
    Quit,         // Editor should quit
}
```

**Example - Plugin Events:**
```rust
// Define event
#[derive(Debug, Clone)]
pub struct MyActionEvent {
    pub data: String,
}

impl Event for MyActionEvent {
    fn priority(&self) -> u32 { 100 }
}

// Subscribe in plugin
bus.subscribe::<MyActionEvent, _>(100, |event, _ctx| {
    tracing::trace!(data = ?event.data, "Action triggered");
    EventResult::Handled
});

// Emit from plugin or runtime
event_bus.emit(MyActionEvent {
    data: "example".to_string(),
});
```

### Event Scope Tracking

**Location:** `lib/core/src/event_bus/scope.rs`

EventScope provides GC-like tracking of event lifecycles for deterministic synchronization:

```rust
use reovim_core::event_bus::{EventScope, ScopeId};

// Create a scope to track events
let scope = EventScope::new();

// Increment when emitting events
scope.increment();  // Event 1 emitted
scope.increment();  // Event 2 emitted (child)

// Decrement when events complete
scope.decrement();  // Event 1 done
scope.decrement();  // Event 2 done, counter = 0

// Wait for all events in scope to complete
scope.wait().await;  // Returns when counter = 0
```

**Key Methods:**

| Method | Description |
|--------|-------------|
| `EventScope::new()` | Create new scope with counter = 0 |
| `scope.increment()` | Track new event (counter++) |
| `scope.decrement()` | Mark event complete (counter--), notify if zero |
| `scope.wait()` | Async wait for counter to reach 0 |
| `scope.wait_timeout(duration)` | Wait with timeout, returns `false` if timed out |
| `scope.in_flight()` | Get current counter value |
| `scope.is_complete()` | Check if counter is 0 |

**Debugging Stuck Scopes:**

Enable trace logging to see scope lifecycle:

```bash
REOVIM_LOG=trace reovim myfile.txt
```

Output:
```
DEBUG EventScope created                    scope_id=1
TRACE EventScope increment                  scope_id=1 in_flight=1
TRACE EventScope increment                  scope_id=1 in_flight=2
TRACE EventScope decrement                  scope_id=1 in_flight=1
TRACE EventScope decrement                  scope_id=1 in_flight=0
DEBUG EventScope completed                  scope_id=1 elapsed_ms=5
```

If a scope hangs, look for increments without matching decrements to find the leak.

**Example Plugin Events:**
| Event | Description |
|-------|-------------|
| `ExplorerRefresh` | Refresh explorer view |
| `MicroscopeOpen` | Open microscope picker |
| `CompletionTrigger` | Trigger completion |
| `SettingsMenuOpen` | Open settings menu |

**Core Events (via Event Bus):**

Core events are defined in `lib/core/src/event_bus/core_events.rs` and allow plugins to request runtime actions.

| Event | Description |
|-------|-------------|
| `RequestSetRegister` | Set register content (unnamed or named) |

**Ex-Command Registry Events (via Event Bus):**

Ex-command events are defined in `lib/core/src/event_bus/core_events.rs` for the plugin ex-command system.

| Event | Priority | Description |
|-------|----------|-------------|
| `RegisterExCommand` | 30 | Plugin registers an ex-command handler |

**RegisterExCommand:**

Emitted by plugins to register ex-command handlers:

```rust
use reovim_core::event_bus::core_events::RegisterExCommand;
use reovim_core::command_line::{ExCommandHandler, HandlerPattern};

bus.emit(RegisterExCommand {
    name: "settings".into(),
    handler: ExCommandHandler::ZeroArg {
        callback: Arc::new(|ctx| {
            ctx.emit(SettingsMenuOpen);
        }),
        description: "Open settings menu".into(),
    },
});
```

Handler patterns:
- `ZeroArg` - Command takes no arguments (e.g., `:settings`)
- `SingleArg` - Command takes one argument (e.g., `:profile load <name>`)
- `Subcommand` - Command has subcommands (e.g., `:profile list|load|save`)

**Profile Events (via Event Bus):**

Profile events are defined in `lib/core/src/event_bus/core_events.rs` for the configuration profile system.

| Event | Priority | Description |
|-------|----------|-------------|
| `RegisterConfigurable` | 30 | Component registers for profile system |
| `ProfileListEvent` | 100 | Profile list requested |
| `ProfileLoadEvent` | 100 | Profile load requested |
| `ProfileSaveEvent` | 100 | Profile save requested |

**RegisterConfigurable:**

Emitted by components that want to participate in the profile save/load system:

```rust
use reovim_core::event_bus::core_events::RegisterConfigurable;
use reovim_core::config::Configurable;

bus.emit(RegisterConfigurable {
    component: Arc::new(RwLock::new(MyConfigurable::new())),
});
```

**Option Events (via Event Bus):**

Option events are defined in `lib/core/src/option/events.rs` for the extensible settings system.

| Event | Priority | Description |
|-------|----------|-------------|
| `RegisterSettingSection` | 40 | Plugin registers a settings section with UI metadata |
| `RegisterOption` | 50 | Plugin registers an option specification |
| `OptionChanged` | 100 | Option value was modified |
| `QueryOption` | 100 | Request to query an option value |
| `ResetOption` | 100 | Request to reset option to default |

**RegisterSettingSection:**

Emitted by plugins to register a settings section for the settings menu:

```rust
use reovim_core::option::RegisterSettingSection;

bus.emit(RegisterSettingSection::new("my_plugin", "My Plugin")
    .with_description("Plugin-specific settings")
    .with_order(100));  // Core sections use 0-50
```

Fields:
- `id: Cow<str>` - Section identifier (must match `OptionSpec.section`)
- `display_name: Cow<str>` - Display name for the section header
- `description: Option<Cow<str>>` - Optional description
- `order: u32` - Display order (lower = earlier)

**OptionChanged:**

Emitted when any option value changes, allowing plugins to react:

```rust
use reovim_core::option::{OptionChanged, ChangeSource};

bus.subscribe::<OptionChanged, _>(100, move |event, ctx| {
    if event.name == "plugin.treesitter.highlight_timeout_ms" {
        if let Some(timeout) = event.as_int() {
            // Update internal timeout setting
        }
        ctx.request_render();
    }
    EventResult::Continue
});
```

Fields:
- `name: String` - Full option name (e.g., `"plugin.treesitter.timeout"`)
- `old_value: OptionValue` - Previous value
- `new_value: OptionValue` - New value
- `source: ChangeSource` - How the change was triggered

`ChangeSource` variants:
- `UserCommand` - User typed a `:set` command
- `ProfileLoad` - Loaded from a profile file
- `Plugin` - Changed programmatically by a plugin
- `SettingsMenu` - Changed via the settings menu UI
- `Rpc` - Changed via RPC (server mode)
- `Default` - Default value applied during initialization

**RequestSetRegister:**

Allows plugins to set register content, enabling features like copy-to-clipboard:

```rust
use reovim_core::event_bus::core_events::RequestSetRegister;

// Copy path to both unnamed register (for 'p' paste) and system clipboard
ctx.emit(RequestSetRegister {
    register: None,      // Unnamed register - used by 'p' paste
    text: path.clone(),
});
ctx.emit(RequestSetRegister {
    register: Some('+'), // System clipboard register
    text: path,
});
```

Register names:
- `None` - Unnamed register (default for yank/paste)
- `Some('+')` - System clipboard
- `Some('a')` to `Some('z')` - Named registers

## Event Design Patterns

When designing events for your plugin, choose the appropriate pattern based on your needs.

### Unified Command-Event Types (Recommended)

**Modern approach:** Use `declare_event_command!` macro to create a single type that serves as both command and event.

```rust
use reovim_core::declare_event_command;

// Single unified type - both command AND event
declare_event_command! {
    ExplorerRefresh,
    id: "explorer_refresh",
    description: "Refresh explorer view",
}

// Register as command
ctx.register_command(ExplorerRefresh);

// Subscribe as event (same type!)
bus.subscribe::<ExplorerRefresh, _>(100, |_event, ctx| {
    ctx.state.with_mut::<ExplorerState, _, _>(|state| {
        state.refresh();
        EventResult::Handled
    }).unwrap_or(EventResult::NotHandled)
});
```

**When to use:**
- ✅ Handler has access to all needed state via `PluginStateRegistry`
- ✅ No parameters required from command
- ✅ Simple trigger/notification
- ✅ **Preferred for most plugin commands**

**Benefits:**
- 50% fewer types (one instead of two)
- ~20 lines of boilerplate eliminated per command
- Minimal memory overhead (zero-sized type)
- Implements `Copy` - cheap to clone
- Simple to construct with `Default`

### Legacy: Separate Event Types (Deprecated)

**Old approach:** Define standalone event types (no longer recommended).

```rust
// DON'T DO THIS - use declare_event_command! instead
#[derive(Debug, Clone, Copy, Default)]
pub struct ExplorerRefreshEvent;

impl Event for ExplorerRefreshEvent {}

// Requires separate command type too (20+ lines of boilerplate)
```

### Unified Types with Data (Counted Commands)

Use `declare_counted_event_command!` for commands that use repeat counts:

```rust
use reovim_core::declare_counted_event_command;

// Single type with count - both command AND event
declare_counted_event_command! {
    ExplorerCursorDown,
    id: "explorer_cursor_down",
    description: "Move cursor down",
}

// Register (Default provides count=1)
ctx.register_command(ExplorerCursorDown::new(1));

// Subscribe - access event.count directly
bus.subscribe::<ExplorerCursorDown, _>(100, |event, ctx| {
    for _ in 0..event.count {
        // Move down
    }
    EventResult::Handled
});
```

**When to use:**
- ✅ Command receives repeat count (e.g., `5j` for "down 5 times")
- ✅ Handler needs count parameter
- ✅ **Preferred over separate Command/Event types**

### Custom Data Fields

For events with custom data that don't fit the standard macros:

```rust
use reovim_core::event_bus::Event;

#[derive(Debug, Clone, Copy)]
pub struct ExplorerInputChar {
    pub c: char,
}

impl ExplorerInputChar {
    pub const fn new(c: char) -> Self {
        Self { c }
    }
}

impl Event for ExplorerInputChar {
    fn priority(&self) -> u32 { 100 }
}

// Still use same type for both command and event
bus.subscribe::<ExplorerInputChar, _>(100, |event, ctx| {
    // Access event.c
    EventResult::Handled
});
```

**When to use:**
- Command receives custom parameters (char, string, complex data)
- Handler needs specific context not available in state
- Information passed from external source

**Benefits:**
- Explicit context passing
- Type-safe parameters
- Self-documenting through fields
- Still uses single type for both command and event

### Decision Guidelines

**Use `declare_event_command!` when:**
- ✅ State is in `PluginStateRegistry`
- ✅ No parameters needed
- ✅ Simple trigger action
- ✅ **This is the default choice for most commands**

**Use `declare_counted_event_command!` when:**
- ✅ Need count from command execution (e.g., `5j`)
- ✅ Command accepts repeat count parameter

**Use custom implementation when:**
- ✅ Passing custom data between plugins
- ✅ External context required (char, string, complex data)
- ✅ Handler can't access needed information from state

### Common Patterns (Modern)

**Pattern 1: Simple Toggle**
```rust
// Unified type - state knows whether it's on/off
declare_event_command! {
    Toggle,
    id: "toggle",
    description: "Toggle feature",
}
```

**Pattern 2: Counted Action**
```rust
// Unified with count - count affects behavior
declare_counted_event_command! {
    Repeat,
    id: "repeat",
    description: "Repeat action",
}
```

**Pattern 3: Custom Data**
```rust
// Custom implementation for specific data
#[derive(Debug, Clone)]
pub struct MoveInDirection {
    pub direction: Direction,
    pub count: usize,
}

impl Event for MoveInDirection {
    fn priority(&self) -> u32 { 100 }
}
```

**Pattern 4: State Change Notification**
```rust
// Unified type - handler queries state
declare_event_command! {
    StateChanged,
    id: "state_changed",
    description: "State has changed",
}
```

### RuntimeEventPayload

**Location:** `lib/core/src/event/inner/mod.rs`

Internal events passed to the runtime via mpsc channel. The payload is wrapped in `RuntimeEvent` which optionally carries an `EventScope` for lifecycle tracking.

```rust
/// Wrapper with optional scope tracking
pub struct RuntimeEvent {
    payload: RuntimeEventPayload,
    scope: Option<EventScope>,
}

/// The actual event data, grouped into logical categories
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

**Sub-enums:**

```rust
pub enum EditingEvent {
    TextInput(TextInputEvent),
    OperatorMotion(OperatorMotionAction),
    VisualTextObject(VisualTextObjectAction),
    MoveCursor { buffer_id: usize, line: u32, column: u32 },
    SetRegister { register: Option<char>, text: String },
}

pub enum ModeEvent {
    Change(ModeState),
    PendingKeys(String),
}

pub enum RenderEvent {
    Signal,
    Highlight(HighlightEvent),
    Syntax(SyntaxEvent),
}

pub enum SettingsEvent {
    LineNumbers { enabled: bool },
    RelativeLineNumbers { enabled: bool },
    Theme { name: String },
    Scrollbar { enabled: bool },
    IndentGuide { enabled: bool },
    SignColumn { mode: SignColumnMode },
    ApplyCmdlineCompletion { text: String, replace_start: usize },
}

pub enum InputEvent {
    Mouse(MouseEvent),
    ScreenResize { width: u16, height: u16 },
}

pub enum FileEvent {
    Open { path: PathBuf },
    OpenAt { path: PathBuf, line: usize, column: usize },
}
```

**Convenience constructors on RuntimeEvent:**

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

### BufferEvent

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

### CommandEvent

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

### Completion Events (Plugin)

**Location:** `plugins/features/completion/src/commands.rs`

Completion is now a plugin using the unified command-event pattern via EventBus:

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

### MicroscopeEvent

Fuzzy finder operations (Microscope plugin):

```rust
// Events are unified command-event types in plugins/features/microscope/src/commands.rs
// Examples:
pub struct MicroscopeOpen { pub picker: String }
pub struct MicroscopeSelectNext;  // via declare_event_command!
pub struct MicroscopeSelectPrev;
pub struct MicroscopePageDown;
pub struct MicroscopePageUp;
pub struct MicroscopeConfirm;
pub struct MicroscopeClose;
pub struct MicroscopeInsertChar { pub c: char }
```

### TextInputEvent

Character input event routed to the currently focused component. Uses a two-tier dispatch system: built-in components (Editor, CommandLine) are handled directly via fast path for synchronous Runtime access, while plugin components implement `UIComponent` trait methods.

```rust
pub enum TextInputEvent {
    InsertChar(char),       // Character to insert
    DeleteCharBackward,     // Backspace/delete
}
```

**Direct Dispatch (Runtime Fast Path):**
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

**Design Rationale:**
- **Built-in components** (Editor, CommandLine) need direct Runtime access (buffers, command_line state)
- **Plugin components** use `PluginStateRegistry` with interior mutability via `UIComponent` trait
- **Synchronous execution** for built-ins eliminates async bounce and race conditions
- **Clear separation** between core and plugin input handling

**Emitted by:**
- `CommandHandler` when processing single printable characters in insert/command mode
- `CommandHandler` when processing Backspace key in input-accepting modes
- Single printable characters → `TextInputEvent::InsertChar(c)`
- Backspace key → `TextInputEvent::DeleteCharBackward`

### SyntaxEvent

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

### Explorer Events (Plugin)

**Location:** `plugins/features/explorer/src/command.rs`

Explorer is now a plugin using the unified command-event pattern via EventBus:

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

### WindowEvent

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

### KeyEvent

Terminal key events from crossterm, broadcast to handlers.

## HandlerContext Methods

The `HandlerContext` passed to event handlers provides several utility methods:

### Basic Methods

```rust
// Emit a new event
ctx.emit(SomeEvent { ... });

// Request a render after event processing
ctx.request_render();

// Request the editor to quit
ctx.request_quit();
```

### Mode Change Helpers

Common patterns for changing editor mode are simplified with helper methods:

```rust
// Enter interactor mode for a plugin component
// Sets both interactor_id and SubMode::Interactor to the component
ctx.enter_interactor_mode(COMPONENT_ID);

// Return to normal editor mode
ctx.exit_to_normal();

// Set arbitrary mode state (for edge cases)
ctx.set_mode(ModeState::with_interactor_id_and_mode(
    COMPONENT_ID,
    EditMode::Normal,
));
```

**When to use:**
- `enter_interactor_mode(id)` - Plugin needs to receive text input (like search, rename dialogs)
- `exit_to_normal()` - Return to normal editing after plugin operation completes
- `set_mode(mode)` - Custom mode transitions that don't fit the standard patterns

## Components

### InputEventBroker

Reads terminal events asynchronously:

```rust
impl InputEventBroker {
    pub async fn subscribe(mut self, mut key_broker: KeyEventBroker) {
        let mut reader = EventStream::new();
        loop {
            tokio::select! {
                event = reader.next() => {
                    // Filter for key Press events
                    // Route to KeyEventBroker
                }
            }
        }
    }
}
```

**Responsibilities:**
- Read crossterm EventStream
- Filter key Press events (ignore Release/Repeat)
- Forward to KeyEventBroker

### KeyEventBroker

**Location:** `lib/core/src/event/key/mod.rs`

Broadcasts scoped key events to subscribed handlers:

```rust
pub struct KeyEventBroker {
    tx: Sender<ScopedKeyEvent>,
}

impl KeyEventBroker {
    pub fn new() -> Self {
        let (tx, _) = broadcast::channel(255);
        Self { tx }
    }

    pub fn enlist(&self, handler: &mut impl Subscribe<ScopedKeyEvent>) {
        handler.subscribe(self.tx.subscribe());
    }

    pub fn handle(&self, ev: ScopedKeyEvent) -> Result<usize, SendError<ScopedKeyEvent>> {
        self.tx.send(ev)
    }
}

/// Key event with optional scope for lifecycle tracking
pub struct ScopedKeyEvent {
    pub event: KeyEvent,
    pub scope: Option<EventScope>,
}
```

### Subscribe Trait

Handlers implement this to receive events:

```rust
pub trait Subscribe<T> {
    fn subscribe(&mut self, rx: broadcast::Receiver<T>);
}
```

## Event Handlers

### CommandHandler

**Location:** `lib/core/src/event/handler/command/mod.rs`

Translates key events to commands:

```rust
pub struct CommandHandler {
    key_event_rx: Option<Receiver<ScopedKeyEvent>>,
    keymap: KeyMap,
    mode_rx: watch::Receiver<ModeState>,
    local_mode: ModeState,
    pending_keys: KeySequence,
    count_parser: CountParser,
    dispatcher: Dispatcher,
    mode_locally_changed: bool,
    interactor_registry: Arc<InteractorRegistry>,
    current_scope: Option<EventScope>,
}
```

**Key Fields:**
- `pending_keys: KeySequence` - Accumulated key sequence (not String)
- `count_parser: CountParser` - Parses numeric prefixes (e.g., "5j")
- `dispatcher: Dispatcher` - Sends events to runtime
- `interactor_registry` - Tracks which components accept text input

**Key Translation Process:**
1. Receive `ScopedKeyEvent` from broadcast (includes scope for tracking)
2. Update `pending_keys` with key sequence
3. Look up in scope-based keymap via `keymap.lookup_binding(mode, keys)`
4. If command found:
   - Create `CommandContext` with count from `CountParser`
   - Send `CommandEvent` via `Dispatcher`
   - Clear pending state
5. If partial match: wait for more keys, trigger which-key panel
6. If no match: handle based on mode and interactor registry
   - Check `interactor_registry.accepts_char_input(mode)`
   - Insert mode → `TextInputEvent::InsertChar(c)`
   - Command mode → CommandLine input
   - Plugin interactor → `PluginTextInput` event via EventBus
   - Normal/Visual → ignore

### TerminateHandler

Handles Ctrl+C for graceful exit:

```rust
impl TerminateHandler {
    pub async fn subscribe(mut self) {
        while let Ok(event) = self.rx.recv().await {
            if event.code == KeyCode::Char('c')
               && event.modifiers == KeyModifiers::CONTROL {
                let _ = self.tx.send(RuntimeEvent::kill()).await;
            }
        }
    }
}
```

## Event Flow

### Complete Flow Diagram

```
1. TERMINAL INPUT
   crossterm EventStream (async)
        │
        ▼
2. INPUT BROKER
   InputEventBroker::subscribe()
   - Filters Key Press events
   - Wraps in ScopedKeyEvent with optional scope
   - Calls KeyEventBroker::handle()
        │
        ▼
3. KEY BROADCAST
   KeyEventBroker (tokio broadcast, buffer: 255)
        │
        ├────────────────────┐
        ▼                    ▼
4. HANDLERS
   CommandHandler        TerminateHandler
   - Keys → Commands     - Ctrl+C → Kill
   - Track pending_keys (KeySequence)
   - Watch mode changes
   - Uses Dispatcher for events
        │                    │
        ▼                    ▼
   RuntimeEventPayload   RuntimeEvent::kill()
        │                    │
        └──────────┬─────────┘
                   ▼
5. RUNTIME EVENT LOOP
   Runtime::rx.recv().await (RuntimeEvent)

   match event.into_payload() {
       Buffer(_) => handle buffer operations
       Window(_) => handle window operations
       Command(_) => execute command via registry
       Editing(EditingEvent::TextInput(_)) => route to component
       Editing(EditingEvent::OperatorMotion(_)) => execute operator
       Mode(ModeEvent::Change(_)) => update mode, broadcast
       Mode(ModeEvent::PendingKeys(_)) => update status line
       Render(RenderEvent::Signal) => trigger render
       Render(RenderEvent::Syntax(_)) => update syntax provider
       Settings(_) => apply setting change
       File(_) => open/navigate to file
       Plugin(_) => emit via EventBus to plugin handlers
       Kill => exit
   }
        │
        ▼
6. EVENT BUS (Plugin Events)
   EventBus handles plugin-defined events:
   - Completion, Explorer, Microscope, etc.
   - Plugins subscribe with handlers
   - May request render or emit more events
        │
        ▼
7. RENDERING
   Screen::render()
```

### Example: "5j" Command

```
Step 1: User presses "5"
┌─────────────────────────────────────────────┐
│ KeyEvent { code: Char('5'), ... }           │
│     │                                       │
│     ▼                                       │
│ CommandHandler                              │
│     pending_keys = "5"                      │
│     pending_count = Some(5)                 │
│     (no command yet, wait for more)         │
└─────────────────────────────────────────────┘

Step 2: User presses "j"
┌─────────────────────────────────────────────┐
│ KeyEvent { code: Char('j'), ... }           │
│     │                                       │
│     ▼                                       │
│ CommandHandler                              │
│     pending_keys = "5j"                     │
│     lookup("j") → CommandRef for cursor_down│
│     clear pending_keys                      │
│     │                                       │
│     ▼                                       │
│ Send CommandEvent {                         │
│     command: ById(CURSOR_DOWN),             │
│     context: { count: 5, ... }              │
│ }                                           │
└─────────────────────────────────────────────┘

Step 3: Runtime processes
┌─────────────────────────────────────────────┐
│ Runtime receives CommandEvent               │
│     │                                       │
│     ▼                                       │
│ Resolve CommandRef → Arc<CursorDownCommand> │
│ Create ExecutionContext                     │
│ cmd.execute(&mut ctx)                       │
│     Move cursor down by 5 lines             │
│     │                                       │
│     ▼                                       │
│ Returns CommandResult::NeedsRender          │
│     │                                       │
│     ▼                                       │
│ Screen::render()                            │
└─────────────────────────────────────────────┘
```

### Example: Plugin Event Flow

```
Step 1: User triggers plugin action
┌─────────────────────────────────────────────┐
│ CommandHandler                              │
│     lookup("Space e") → ExplorerToggle      │
│     │                                       │
│     ▼                                       │
│ EventBus.emit(ExplorerToggle)               │
│     │                                       │
│     ▼                                       │
│ Plugin subscriber receives event            │
│ Plugin updates state, requests render       │
└─────────────────────────────────────────────┘

Step 2: Plugin interacts with runtime
┌─────────────────────────────────────────────┐
│ Plugin emits core event                     │
│     EventBus.emit(RequestCursorMove)        │
│     │                                       │
│     ▼                                       │
│ Runtime handler processes event             │
│ Updates buffer/window state                 │
│ Requests render                             │
└─────────────────────────────────────────────┘
```

## Key Bindings

**Location:** `lib/core/src/bind/mod.rs`

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

/// Identifies which keymap a binding belongs to
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

### Scope-Based Keymaps

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

### Keymap Resolution

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

### Plugin Keybinding Registration

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

### Default Bindings

**Normal Mode:**
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

**Insert Mode:**
| Key | Command |
|-----|---------|
| Escape | Normal mode |
| Backspace | Delete backward |
| Enter | Newline |
| Alt-Space | Trigger completion |
| Ctrl-n/Ctrl-p | Next/prev completion |
| Tab | Confirm completion |
| Ctrl-e | Dismiss completion |
| (any char) | Insert character |

**Visual Mode:**
| Key | Command |
|-----|---------|
| Escape | Normal mode |
| h/j/k/l | Extend selection |
| d | Delete selection |
| y | Yank selection |

**Command Mode:**
| Key | Command |
|-----|---------|
| Escape | Normal mode |
| Enter | Execute command |
| Backspace | Delete char |
| (any char) | Append to command |

## Mode-Specific Behavior

The CommandHandler uses `InteractorRegistry` to determine how to handle unmapped keys:

```rust
// Check if current interactor accepts character input
if interactor_registry.accepts_char_input(&mode) {
    match mode.interactor_id {
        ComponentId::EDITOR if mode.edit_mode.is_insert() => {
            // Editor insert mode: TextInputEvent::InsertChar(c)
            dispatcher.send(RuntimeEvent::text_input(TextInputEvent::InsertChar(c)));
        }
        ComponentId::COMMAND_LINE => {
            // Command mode: direct command line input
        }
        other_component => {
            // Plugin component: emit PluginTextInput via EventBus
            event_bus.emit(PluginTextInput { c, target: other_component });
        }
    }
} else if mode.is_normal() || mode.is_visual() {
    // Unmapped keys in normal/visual mode are ignored
}
```

**Key methods on ModeState:**
- `is_normal()` - Check if in normal editing mode
- `is_insert()` - Check if in insert mode
- `is_visual()` - Check if in visual selection mode
- `is_command()` - Check if in command-line mode (`:` prefix)
- `is_operator_pending()` - Check if waiting for motion after operator
- `is_editor_focus()` - Check if editor component has focus

## Mode Change Broadcasting

Mode changes are broadcast via `watch` channel:

```rust
// In Runtime
pub fn set_mode(&mut self, mode: ModeState) {
    self.mode_state = mode.clone();
    let _ = self.mode_tx.send(mode);
}

// In CommandHandler
loop {
    tokio::select! {
        Ok(key) = self.rx.recv() => { /* handle key */ }
        Ok(()) = self.mode_rx.changed() => {
            self.local_mode = self.mode_rx.borrow().clone();
        }
    }
}
```
