# 5.3 — Projections

**Scope.** What modules write to be rendered: the projection
contract, projector dispatch, projection slots.

**Heritage.** v3 implicit (split across `04-Substrate` and
`05-View`); v4 README does not detail. Minimum-viable shape here.

**Locked rules.** Carried-from-v3 implicit; reshape pending.

---

## 1. Projection contract

A **projection** is a domain-neutral view of buffer content prepared
by a projector for a given `(client, buffer, window)`. It carries:

- byte-range to position-carrier mapping,
- per-range style/glyph hints,
- per-range cursor / selection markers,
- arbitrary opaque overlays per-projector.

```rust
pub struct Projection {
    pub client_id:   ClientId,
    pub buffer_id:   BufferId,
    pub window_id:   WindowId,
    pub viewport:    PositionCarrier,
    pub spans:       Vec<ProjectionSpan>,
    pub cursors:     Vec<CursorCarrier>,
    pub overlays:    Vec<OverlayBlob>,
}

pub struct ProjectionSpan {
    pub range:  Range<usize>,            // bytes in buffer
    pub style:  StyleRef,
    pub glyph:  GlyphHint,
}
```

## 2. Projector dispatch

`OnProjectorRender` runs after handler dispatch produces a fresh
buffer state. Walking order: projectors registered by Domains in
the focus chain, leaf-to-root, banded.

Projectors output into a kernel-owned `ProjectionBuffer`. The
client-side render driver consumes it (8.3).

## 3. Projection slot ownership

Each projector gets a per-`(client, buffer, window, projector_id)`
projection slot. Lifecycle mirrors view-slot (3.2).

## 4. Cross-Domain composition

When the focus chain has multiple resolved Domains, projectors
compose:

- ranges from inner Domains override outer-Domain ranges in
  overlapping byte regions,
- cursors aggregate across Domains (carrier-equality dedup; CR5),
- overlays append in band order; same-band ordering follows the
  DT17 tie-break (4.1 §6), so composition is deterministic across
  boots and installs.

## 5. Out of v4 target

- Composable projection trees beyond per-Domain banded order.
- Live re-projection during input dispatch (project on apply,
  not on input).

## Open items (must resolve before lock)

1. Whether projection is push (kernel signals client) or pull
   (client requests). v3 was push; default for v4 same.
2. Style/glyph vocabulary — currently `StyleRef` is opaque; concrete
   set in 8.3.
3. Projection diff vs full projection — bandwidth concern. Default:
   diff with full re-send on viewport jump.

## Conformance

This chapter is a sketch. Conformance arrives with the render
driver chapter (8.3).
