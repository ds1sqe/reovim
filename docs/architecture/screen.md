# Screen & Rendering

Terminal output management and rendering pipeline.

## Module Location

```
lib/core/src/
├── screen/         # Terminal rendering
│   ├── mod.rs      # Screen struct, z-order constants
│   ├── window.rs   # Window rendering
│   ├── border.rs   # Border styles and rendering
│   ├── cusor.rs    # Cursor rendering
│   ├── layout.rs   # Screen layout management
│   ├── split.rs    # Split window management
│   └── tab.rs      # Tab management
├── frame/          # Frame buffer for diff-based rendering
│   ├── mod.rs      # Public API exports
│   ├── buffer.rs   # FrameBuffer - 2D cell grid
│   ├── cell.rs     # Cell - char + fg + bg + modifiers
│   └── renderer.rs # FrameRenderer - double-buffer diff rendering
├── overlay/        # Overlay compositing system
│   ├── mod.rs      # Overlay trait definition
│   ├── compositor.rs # OverlayCompositor - z-order management
│   ├── geometry.rs # OverlayBounds, positioning helpers
│   ├── render.rs   # OverlayRender trait
│   └── selectable.rs # Scrollable, Selectable traits
```

## Screen & Window

```rust
pub struct Screen {
    size: ScreenSize,
    out_stream: Box<dyn Write>,
    windows: Vec<Window>,
}

pub struct Window {
    pub anchor: Anchor,           // Position on screen
    pub buffer_id: usize,
    pub buffer_anchor: Anchor,    // Scroll position
    pub line_number: LineNumber,
}
```

## Frame Buffer System

The frame buffer provides diff-based rendering to eliminate terminal flickering using a double-buffer architecture:

```rust
pub struct FrameBuffer {
    cells: Vec<Cell>,
    width: u16,
    height: u16,
}

pub struct Cell {
    pub char: char,
    pub style: Style,       // fg, bg, attributes
    pub width: u8,          // 1 for ASCII, 2 for wide chars
}

pub struct FrameRenderer {
    front: FrameBuffer,     // Latest complete frame (external readers)
    back: FrameBuffer,      // Currently being rendered to
    capture: Option<Arc<RwLock<FrameBuffer>>>,  // For RPC CellGrid
}
```

### Components

- `FrameBuffer` - 2D grid of cells with `get()`/`set()` accessors
- `Cell` - Individual terminal cell (char, style, display width)
- `FrameRenderer` - Double-buffer renderer with cell-by-cell diff
- `FrameBufferHandle` - Thread-safe handle for external readers (RPC)

### Rendering Flow

1. Content is rendered to `back` buffer via `buffer_mut()`
2. `flush()` computes diff between `back` and `front`
3. Only changed cells are written to terminal
4. Buffers swap: `front ↔ back`
5. Capture buffer (if enabled) is updated for RPC clients

### Frame Buffer Capture

For RPC `CellGrid` format, the renderer can provide a thread-safe capture handle:

```rust
let handle = renderer.enable_capture();  // Returns FrameBufferHandle
let snapshot = handle.snapshot();        // Clone of current frame
```

## Layer System

Layers provide structured z-order rendering:

```rust
pub trait Layer {
    fn render_to_buffer(&self, buffer: &mut FrameBuffer, theme: &Theme, color_mode: ColorMode);
    fn z_order(&self) -> u8;
}
```

### Z-Order Constants

Core defines only base z-order constants. Plugins define their own via `OverlayRenderer::z_order()`:

| Layer | Z-Order | Source |
|-------|---------|--------|
| BaseLayer | 0 | Core (`z_order::BASE`) |
| EditorLayer | 2 | Core (`z_order::EDITOR`) |
| Plugin overlays | 100-400 | `OverlayRenderer::z_order()` |

Plugins register overlays with their own z-orders:
- Range-Finder (jump labels): 110
- Completion: 200
- Microscope: 300
- Settings: 400

`Screen::render_buffered()` renders all layers in z-order to the frame buffer.

## Overlay System

Overlays are composable popup components:

```rust
pub trait Overlay {
    fn render_to_buffer(&self, buffer: &mut FrameBuffer, theme: &Theme);
    fn bounds(&self) -> OverlayBounds;
    fn z_order(&self) -> u16;
}
```

### Components

- `OverlayCompositor` - Manages overlay stack, renders in z-order
- `OverlayBounds` - Position and size (x, y, width, height)
- `OverlayGeometry` - Helpers for centered/anchored positioning

## Complete Rendering Flow

```
Runtime::render()
    │
    ▼
Screen::render_buffered()
    │
    ├── FrameRenderer::buffer_mut() ──► get back buffer
    │
    ├── Render core layers:
    │   ├── Base layer (tab line, status line)    (z=0)
    │   └── Editor windows                        (z=2)
    │
    ├── Render plugin overlays (via OverlayRegistry):
    │   └── For each visible overlay in z-order:
    │       overlay.render_to_buffer(buffer, theme)
    │       (z-order defined by plugin's OverlayRenderer::z_order())
    │
    └── FrameRenderer::flush()
        │
        ├── Diff: back vs front (cell-by-cell)
        ├── Generate ANSI codes for changed cells only
        ├── Swap buffers: front ↔ back
        └── Update capture buffer (for RPC clients)
        │
        ▼
    Terminal Output (minimal I/O)
```

## Border System

Configurable window borders for visual separation.

### Components

- `BorderStyle` - Predefined styles (None, Single, Double, Rounded, Heavy, Custom)
- `BorderSides` - Selective borders (top, bottom, left, right)
- `BorderConfig` - Builder pattern for border configuration
- `BorderMode` - Layout behavior (Collide for shared borders, Float for individual)

### Border Styles

| Style | Characters |
|-------|------------|
| Single | `─│┌┐└┘` |
| Double | `═║╔╗╚╝` |
| Rounded | `─│╭╮╰╯` |
| Heavy | `━┃┏┓┗┛` |

### Usage

```rust
let config = BorderConfig::new()
    .style(BorderStyle::Rounded)
    .sides(BorderSides::all())
    .mode(BorderMode::Collide);
```

## Related Documentation

- [Rendering System](../rendering/overview.md) - Full render pipeline
- [Runtime](./runtime.md) - Screen ownership
- [Buffer](./buffer.md) - Content source
