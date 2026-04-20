# Domain Abstraction Layer

## Overview

The domain abstraction layer decouples content-type concepts (text, and in the
future audio, video, binary) from the shared substrate (byte identity, kernel
lifecycle, codec pipeline). Each content type is a domain. Domains own their
own view-state types; they share only the kernel substrate beneath them.

Design principle:

> Share the substrate (bytes, identity, lifecycle). Do not share the structure.
> Each domain owns its own view state types.

## `Domain` Trait

Crate: `reovim-domain` (`ext/server/domain/domain/`)

Zero dependencies — defines only the trait contract.

```rust
pub trait Domain: Send + Sync + 'static {
    type Position: Send + Sync + Clone + Debug + PartialEq + 'static;
    type Edit:     Send + Sync + Clone + Debug + 'static;
    type Content:  Send + Sync + 'static;
}
```

Three associated types:

| Associated type | Purpose |
|-----------------|---------|
| `Position` | A location within the content domain |
| `Edit` | A domain-level modification |
| `Content` | Decoded domain content |

These types flow through generic codec and provider traits (`Decode<D>`,
`Encode<D>`, `Index<D>`), enabling domain-agnostic infrastructure that never
needs to know the concrete content type.

## Current Domains

### Text (`reovim-domain-text`)

**Path**: `ext/server/domain/text/`

Binds the abstract domain types to concrete text representations:

- `Position` = `TextPosition` (line:column)
- `Edit` = `TextEdit` (insert/delete with text)
- `Content` = `String` (decoded UTF-8)

See `docs/architecture/domains/text/overview.md` for full type inventory,
`TextGeometry`, `LineIndex`, and events.

### Future domains

The architecture anticipates additional domains as the editor expands beyond
text. Each new domain defines its own marker type implementing `Domain` and
lives in `ext/server/domain/<name>/`. No changes to the kernel or codec pipeline
are required — generic traits parameterize over the new `Domain` implementor.

## Design Principle

The domain system enforces a strict ownership boundary:

- The kernel substrate (`BufferId`, byte storage, event bus, lifecycle) is
  shared across all domains.
- View-state types — positions, edits, cursors, selections, undo history —
  belong exclusively to the domain that defines them.
- Two domains never share a `Position` or `Edit` type. A codec that operates on
  text and a codec that operates on audio produce unrelated edit types.

This boundary prevents cross-contamination of domain concepts and keeps each
domain's type system self-contained and auditable.

## Related Documents

- `docs/architecture/domains/text/overview.md` — text domain types and events
- `docs/architecture/event-layers.md` — event taxonomy across all layers
- `docs/architecture/type-layers.md` — type layers and dependency graph
