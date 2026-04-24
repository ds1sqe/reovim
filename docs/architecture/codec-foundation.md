# Codec Foundation — chain-level architecture reference

This document ties together the five flights of the
`753-phase-a-codec-foundation-continuation` chain (Flights 74–78
under epic #753) into one reference. Each flight shipped as its own
commit with its own crate-level doc-comments; this file is the
cross-flight story.

## Overview

The `render-codec` substrate is the client-side pipeline that
decodes render-payload byte slices into capability slots on a
`FrameTarget` and then projects those capabilities onto a concrete
surface (terminal, SVG document, etc.). The pipeline is
**surface-neutral** — the same `FrameTarget` shape is consumed by
`FullBlockRasterizer` on the TUI, `HalfBlockRasterizer` and
`BrailleRasterizer` for denser TUI modes, and a `WebFrame` +
`render_frame_svg` on the web platform.

Alongside, the `Domain` trait (`lib/domain/`) defines the closed
mechanism for content-type abstraction (`Position` / `Edit` /
`Content` associated types). The substrate is **domain-neutral**
— `Text` and `Mesh` are both concrete `Domain` impls sitting in
`ext/server/domain/`, and neither requires a kernel, subsys, or
render-pipeline edit to land.

The five-flight chain proves both claims simultaneously.

## Client-side surface breadth

| Variant | Rasterizer / renderer | Scaling | Flight | Commit |
|---|---|---|---|---|
| `ViewHint::FullBlock` | `FullBlockRasterizer` | 1:1 identity | 73 (predecessor chain) | pre-`887e0ae4` |
| `ViewHint::HalfBlock` | `HalfBlockRasterizer` | 1 terminal cell = 2 stacked logical cells (`▀` / `▄` / space) | 74 | `887e0ae4` |
| `ViewHint::Braille` | `BrailleRasterizer` | 1 terminal cell = 2×4 logical sub-grid (`⠀`…`⣿`) | 75 | `d379a0e3` |
| Web SSR (spike) | `WebFrame` + `render_frame_svg` | `(width × 1, height × 1)` canonical frame → inline SVG | 76 | `7129c8cc` |

See `docs/architecture/client/rendering.md` for the full
rasterization seam, the chrome-capabilities doubling invariant
(HalfBlock's `(width, height × 2)` and Braille's
`(width × 2, height × 4)`), and the `REOVIM_VIEW_HINT` env-var
enablement.

## Server-side domain breadth

| Domain | `Position` | `Edit` | `Content` | Provider | Flight | Commit |
|---|---|---|---|---|---|---|
| `Text` (pre-existing) | `TextPosition { line, col }` | `TextEdit` (range + string) | `String` | `reovim-provider-text` | — | — |
| `Mesh` (new) | `MeshPosition { vertex: u32 }` | `MeshEdit::Replace(Vec<Vertex>)` | `Vec<Vertex>` | `reovim-provider-mesh` (stub) | 77 | `bf84303b` |

See `docs/architecture/domains/mesh/overview.md` for the mesh
scaffold's explicit deferrals (no topology, no per-vertex edits,
no file formats, no rasterizer).

## Three-tier client codec model

```
uapi/content-codec/             — CLOSED envelope + traits +
                                  DecodedEdit + inode/mount primitives
clients/lib/subsys/codec/       — CONTRACT + registry
                                  (FrameTarget, RenderHandler,
                                   SurfaceEncoder)
ext/client/**/capabilities/*    — per-capability impls (cell, cell-view)
ext/client/platforms/<p>/       — per-surface impls (TUI, Web)
```

The middle tier is shape-blind: it knows `FrameTarget` as a typeid-
keyed bag of capability slots, nothing more. Concrete capability
types (`CellCapability`, `WebFrame`) live on the ext tier and are
never re-exported through subsys. The authoritative wording is in
the crate-level doc-comments at
`clients/lib/subsys/codec/src/lib.rs`.

## Three-tier domain model

```
lib/domain/                     — CLOSED Domain trait (zero deps)
ext/server/domain/<d>/          — per-domain marker + assoc types
ext/server/providers/<d>/       — per-domain storage + edit application
```

