# Event System

The event system handles all input and internal communication in reovim.

## Overview

```
lib/core/src/
├── event_bus/          # Type-erased event system (NEW)
│   ├── mod.rs          # Event trait, DynEvent, exports
│   └── bus.rs          # EventBus implementation
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
│       └── mod.rs      # InnerEvent enum
```

## Event Types

### Event Bus (Type-Erased Events)

The event bus provides a type-erased event system for plugin communication. Unlike `InnerEvent`, the event bus allows plugins to define their own event types without modifying core enums.

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
        F: Fn(&E, &EventContext) -> EventResult + Send + Sync + 'static;

    // Emit an event to all subscribers
    pub fn emit<E: Event>(&self, event: E);
}
```

**Event Result:**
```rust
pub enum EventResult {
    Handled,      // Event was handled, stop propagation
    Continue,     // Event was handled, continue propagation
    NotHandled,   // Event was not handled
}
```

**Example - Leap Events:**
```rust
// Define event
#[derive(Debug, Clone)]
pub struct LeapStartEvent {
    pub direction: LeapDirection,
    pub operator: Option<OperatorType>,
    pub count: Option<usize>,
}

impl Event for LeapStartEvent {
    fn priority(&self) -> u32 { 50 }  // High priority for mode changes
}

// Subscribe in plugin
bus.subscribe::<LeapStartEvent, _>(100, |event, _ctx| {
    tracing::trace!(direction = ?event.direction, "Leap started");
    EventResult::Handled
});

// Emit from runtime
event_bus.emit(LeapStartEvent {
    direction: LeapDirection::Forward,
    operator: None,
    count: None,
});
```

**Leap Events (via Event Bus):**
| Event | Description |
|-------|-------------|
| `LeapStartEvent` | Leap mode activated |
| `LeapFirstCharEvent` | First character entered |
| `LeapSecondCharEvent` | Second character entered |
| `LeapSelectLabelEvent` | Label selected for jump |
| `LeapCancelEvent` | Leap mode cancelled |
| `LeapJumpEvent` | Jump completed (from, to, direction) |
| `LeapMatchesFoundEvent` | Matches found (count, pattern) |

## Event Design Patterns

When designing events for your plugin, choose the appropriate pattern based on your needs.

### Zero-Sized Events

Use zero-sized events for simple notifications where the handler has access to all needed state.

```rust
#[derive(Debug, Clone, Copy, Default)]
pub struct RefreshEvent;

impl Event for RefreshEvent {}
```

**When to use:**
- Handler has access to all needed state via `PluginStateRegistry`
- No parameters required from command
- Simple trigger/notification
- State is managed elsewhere

**Benefits:**
- Minimal memory overhead (zero-sized type)
- Implements `Copy` - cheap to clone
- Simple to construct with `Default`

**Example:**
```rust
// Define event
#[derive(Debug, Clone, Copy, Default)]
pub struct ExplorerRefreshEvent;

impl Event for ExplorerRefreshEvent {}

// Handler accesses state directly
bus.subscribe::<ExplorerRefreshEvent, _>(100, |_event, ctx| {
    ctx.state.with_mut::<ExplorerState, _, _>(|state| {
        state.refresh();
        EventResult::Handled
    }).unwrap_or(EventResult::NotHandled)
});
```

### Data-Carrying Events

Use data-carrying events when the handler needs specific context that isn't available in state.

```rust
#[derive(Debug, Clone)]
pub struct MoveEvent {
    pub count: usize,
    pub direction: Direction,
}

impl MoveEvent {
    pub fn new(count: usize, direction: Direction) -> Self {
        Self { count, direction }
    }
}

impl Event for MoveEvent {}
```

**When to use:**
- Command receives parameters (count, direction, arguments)
- Handler needs specific context not in state
- Information passed from external source
- Event carries data between different components

**Benefits:**
- Explicit context passing
- Type-safe parameters
- Self-documenting through fields

**Example:**
```rust
// Define event with data
#[derive(Debug, Clone)]
pub struct CursorMoveEvent {
    pub count: usize,
    pub direction: Direction,
}

