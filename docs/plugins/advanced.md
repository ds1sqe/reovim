# Plugin Advanced Guide

This guide covers advanced plugin development patterns in reovim.

## Plugin State Management

### Using PluginStateRegistry

For shared state accessible across handlers:

```rust
use reovim_core::plugin::PluginStateRegistry;
use std::sync::{Arc, RwLock};

#[derive(Default)]
pub struct MyPluginState {
    pub counter: u32,
    pub last_action: Option<String>,
}

pub struct MyPlugin;

impl Plugin for MyPlugin {
    fn name(&self) -> &'static str { "my-plugin" }

    fn build(&self, ctx: &mut PluginContext) {
        // Register state in the registry
        ctx.state.register::<MyPluginState>();
    }

    fn subscribe(&self, bus: &EventBus, state: Arc<PluginStateRegistry>) {
        let state_clone = state.clone();
        bus.subscribe::<MyEvent, _>(100, move |event, ctx| {
            // Access state
            if let Some(my_state) = state_clone.get::<MyPluginState>() {
                let mut guard = my_state.write().unwrap();
                guard.counter += 1;
                guard.last_action = Some("incremented".to_string());
            }
            EventResult::Handled
        });
    }
}
```

### Lock-Free Cache Pattern

For high-frequency reads (like rendering), use `ArcSwap`:

```rust
use arc_swap::ArcSwap;
use std::sync::Arc;

pub struct MyCache {
    data: ArcSwap<Vec<String>>,
}

impl MyCache {
    pub fn new() -> Self {
        Self {
            data: ArcSwap::new(Arc::new(Vec::new())),
        }
    }

    // Fast lock-free read
    pub fn get(&self) -> Arc<Vec<String>> {
        self.data.load_full()
    }

    // Update (can be called from background task)
    pub fn update(&self, new_data: Vec<String>) {
        self.data.store(Arc::new(new_data));
    }
}
```

## Async Background Processing (Saturator Pattern)

For non-blocking computation, use the saturator pattern:

```rust
use tokio::sync::mpsc;

pub enum SaturatorMessage {
    Compute { buffer_id: usize },
    Shutdown,
}

pub struct MySaturator {
    tx: mpsc::Sender<SaturatorMessage>,
}

impl MySaturator {
    pub fn spawn(cache: Arc<MyCache>, event_tx: EventSender) -> Self {
        let (tx, mut rx) = mpsc::channel(32);

        tokio::spawn(async move {
            while let Some(msg) = rx.recv().await {
                match msg {
                    SaturatorMessage::Compute { buffer_id } => {
                        // Do expensive computation
                        let result = compute_expensive_thing(buffer_id);

                        // Update cache (lock-free)
                        cache.update(result);

                        // Notify completion
                        event_tx.try_send(MyComputeReady { buffer_id });
                    }
                    SaturatorMessage::Shutdown => break,
                }
            }
        });

        Self { tx }
    }

    pub fn request_compute(&self, buffer_id: usize) {
        let _ = self.tx.try_send(SaturatorMessage::Compute { buffer_id });
    }
}
```

## Creating UI Overlays

### Overlay Renderer

For popup windows (completion menus, pickers):

```rust
use reovim_core::{
    frame::FrameBuffer,
    overlay::{OverlayRenderer, OverlayBounds},
    component::RenderContext,
    theme::Theme,
};

pub struct MyPopup {
    visible: bool,
    items: Vec<String>,
    selected: usize,
}

impl OverlayRenderer for MyPopup {
    fn z_order(&self) -> u8 {
        200  // Higher = rendered on top
    }

    fn is_visible(&self) -> bool {
        self.visible
    }

    fn bounds(&self, ctx: &RenderContext<'_>) -> OverlayBounds {
        OverlayBounds::Centered {
            width: 40,
            height: self.items.len().min(10) as u16 + 2,
        }
    }

    fn render(&self, buffer: &mut FrameBuffer, bounds: &OverlayBounds, theme: &Theme) {
        let (x, y, w, h) = bounds.to_rect();

        // Draw border
        buffer.draw_box(x, y, w, h, theme.popup.border);

        // Draw items
        for (i, item) in self.items.iter().enumerate() {
            let style = if i == self.selected {
                theme.popup.selected
            } else {
                theme.popup.normal
            };
            buffer.write_str(x + 1, y + 1 + i as u16, item, style);
        }
    }
}
```

