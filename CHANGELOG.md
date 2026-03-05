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

- **LSP dynamic capability registration (#533)**: Handle `client/registerCapability`
  and `client/unregisterCapability` server-to-client requests by parsing
  `RegistrationParams`/`UnregistrationParams` and updating stored
  `ServerCapabilities`. New `CapabilityStore` in driver layer wraps
  `ArcSwap<ServerCapabilities>` for lock-free reads and atomic updates,
  following the `DiagnosticCache` pattern. Supports 8 LSP methods:
  completion, hover, definition, references, signatureHelp, codeAction,
  formatting, documentSymbol. `LspProvider::capabilities()` trait signature
  changed from `Option<&ServerCapabilities>` to `Option<Arc<ServerCapabilities>>`
  to support dynamic updates (consumers unchanged via Arc deref). Client
  capabilities now advertise `dynamic_registration: true` for completion,
  hover, definition, and references. Registration ID tracking enables
  proper unregistration by ID. 41 new tests across driver and module crates.

- **FFI execution API for external modules (#384)**: Full runtime bridge enabling
  C, Python, and Haskell modules to access the same capabilities as native Rust
  modules. Thread-local `RuntimeGuard`/`InitGuard` RAII pattern (inspired by
  Neovim's Lua bridge) provides safe access to `SessionRuntime` during command
  callbacks. Buffer API (10 functions: active buffer, line read, content read,
  line count, line length, text range, insert, delete range, create, delete).
  Window API (7 functions: active window, cursor position, window count, buffer
  lookup, create, close, focus). Mode API (5 functions: current mode, mode depth,
  push, pop, set). Command API (2 functions: register callback, execute by ID).
  Event API (2 functions: subscribe with callback, unsubscribe). All FFI functions
  use caller-owned buffer pattern for string reads, `#[repr(C)]` types
  (`ReovimPosition`, `ReovimStringResult`, `ReovimCommandArgs`), and 11 distinct
  negative `i32` error codes. `FfiCommandHandlerStore` and
  `FfiEventSubscriptionStore` integrate with `ServiceRegistry` during module init.
  Python bindings via PyO3 (`PyRuntimeApi` class) wrap the runtime bridge
  directly. ABI version bumped to 1.1.0 (backward compatible). C header
  (`reovim.h`) updated with all new type definitions, function declarations, and
  callback types. 270 unit tests (206 FFI + 64 Python), zero clippy warnings.

### Changed

- **Architecture**: Extract vim-mode coupling from feature modules into adapter crates.
  Five new `vim-*` adapter crates (`vim-microscope`, `vim-explorer`, `vim-completion`,
  `vim-range-finder`, `vim-snippet`) isolate vim-specific keybindings from feature
  modules. Feature modules (microscope, explorer, completion, range-finder, snippet)
  no longer hardcode `"vim:normal"` / `"vim:insert"` mode targets. Follows adapter
  pattern established by `vim-lsp` in #532. Defaults bundle updated to 33 modules.

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