impl CursorMoveEvent {
    pub fn new(count: usize, direction: Direction) -> Self {
        Self { count, direction }
    }
}

impl Event for CursorMoveEvent {}

// Handler uses event data
bus.subscribe::<CursorMoveEvent, _>(100, |event, ctx| {
    for _ in 0..event.count {
        ctx.state.move_cursor(event.direction);
    }
    EventResult::Handled
});
```

### Decision Guidelines

**Prefer zero-sized events when:**
- ✅ State is in `PluginStateRegistry`
- ✅ No parameters needed
- ✅ Simple trigger action

**Prefer data-carrying events when:**
- ✅ Need count from command execution
- ✅ Passing data between plugins
- ✅ External context required
- ✅ Handler can't access needed information from state

### Common Patterns

**Pattern 1: Simple Toggle**
```rust
// Zero-sized - state knows whether it's on/off
#[derive(Debug, Clone, Copy, Default)]
pub struct ToggleEvent;
```

**Pattern 2: Counted Action**
```rust
// Data-carrying - count affects behavior
#[derive(Debug, Clone)]
pub struct RepeatEvent {
    pub count: usize,
}
```

**Pattern 3: Directional Movement**
```rust
// Data-carrying - direction is essential context
#[derive(Debug, Clone)]
pub struct MoveEvent {
    pub direction: Direction,
    pub count: usize,
}
```

**Pattern 4: State Change Notification**
```rust
// Zero-sized - handler queries state
#[derive(Debug, Clone, Copy, Default)]
pub struct StateChangedEvent;
```

### InnerEvent (Legacy)

Internal events passed to the runtime via mpsc channel.

> **Note:** Features are being migrated to the Event Bus. New plugins should use `Event` trait and `EventBus` instead of adding variants to `InnerEvent`.

```rust
pub enum InnerEvent {
    // Core events
    BufferEvent(BufferEvent),
    CommandEvent(CommandEvent),
    ModeChangeEvent(ModeState),
    PendingKeysEvent(String),
    WindowEvent(WindowEvent),
    HighlightEvent(HighlightEvent),

    // Feature events
    CompletionEvent(CompletionEvent),
    ExplorerEvent(ExplorerEvent),
    TelescopeEvent(TelescopeEvent),
    LeapEvent(LeapEvent),
    TreesitterEvent(TreesitterEvent),
    OperatorMotionEvent(OperatorMotionAction),

    // Text input events (direct dispatch to components)
    TextInputEvent(TextInputEvent),
    VisualTextObjectEvent(VisualTextObjectAction),

    // UI events
    WhichKeyShow { prefix: String, bindings: Vec<WhichKeyBinding> },
    WhichKeyHide,

    // System
    RenderSignal,
    KillSignal,
}
```

### BufferEvent

Buffer management operations:

```rust
pub enum BufferEvent {
    SetContent { buffer_id: usize, content: String },
    LoadFile { path: String },
    Create,
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

### CompletionEvent

Text completion operations:

```rust
pub enum CompletionEvent {
    Trigger { buffer_id: usize },
    Update { items: Vec<CompletionItem>, prefix: String, start_col: usize, start_row: usize },
    SelectNext,
    SelectPrev,
    Confirm,
    Dismiss,
    UpdateFilter { new_prefix: String },
}
```

### TelescopeEvent

Fuzzy finder operations:

```rust
pub enum TelescopeEvent {
    Open { picker: String },
    UpdateQuery { query: String },
    UpdateItems { items: Vec<TelescopeItem> },
    SelectNext,
    SelectPrev,
    PageDown,
    PageUp,
    Confirm,
    Close,
    UpdatePreview { content: String },
}
```

### LeapEvent

Leap motion operations:

```rust
pub enum LeapEvent {
    Start { direction: LeapDirection, operator: Option<OperatorType>, count: Option<usize> },
    FirstChar { char: char },
    SecondChar { char: char },
    SelectLabel { label: char },
    Cancel,
}
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

### TreesitterEvent

Treesitter parsing and highlight events:

```rust
pub enum TreesitterEvent {
    /// Request reparse for buffer
    Reparse { buffer_id: usize },
}
```

### ExplorerEvent

File explorer operations:

```rust
pub enum ExplorerEvent {
    Toggle,
    Focus,
    Unfocus,
    CursorUp,
    CursorDown,
    ToggleNode,
    OpenNode,
    // ... more variants
}
```

### WindowEvent

Window management:

```rust
pub enum WindowEvent {
    ToggleExplorer,
    FocusExplorer,
    FocusEditor,
}
```

### KeyEvent

Terminal key events from crossterm, broadcast to handlers.

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

Broadcasts key events to subscribed handlers:

```rust
pub struct KeyEventBroker {
    tx: broadcast::Sender<KeyEvent>,
}

impl KeyEventBroker {
    pub fn new() -> Self {
        let (tx, _) = broadcast::channel(255);
        Self { tx }
    }

