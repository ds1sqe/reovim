# Client Architecture

Architectural specification for reovim clients (Client Layer Model v6.3).

Defines the ideal architecture — what clients SHOULD be.
Looks beyond TUI to Web, Android, and iOS.

## Reading Order

1. [Principles](principles.md) — Semantic, Presence, Adoption, Specialization
2. [Layers](layers.md) — Layer diagram, rules, dependency graph
3. [Platform](platform.md) — PlatformCapabilities, RenderSurface, InputSource
4. [Module](module.md) — ClientModule trait, lifecycle, roles
5. [Rendering](rendering.md) — ViewportRenderer, chrome compositor, gutter
6. [Data Flow](data-flow.md) — All flows, ServerHandle, cross-module
7. [Types](types.md) — All type definitions
8. [Extensibility](extensibility.md) — Cargo crate model, no god crate
9. [Examples](examples.md) — Worked examples
10. [Gaps](gaps.md) — Known gaps, future work, migration

## Platform Binaries

| Platform | Documentation | Source |
|----------|---------------|--------|
| TUI | [TUI Client](./tui.md) | `clients/tui/` |
| CLI | [CLI Client](./cli.md) | `clients/cli/` |
| Web | [Web Client](./web.md) | `clients/web/` (TypeScript + WASM, gRPC-Web, Playwright tests) |

## Version History

- **v1-v3**: Initial proposals, symmetric model rejected by audit
- **v4**: 3-layer model, 4-aspect philosophy, separate ChromeRegion/BufferContrib traits
- **v5**: Single ClientModule trait, ViewportRenderer extraction, ABI-unstable extensibility
- **v6**: Directory structure. All audit findings resolved. Gaps honestly acknowledged.
- **v6.1**: Address 9 findings from v6 audit.
- **v6.2**: Address all 16 findings from v6.1 5-agent audit.
- **v6.3**: Address final NITs from v6.2 unanimous A+ audit.

## Changes

v6.3 changes (3 NITs from v6.2 Round 4 audit, unanimous A+ GO):

NIT fixes:
- `id()`, `kind()`, `name()` return `&'static str` (module identifiers are always
  string literals; fixes lifetime mismatch with `server_kinds() -> Vec<&'static str>`)
- `server_kinds()` return type: `Vec<&str>` -> `Vec<&'static str>` for consistency
- Migration note corrected: "removes `is_focused` from `WindowLayout` and adds
  `focused: WindowId` as input to `layout()`" (was "adds is_focused")
- All examples updated to match `&'static str` return types

v6.2 changes (16 findings from v6.1 audit, 2 oracle + 3 FD):

BLOCKING fixes:
- Cross-module example rewritten: `self as *const _` raw pointer replaced
  with safe `Arc<GitBranchState>` shared ownership (unanimous 5/5 finding)
- `ColorDepth` enum defined in types.md
- `BufferMetadata` struct defined in types.md
- `ComponentContext` removed; `ComponentProvider::render()` takes no params
- `ServiceRegistry` usage documented with `ComponentProviderRegistry` pattern
- `ComponentProvider` trait and `ComponentProviderKey` defined in types.md

CONCERN fixes:
- `Defer` retry protocol specified: 3 rounds, circular detection, fail on no progress
- Single-task ownership model documented: explains why Phase 1/Phase 2 never interleave
- Multi-window compositor flow: explicit per-window iteration with WindowId->BufferId mapping
- `write_styled` returns `u16` (column count) matching existing RenderBackend
- Mode cache purpose documented: replay current state to late-joining modules
- `server_kinds()` method added to ClientModule trait
- Migration table corrected: init/exit signatures, on_mode_change drops is_insert,
  render_with_viewport split into chrome/buffer-contrib, LayoutPolicy renames
- `GutterCell::text` changed from `Cow<'static, str>` to `String` with justification
- `InputSource` moved from CLIENT DRIVER to CLIENT CORE (keeps DRIVER platform-agnostic)
- First-frame rendering documented: CORE renders immediately, modules show default state

NIT fixes:
- `fold_ranges` changed from `(u32, u32)` to `(usize, usize)` for consistency
- `AnnotationContext.theme` renamed to `gutter_style` for clarity
- `Style`/`Color` sourcing documented (re-exported from reovim-arch)
