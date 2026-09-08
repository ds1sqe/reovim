# Buffer provider proposal

Historical design deferred from #211 because virtual buffers needed a
dedicated ownership and lifecycle decision. No option was selected in this
note. The [heritage index](README.md) explains how to read the full original.

## Problem

Plugins generated directory listings, diagnostics, terminal content, help,
and search results through `PluginBufferProvider` in
`lib/core/src/content/provider.rs`.

The old API combined content production and client interaction:

| Method | Responsibility |
| --- | --- |
| `get_lines` | Return `Vec<String>` synchronously on every render. |
| `on_cursor_move` | Respond to a text position change. |
| `on_input` | Handle a key; return handled, unhandled, or close requested. |
| `is_editable` | Declare whether the virtual buffer accepts edits. |

`BufferContext` carried the buffer ID, viewport width/height, and plugin state.
`WindowContentSource` selected a normal `BufferId` or a plugin ID with an
`Arc<dyn PluginBufferProvider>`. The explorer was a concrete consumer.

## Unresolved choices

| Concern | Question left open |
| --- | --- |
| Loading | Async streaming or cached results for expensive/network content? |
| Lifecycle | How do virtual buffers join `:ls`, windows, persistence, and undo? |
| Input | Route through keymaps, plugin handlers, or both? |
| Memory | How are large results paged, bounded, and reclaimed? |

Three placements were considered:

- Display driver: define provider/source contracts beside rendering, with
  implementations in `modules/plugins/buffer_providers/`.
- Kernel extension: add virtual-buffer primitives under `kernel/block/` and
  let display consume them.
- Plugin subsystem: put provider contracts, registration, and lifecycle in
  a dedicated `lib/plugins/`.

The useful unresolved problem is the coupling: synchronous content generation
ran on the render path, while one interface mixed storage, viewport dimensions,
and key handling. The alternatives above recorded possible owners without
settling that boundary.

For current ownership, read
[resources and state](../interfaces/resources-and-state.md)
and [clients](../interfaces/clients.md).
