# Rendering

## CORE Compositor Flow

```
Render tick
    |
    v
CORE compositor:
    |
    1. Query LayoutPolicy for window positions:
    |      layouts = policy.layout(screen_rect, &window_ids, focused_window)
    |      (CORE maintains the WindowId list, WindowId->BufferId mapping,
    |       and the focused window — determined from server state)
    |
    2. Allocate chrome regions (global, not per-window):
    |      collect modules where has_chrome() == true (checked every frame)
    |      sort by chrome_priority()
    |      query chrome_requested_size(caps) — modules adapt to platform
    |      respect safe_area() insets
    |      allocate from screen edges inward
    |      remaining rect = total viewport area for windows
    |
    3. For EACH window in layouts:
    |      a. Resolve buffer: buffer = state_cache.get(layout.window_id)
    |      b. Query gutter width:
    |         gutter_width = renderer.gutter_width(&modules, buffer, caps)
    |      c. Compute window viewport = layout.bounds minus gutter_width
    |      d. Delegate to ViewportRenderer:
    |         renderer.render_viewport(surface, viewport, &modules, buffer, theme, caps)
    |         (ViewportRenderer handles everything: gutter, folds, transforms,
    |          tokens, decorations — CORE does not touch any of these)
    |
    4. Render chrome: module.chrome_render(surface, bounds, caps)
    |
    5. Render Overlay chrome (z-ordered by chrome_z_order())
    |
    6. Apply cursor_position overrides (focused window only)
    |
    7. Platform adapter flushes surface
```

CORE never touches buffer-specific concepts. The grep test passes.

### Render scheduling

CORE uses **event-driven rendering with coalescing**:

- A render is scheduled when: a server event arrives, `tick()` returns true,
  or `on_capabilities_changed` fires.
- Multiple events within the same frame are coalesced — CORE processes all
  pending events, THEN renders once.
- A minimum frame interval (e.g., 16ms / ~60fps) prevents runaway rendering
  during rapid event bursts (typing, scrolling).
- When idle (no events, no tick changes), CORE does NOT render. Zero CPU at rest.

## LayoutPolicy

```rust
/// Window arrangement. Defined in CLIENT DRIVER.
trait LayoutPolicy: Send + Sync {
    /// Assign bounds to each window. CORE provides the focused window ID
    /// so the policy can give it preferential sizing (e.g., larger in tiling).
    /// CORE decides focus (from server state). The policy only arranges.
    fn layout(
        &self,
        viewport: Rect,
        windows: &[WindowId],
        focused: WindowId,
    ) -> Vec<WindowLayout>;
}

struct WindowLayout {
    pub window_id: WindowId,
    pub bounds: Rect,
}
```

A default `SingleWindowLayout` fills the entire viewport with one window.
Modules can provide alternative implementations (tiling, floating).

## ViewportRenderer

Buffer rendering pipeline. Lives in CLIENT DRIVER with a default implementation.
CORE delegates the entire buffer viewport to this trait.

```rust
trait ViewportRenderer: Send + Sync {
    /// Compute total gutter width from annotation modules.
    /// Called by CORE before chrome allocation.
    fn gutter_width(
        &self,
        modules: &[Box<dyn ClientModule>],
        buffer: &BufferContent,
        caps: &dyn PlatformCapabilities,
    ) -> u16;

    /// Render buffer content + gutter into the viewport.
    /// Receives ALL modules — filters internally by has_buffer_contrib()
    /// and has_annotations(). No per-frame allocation from CORE.
    fn render_viewport(
        &self,
        surface: &mut dyn ChromeSurface,
        viewport: Rect,
        modules: &[Box<dyn ClientModule>],
        buffer: &BufferContent,
        theme: &dyn ThemeProvider,
        caps: &dyn PlatformCapabilities,
    );
}
```

### Default implementation pipeline

The default `ViewportRenderer` orchestrates:

```
1. Compute gutter:
   - Collect modules with has_annotations() == true
   - Sort by annotation_priority()
   - Query annotation_column_width(ctx, caps) for total width

2. For each visible buffer line:
   a. Gutter: call annotate(line, ctx) on each annotation module, compose
   b. Folds: query fold_ranges() from all buffer contrib modules — skip folded
   c. Virtual lines: query virtual_lines() — insert between real lines
   d. Transform: query transform_line() — apply line transforms
   e. Tokens: call classify_token(category) — priority-ordered, first-match-wins
   f. Decorations: apply inline_decorations(line)
   g. Write results to surface

3. Render cursors (apply map_cursor_column)
```

### Token classify merge strategy

When multiple modules with `has_buffer_contrib()` both classify the same token:

**Priority-ordered, first-match-wins.**

ViewportRenderer sorts buffer contrib modules by `buffer_contrib_priority()`
(highest first). For each token, it calls `classify_token()` on each module
in priority order. The first `Some(behavior)` wins. Remaining modules are skipped.

This means a syntax-highlighting module (high priority) takes precedence over
a bracket-pair module (lower priority) for the same token category.

**Tie-breaking**: When two modules have the same priority value, registration
order wins (first registered = higher effective priority). This applies to
all priority-ordered operations: `chrome_priority()`, `buffer_contrib_priority()`,
`annotation_priority()`, and `chrome_z_order()`.

## Chrome Region Model

CORE does not know about "statuslines" or "gutters." It knows about **chrome** —
rectangular areas that modules claim and render into.

```rust
enum ChromePosition {
    Top,        // e.g., tabline, breadcrumb, sticky context
    Bottom,     // e.g., statusline, command line
    Left,       // e.g., file explorer sidebar (NOT gutter)
    Right,      // e.g., minimap, scrollbar
    Overlay,    // e.g., popups, completion menus, floating windows
}
```

CORE never asks "what kind of chrome are you?" It asks "how big?" and "where?"

### Chrome overflow

When the sum of all chrome requests exceeds the screen:
- Allocate in priority order (highest `chrome_priority()` first)
- Once remaining space reaches a minimum viewport size (configurable), stop
- Lower-priority chrome that doesn't fit is not rendered this frame
- Overlay chrome is always rendered (it overlaps, doesn't consume space)

## The Gutter: Above Chrome

The gutter is NOT chrome. It is part of the buffer viewport, managed by
`ViewportRenderer`. It composes multiple annotation sources into a multi-column
area that renders per-line alongside the buffer content.

```
                    ┌──────────────────────────────────┐
                    │        Gutter (inside             │
                    │        ViewportRenderer)          │
                    │                                   │
                    │  Composes annotation methods from │
                    │  modules with has_annotations()   │
                    └──────────┬───────────────────────┘
                               │
              ┌────────────────┼────────────────┐
              │                │                │
     ┌────────┴─────┐  ┌──────┴──────┐  ┌──────┴──────┐
     │ line-numbers  │  │  git-signs  │  │ diagnostics │
     │   MODULE      │  │   MODULE    │  │   MODULE    │
     └──────────────┘  └─────────────┘  └─────────────┘
```

CORE only knows: "gutter_width() returned N. Subtract N from viewport."
