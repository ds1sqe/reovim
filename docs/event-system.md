# Event System

The event system handles all input and internal communication in reovim.

## Overview

```
lib/core/src/event/
├── mod.rs          # Trait definitions, exports
├── input.rs        # InputEventBroker
├── key/
│   └── mod.rs      # KeyEventBroker
├── handler/
│   ├── mod.rs      # TerminateHandler
│   ├── command/    # CommandHandler
│   │   ├── mod.rs
│   │   ├── dispatcher.rs
│   │   └── count_parser.rs
│   └── completion.rs # CompletionHandler
└── inner/
    └── mod.rs      # InnerEvent enum
```

## Event Types

### InnerEvent

Internal events passed to the runtime via mpsc channel:

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

    // Focus input events (enlist pattern)
    FocusInput { char: Option<char>, delete: bool, clear_landing: bool },
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

### FocusInput Event

Generic focus input event using the enlist-based handler registration pattern. Dispatched to registered handlers based on active `InteractorId`:

```rust
InnerEvent::FocusInput {
    char: Option<char>,    // Character to insert (None for delete-only)
    delete: bool,          // Whether to delete backward
    clear_landing: bool,   // Whether to clear landing page (editor-specific)
}
```

**Enlist Pattern (Runtime Dispatch):**
```rust
// Fully generic - no match on InteractorId!
InnerEvent::FocusInput { char, delete, clear_landing } => {
    if let Some(&handler) = self.focus_input_handlers.get(&self.mode_state.interactor_id) {
        handler(self, char, delete, clear_landing);
    }
}
```

**Registered Handlers (enlist.rs):**
- `InteractorId::TELESCOPE` → Updates telescope query, triggers async filtering
- `InteractorId::EDITOR` → Handles command line (command mode) or buffer editing (insert mode)
- `InteractorId::EXPLORER` → Handles input internally via `InputResult::Handled`

**Emitted by:**
- Interactor implementations via `InputResult::SendEvent(InnerEvent::FocusInput { ... })`
- Single printable characters → `char: Some(c), delete: false`
- Backspace key → `char: None, delete: true`

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
