# 5.3 — Projections

**Scope.** What modules write to be rendered: the projection
contract, projector dispatch, projection slots.

**Heritage.** Minimum-viable shape here.

**Locked rules.** Carried-forward implicit; reshape pending.

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

`StyleRef` is an opaque numeric style handle in Phase 4. The kernel
does not know color names, terminal attributes, or theme policy. The
client/render subsystem resolves a `StyleRef` to concrete paint
attributes in Phase 7. `GlyphHint` is likewise an opaque hint carried
through the projection; concrete glyph vocabulary is not interpreted by
the kernel.

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

## 5. Out of target

- Composable projection trees beyond per-Domain banded order.
- Live re-projection during input dispatch (project on apply,
  not on input).

## 6. Delivery form

Phase 4 supports full projection resend as the baseline. Projection
diffs are optional and may be emitted only when the server can prove
the client has the immediately preceding projection for the same
`(client, buffer, window)` and no viewport, focus-chain, DomainTable,
or codec-status change occurred between the two projections. Any such
boundary change forces a full projection resend.

## Open items (resolved for v0.16)

1. ~~Push vs pull~~ — resolved: **push** is the default (prior art;
   the kernel signals the client after each input that mutates state).
2. ~~Style/glyph vocabulary~~ — resolved (§1): `StyleRef` and
   `GlyphHint` are opaque through Phase 4; concrete paint vocabulary
   belongs to Phase 7.
3. ~~Projection diff vs full projection~~ — resolved (§6): full
   projection resend is the baseline; diff is optional and only valid
   across adjacent projections with no viewport/focus/DomainTable/
   codec-status boundary change.

## Walking-skeleton subset note (#797)

The walking skeleton realizes a **minimal `Projection` subset**:
one `ProjectionSpan` over the full buffer byte range plus a cursor
marker. Overlays (`Vec<OverlayBlob>`) are present but empty. The §1
struct is the spec target; the skeleton encodes the projection into
the `AttachEvent::Projection` body (`projection: bytes`) through a
domain-neutral byte representation serialized by the runtime (Phase 3).

## Conformance

| Behaviour | Fixture |
|---|---|
| Push default | Input that mutates buffer state emits a projection notification without a client poll. |
| Opaque style | Kernel stores and transmits `StyleRef` byte-identically; no kernel branch interprets color/theme names. |
| Composition | Inner Domain span overrides outer span over the same byte range; overlays append in DT17 order; cursor carriers dedup by CR5 byte equality. |
| Full resend boundary | Viewport/focus/DomainTable/codec-status change forces a full projection resend, not a diff. |
