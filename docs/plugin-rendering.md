# Plugin Rendering Guide

This guide explains the three rendering systems available in Reovim and when to use each one.

## Overview

Reovim provides three distinct rendering systems, each optimized for different UI patterns:

1. **Overlays** - Temporary popups and modals
2. **Window Providers** - Persistent side panels
3. **UIComponent Rendering** - Fixed UI elements (status line, tab line)

## Decision Matrix

| Use Case | System | When to Use |
|----------|--------|-------------|
| Popup/Modal | **Overlay** | Temporary UI over editor (completion, telescope) |
| Sidebar/Panel | **WindowProvider** | Persistent side panel (explorer, outline) |
| Status/Tab line | **UIComponent::render_to_frame()** | Fixed position UI elements |

## Overlays - Temporary Popups

### When to Use

Use overlays for temporary UI elements that:
- Appear over other content (z-ordered)
- Disappear when dismissed
- Are centered or positioned relative to cursor
- Don't persist across mode changes

### Examples

- Completion menus
- Fuzzy finders (telescope)
- Command palettes
- Quick pick dialogs

### Characteristics

- **Z-ordered**: Can appear over other UI components
- **Transient**: Automatically dismissed when focus changes
- **Positioned**: Can be centered, cursor-relative, or explicitly positioned
- **Non-blocking**: Don't affect window layout

### Implementation

1. **Implement the `OverlayRenderer` trait**:

```rust
use reovim_core::overlay::{OverlayRenderer, OverlayBounds};
use reovim_core::frame::FrameBuffer;
use reovim_core::component::RenderContext;

struct MyOverlay {
    items: Vec<String>,
    selected: usize,
}

impl OverlayRenderer for MyOverlay {
    fn z_order(&self) -> u8 {
        reovim_core::screen::z_order::OVERLAY
    }

    fn bounds(&self, ctx: &RenderContext<'_>) -> OverlayBounds {
        let width = 40;
        let height = self.items.len().min(10);

        OverlayBounds::Centered { width, height }
    }

    fn render_to_frame(&self, buffer: &mut FrameBuffer, ctx: &RenderContext<'_>) {
        // Render overlay content
        for (i, item) in self.items.iter().enumerate() {
            let style = if i == self.selected {
                Style::default().bg(Color::Blue)
            } else {
                Style::default()
            };
            buffer.set_string(0, i, item, style);
        }
    }
}
```

2. **Register the overlay**:

```rust
fn build(&self, ctx: &mut PluginContext) {
    ctx.register_overlay(Box::new(MyOverlay::new()));
}
```

### Best Practices

- Keep overlays focused on a single task
- Provide clear visual hierarchy
- Handle dismissal gracefully
- Consider terminal size constraints

## Window Providers - Persistent Panels

### When to Use

Use window providers for UI elements that:
- Are part of the window layout
- Persist across mode changes
- Occupy dedicated screen space
- Can be toggled on/off

### Examples

- File explorer
- Outline/symbol view
- Terminal panes
- Git status panel

### Characteristics

- **Persistent**: Stay open until explicitly closed
- **Layout-integrated**: Part of window split system
- **Stateful**: Maintain state across mode changes
- **Resizable**: Can participate in window sizing

### Implementation

1. **Implement the `WindowProvider` trait**:

```rust
use reovim_core::screen::window::WindowProvider;
use reovim_core::buffer::BufferId;
use reovim_core::frame::FrameBuffer;

struct MyPanel {
    items: Vec<String>,
    cursor: usize,
}

impl WindowProvider for MyPanel {
    fn id(&self) -> ComponentId {
        ComponentId::new("my_panel")
    }

    fn name(&self) -> &str {
        "My Panel"
    }

    fn buffer_id(&self) -> Option<BufferId> {
        None  // Not backed by a buffer
    }

    fn render(
        &self,
        buffer: &mut FrameBuffer,
        bounds: LayerBounds,
        ctx: &RenderContext<'_>,
    ) {
        // Render panel content within bounds
        for (i, item) in self.items.iter().enumerate() {
            if i >= bounds.height {
                break;
            }
            buffer.set_string(bounds.x, bounds.y + i, item, Style::default());
        }
    }

    fn wants_focus(&self) -> bool {
        true  // Can receive focus
    }
}
```

2. **Register the window provider**:

```rust
fn init_state(&self, registry: &PluginStateRegistry) {
    let state = Arc::new(MyPanelState::new());
    registry.register(Arc::clone(&state));

    // Register as window provider
    registry.register_window_provider(
        Arc::clone(&state) as Arc<dyn WindowProvider>
    );
}
```

### Best Practices

- Respect provided bounds
- Handle focus state correctly
- Provide meaningful cursor positioning
- Support window resize gracefully

## UIComponent Rendering - Fixed UI Elements

### When to Use

Use UIComponent rendering for:
- Fixed position elements (top/bottom of screen)
- Always-visible chrome
- Status information
- Tab/buffer lists

### Examples

- Status line
- Tab line
- Title bar
- Command line area

### Characteristics

