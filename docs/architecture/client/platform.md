# Platform Contracts

Defined in CLIENT DRIVER. Implemented by PLATFORM ADAPTER.

## PlatformCapabilities

Declared at startup. Updated via `on_capabilities_changed()` when they change
at runtime (resize, focus, dark mode toggle, device rotation).

```rust
enum RenderingModel {
    CellGrid,     // TUI: fixed character cells
    Canvas,       // Web: pixel-addressable 2D surface
    NativeLayout, // Mobile: platform UI framework
}

trait PlatformCapabilities: Send + Sync {
    // --- Rendering ---
    fn rendering_model(&self) -> RenderingModel;
    fn grid_size(&self) -> Option<(u16, u16)>;  // None for NativeLayout
    fn color_depth(&self) -> ColorDepth;  // see 07-types.md for ColorDepth enum
    fn pixel_size(&self) -> Option<(u32, u32)>;
    fn reliable_unicode_width(&self) -> bool;
    fn dark_mode(&self) -> bool;

    // --- Input ---
    fn smooth_scroll(&self) -> bool;
    fn pointer_events(&self) -> bool;
    fn touch_input(&self) -> bool;
    fn haptic(&self) -> bool;

    // --- Layout ---
    fn safe_area(&self) -> Insets;
    fn has_focus(&self) -> bool;

    // --- System ---
    fn clipboard_available(&self) -> bool;
    fn screen_reader_active(&self) -> bool;
}
```

### Runtime capability changes

Capabilities can change at runtime. When the platform adapter detects a change,
CORE calls `on_capabilities_changed(caps)` on all modules. This is the SINGLE
event for all capability changes — there is no separate `on_resize`.

Examples of runtime changes:
- Terminal resize: `grid_size()` changes
- Focus/blur: `has_focus()` changes
- Dark mode toggle: `dark_mode()` changes
- Device rotation on mobile: `safe_area()` and `grid_size()` change
- Bluetooth keyboard disconnect: `touch_input()` may change

Modules that went dormant at init (capability not available) can activate
when `on_capabilities_changed` delivers a newly available capability.

## RenderSurface

Abstract output target. Modules write styled text at logical positions.
How that maps to physical output is the platform adapter's concern.

```rust
trait RenderSurface: Send {
    /// Write styled text at a logical position.
    /// Returns the number of columns consumed (needed for positioning
    /// subsequent content on the same line).
    fn write_styled(&mut self, x: u16, y: u16, text: &str, style: Style) -> u16;

    /// Apply a style to an existing cell without changing its character.
    /// Used for selection highlighting, cursor overlay, search highlights.
    fn apply_style(&mut self, x: u16, y: u16, style: Style);

    /// Overlay only the background color on an existing cell.
    fn overlay_bg(&mut self, x: u16, y: u16, bg: Color);

    /// Fill a rectangular region with a styled character.
    fn fill(&mut self, rect: Rect, ch: char, style: Style);

    /// Clear a rectangular region.
    fn clear(&mut self, rect: Rect);

    /// Logical dimensions.
    fn size(&self) -> (u16, u16);
}
```

### Offset-based rendering

CORE passes absolute `Rect` bounds to each module. The module renders using
absolute coordinates within its allocated bounds. No sub-surface, no borrow
conflicts, no lifetime complexity.

```rust
// CORE passes bounds to module:
module.chrome_render(&mut surface, bounds, &caps);

// Module writes within its bounds:
fn chrome_render(&self, surface: &mut dyn RenderSurface, bounds: Rect, ...) {
    surface.write_styled(bounds.x, bounds.y, "content", style);
}
```

Bounds are not enforced by the surface — a module CAN write outside its rect.
This is a design-time covenant, consistent with every immediate-mode GUI.
See [Gaps](gaps.md) for discussion.

### Scope of abstraction

`RenderSurface` covers interactive rendering: TUI, Canvas, Native.

Document export (PDF, HTML, Markdown) is a different concern with a different
model (semantic structure, not positioned text). A future `DocumentRenderer`
trait is the right answer for export. `RenderSurface` does not try to be both.

## InputSource

Channel-based, not synchronous poll. Matches the existing TUI architecture
(tokio async event stream) and works naturally for Web (JS callback sends to
channel) and Mobile (native callback posts to channel).

```rust
/// Platform-agnostic input event.
enum PlatformEvent {
    Key(KeyEvent),
    Pointer(PointerEvent),
    Touch(TouchEvent),
    Focus(FocusEvent),
    Paste(String),
}
```

`InputSource` is NOT a trait and NOT defined in CLIENT DRIVER. It lives in
CLIENT CORE as an implementation detail — modules never touch input.

CORE receives input via `tokio::sync::mpsc::Receiver<PlatformEvent>`. The
platform adapter creates the channel pair at startup and hands the receiver
to CORE:

```rust
// In platform binary main.rs:
let (input_tx, input_rx) = tokio::sync::mpsc::channel::<PlatformEvent>(64);

// Platform adapter sends events:
input_tx.send(PlatformEvent::Key(key)).await;

// CORE receives and forwards to server:
while let Some(event) = input_rx.recv().await {
    match event {
        PlatformEvent::Key(k) => server.send_keys(&[k]),
        // ... other event types
    }
}
```

This keeps the tokio dependency in CORE (not DRIVER), preserving CLIENT
DRIVER's platform-agnostic status. For WASM, the platform binary would use
a WASM-compatible channel with the same send/recv semantics.

Modules do NOT intercept input. The server decides what keys mean.

## ThemeProvider

Color scheme and highlight group provider. Modules get theme at init
and via `on_theme_changed()`. Never hardcode colors.

```rust
trait ThemeProvider: Send + Sync {
    /// Get style for a named highlight group.
    /// Falls back through link chain, then to foreground().
    fn highlight(&self, group: &str) -> Style;

    /// Try multiple groups in order, return first match.
    fn highlight_with_fallback(&self, groups: &[&str]) -> Style;

    fn foreground(&self) -> Style;
    fn background(&self) -> Style;
    fn is_dark(&self) -> bool;
}
```

`ThemeProvider::is_dark()` is the resolved editor theme darkness.
`PlatformCapabilities::dark_mode()` is the OS signal. The theme module reads
`dark_mode()` as a hint to select its initial theme, but the user can override.
Modules should check `ThemeProvider::is_dark()`, not `PlatformCapabilities::dark_mode()`.
