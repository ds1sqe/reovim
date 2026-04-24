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

### Rasterization seam (17-γ)

After chrome modules have drawn into their logical surface, the TUI
shell picks a rasterization mode per window via `WindowViewHints` in
`TuiCoreState`. `ViewHint::FullBlock` (1 logical cell → 1 terminal
cell, identity) runs on the existing direct-write path;
`ViewHint::HalfBlock` (2 stacked logical cells → 1 terminal cell via
`▀`, live since Flight 74) buffers chrome into a `CellCapability`
twice the terminal height and then projects it onto the backend via
`HalfBlockRasterizer`; `ViewHint::Braille` (2×4 logical sub-grid → 1
terminal cell via `⠀…⣿`, live since Flight 75) buffers chrome into a
`CellCapability` twice as wide and four times as tall as the terminal
and projects via `BrailleRasterizer` (monochrome per glyph). The
`ViewRasterizer` trait and the narrow terminal-backend-level
`RasterOutput` trait live in
`ext/client/tui/capabilities/cell-view/`.  `RasterOutput` is
deliberately separate from the module-facing `ChromeSurface`: input
(where chrome modules write) and output (where rasterized terminal
cells land) are different sides of the seam, and conflating them
would make HalfBlock's fg=top / bg=bottom encoding unexpressible.

**Chrome capabilities doubling invariant.** When a non-FullBlock
window dispatches chrome, the `TuiPlatformCapabilities` handed to each
module reports the *logical* grid size — not the physical terminal
size. The factor depends on the hint:

| Hint | Logical grid size | Anchor for bottom-docked chrome |
|------|-------------------|--------------------------------|
| `FullBlock` | `(width, height)` | terminal row `height − 1` |
| `HalfBlock` | `(width, height × 2)` | logical row `height × 2 − 1`, rasterizes to terminal row `height − 1` |
| `Braille` | `(width × 2, height × 4)` | logical row `height × 4 − 1`, rasterizes to terminal row `height − 1` |

Modules that compute their Rect from `caps.grid_size()` MUST NOT
assume the reported size equals the physical terminal size. The
rasterizer maps the logical grid back onto the terminal; the invariant
is that bottom-docked chrome always lands at the terminal's bottom row
regardless of hint.

**Enablement.** Set `REOVIM_VIEW_HINT=halfblock` (also accepted:
`full`, `fullblock`, `half`, `half_block`, `half-block`, `braille`)
before launching the TUI.  The value is parsed by
`reovim_client_tui::view_hint_env::parse_view_hint` and stored in
`TuiCoreState::default_view_hint`; windows with no explicit hint fall
through to it.

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
