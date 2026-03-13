# Known Gaps and Future Work

Gaps are not failures. They are honest acknowledgments of what the model
does not yet address. We admire the gaps — acknowledge them, document them,
and defer them to when the need is concrete.

## Acknowledged Gaps

### PlatformCapabilities: font metrics, DPI, IME

**Font metrics**: On Canvas platforms (Web), character width is variable and
font-dependent. Modules rendering aligned columns need `char_width_px(ch) -> f32`.
Not needed for TUI (fixed-width cells). Deferred until Web client is prototyped.

**DPI / scale factor**: High-DPI displays need `device_pixel_ratio() -> f32`.
Not needed for TUI. Deferred until Canvas/Native rendering is prototyped.

**IME composition state**: Mobile and desktop IME composes text before confirming.
A cmdline module should render pre-edit strings differently. Deferred until
Mobile/CJK input support is prioritized.

These will be added to `PlatformCapabilities` when the first non-TUI client
is built. The trait is extensible — new methods with default implementations
do not break existing modules.

### RenderSurface bounds enforcement

Modules CAN write outside their allocated `Rect`. This is a design-time covenant,
not a compile-time or runtime constraint. Consistent with every immediate-mode GUI.

For defense-in-depth, CORE could wrap the surface in a `ClippedSurface` that
clamps coordinates before forwarding. Trivial to implement, near-zero cost.
Not architecturally necessary — a quality-of-life improvement for debugging.

### Hot reload

The server Module trait has `save_state()` / `restore_state()` for hot reload.
The client model has no equivalent. Deferred because:
- The primary extensibility mechanism is Cargo crate + rebuild
- Hot reload is a development convenience, not an architectural requirement
- When needed, `save_state()` and `restore_state()` can be added to
  `ClientModule` with default no-op implementations without breaking changes

### `provides()` / `requires()` capability matching

The server supports abstract dependency matching (module A `provides`
"syntax-highlighting", module B `requires` it). The client model has only
`dependencies()` with explicit module IDs and `optional_dependencies()`.

Deferred because the client module set is smaller and concrete dependencies
are sufficient. When the module ecosystem grows, abstract capability matching
can be added with the same pattern as the server.

### Document export (PDF, HTML, Markdown)

`RenderSurface` covers interactive rendering. Document export needs a semantic
model (`begin_block`, `write_span`, `end_block`) — fundamentally different from
positioned styled text. A future `DocumentRenderer` trait is the right answer.
Modules that support export would implement both traits.

### `api_version()` on ClientModule

The server Module has `api_version()` for runtime compatibility checking.
The client model relies on Cargo semver for compiled-in modules (caught at
compile time) and `REOVIM_TOOLCHAIN_HASH` for dynamic modules. An explicit
`api_version()` is redundant for compiled-in modules but could be useful for
dynamic loading. Deferred until the dynamic loading story matures.

## Migration Strategy

### From current TuiExtension to ClientModule

The existing `TuiExtension` trait has ~20 methods. The mapping is largely 1:1:

| TuiExtension | ClientModule | Notes |
|---|---|---|
| `init()` | `init(ctx) -> ProbeResult` | Gains context param and return type |
| `exit()` | `exit() -> Result<(), ClientModuleError>` | Gains error return |
| `kind()` | `kind()` | Same |
| `server_kinds()` | `server_kinds()` | Same (1:N kind mapping for startup validation) |
| `is_active()` | `has_chrome()/has_buffer_contrib()` | Split by role |
| `render()` | `chrome_render()` | Chrome role |
| `render_with_viewport()` | Buffer contrib methods | Buffer contrib role (classify_token, transform_line, etc) |
| `tick()` | `tick()` | Same |
| `on_buffer_update()` | `on_buffer_update()` | Now incremental (BufferUpdateEvent) |
| `on_cursor_update()` | `on_cursor_update()` | Same |
| `on_mode_change(mode, is_insert)` | `on_mode_change(mode)` | Drops `is_insert` param (modules derive from mode name) |
| `apply_notification()` | `on_notification()` | Renamed |
| `classify_token()` | `classify_token()` | Same |
| `transform_line()` | `transform_line()` | Same |
| `virtual_lines()` | `virtual_lines()` | Same |
| `fold_hidden_lines()` | `fold_ranges()` | Renamed |
| `content_offset_left()` | `chrome_requested_size() + Left position` | Refactored |
| `map_cursor_column()` | `map_cursor_column()` | Same |
| `cursor_position()` | `cursor_position()` | Same |
| `inline_decorations()` | `inline_decorations()` | Same |

### Migration phases

1. **Introduce `ClientModule` trait** alongside `TuiExtension` in the driver crate.
   Create a bridge adapter that wraps `TuiExtension` as `ClientModule`.

2. **Migrate extensions one at a time** from `TuiExtension` to `ClientModule`.
   Both coexist during migration — the bridge adapter handles old extensions.

3. **Extract `ViewportRenderer`** from existing compositor logic.
   The render engine's buffer rendering code becomes the default implementation.

4. **Extract Gutter framework** from window renderer into ViewportRenderer.
   Line numbers and git signs become annotation modules.

5. **Replace `RenderBackend`** with `RenderSurface`.
   The TUI adapter wraps `Screen`/`FrameBuffer` behind the new trait.

6. **Remove `TuiExtension`** and the bridge adapter when all extensions are migrated.

Each phase is independently shippable. The editor works throughout.

### What stays the same

- `LayoutPolicy` / `FocusPolicy` — architecturally same pattern; method renames
  (`arrange` -> `layout`, `WindowView` -> `WindowLayout`, removes `is_focused` from
  `WindowLayout` and adds `focused: WindowId` as input to `layout()`)
- `ServiceRegistry` / `ComponentProviderRegistry` — already working pattern
- `AnnotationSource` / `AnnotationStore` pipeline — existing gutter system
  maps closely to the annotation method group
- The `display` driver crate — becomes the foundation of `reovim-client-driver`
