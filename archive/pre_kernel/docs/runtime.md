# Runtime

The Runtime is the central event loop that owns all editor state.

## Module Overview

```
lib/core/src/
├── runtime/        # Central event loop
│   ├── core.rs     # Runtime struct
│   ├── event_loop.rs
│   └── handlers.rs
```

## Runtime Struct

```rust
pub struct Runtime {
    // Buffer management
    pub buffers: BTreeMap<usize, Buffer>,
    next_buffer_id: usize,
    // Note: active_buffer_id is derived from screen.active_buffer_id()

    // Display
    pub screen: Screen,
    pub highlight_store: HighlightStore,
    pub color_mode: ColorMode,
    pub theme: Theme,

    // Mode and state
    pub mode_state: ModeState,
    pub command_line: CommandLine,
    pub pending_keys: String,
    pub last_command: String,
    pub keymap: KeyMap,

    // Event channels (dual-channel for priority routing)
    pub hi_tx: mpsc::Sender<RuntimeEvent>,  // High-priority: user input, mode changes
    hi_rx: mpsc::Receiver<RuntimeEvent>,    // 64 capacity
    lo_tx: mpsc::Sender<RuntimeEvent>,      // Low-priority: render signals, plugins
    lo_rx: mpsc::Receiver<RuntimeEvent>,    // 255 capacity
    pub mode_tx: watch::Sender<ModeState>,
    mode_rx: watch::Receiver<ModeState>,

    // Plugin infrastructure
    pub event_bus: Arc<EventBus>,
    pub plugin_state: Arc<PluginStateRegistry>,
    pub rpc_handler_registry: RpcHandlerRegistry,
    pub display_registry: DisplayRegistry,
    pub interactor_registry: Arc<InteractorRegistry>,

    // Command system
    pub command_registry: Arc<CommandRegistry>,
    pub registers: Registers,

    // Registries
    pub modifier_registry: ModifierRegistry,
    pub decoration_store: DecorationStore,
    pub renderer_registry: LanguageRendererRegistry,
    pub render_stages: Arc<RwLock<RenderStageRegistry>>,
    pub option_registry: Arc<OptionRegistry>,
    pub ex_command_registry: Arc<ExCommandRegistry>,
    pub profile_registry: Arc<ProfileRegistry>,

    // Navigation and editing
    pub jump_list: JumpList,
    pub indent_analyzer: IndentAnalyzer,
    pub profile_manager: ProfileManager,
    pub current_profile_name: String,

    // Internal state
    render_pending: bool,
    plugins: Vec<Box<dyn Plugin>>,
    current_scope: Option<EventScope>,
    // ... additional internal fields
}
```

> **Note:** Feature plugins (Explorer, Microscope, Completion, etc.) store their state in `PluginStateRegistry`, not directly in Runtime.

## Responsibilities

- Process events sequentially through single-threaded loop
- Own all buffers and screen state
- Handle mode transitions via `set_mode()`
- Coordinate rendering
- Dispatch deferred actions to feature handlers

## Event Loop Pattern

The runtime uses a single-threaded event loop pattern:

```rust
impl Runtime {
    pub async fn run(&mut self) {
        loop {
            tokio::select! {
                event = self.rx.recv() => {
                    match event {
                        Some(ev) => self.handle_event(ev).await,
                        None => break,
                    }
                }
            }

            if self.render_pending {
                self.render();
                self.render_pending = false;
            }
        }
    }
}
```

## Mode Broadcasting

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

## Architecture Patterns

### Single-Threaded Event Loop
- Runtime processes events sequentially
- No locks needed - prevents race conditions
- Responsive via async I/O

### Trait-Based Command System
- All commands implement `CommandTrait`
- Commands registered in `CommandRegistry`
- Commands return `CommandResult` indicating side effects
- Complex actions deferred via `DeferredAction`

### Trait-Based Subscriptions
```rust
pub trait Subscribe<T> {
    fn subscribe(&mut self, rx: broadcast::Receiver<T>);
}
```

## State Flow

```
Terminal Input
      │
      ▼
InputEventBroker (crossterm EventStream)
      │
      ▼
KeyEventBroker (tokio broadcast)
      │
      ├──────────────────┐
      ▼                  ▼
CommandHandler      TerminateHandler
      │                  │
      ▼                  ▼
CommandEvent         KillSignal
      │                  │
      └───────┬──────────┘
              ▼
         Runtime (mpsc)
              │
              ▼
         Process Event
              │
              ├── CommandEvent → execute command
              ├── ModeChangeEvent → update mode
              ├── Plugin events → dispatch via EventBus:
              │   ├── Completion events → completion plugin
              │   ├── Microscope events → fuzzy finder plugin
              │   ├── Explorer events → file browser plugin
              │   └── RangeFinder events → jump/fold plugin
              └── ...
              │
              ▼
           Render
```

## Related Documentation

- [Event System](../events/overview.md) - Detailed event flow
- [Buffer](./buffer.md) - Text storage
- [Screen](./screen.md) - Terminal rendering
