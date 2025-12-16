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
│   └── command.rs  # CommandHandler
└── inner/
    └── mod.rs      # InnerEvent enum
```

## Event Types

### InnerEvent

Internal events passed to the runtime via mpsc channel:

```rust
pub enum InnerEvent {
    BufferEvent(BufferEvent),       // Buffer content updates
    CommandEvent(CommandEvent),     // Command to execute
    ModeChangeEvent(Mod),           // Mode transition
    PendingKeysEvent(String),       // Multi-key sequence display
    HighlightEvent(HighlightEvent), // Syntax highlighting
    WindowEvent,                    // Window operations
    RenderSignal,                   // Trigger render
    KillSignal,                     // Exit editor
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
    current_mode: Mod,
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
5. If partial match: wait for more keys
6. If no match: handle based on mode
   - Insert: send InsertChar
   - Command: send CommandLineChar
   - Normal/Visual: ignore

### TerminateHandler

Handles Ctrl+D for exit:

```rust
impl TerminateHandler {
    pub async fn subscribe(mut self) {
        while let Ok(event) = self.rx.recv().await {
            if event.code == KeyCode::Char('d')
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
        ├────────────────────┐
        ▼                    ▼
4. HANDLERS
   CommandHandler        TerminateHandler
   - Keys → Commands     - Ctrl+D → KillSignal
   - Track pending_keys
   - Build CommandContext
        │                    │
        ▼                    ▼
   CommandEvent          KillSignal
        │                    │
        └─────────┬──────────┘
                  ▼
5. RUNTIME EVENT LOOP
   Runtime::rx.recv().await

   match event {
       CommandEvent => execute command
       ModeChangeEvent => update mode
       HighlightEvent => update highlights
       PendingKeysEvent => update display
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
│     lookup("j") → Command::CursorDown       │
│     clear pending_keys                      │
│     │                                       │
│     ▼                                       │
│ Send CommandEvent {                         │
│     command: CursorDown,                    │
│     context: { count: 5, ... }              │
│ }                                           │
└─────────────────────────────────────────────┘

Step 3: Runtime processes
┌─────────────────────────────────────────────┐
│ Runtime receives CommandEvent               │
│     │                                       │
│     ▼                                       │
│ BufferCommandExecutor::execute_on_buffer()  │
│     Move cursor down by 5 lines             │
│     │                                       │
│     ▼                                       │
│ Returns CommandResult::NeedsRender          │
│     │                                       │
│     ▼                                       │
│ Screen::render()                            │
└─────────────────────────────────────────────┘
```

## Key Bindings

The keymap uses a trie structure for multi-key sequences:

```rust
pub struct KeyMapInner {
    pub command: Option<Command>,
    pub next: HashMap<String, Self>,
}

pub struct KeyMap {
    pub nmap: HashMap<String, KeyMapInner>,  // Normal
    pub imap: HashMap<String, KeyMapInner>,  // Insert
    pub vmap: HashMap<String, KeyMapInner>,  // Visual
    pub cmap: HashMap<String, KeyMapInner>,  // Command
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
| v | Visual mode |
| : | Command mode |
| x | Delete char |
| p/P | Paste after/before |

**Insert Mode:**
| Key | Command |
|-----|---------|
| Escape | Normal mode |
| Backspace | Delete backward |
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
match mode {
    Mod::Insert(_) => {
        // Unmapped keys become InsertChar(c)
    }
    Mod::Command => {
        // Unmapped keys become CommandLineChar(c)
    }
    Mod::Normal | Mod::Visual(_) => {
        // Unmapped keys are ignored
    }
}
```
