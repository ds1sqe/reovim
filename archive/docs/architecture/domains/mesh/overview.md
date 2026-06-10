# Mesh domain (scaffold)

Flight 77 of the #753 Codec Foundation Continuation chain adds the
mesh domain as a **scaffold** — a second concrete implementation of
the closed `reovim_domain::Domain` trait, sitting beside the
pre-existing text domain. Its job is to prove that
`render-codec` / `content-codec` / `Domain` are *domain-neutral*:
new domains land in `ext/server/` without requiring any edit to the
kernel, the server or client subsys crates, or the render pipeline.

## Crates

| Crate | Path | Role |
|---|---|---|
| `reovim-domain-mesh` | `ext/server/domain/mesh/` | `Mesh` marker + `impl Domain` + placeholder types |
| `reovim-provider-mesh` | `ext/server/providers/mesh/` | `MeshStore` stub: holds content, applies `MeshEdit` |

`reovim-domain-mesh` depends on `reovim-domain` only.
`reovim-provider-mesh` depends on `reovim-domain-mesh` only. No
kernel, subsys, or third-party deps participate.

## Types (locked for the scaffold)

```rust
pub struct Vertex { pub x: f32, pub y: f32, pub z: f32 }
pub struct MeshPosition { pub vertex: u32 }
pub enum   MeshEdit { Replace(Vec<Vertex>) }
pub type   MeshContent = Vec<Vertex>;

impl Domain for Mesh {
    type Position = MeshPosition;
    type Edit     = MeshEdit;
    type Content  = MeshContent;
}
```

The single `MeshEdit::Replace` operation is the narrowest expressive
surface that satisfies the `Domain` trait bounds. Flight 77 does
**not** commit to a topology representation (edges, faces,
half-edges, BVH) and does not promise that `MeshPosition` survives
structural edits — it doesn't, because `Replace` invalidates every
prior position by construction.

## Domain-neutrality invariant

The test at
`ext/server/domain/mesh/tests/domain_neutrality.rs` instantiates a
single generic function against both `Text` and `Mesh` and asserts
that the trait resolves to distinct associated types for each:

```rust
fn describe<D: Domain>() -> (&'static str, &'static str, &'static str) {
    (
        core::any::type_name::<D::Position>(),
        core::any::type_name::<D::Edit>(),
        core::any::type_name::<D::Content>(),
    )
}
```

The function body knows nothing about text or mesh. If it compiles
and passes for both instantiations, the `Domain` substrate is
proven domain-neutral.

## Explicit deferrals

The following are all explicitly out of scope and belong to a
future flight or a separate plan chain:

- Mesh topology (edges, faces, half-edges, BVH, subdivision).
- Per-vertex / per-face edit operations.
- Mesh file-format codecs (OBJ, glTF, STL).
- A client-side mesh rasterizer or a new `ViewHint` variant.
- gRPC / session integration; mesh storage cannot be selected via
  server config in Flight 77.

## References

- Sub-plan: `~/docs/plans/reovim/753-phase-a-codec-foundation-continuation/04-mesh-domain-scaffold.md`
- Master plan: `~/docs/plans/reovim/753-phase-a-codec-foundation-continuation/00-master-plan.md`
- Flight 77 commit lands at `/rendezvous`.
- Related: `docs/architecture/domains/text/overview.md` (companion
  domain).