### Registering Overlays

```rust
impl Plugin for MyPlugin {
    fn build(&self, ctx: &mut PluginContext) {
        // Create overlay instance
        let popup = Arc::new(RwLock::new(MyPopup::default()));

        // Register as overlay renderer
        ctx.overlays.register(popup.clone());

        // Store reference for event handlers
        ctx.state.register_with(popup);
    }
}
```

## Plugin Window (Side Panels)

For persistent panels like file explorer:

```rust
use reovim_core::plugin::PluginWindow;

pub struct MySidePanel {
    visible: bool,
    width: u16,
}

impl PluginWindow for MySidePanel {
    fn id(&self) -> &'static str {
        "my-panel"
    }

    fn is_visible(&self) -> bool {
        self.visible
    }

    fn width(&self) -> u16 {
        self.width
    }

    fn position(&self) -> WindowPosition {
        WindowPosition::Left
    }

    fn render(&self, buffer: &mut FrameBuffer, bounds: Bounds, theme: &Theme) {
        // Render panel content
    }
}
```

## Component Input Handling

### Registering as Input Target

For plugins that capture keyboard input:

```rust
use reovim_core::{
    modd::ComponentId,
    interactor::{InteractorConfig, InteractorRegistry},
};

// Define your component ID
pub const MY_COMPONENT: ComponentId = ComponentId("my_component");

impl Plugin for MyPlugin {
    fn build(&self, ctx: &mut PluginContext) {
        // Register as interactor (can receive input)
        ctx.interactor_registry.register(
            MY_COMPONENT,
            InteractorConfig {
                accepts_char_input: true,  // Receives character input
            },
        );
    }
}
```

### Handling Targeted Events

```rust
use reovim_core::event_bus::core_events::{PluginTextInput, PluginBackspace};

impl Plugin for MyPlugin {
    fn subscribe(&self, bus: &EventBus, state: Arc<PluginStateRegistry>) {
        // Only receive events targeted at MY_COMPONENT
        bus.subscribe_targeted::<PluginTextInput, _>(
            MY_COMPONENT,
            100,
            |event, ctx| {
                // Handle character input
                let char = event.c;
                // ...
                EventResult::Handled
            },
        );

        bus.subscribe_targeted::<PluginBackspace, _>(
            MY_COMPONENT,
            100,
            |event, ctx| {
                // Handle backspace
                EventResult::Handled
            },
        );
    }
}
```

## Inter-Plugin Communication

### Define Shared Events

Create events that other plugins can subscribe to:

```rust
// In my-plugin/src/events.rs
use reovim_core::event_bus::Event;

#[derive(Debug, Clone)]
pub struct MyPluginDataReady {
    pub data: Vec<String>,
}

impl Event for MyPluginDataReady {
    fn priority(&self) -> u32 { 100 }
}

// Export for other plugins
pub use events::MyPluginDataReady;
```

### Emit Events for Other Plugins

```rust
// In your handler
ctx.emit(MyPluginDataReady {
    data: vec!["item1".to_string(), "item2".to_string()],
});
```

### Subscribe from Another Plugin

```rust
// In another-plugin/src/lib.rs
use reovim_plugin_my_plugin::MyPluginDataReady;

impl Plugin for AnotherPlugin {
    fn subscribe(&self, bus: &EventBus, state: Arc<PluginStateRegistry>) {
        bus.subscribe::<MyPluginDataReady, _>(100, |event, ctx| {
            // React to my-plugin's data
            for item in &event.data {
                tracing::info!("Received: {}", item);
            }
            EventResult::Handled
        });
    }
}
```

## Registering Settings

### Add Configurable Options

