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
- Theme system enhancements (#541)
  - Base theme inheritance: `base = "dark"` in TOML overlays custom styles on built-in theme
  - `[decoration]` section in theme files with `decoration.` prefix
  - `ThemeInfo` struct and `ThemeLoader::discover()` for structured theme enumeration
  - `$REOVIM_THEME_DIR` environment variable for custom theme search path
  - `ThemeLoader::load()` falls back to built-in themes when file not found
  - `SharedThemeManager` and `ThemeLoader` registered in server `ServiceRegistry` at bootstrap
  - `:colorscheme` command loads file-based themes through `ThemeLoader`
- `CompositeFactory` routes `create()` across all registered `SyntaxDriverFactory` instances
- `LanguageInfoStore` for module self-registration of language metadata during `init()`
- `DefaultLanguageRegistry` detects language from file path extensions and MIME types
- `SyntaxSessionState.ensure_driver_from_path()` for automatic language detection and driver creation
- `emit_syntax_updates` broadcasts token updates to stream subscribers on buffer modification
- `build_token_update` standalone helper for extracting highlights from drivers
- **Event-driven LSP lifecycle (#531, Phase 10)**: LSP server startup moved from
  completion-trigger to kernel event subscriptions. `:e` command now emits
  `FileOpened` and `FileTypeChanged` kernel events after file load, with
  case-insensitive `file_type_from_extension()` mapping 17 extensions to LSP
  language IDs. LSP module (`reovim-module-lsp`) subscribes to `FileOpened`,
  `BufferClosed`, `BufferModified`, and `BufferSaved` events at
  `priority::PLUGIN` via RAII subscription pattern. `LspBufferTracker`
  (ServiceRegistry, RwLock-backed) tracks buffer-to-path/language mappings with
  monotonically increasing version numbers for LSP spec compliance.
  `LspStartingGuard` (AtomicBool) prevents concurrent server starts. Double-open
  guard prevents duplicate `DidOpen` notifications. `send_request` failures are
  logged as warnings. Four event handlers: `handle_file_opened` (auto-start +
  DidOpen), `handle_buffer_closed` (DidClose), `handle_buffer_modified`
  (DidChange with versioning), `handle_buffer_saved` (DidSave placeholder).
  Completion module decoupled: removed `reovim-module-lsp` and `tokio`
  dependencies, `fire_lsp_completion()` returns early without active provider.
  `RecordingLspProvider` mock enables full send-path test coverage. 325 unit
  tests across affected modules, zero clippy warnings.

- **LSP navigation gd/gr (#532)**: New `reovim-module-lsp-navigation` module
  providing Go-to-Definition (`gd`) and Find References (`gr`) commands via LSP.
  `LspLocationPicker` implements `Picker` trait for multi-result navigation with
  file preview. Pure helper functions: `definition_to_locations` (handles Scalar,
  Array, Link response variants), `path_from_uri` (fluent_uri `file://` stripping),
  `location_to_picker_item` (0→1 indexed conversion), `language_from_path`
  (case-insensitive extension matching), `find_provider` (language key + default
  fallback), `has_definition_capability` / `has_references_capability` (server
  capability checks). Single-result jumps directly; multi-result opens picker via
  MicroscopeState injection. New `reovim-module-vim-lsp` adapter crate bridges
  vim (editor personality) and lsp-navigation (code intelligence) with explicit,
  visible coupling: `gd`/`gr` keybindings target `vim:normal` mode with "lsp"
  category. Neither vim nor lsp-navigation knows about the other — the adapter
  is the sole coupling point, swappable for alternative editor personalities.
  Modules wired into defaults bundle (28 modules). 65 unit tests (54 lsp-navigation
  + 11 vim-lsp adapter) with full coverage.

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
