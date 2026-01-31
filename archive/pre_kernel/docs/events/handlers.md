# Event Handlers

Components that process events in the reovim event system.

## InputEventBroker

Reads terminal events asynchronously.

**Location:** `lib/core/src/event/input.rs`

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

## KeyEventBroker

Broadcasts scoped key events to subscribed handlers.

**Location:** `lib/core/src/event/key/mod.rs`

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

## Subscribe Trait

Handlers implement this to receive events:

```rust
pub trait Subscribe<T> {
    fn subscribe(&mut self, rx: broadcast::Receiver<T>);
}
```

## CommandHandler

Translates key events to commands.

**Location:** `lib/core/src/event/handler/command/mod.rs`

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

### Key Fields

- `pending_keys: KeySequence` - Accumulated key sequence (not String)
- `count_parser: CountParser` - Parses numeric prefixes (e.g., "5j")
- `dispatcher: Dispatcher` - Sends events to runtime
- `interactor_registry` - Tracks which components accept text input

### Key Translation Process

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

## TerminateHandler

Handles Ctrl+C for graceful exit.

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

## Example Flow: "5j" Command

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

## Example Flow: Plugin Event

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

## Related Documentation

- [Keybindings](./keybindings.md) - Key binding system
- [Runtime Events](./runtime-events.md) - Event payloads
- [Overview](./overview.md) - Event system overview