```rust
use reovim_core::option::{
    RegisterSettingSection, RegisterOption, OptionSpec, OptionValue,
};

impl Plugin for MyPlugin {
    fn subscribe(&self, bus: &EventBus, _state: Arc<PluginStateRegistry>) {
        // Register settings section
        bus.emit(RegisterSettingSection::new("my_plugin", "My Plugin")
            .with_description("My plugin settings")
            .with_order(100));

        // Register options
        bus.emit(RegisterOption::new(
            OptionSpec::new("enabled", "Enable feature", OptionValue::Bool(true))
                .with_section("My Plugin")
                .with_display_order(10),
        ));

        bus.emit(RegisterOption::new(
            OptionSpec::new("max_items", "Maximum items", OptionValue::Number(50))
                .with_section("My Plugin")
                .with_constraint(OptionConstraint::Range { min: 1, max: 100 })
                .with_display_order(20),
        ));
    }
}
```

### Listen for Option Changes

```rust
use reovim_core::option::OptionChanged;

bus.subscribe::<OptionChanged, _>(100, |event, ctx| {
    if event.name == "my_plugin.enabled" {
        if let OptionValue::Bool(enabled) = &event.value {
            // React to setting change
        }
    }
    EventResult::Handled
});
```

## Render Stages

### Custom Render Pipeline Stage

```rust
use reovim_core::render::{RenderStage, RenderData, RenderStageRegistry};
use std::sync::Arc;

pub struct MyRenderStage;

impl RenderStage for MyRenderStage {
    fn name(&self) -> &'static str {
        "my-render-stage"
    }

    fn transform(&self, mut data: RenderData, ctx: &RenderContext<'_>) -> RenderData {
        // Modify render data
        for (line_idx, highlights) in data.highlights.iter_mut().enumerate() {
            // Add custom highlights
        }
        data
    }
}

impl Plugin for MyPlugin {
    fn build(&self, ctx: &mut PluginContext) {
        ctx.render_stages
            .write()
            .unwrap()
            .register(Arc::new(MyRenderStage));
    }
}
```

## Ex-Commands

### Register Custom Ex-Commands

```rust
use reovim_core::command_line::{ExCommand, ExCommandRegistry, ExCommandResult};

pub struct MyExCommand;

impl ExCommand for MyExCommand {
    fn name(&self) -> &'static str { "mycommand" }
    fn aliases(&self) -> &[&'static str] { &["mc"] }

    fn execute(&self, args: &str, ctx: &mut ExCommandContext) -> ExCommandResult {
        // Handle :mycommand <args>
        tracing::info!("MyCommand executed with: {}", args);
        ExCommandResult::Ok
    }
}

impl Plugin for MyPlugin {
    fn build(&self, ctx: &mut PluginContext) {
        ctx.ex_commands.register(Box::new(MyExCommand));
    }
}
```

## Best Practices

### 1. Minimize Lock Contention

```rust
// BAD: Hold lock during expensive operation
let guard = state.write().unwrap();
let result = expensive_computation(&guard.data);
guard.result = result;

// GOOD: Clone data, compute, then update
let data = {
    let guard = state.read().unwrap();
    guard.data.clone()
};
let result = expensive_computation(&data);
state.write().unwrap().result = result;
```

### 2. Use Appropriate Event Priorities

| Priority | Use Case |
|----------|----------|
| 0-50 | Core events (reserved) |
| 100 | Normal plugin handlers |
| 200+ | After other handlers |

### 3. Request Render Appropriately

```rust
// Only request render if UI changed
if ui_changed {
    ctx.request_render();
}
EventResult::Handled

// Or use NeedsRender variant
EventResult::NeedsRender
```

### 4. Clean Shutdown

```rust
impl Plugin for MyPlugin {
    fn shutdown(&self) {
        // Clean up resources
        if let Some(saturator) = self.saturator.take() {
            saturator.shutdown();
        }
    }
}
```

## Example: Complete Feature Plugin

See these plugins for real-world examples:

- `plugins/features/completion/` - Async completion with saturator
- `plugins/features/explorer/` - Side panel with state management
- `plugins/features/microscope/` - Overlay picker with fuzzy matching
- `plugins/features/which-key/` - Keybinding hints overlay

## Related Documentation

- [Plugin Tutorial](./tutorial.md) - Beginner guide
- [Plugin System](./system.md) - Full API reference
- [Plugin Rendering](../rendering/ui-systems.md) - UI rendering guide
- [Event System](../events/overview.md) - Event flow details