- **Fixed position**: Top or bottom of screen
- **Always visible**: Part of editor chrome
- **Non-interactive rendering**: For display only (use other traits for input)
- **Simple**: Straightforward rendering API

### Implementation

1. **Implement UIComponent with render_to_frame**:

```rust
use reovim_core::ui_component::UIComponent;
use reovim_core::frame::FrameBuffer;
use reovim_core::component::RenderContext;

struct MyStatusLine;

impl UIComponent for MyStatusLine {
    fn id(&self) -> ComponentId {
        ComponentId::new("my_status")
    }

    fn display_name(&self) -> &'static str {
        "STATUS"
    }

    fn z_order(&self) -> u8 {
        reovim_core::screen::z_order::STATUS_LINE
    }

    fn is_visible(&self, _ctx: &RenderContext<'_>) -> bool {
        true
    }

    fn bounds(&self, ctx: &RenderContext<'_>) -> LayerBounds {
        // Bottom line of screen
        LayerBounds {
            x: 0,
            y: ctx.screen_height - 1,
            width: ctx.screen_width,
            height: 1,
        }
    }

    fn render_to_frame(&self, buffer: &mut FrameBuffer, ctx: &RenderContext<'_>) {
        let bounds = self.bounds(ctx);
        let status = format!(" {} | Line {} ",
            ctx.mode_state.edit_mode,
            ctx.cursor.line + 1
        );

        buffer.set_string(
            bounds.x,
            bounds.y,
            &status,
            Style::default().bg(Color::DarkGray)
        );
    }
}
```

2. **Register the component**:

```rust
fn build(&self, ctx: &mut PluginContext) {
    ctx.register_component(Box::new(MyStatusLine));
}
```

### Best Practices

- Keep rendering efficient (called every frame)
- Respect terminal width
- Use consistent styling
- Cache computed values when possible

## Choosing the Right System

### Decision Flow

1. **Is it temporary/modal?**
   - Yes → Use **Overlay**
   - No → Continue

2. **Is it part of the window layout?**
   - Yes → Use **Window Provider**
   - No → Continue

3. **Is it fixed position (top/bottom)?**
   - Yes → Use **UIComponent::render_to_frame()**
   - No → Reconsider your design

### Common Mistakes

1. **Using Overlay for persistent UI**
   - Overlays are for temporary popups only
   - Use WindowProvider for persistent panels

2. **Using WindowProvider for status line**
   - Status lines should use UIComponent::render_to_frame()
   - WindowProvider is for layout-integrated panels

3. **Using UIComponent for popups**
   - Popups should use Overlay system
   - UIComponent is for fixed chrome elements

## Performance Considerations

### Overlays

- Only rendered when visible
- Composited on top of main screen
- Keep rendering logic simple

### Window Providers

- Rendered every frame when visible
- Part of main rendering pipeline
- Cache complex calculations

### UIComponent

- Rendered every frame
- Keep rendering very efficient
- Avoid allocations in render path

## Z-Order Constants

Available z-order constants (from `reovim_core::screen::z_order`):

- `BASE` (0) - Base layer for main content
- `WINDOW` (10) - Window content
- `STATUS_LINE` (50) - Status line elements
- `OVERLAY` (100) - Overlay popups
- `COMPLETION` (110) - Completion menu (above other overlays)
- `COMMAND_LINE` (120) - Command line (topmost)

Higher values render on top of lower values.

## Examples by Use Case

### File Explorer

**System**: WindowProvider
**Reason**: Persistent panel, part of layout

```rust
impl WindowProvider for ExplorerState {
    fn id(&self) -> ComponentId { EXPLORER_ID }
    fn name(&self) -> &str { "Explorer" }
    // ... render implementation
}
```

### Fuzzy Finder

**System**: Overlay
**Reason**: Temporary popup, dismissible

```rust
impl OverlayRenderer for TelescopePicker {
    fn z_order(&self) -> u8 { z_order::OVERLAY }
    fn bounds(&self, ctx: &RenderContext<'_>) -> OverlayBounds {
        OverlayBounds::Centered { width: 80, height: 20 }
    }
    // ... render implementation
}
```

### Status Line

**System**: UIComponent::render_to_frame()
**Reason**: Fixed position, always visible

```rust
impl UIComponent for StatusLine {
    fn bounds(&self, ctx: &RenderContext<'_>) -> LayerBounds {
        LayerBounds {
            x: 0,
            y: ctx.screen_height - 1,
            width: ctx.screen_width,
            height: 1,
        }
    }

    fn render_to_frame(&self, buffer: &mut FrameBuffer, ctx: &RenderContext<'_>) {
        // Render status info
    }
}
```

## Migration Guide

If you're updating existing code:

1. **Identify current pattern** - Which system are you using?
2. **Verify it's correct** - Does it match the decision matrix?
3. **Migrate if needed** - Follow implementation guide above
4. **Test thoroughly** - Verify rendering and interactions

## Further Reading

- [Plugin System Guide](./plugin-system.md) - General plugin development
- [Architecture](./architecture.md) - Overall system architecture
- [UIComponent Trait](./plugin-system.md#uicomponent-trait) - Input handling with UIComponent