The `Text` domain has a substantial provider (`Rope`, `VirtualBuffer`,
motion engines) because the text-editing feature surface is mature.
The `Mesh` domain is a scaffold — provider holds a `Vec<Vertex>` and
applies `MeshEdit::Replace` — deliberately narrow to prove the
pattern works for a non-text shape.

## Zero-edit invariant

None of the five flights required a kernel, subsys, or render-
pipeline edit. Four preexisting depgraph probes enforce this as a
compile-time property of the workspace:

- `lib/depgraph/tests/kernel_no_domain.rs` — kernel sees only
  `reovim-domain`, never a concrete domain crate.
- `lib/depgraph/tests/server_no_domain.rs` — the general-purpose
  prefix-match probe covering any `reovim-domain-*` crate (text,
  mesh, and any future domain) against any server crate that is
  not itself a domain.
- `lib/depgraph/tests/server_no_domain_text.rs` — text-specific
  sibling; the mesh sibling is carried by the generic probe above.
- `lib/depgraph/tests/subsys_no_domain.rs` — both client and
  server subsys tiers stay domain-blind.

Four flight-specific shape-lock probes complement these:

- `cell_view_half_block_exists.rs` — locks the HalfBlock surface
  and the `cell-view → reovim-driver-display` dep.
- `cell_view_braille_exists.rs` — locks the Braille surface.
- `platforms_web_exists.rs` — locks the web platform crate; asserts
  no `capabilities/*` or `driver/*` dep.
- `domain_mesh_exists.rs` — locks the mesh domain + provider shape.

## What this chain did not build

The five-flight chain is deliberately narrow. All of the following
are explicit deferrals and belong to future flights or separate
plan chains:

1. gRPC / WebSocket streaming for the web surface.
2. TypeScript-client integration with `reovim-web` (the existing
   `clients/web/` TS pipeline continues to render via its own
   pipeline, untouched by this chain).
3. Background rectangles, styled underlines, bold / italic on the
   web surface.
4. `WebFrame` ↔ `CellCapability` unification — `ext/client/platforms/*`
   cannot import `ext/client/capabilities/*` per the locked CLM v7
   edges, so the web platform carries its own ≤60-LOC cell-grid
   primitives. Absorption deferred to Phase E.1 of #753, where
   client-side rendering becomes platform-owned.
5. Mesh topology (edges, faces, half-edges, BVH, subdivision).
6. Per-vertex / per-face mesh edit operations.
7. Mesh file-format codecs (OBJ, glTF, STL).
8. Client-side mesh rasterizer or a `ViewHint::Mesh` variant.
9. gRPC / session integration for the mesh domain (mesh storage
   cannot be selected via server config from this chain alone).
10. `/e2e` live-server smokes across any of Flights 74–78 —
    integration tests plus snapshot tests were the feature gate
    throughout.

## References

- Master plan:
  `~/docs/plans/reovim/753-phase-a-codec-foundation-continuation/00-master-plan.md`
- Per-flight sub-plans:
  `01-halfblock-rasterizer.md` (74),
  `02-braille-rasterizer.md` (75),
  `03-web-pipe-up.md` (76),
  `04-mesh-domain-scaffold.md` (77),
  `05-docs.md` (78).
- Per-flight landing commits: `887e0ae4`, `d379a0e3`, `7129c8cc`,
  `bf84303b`.
- Companion docs:
  - `docs/architecture/client/rendering.md` — rasterization seam,
    per-hint caps table, Web SSR subsection.
  - `docs/architecture/domains/mesh/overview.md` — mesh scaffold.
  - `docs/architecture/overview.md` — workspace-level layer map.
- Crate-level canonical wording:
  - `clients/lib/subsys/codec/src/lib.rs`
  - `ext/client/tui/capabilities/cell-view/src/lib.rs`
  - `ext/client/platforms/web/src/lib.rs`
  - `ext/server/domain/mesh/src/lib.rs`
- Parent roadmap: `post-769-roadmap.md` §3 (Wave 2), sibling to
  this repository root (developer-local planning artifact; not
  carried in-tree).