    pub fn enlist<T: Subscribe<KeyEvent>>(&self, handler: &mut T) {
        handler.subscribe(self.tx.subscribe());
    }

    pub fn handle(&self, event: KeyEvent) {
        let _ = self.tx.send(event);
    }
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

Translates key events to commands:

```rust
pub struct CommandHandler {
    keymap: KeyMap,
    rx: Option<broadcast::Receiver<KeyEvent>>,
    tx: mpsc::Sender<InnerEvent>,
    pending_keys: String,
    pending_count: Option<usize>,
    local_mode: ModeState,
    mode_rx: watch::Receiver<ModeState>,
    command_registry: Arc<CommandRegistry>,
}
```

**Key Translation Process:**
1. Receive KeyEvent from broadcast
2. Update pending_keys with key representation
3. Look up in mode-specific keymap
4. If command found:
   - Create CommandContext with count
   - Send CommandEvent to runtime
   - Clear pending state
5. If partial match: wait for more keys, show which-key
6. If no match: handle based on mode
   - Insert: send InsertChar
   - Command: send CommandLineChar
   - Telescope Insert: send TelescopeInsertChar
   - Normal/Visual: ignore

### CompletionHandler

Handles async completion item fetching:

```rust
pub struct CompletionHandler {
    rx: mpsc::Receiver<CompletionRequest>,
    tx: mpsc::Sender<InnerEvent>,
    engine: Arc<CompletionEngine>,
}
```

**Process:**
1. Receive trigger request
2. Fetch completion items asynchronously
3. Send CompletionEvent::Update with results

### TerminateHandler

Handles Ctrl+C for graceful exit:

```rust
impl TerminateHandler {
    pub async fn subscribe(mut self) {
        while let Ok(event) = self.rx.recv().await {
            if event.code == KeyCode::Char('c')
               && event.modifiers == KeyModifiers::CONTROL {
                let _ = self.tx.send(InnerEvent::KillSignal).await;
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
   - Calls KeyEventBroker::handle()
        │
        ▼
3. KEY BROADCAST
   KeyEventBroker (tokio broadcast, buffer: 255)
        │
        ├────────────────────┬────────────────────┐
        ▼                    ▼                    ▼
4. HANDLERS
   CommandHandler        TerminateHandler    CompletionHandler
   - Keys → Commands     - Ctrl+C → Kill    - Async fetching
   - Track pending_keys
   - Watch mode changes
        │                    │                    │
        ▼                    ▼                    ▼
   CommandEvent          KillSignal        CompletionEvent
   TelescopeEvent
   LeapEvent
   ModeChangeEvent
        │                    │                    │
        └─────────┬──────────┴────────────────────┘
                  ▼
5. RUNTIME EVENT LOOP
   Runtime::rx.recv().await

   match event {
       CommandEvent => execute command
       ModeChangeEvent => update mode, broadcast
       CompletionEvent => update completion state
       TelescopeEvent => update telescope state
       LeapEvent => handle leap motion
       TreesitterEvent => update highlights
       ExplorerEvent => handle explorer
       OperatorMotionEvent => execute operator+motion
       WhichKeyShow/Hide => update which-key panel
       RenderSignal => render()
       KillSignal => exit
   }
        │
        ▼
6. RENDERING
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

### Example: Leap Motion "sab"

```
Step 1: User presses "s"
┌─────────────────────────────────────────────┐
│ CommandHandler                              │
│     lookup("s") → LeapForwardCommand        │
│     │                                       │
│     ▼                                       │
│ Execute → DeferToRuntime(Leap(Start))       │
│     │                                       │
│     ▼                                       │
│ Runtime: set_mode(leap(Forward))            │
│ LeapState: WaitingFirstChar                 │
└─────────────────────────────────────────────┘

Step 2: User presses "a" (first char)
┌─────────────────────────────────────────────┐
│ CommandHandler (in Leap mode)               │
│     Send LeapEvent::FirstChar { char: 'a' } │
│     │                                       │
│     ▼                                       │
│ Runtime: Find all "a?" matches              │
│ LeapState: WaitingSecondChar                │
│ Render targets with labels                  │
└─────────────────────────────────────────────┘

Step 3: User presses "b" (second char)
┌─────────────────────────────────────────────┐
│ LeapEvent::SecondChar { char: 'b' }         │
│     │                                       │
│     ▼                                       │
│ Runtime: Find "ab" matches                  │
│ If single match: jump directly              │
│ If multiple: show labels for selection      │
│ set_mode(normal())                          │
│ Render                                      │
└─────────────────────────────────────────────┘
```

## Key Bindings

The keymap uses a trie structure for multi-key sequences:

```rust
pub struct KeyMapInner {
    pub command: Option<CommandRef>,
    pub next: HashMap<String, Self>,
}

pub struct KeyMap {
    pub normal: HashMap<String, KeyMapInner>,
    pub insert: HashMap<String, KeyMapInner>,
    pub visual: HashMap<String, KeyMapInner>,
    pub command: HashMap<String, KeyMapInner>,
    pub explorer: HashMap<String, KeyMapInner>,
    pub explorer_input: HashMap<String, KeyMapInner>,
    pub operator_pending: HashMap<String, KeyMapInner>,
    pub telescope_normal: HashMap<String, KeyMapInner>,
    pub telescope_insert: HashMap<String, KeyMapInner>,
    pub leap: HashMap<String, KeyMapInner>,
}
```

### Mode-Specific Keymaps

| Mode | Keymap | Purpose |
|------|--------|---------|
| Normal | `normal` | Standard editing commands |
| Insert | `insert` | Text input, Escape, completion |
| Visual | `visual` | Selection extension, operations |
| Command | `command` | Ex-command input |
| Explorer | `explorer` | File browser navigation |
| Explorer Input | `explorer_input` | File creation/rename input |
| Operator Pending | `operator_pending` | Motion after d/y/c |
| Telescope Normal | `telescope_normal` | Navigation with j/k |
| Telescope Insert | `telescope_insert` | Query typing |
| Leap | `leap` | Leap motion key handling |

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
| s/S | Leap forward/backward |
| Ctrl-o/Ctrl-i | Jump list |
| Space e | Toggle explorer |
| Space ff/fb/fg/fr | Telescope pickers |

**Insert Mode:**
| Key | Command |
|-----|---------|
| Escape | Normal mode |
| Backspace | Delete backward |
| Enter | Newline |
| Ctrl-Space | Trigger completion |
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

The CommandHandler adjusts behavior based on current mode:

```rust
if mode.is_insert() {
    // Unmapped keys become InsertChar(c)
} else if mode.is_command() {
    // Unmapped keys become CommandLineChar(c)
} else if mode.is_telescope_focus() && mode.is_insert() {
    // Unmapped keys become TelescopeInsertChar(c)
} else if mode.is_leap() {
    // Keys go to LeapEvent handling
} else if mode.is_normal() || mode.is_visual() {
    // Unmapped keys are ignored
}
```

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
