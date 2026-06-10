# 5.1 — Windows

**Scope.** Window tree, layout nodes, viewport, window-level
attached state.

**Heritage.** v3 `05-View/01-Windows.md`. Flagged in v4 README §0.4
as "not yet chapter-consolidated"; this chapter is a minimum-viable
shape.

**Locked rules.** Carried from v3.

---

## 1. Window

```rust
pub struct Window {
    pub id:        WindowId,
    pub session:   SessionId,
    pub buffer:    BufferId,
    pub layout:    LayoutNodeId,
    pub viewport:  PositionCarrier,    // top-left or scroll anchor
    pub size:      WindowSize,
}
```

`viewport` is a position carrier in the buffer's root Domain
vocabulary. It is not "top byte" or "top line" — those are
text-Domain-specific.

## 2. Layout tree

```rust
pub enum LayoutNode {
    Window(WindowId),
    Split { dir: SplitDir, ratio: f32, left: LayoutNodeId, right: LayoutNodeId },
}

pub enum SplitDir { Horizontal, Vertical }
```

Layout-position path: a stable string like `root/h0/v1` derived
from tree walk. Used as durable identity for persistence (2.4 §4).

## 3. Operations

```c
hostapi_window_open    (SessionId, BufferId, LayoutPath, WindowId* out);
hostapi_window_close   (WindowId);
hostapi_window_split   (WindowId, SplitDir, f32 ratio, WindowId* out_new);
hostapi_window_resize  (WindowId, WindowSize);
hostapi_window_focus   (ClientId, WindowId);
```

## 4. Window-level state

`(ClientId, BufferId, WindowId)` triples key the view-slot map (3.2
§1).

## 5. Bounded resources

| Cap | Field |
|---|---|
| Max windows per session | `max-windows-per-session` |
| Max layout depth | `max-layout-depth` |

## Open items (must resolve before lock)

1. Layout operation set — split/close/resize/balance/focus is a
   minimum. Floats / tabs / popovers TBD.
2. Multi-buffer windows (zellij-style layout) vs strict 1:1 — v4
   default is 1:1; revisit at v1.1.
3. Whether window resize triggers a re-layout event observable by
   modules. Default: yes, via DS12 `window.resize`.

## Conformance

This chapter is a sketch. Layout-related rule fixtures land when
operation set is finalised.
