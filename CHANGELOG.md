# Changelog

For old changelog, see `changelog/CHANGELOG-{version}.md`

## [Unreleased] - v0.10.0-dev

### Added

- Syntax session integration: wire SyntaxDriver into session model for live token streaming (#539)
- Layered annotation system: open `HighlightCategory(Arc<str>)` replaces closed `HighlightGroup` enum (#540)
  - `Annotation` type with `AnnotationKind` (Highlight, Conceal, Background, VirtualText)
  - Remove `CaptureMapper` — tree-sitter capture names map directly to `HighlightCategory` strings
  - Multi-layer `LayeredTokenCache` with priority-based compositing in display driver
  - Expanded theme groups: 42 base categories + 30 sub-categories with hierarchical fallback
  - Protocol updated with `AnnotationKind` and layer support
  - Dynamic injection layer creation: `InjectionManager` lazily creates layers via `InjectionLayerStore`
- `CompositeFactory` routes `create()` across all registered `SyntaxDriverFactory` instances
- `LanguageInfoStore` for module self-registration of language metadata during `init()`
- `DefaultLanguageRegistry` detects language from file path extensions and MIME types
- `SyntaxSessionState.ensure_driver_from_path()` for automatic language detection and driver creation
- `emit_syntax_updates` broadcasts token updates to stream subscribers on buffer modification
- `build_token_update` standalone helper for extracting highlights from drivers

### Changed

- **Architecture**: Fix mechanism-vs-policy violations and cross-module coupling (#542)
  - Move `VimLookupPolicy` from driver-input to module-vim (policy belongs in modules)
  - Make `KeymapRegistry` lookup policy configurable via `Arc<dyn KeyLookupPolicy>`
  - Promote `PendingNotificationQueue` from module-completion to driver-session (shared mechanism)
  - Add `LspLifecycle` trait in driver-lsp to decouple completion from module-lsp
  - Add `SnippetExpander` trait in driver-session to decouple completion from module-snippet
  - Add `NotificationDrain` trait in driver-session to decouple completion from module-notification
  - Remove all `reovim-module-*` dependencies from completion module's Cargo.toml

### Performance

### Fixed


### Security

### Removed

---

## Version History

- v0.9.5 - Completion engine, snippet system, explorer, microscope, range-finder, notifications - see [CHANGELOG-0.9.5.md](changelog/CHANGELOG-0.9.5.md)
- v0.9.3 - Per-client architecture, TUI unification, presence rendering, integrated mode - see [CHANGELOG-0.9.3.md](changelog/CHANGELOG-0.9.3.md)
- v0.9.2 - Cmdline UI, themes, annotations, gRPC v2, web client, syntax service - see [CHANGELOG-0.9.2.md](changelog/CHANGELOG-0.9.2.md)
- v0.9.1 - Phase 7: E2E Test Suite & Mechanism/Policy Separation - see [CHANGELOG-0.9.1.md](changelog/CHANGELOG-0.9.1.md)
- v0.9.0 - New architecture (lib/arch, lib/kernel, lib/drivers/*) - see [CHANGELOG-0.9.0.md](changelog/CHANGELOG-0.9.0.md)
- v0.8.x and earlier - Legacy crates (lib/core, lib/sys, plugins) - see [CHANGELOG-archive.md](changelog/CHANGELOG-archive.md)
