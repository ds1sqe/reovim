# Changelog

For old changelog, see `changelog/CHANGELOG-{version}.md`

## [Unreleased] - v0.13.0-dev

### Added

- **v0.8.1 feature parity audit (#652, part of #645)**: Comprehensive feature
  matrix document comparing the pre-kernel v0.8.1 archive against the current
  codebase across 11 categories (core editing, motions, operators, text objects,
  visual mode, search, ex commands, window management, plugins/modules, language
  support, UI features). Overall parity: ~95%. Identified 5 gaps: semantic text
  objects (treesitter-based), marks (kernel ready, vim commands stubbed), jump
  list (kernel ready, keybindings not wired), yank animation, animated landing
  mascot. Documented 20+ features new in current that did not exist in v0.8.1.

- **Web CLM rendering infrastructure (#651, part of #645)**: Chrome compositor,
  DOM render surface, and chrome dispatcher for the web client.
  `ChromeCompositor` allocates non-overlapping screen regions using TUI-matching
  edge-inward algorithm with four independent counters and priority sorting.
  `DomChromeSurface` implements `RenderSurface` for DOM-based character grid
  rendering with virtual buffer and style-to-CSS conversion. `ChromeDispatcher`
  orchestrates layout, container lifecycle, and dual rendering paths (DOM for
  existing `WebExtensionAdapter` modules, cell-grid for future native CLM).
  `Editor.extensionUpdated` handler decouples notification dispatch from
  rendering. Z-index strategy: edge chrome by allocation order, overlays at
  1000+zOrder. Fixed pre-existing `LandingExtension` test count bug in
  `extensions.test.ts`. 32 new tests across 3 test files.

- **Web CLM contracts and module lifecycle (#650, part of #645)**: TypeScript
  interfaces mirroring all Rust CLM traits (`PlatformCapabilities`,
  `RenderSurface`, `ServerHandle`, `ClmThemeProvider`, `ClientModuleRegistry`,
  `ClientServiceRegistry`, `ModuleContext`, `ClientModule`) in
  `clients/web/src/core/`. `BrowserPlatformAdapter` queries browser
  capabilities with SSR-safe guards. `GrpcServerHandle` wraps the existing
  `ReovimClient`. `ThemeProviderAdapter` bridges web `ThemeManager` to CLM
  interface. `WebExtensionAdapter` wraps all 9 existing `WebExtension`
  implementations into `ClientModule` without behavior change.
  `ClientModuleLoader` provides topological sort, 3-pass deferral init, and
  reverse-order shutdown. `Editor` and `HeadlessWebClient` upgraded from raw
  `createExtensions()` to CLM loader. 119 new tests across 9 test files.

- **ClientModule FFI event and role declaration trampolines (#648, part of #645)**:
  Extended the client module FFI boundary with 16 new trampolines for event
  dispatch (`on_notification`, `on_mode_change`, `on_cursor_update`,
  `on_buffer_focus`, `on_buffer_update`, `on_option_changed`, `tick`) and role
  declaration (`has_chrome`, `has_buffer_contrib`, `has_annotations`,
  `chrome_position`, `chrome_requested_size`, `chrome_priority`, `chrome_z_order`,
  `buffer_contrib_priority`, `annotation_priority`). `ClientModuleHandle` now
  dispatches all event and role methods through both static trait and dynamic FFI
  paths. `ClientModuleProbe` gains `capabilities` bitflags for probe-time role
  detection. `declare_client_module!` proc macro generates all new trampoline
  symbols. `CLIENT_MODULE_API_VERSION` bumped to 0.3.0.

- **ClientModule SDK refinement (#646, part of #645)**: Enriched the client driver
  crate for external module development (primary consumer: Tilth game).
  `ClientModuleError` is now an enum with `InitFailed`, `ExitFailed`,
  `NotificationParse`, `Other` variants, `Display`/`Error` impls, and `source()`
  chain. New `ScopedSurface` wrapper provides automatic coordinate offset and
  clipping for bounded chrome rendering. New `ClientServiceRegistry` (TypeId-based,
  thread-safe) and `ClientModuleRegistry` trait for cross-module service discovery
  and module state introspection. `ModuleContext` gains `services` and
  `module_registry` fields (both `Option` for FFI backward compat). `ServerHandle`
  expanded with `list_commands()` and `get_option_metadata()` (default impls).
  New `OptionMetadata` and `OptionKind` types. `Rect::intersect()` and
  `Rect::contains_point()` helpers. Notification parsing helpers
  (`parse_notification`, `parse_notification_field`) behind `serde` feature gate.
  `CLIENT_MODULE_API_VERSION` bumped to 0.2.0. 357 tests, zero clippy warnings.

- **ClientModule testing utilities (#647, part of #645)**: New `testing` module in
  `reovim-client-driver` (behind `testing` feature gate) with `RecordingSurface`
  (cell-grid with assertion methods), `WriteSurface` (write-log pattern),
  `MockPlatformCapabilities` (builder), `MockServerHandle` (option seeding,
  command recording), `MockThemeProvider` (highlight seeding),
  `MockModuleRegistry`, and `TestModuleContext` builder. Migrated 15 TUI module
  test files to shared mocks, eliminating duplicated boilerplate. Driver crate
  dogfoods its own testing API. 378 driver tests, 481 TUI tests, zero warnings.

- **Codec metadata end-to-end pipeline (#649, part of #645)**: Fixed latent bug where
  `CodecSessionState` was stored in per-client extensions instead of shared (per-buffer)
  extensions. gRPC `ListBuffers` response now populates `codec_metadata` (codec name,
  line ending, BOM flag) from `CodecSessionState`. CLI `buffers` command displays
  codec info in both plain (`[utf-8, lf]`) and JSON formats.

### Changed

- Version bump to 0.12.1-dev for Phase 13 (SDK polish, integration, web alignment)

## [0.12.0-dev] - 2026-03-14

### Added

- **Client driver crate** (`reovim-client-driver`) (#632): Platform-agnostic trait contracts
  for the Client Layer Model. Defines `ClientModule` trait (single trait CORE interacts with),
  `PlatformCapabilities`, `RenderSurface`, `ServerHandle`, `ThemeProvider`, `ViewportRenderer`,
  and `LayoutPolicy` traits. All supporting types: `ProbeResult`, `Version`, `BufferId`,
  `ChromePosition`, `ColumnWidth`, `GutterCell`, `AnnotationContext`, `RenderBehavior`,
  `TransformedLine`, `VirtualLine`, `InlineDecoration`, `Style`, `Attributes`, and more.
  87 unit tests with zero clippy warnings.

- **TuiExtensionBridge adapter** (#632): Bridge wrapping all 15 existing `TuiExtension`
  implementations as `ClientModule`. Per-extension semantic classification (chrome vs
  buffer-contrib), mode derivation (`is_insert` from mode name), type conversions between
  display driver and client driver types, `SurfaceBackendAdapter` for legacy rendering.
  80 unit tests covering all 15 extensions, all conversion functions, and adapter delegation.

- **Platform render surface and input abstraction** (#633): Upgrade `TuiPlatformCapabilities`
  to wrap `DisplayCapabilities` with mutable terminal state (focus, dark mode via COLORFGBG
  heuristic, grid size). Add `TuiRenderSurface<B>` generic adapter (monomorphized
  `RenderBackend` → `RenderSurface`). Add `InputEvent` system to client-driver crate:
  `KeyEvent`, `KeyCode`, `Modifiers` (bitflag), `PointerEvent`, `TouchEvent`, `FocusEvent`.
  Add `convert_key_event()` and `convert_mouse_event()` crossterm-to-CLM conversion layer.
  67 bridge tests + 26 input type tests.

- **Chrome module migration** (#634): Migrate all 10 chrome extensions to native `ClientModule`
  implementations, bypassing the `TuiExtensionBridge`. Includes statusline (extracted from
  render engine), hover, signature-help, landing, completion, notification, which-key,
  cmdline, microscope, explorer, and tetromino (polyblocks). Each module uses `RenderSurface`
  directly with `chrome_utils` and `ui` utilities. Explorer uses `ChromePosition::Left` with
  `chrome_requested_size()` for sidebar width. Shared infrastructure:
  `chrome_utils::render_box_border()`, `chrome_utils::popup_width()`,
  `chrome_utils::popup_x()`, `ui::truncate_end()`, `ui::display_width()`.
  Re-export `reovim_arch` from `reovim-client-driver` for Clock access.
  Bridge `classify_extension()` cleaned: only 5 buffer-contrib extensions remain.
  11 native module crates total.

- **ViewportRenderer extraction** (#635): Extract the buffer rendering pipeline from
  `render_engine.rs` into a platform-agnostic `DefaultViewportRenderer` in the client-driver
  crate. Port conceal system (`SyntaxToken`, `ConcealDecoration`, `ConcealedLine`,
  `apply_conceals()`, `source_to_display_col()`, `dim_style()`, color blending) to
  `shared/clients/driver/src/conceal.rs`. Add viewport types: `ViewportContext`,
  `CursorInfo`, `SelectionInfo`, `SelectionMode`, `RemoteClientInfo`, `LineNumberMode`.
  Add `TokenProvider` trait for syntax token abstraction. `DefaultViewportRenderer`
  handles buffer content (syntax highlighting, conceals, folds, virtual lines, line
  numbers, transformed lines), selections (local + remote, char/line/block modes),
  and cursors (self + remote with CBF-8 colorblind-friendly palette and labels).
  Expand `ViewportRenderer::render_viewport()` with `ViewportContext` + `TokenProvider`.
  `render_engine.rs` reduced from 1232 to ~320 lines (thin compositor).
  Add `TokenProviderAdapter` and `ThemeProviderAdapter` bridge adapters.
  67 viewport tests, 27 conceal tests.

- **Gutter framework and LineNumbersModule** (#636): Extract line number rendering
  from viewport.rs into `LineNumbersModule` annotation module. Add gutter composition
  system to `DefaultViewportRenderer` — modules with `has_annotations()` provide
  `GutterCell` values per line, rendered left-to-right by priority. Supports absolute,
  relative, hybrid modes via `on_option_changed("number"/"relativenumber")`. Dynamic
  column width based on total line count. 24 module tests, 12 native modules total.

- **Buffer contribution migration** (#637): Migrate all 5 buffer-contrib TuiExtension
  implementations to native ClientModule. FoldModule (fold_ranges + transform_line for
  fold markers, 33 tests). JumpModule (transform_line for jump label overlays with
  bright/dim two-char styling, 29 tests). PairModule (inline_decorations via HashMap
  for rainbow brackets + matchpair highlighting, 23 tests). DiagnosticsModule
  (inline_decorations for underlines + virtual_lines for diagnostic messages, 28 tests).
  MarkdownModule (classify_token for heading/bullet/code conceals, transform_line for
  table rows with segment-based TransformedLine, virtual_lines for table borders,
  map_cursor_column for cursor in expanded rows, 4 sub-modules: behaviors/detect/layout/mapping,
  70 tests). Legacy extension list now empty (0 TuiExtensions). Bridge classify_extension()
  simplified to const fn. 17 native ClientModule instances registered.

- **Client module loader** (#638): `ClientModuleLoader` with factory map consumption,
  dependency resolution via `reovim-depgraph` topological sort, and multi-pass initialization
  with deferral (3 passes, matching server). `TuiServerHandle` adapter wrapping `TuiGrpcClient`
  for sync `ServerHandle` trait using `block_in_place`. Module lifecycle wiring: `init_all()`
  before initial state fetch, `on_all_loaded()` after init, `exit_all()` on shutdown.
  Factory map (`static_client_modules.rs`) replaces `create_native_modules()` from defaults
  crate. `TuiGrpcClient` cloneable for server handle sharing. 26 loader tests, 6 server
  handle tests, 3 factory map tests.

- **`declare_client_module!` macro** (#638): Proc-macro generating FFI entry points for
  dynamic client module loading (`reovim_client_module_*` prefix, avoiding server symbol
  collision). `ClientModuleProbe` type for FFI-safe metadata, `CLIENT_MODULE_API_VERSION`
  constant, `is_client_compatible()` semver check. 10 type tests.

- **`ClientModuleHandle` and dynamic loading infrastructure** (#638): Unified handle
  wrapping static `Box<dyn ClientModule>` and dynamic `*mut c_void` with FFI symbols.
  `ClientFfiSymbols` for resolved trampolines. Discovery module with XDG-compliant search
  paths, `REOVIM_CLIENT_MODULE_PATH` env var, and `.so` filename conventions. 13 handle
  tests, 7 discovery tests.

- **CORE purity grep test** (#638): `scripts/grep-test-core.sh` enforces that
  `clients/tui/src/` (excluding factory map and test files) has zero references to
  concrete module type names or crate prefixes.

### Changed

- TUI modules registered via `builtin_client_modules()` factory map instead of
  `create_native_modules()` (#638)
- `TuiApp` stores `ClientModuleLoader` instead of `Vec<Box<dyn ClientModule>>`
  for lifecycle management (#638)
- `TuiGrpcClient` is now `Clone` for server handle sharing (#638)

### Removed

- **Legacy extension cleanup** (#638): Delete `TuiExtensionBridge` adapter and all 14
  legacy `TuiExtension` crates (~13,000 lines removed). Remove `create_extensions()`,
  `create_extensions_filtered()`, `validate_extensions()`, `shutdown_extensions()`.
  Simplify `TuiApp` to use `create_native_modules()` directly with `set_disabled_kinds()`
  for filtering. Defaults meta-crate reduced from 178 to 42 lines. Only the
  `extensions/defaults/` meta-crate remains; all individual extension crates deleted.
  Delete `reovim-tui-ext-defaults` crate (replaced by `static_client_modules.rs` factory
  map and `ClientModuleLoader`). Delete `set_disabled_kinds()` method (filtering at
  construction time via loader).
  Delete `TuiExtension` trait, `ViewportContext`, `RenderBehavior`, `VirtualLine`,
  `TransformedLine`, and `VirtualLinePosition` from display driver (superseded by
  client-driver types). Delete `declare_extension!` macro (no consumers). Remove dead
  bridge conversion functions (`collect_virtual_lines`, `classify_with_extensions`,
  `transform_line`, `map_cursor_column`, `visual_line_len`, `convert_driver_rb_to_display`,
  `convert_driver_tl_to_display`). ~1,200 additional lines removed.

### Changed

- **Version bump**: Start CLM (Client Layer Model) epic (#628)

## [0.11.0-dev] - 2026-03-13

### Changed

- **Server-to-display boundary refactor (#625)**: Extract 3 new server-side driver crates
  from the display driver to enforce mechanism-vs-policy separation. `reovim-driver-layout`
  (compositor types, window layout), `reovim-driver-annotation` (annotation data types,
  `AnnotationSource` trait, `AnnotationStore`), and `reovim-driver-statusline`
  (`ComponentDataProvider` trait, `ComponentData`, `ComponentDataContext`). Server modules now
  register data sources only -- the display layer creates presenters (Style, Color) and pairs
  them with sources via bootstrap bridges. `DataProviderAdapter` wraps server-side
  `ComponentDataProvider` into display-side `ComponentProvider`. `LineNumberPresenter`,
  `GitSignsPresenter`, and `BlamePresenter` moved from server modules to display driver.
  12 server crates no longer depend on `reovim-driver-display`. Only `reovim-module-commands`
  retains the display dependency (colorscheme needs `ThemeManager`).

- **Eliminate shared/extension-kinds crate (#625)**: Replace centralized
  `reovim-extension-kinds` closed registry with local `const KIND: &str` constants in each
  consumer. Server modules (12 crates), TUI extensions (14 crates + defaults), and web
  extensions (8 files) now define their own kind strings locally. Multi-kind modules
  (range-finder, lsp-navigation) use `pub(crate)` constants. The `shared/extension-kinds/`
  crate and `clients/web/src/extensions/extension-kinds.ts` are deleted. Runtime extension
  discovery via `collect_available_kinds()` is unchanged. Any module (builtin or third-party)
  can now define its own extension kind without modifying a central registry.

- **Decouple protocol crate from kernel (#625)**: Move undo tree kernel↔serializable
  conversion code (`from_undo_tree`, `to_undo_tree`, and 7 From/Into impls) from
  `shared/protocol/src/v1/undo.rs` to `server/modules/undo/src/conversion.rs`. Remove
  unused `WireWindowId↔WindowId` From impls from `shared/protocol/src/v1/types.rs`.
  Delete `reovim-kernel` dependency from `shared/protocol/Cargo.toml`. Conversion
  functions use explicit functions instead of From/Into impls (orphan rule). Protocol
  crate is now kernel-agnostic — TUI/web clients no longer transitively depend on
  the kernel.

### Added

- **Content Codec Pipeline foundation (#629, part of #627)**: Pluggable content codec system
  replacing hardcoded `String::from_utf8(bytes)` in the file-loading path. New
  `reovim-driver-codec` driver crate defines `ContentCodec`, `ContentClassifier`,
  `ContentCodecFactory`, `ContentCodecFactoryStore`, `ContentClassifierStore`, and
  `CodecSessionState` traits/stores following the established `SyntaxDriverFactory` pattern.
  New `reovim-module-codec-utf8` module provides UTF-8 codec with BOM detection/stripping and
  CRLF-to-LF normalization on decode, with round-trip restoration on save. `:e` command now
  decodes files through the codec pipeline with graceful UTF-8 fallback when no codec module is
  loaded. `:w` command encodes through the codec pipeline, respecting original line endings and
  BOM. Read-only enforcement for lossy codec decodes (binary files). `CODEC_PROVIDER` capability
  added. `BufferInfo` protocol type extended with optional `content_type`, `readonly`, and
  `codec_metadata` fields (backward-compatible via `skip_serializing_if`). v2 protobuf
  `BufferInfo` and `CodecMetadata` messages extended with corresponding optional fields.

- **Binary hex-dump codec (#630, part of #627)**: New `reovim-module-codec-hex` module provides
  binary file detection and hex dump viewing. `BinaryClassifier` (priority 20) detects binary
  files via null byte detection, non-printable character ratio (>30%), and known binary extension
  fast-path (.exe, .dll, .so, .png, .zip, .pdf, .wasm, etc.). `HexCodec` formats raw bytes as
  standard hex dump output (8-digit offset, 16 bytes/line in two groups of 8, ASCII sidebar).
  One-way codec: `decode()` produces readonly/lossy hex view, `encode()` returns `None`. Emits
  `content.hex.address`, `content.hex.byte`, and `content.hex.ascii` annotations per line for
  syntax highlighting. Binary files open in read-only hex view; `:w` blocked with clear error.

- **CJK encoding codecs (#631, part of #627)**: New `reovim-module-codec-cjk` module adds
  EUC-KR, Shift-JIS, GB2312/GBK, and Big5 encoding support. `CjkClassifier` (priority 50)
  detects CJK-encoded files by attempting decode with `encoding_rs` and checking for zero
  replacement characters, with UTF-8 boundary-safe sampling. `CjkCodec` provides bidirectional
  encode/decode with round-trip guarantees for all supported CJK character sets.

- **Legacy encoding codecs (#631, part of #627)**: New `reovim-module-codec-legacy` module adds
  Latin-1 (ISO-8859-1) and Windows-1252 encoding support. `LegacyClassifier` (priority 40)
  distinguishes Windows-1252 from Latin-1 by detecting CP1252-specific bytes (0x80-0x9F range:
  smart quotes, em-dash, Euro sign). Latin-1 codec uses direct byte-to-Unicode mapping;
  Windows-1252 uses `encoding_rs`. Both are bidirectional with round-trip guarantees.
  `encoding_rs` is the only new external dependency.

- **PDF content codec (#640, part of #627)**: New `reovim-module-codec-pdf` module provides
  read-only text extraction from PDF files. `PdfClassifier` (priority 35) detects `%PDF-` magic
  bytes at offset 0 with `.pdf` extension fast-path. `PdfCodec` extracts text via `pdf-extract`
  crate, producing page-delimited content with `--- Page N ---` separators and
  `content.pdf.page` annotations with page number payloads. One-way codec: `decode()` extracts
  text, `encode()` returns `None`. `lossy: true`, `readonly: true`. Metadata includes
  `page_count`. New workspace dependency: `pdf-extract = "0.7"`. 44 tests.

- **ELF/ZIP structured binary codec (#641, part of #627)**: New
  `reovim-module-codec-binary-struct` module provides structured summaries for ELF binaries and
  ZIP archives. `ElfClassifier` (priority 33) detects `\x7fELF` magic, `ZipClassifier`
  (priority 31) detects `PK\x03\x04` magic with extension fast-path for `.zip`, `.jar`, `.war`,
  `.apk`, `.xlsx`, `.docx`, etc. `ElfCodec` produces header info, section table with
  name/size/offset/type columns. `ZipCodec` produces entry listing. Annotations:
  `content.elf.header`, `content.elf.section`, `content.zip.header`, `content.zip.entry`. Both
  one-way codecs: `lossy: true`, `readonly: true`. New workspace dependencies: `goblin = "0.9"`,
  `zip = "8"`. 76 tests.

- **CSV/TSV/PSV content codec (#642, part of #627)**: New `reovim-module-codec-csv` module
  provides bidirectional column-aligned tabular view for delimiter-separated files.
  `CsvClassifier` (priority 15) detects CSV/TSV/PSV by delimiter analysis (consistent columns
  across rows, minimum 2 columns and 2 rows) with extension fast-path for `.csv`, `.tsv`,
  `.psv`, `.tab`. `CsvCodec` decodes into column-aligned text with 2-space separators, minimum
  column width 3. Full round-trip: `encode()` reconstructs original delimiter/quoting/line
  endings from metadata. Header detection via numeric-vs-text heuristic. Annotations:
  `content.csv.header` (header row), `content.csv.column` (every row with column count payload).
  `lossy: false`, `readonly: false`. New workspace dependency: `csv = "1.3"`. 71 tests.

- **Defaults god-crate removal (#620)**: Replace `reovim-module-defaults` centralized module
  registry with data-driven `builtins.toml` manifest and feature-gated `static_modules.rs`
  factory map. `TrackedModule` now uses `ModuleHandle` from `reovim-driver-module-loader` to
  unify static and dynamic module loading behind a single interface. Textobjects module
  promoted from `REOVIM_EXTRA_MODULES` env var special case to regular builtin (41 modules
  total). `collect_extra_modules()` and `create_extra_module()` removed. CI updated to build
  treesitter modules as `.so` files. Build script paths fixed for `server/modules/` layout.
  Both static (`--features static-modules`) and dynamic (`--no-default-features`) compilation
  paths verified clean.

- **Extension manager phases 10-12 (#562)**: Four new crates completing the extension manager
  system. Phase 10 (#621): `reovim-driver-module-registry` with `ModuleManifest` parser for
  third-party `module.toml` files, `ModuleSource` (Git/Path) enum for install provenance, and
  `InstalledModules` JSON persistence for tracking installed modules. Workflow operations
  (install, remove, update, check, resolve, info, list) for offline module lifecycle. CLI
  subcommands via `reovim module <subcommand>` for all workflow operations. Phase 10.5 (#622):
  `reovim-module-module-manager` with interactive panel: `ModuleManagerState` (per-client
  state with filter/navigation), `ModuleManagerBridge` (extension state bridge for JSON
  serialization), `ManagerMode` (navigation-only mode with Block cursor), `ManagerResolver`
  (keymap-based key resolver), and navigation command handlers (open/close/next/prev/
  toggle-filter/toggle-detail). Keybindings: j/k/Up/Down for navigation, Tab for filter
  cycling, Enter for detail toggle, q/Esc to close. `MODULE_MANAGER` extension kind constant
  in `reovim-extension-kinds`. Phase 11 (#623): Cross-personality initial mode support via
  `InitialModeProvider` service in session driver. VimModule registers `vim:normal`, EmacsModule
  registers `emacs:default` during init(). Bootstrap reads from provider with `vim:normal`
  fallback, replacing hardcoded mode. Personality selection via module enable/disable in
  `modules.toml`. Also includes `reovim-module-emacs` skeleton personality module with
  `MODE_MANAGEMENT` capability and emacs keybindings. Phase 12 (#624):
  `reovim-driver-extension-loader` for TUI `.so` dynamic loading via `libloading` with
  `probe_extension_kind()` and `load_extension()` APIs; `declare_extension!` proc macro in
  `reovim-module-macros` generating FFI entry points with double-box fat-pointer pattern.
  Also adds `BuiltinManifest` filtering to module-config, `ModuleLoadReport` type to kernel API,
  and wires module-manager into the 40-module defaults bundle.

- **Language-aware bracket pair highlighting (#613)**: New `reovim-module-pair` server module
  and `reovim-tui-ext-pair` TUI extension providing rainbow bracket coloring (6-color depth
  cycling), matched-pair highlighting (bold + underline for innermost pair around cursor),
  unmatched bracket warning (red + underline), and context-aware auto-pair insertion (skips
  strings/comments). Language-agnostic design: treesitter language modules register per-language
  `BracketConfig` via `BracketConfigStore` in `reovim-driver-syntax`. Adds `SyntaxContext` enum
  (Code/String/Comment) and `context_at_byte()` to `SyntaxDriver` trait for syntax-aware
  auto-pairing. Extension bridge serializes bracket state to JSON for client rendering via
  `ExtensionStateBridge` with per-client scope.

- **Module/config/setting separation and integration (#610)**: Unifies three disconnected
  systems -- personality manifests (#585), user config (#586), and kernel OptionRegistry --
  into a coherent three-layer architecture (L1 loading, L2 config, L3 options). Phase 1
  removes legacy `SnippetParentMode`/`JumpParentMode` fallback paths, making `ModeBridgeStore`
  the sole cross-module mode parent mechanism. Phase 2 moves 9 vim options (scrolloff,
  ignorecase, number, etc.) from hardcoded `vim_option_specs()` into `vim.toml` personality
  manifest via new `ManifestOptionSpec` types in `reovim-driver-manifest`. Phase 3 adds typed
  extraction helpers (`get_bool`/`get_int`/`get_str`) with verbose `ConfigFieldError`
  diagnostics to `ModuleConfigStore`, and wires completion (pumheight/pumwidth overrides) and
  LSP (auto_start toggle) modules as config consumers. Phase 4 adds `$REOVIM_CONFIG_DIR`,
  `$REOVIM_DATA_DIR`, `$REOVIM_CACHE_DIR` environment variable overrides to `ConfigPaths`
  for worktree isolation, and consolidates bootstrap path helpers. Phase 5 introduces
  `ModuleLoadReport` service in `reovim-driver-module-loader` and three new `:checkhealth`
  diagnostic sections (Modules, Dependencies, Configuration). Phase 6 adds extension kind
  filtering to web client `createExtensions()`.

- **Treesitter language modules: Python, C, JavaScript, TypeScript (#525), JSON, TOML, Bash, Go
  (#526)**: Eight new treesitter syntax modules following the established self-registration pattern.
  Each module provides a `{Lang}SyntaxFactory` implementing `SyntaxDriverFactory` with pre-compiled
  highlight and fold queries, and a `Treesitter{Lang}Module` implementing `Module` for
  self-registration into `SyntaxFactoryStore` and `LanguageInfoStore`. TypeScript module handles
  dual grammars (TypeScript + TSX). All modules registered in `DefaultsModule` (47 total modules).
  Grammar versions pinned to tree-sitter 0.23.x for compatibility with workspace tree-sitter 0.24.

- **Recursive injection highlighting (#611)**: Replace flat `InjectionLayer` / `InjectionLayerStore`
  system with recursive `Box<dyn SyntaxDriver>` children managed by `InjectionManager`. Child
  drivers are created on-demand via `SyntaxDriverFactory` and can recursively detect and highlight
  their own injections (e.g., Rust doc comments -> Markdown -> fenced code blocks). Depth capped
  at `MAX_INJECTION_DEPTH` (4). Add `injection.combined` support for consecutive doc comment lines
  (`///`, `//!`) that are concatenated with prefix stripping before parsing as a single Markdown
  document, with coordinate translation back to parent document space. Add `set_injection_factory()`
  and `set_injection_depth()` to `SyntaxDriver` trait for configuring child driver creation and
  depth propagation. Delete `InjectionLayer`, `InjectionLayerFactory`, and `InjectionLayerStore`
  types.

- **Dynamic .so module loading with search paths and lock file (#587)**: New
  `reovim-driver-module-loader` crate provides safe dynamic loading of server modules from
  `.so` files. Search paths follow XDG conventions. Lock file prevents concurrent loading.
  Hot-reload support via file watcher with debounce. Integration with module-config for
  filtering. Full test coverage including error paths.

- **User module enable/disable and settings configuration (#586)**: New
  `reovim-driver-module-config` crate provides TOML parsing for
  `~/.config/reovim/modules.toml`. Controls which server modules and TUI extensions are loaded.
  Config file optional — missing file uses "official" preset (all enabled). Per-module settings
  accessible via `ModuleConfigStore` service in `ServiceRegistry`. `DefaultsModule` gains
  `builtin_registry()` factory map (29 modules), `builtin_order()` canonical ordering, and
  `create_modules_filtered()` predicate-based creation. Bootstrap wires config loading, store
  registration, and filtered initialization. Disabled modules do not register extension kinds.
  TUI extensions filtered consistently via `create_extensions_filtered()`. Full backwards
  compatibility: existing `create_modules()` and `create_extensions()` APIs unchanged.

- **Personality manifests replace vim adapter crates (#585)**: Introduce TOML-based personality
  manifests that declare keybinding tables and mode bridges as data, collapsing the O(N x M) adapter
  problem to O(M). New `reovim-driver-manifest` crate provides TOML parser, `ModeBridgeStore`, and
  validation. VimModule loads embedded `vim.toml` (20 keybindings, 2 mode bridges) via
  `include_str!`. Feature modules (snippet, range-finder) self-register parent modes by reading
  `ModeBridgeStore` with `optional_dependencies` on vim. `EnhancedFindCharCommand` relocated from
  `vim-range-finder` to `range-finder`. Six adapter crates deleted: vim-completion, vim-lsp,
  vim-explorer, vim-microscope, vim-range-finder, vim-snippet (-2145 lines, 35 -> 29 modules).

- **Cross-boundary extension contracts (#584)**: Server modules declare `extension_kinds()` on the
  `Module` trait (kernel-pure `&[&'static str]`). TUI extensions declare `server_kinds()`, web
  extensions declare `serverKinds()`. Server startup validates bridge-module contracts and logs
  warnings for orphaned bridges or module kinds (non-fatal, graceful degradation). `ListExtensions`
  RPC response includes `available_kinds` for client-side validation. TUI `validate_extensions()`
  and web `validateExtensions()` compare client extensions against server-declared kinds.
  Documentation at `docs/architecture/modules/extension-contracts.md`.

- **Client extension lifecycle and shared kinds (#583)**: Add `dependencies()`, `init()`, `exit()`
  lifecycle methods to `TuiExtension` trait and `WebExtension` interface. Both TUI and web clients
  topologically sort extensions via Kahn's algorithm before initialization. `shutdown_extensions()`
  calls `exit()` in reverse dependency order. New `shared/extension-kinds/` crate provides 13
  shared kind constants (`WHICHKEY`, `CMDLINE`, etc.) as single source of truth, replacing 50+
  magic string literals across 13 TUI extensions, 12 server bridges, TUI defaults, and 8 web
  extensions. Web equivalent `extension-kinds.ts` for TypeScript client.

- **Server module dependency resolution (#582)**: New `reovim-driver-depgraph` crate providing
  generic Kahn's topological sort for dependency resolution. 9 modules declare explicit
  `dependencies()` (vim, vim-microscope, vim-lsp, vim-completion, vim-explorer, vim-snippet,
  vim-range-finder, snippet, range-finder). Bootstrap uses toposort-ordered initialization
  instead of hardcoded Vec position. `on_all_loaded()` lifecycle hook wired for all Running
  modules. `TrackedModule` with `ModuleState` FSM tracks init state. Shadow-mode logging
  compares relative ordering of dependent pairs during transition.

- **Dot repeat command (#577)**: Implement the `.` (dot) command for replaying operator
  changes. Records the actual key sequence during operator+motion and operator+insert
  operations (e.g., `dw`, `dd`, `cwbar<Esc>`, `ccnew<Esc>`), then replays via `InjectKeys`
  on `.` press. Supports delete operators (`dw..`, `dd..`), change+insert operators
  (`cwbar<Esc>w.w.`, `ccnew<Esc>j.`), and standalone insert (`ihello<Esc>.`). Also fixes
  `cw` at end of buffer incorrectly deleting only the first character of the last word.

- **Configuration profiles module (#593)**: New `reovim-module-profiles` crate implementing
  `:profile-save`, `:profile-load`, `:profile-list` commands for named option snapshots.
  Profiles are stored as versioned TOML files with explicit type tags. Save captures
  non-default option overrides from the registry; load applies them with warnings for
  unknown/invalid options. Includes profile name validation (alphanumeric + hyphen/underscore,
  max 64 chars) and VFS-based I/O for testability. Integrated into the defaults module bundle.

- **Git provider architecture (#530)**: Shared provider pattern for cross-cutting
  git services. Phase A: `reovim-driver-git` crate defines `GitProvider` trait (7 methods:
  `current_branch`, `branches`, `status`, `log`, `stash_list`, `diff_hunks`, `blame`) with
  typed data structs and `GitProviderStore` for `ServiceRegistry` integration.
  `reovim-module-git` provides `SubprocessGitProvider` implementation via `git` CLI.
  Phase B: 4 picker modules consuming `GitProvider` -- `picker-git-branches` (static,
  lists branches with current marker), `picker-git-log` (dynamic, query-filters commits),
  `picker-git-status` (static, opens changed files), `picker-git-stash` (static, lists
  stash entries). Phase C: 3 consumer modules -- `git-signs` (gutter annotations for
  add/change/delete diff hunks), `git-statusline` (branch name statusline component),
  `git-blame` (on-demand blame annotations with toggle and per-path caching).
  `GutterRenderer` changed to interior mutability (`RwLock`) for multi-module registration.
  `AnnotationContext` extended with `file_path` for git-aware sources.
  Phase D: `CachedGitProvider` TTL-based caching wrapper (2s default) for
  `current_branch`, `status`, `diff_hunks`, and `blame`; pass-through for `branches`,
  `log`, `stash_list`.

- **`:set` ex-command (#572)**: Implement `:set` with vim-style syntax — `:set option`,
  `:set nooption`, `:set option!` (toggle), `:set option?` (query), `:set option=value`,
  `:set option&` (reset), `:set all`, `:set` (list changed). Supports short aliases,
  type-safe value parsing, constraint validation. Emits `OptionChanged`/`OptionReset`
  via EventBus for module notification and records `StateChanges` for client notification.

- **Completion and microscope options (#574)**: Register 7 options for plugin modules.
  Completion: `pumheight` (ph), `pumwidth` (pw) with range constraints. Microscope:
  `picker_height`, `picker_preview`, `picker_ignorecase`, `picker_border` (Choice type),
  `picker_prompt` (String with length constraint). All with `.with_owner()` ownership.

- **Standard vim/editor options (#573)**: Register 12 standard options with proper ownership,
  scope, and constraints. Vim module: `scrolloff`, `sidescrolloff`, `ignorecase`, `smartcase`,
  `hlsearch`, `incsearch`, `wrapscan` (7 Global options). Editor module: `tabstop`,
  `shiftwidth`, `expandtab`, `autoindent`, `textwidth` (5 Buffer-scoped options). Test helpers
  consolidated to use module-level option specs instead of per-test inline registrations.

- **Option picker with detail panel (#599)**: New `picker-options` module provides a fuzzy finder
  for editor options (`<Space>o`). Shows `[module] option_name` display with type/value detail.
  Preview panel shows full metadata: type, current value, default, constraint, scope, owner module,
  description, and available choices. Also fixes: command picker now shows qualified names with
  module prefix (e.g., `editor:save` instead of `save`), and preview panel now refreshes on
  navigation for all pickers.

- **Health-check diagnostic command (#594)**: New `:checkhealth` ex-command that opens a
  read-only scratch buffer with system diagnostics. Reports: reovim/API version and platform,
  registered LSP providers with active status, syntax highlighting factories, clipboard
  availability, and option registry state. Gracefully handles missing services. Part of
  Editor Customization Epic (#527).

- **Startup landing screen (#595)**: New TUI and web client extension that displays a centered
  overlay with ASCII art logo, version info, and quick-action hints on startup. Dismissed on
  any user interaction (cursor move, mode change, or buffer update). 16 unit tests. Part of
  Editor Customization Epic (#527).

### Changed

- **Depgraph crate relocation (#625)**: Move `reovim-driver-depgraph` from
  `server/lib/drivers/depgraph/` to `shared/depgraph/` and rename to `reovim-depgraph`.
  The crate is a zero-dependency generic topological sort algorithm used by both server
  (module loading) and client (extension ordering) — it belongs in `shared/`, not under
  `server/lib/drivers/`. No API changes.

- **Test helper deduplication (#569)**: Create `reovim_kernel::testing` module with
  shared `TestBufferManager`, `create_test_context()`, `setup_buffer()`, and
  `test_mode()`. Replaced 32 copies of `TestBufferManager`, 30 copies of
  `create_test_context()`, 17 copies of `test_mode()`, and 34 copies of
  `StubExecutor` across `server/modules/` with imports from shared modules.
  Made `StubExecutor` pub in `reovim_driver_session::testing`. Added
  `with_buffer_and_mode()` and `with_window()` constructors to
  `TestSessionRuntime`. Created tracking issues for ignored tests (#576, #577, #578).
- **Test organization standards (#569)**: Document test file layout rules (3 rules
  based on module structure and test size), shared helper usage, test naming
  convention (`test_{action}_{scenario}`), and ignored test policy in
  `docs/contributing/guides/testing.md`.
- **Inline test migration (#569)**: Migrate all inline `#[cfg(test)] mod tests`
  blocks from source files into dedicated test files following the three-rule
  layout standard. 142 files changed across 4 modules: textobjects (5 files),
  motions (5 files), editor (14 files), vim (44 files). Source files now contain
  only implementation code. Uses `#[path]` technique for 7 files with private
  item access. All 2540 module tests pass.

- **Test layout enforcement (#569)**: Add `scripts/check-test-layout.sh` to
  detect inline `#[cfg(test)] mod tests { ... }` blocks that violate the test
  organization standard. Integrated into `scripts/check.sh` as `test-layout`
  step (runs after fmt, before clippy/tests). Migrated final 4 inline test
  blocks from upstream code (bootstrap, main, set command, picker-options)
  using `#[path]` technique. Zero violations across the workspace.

- **Parallel check.sh (#588)**: Rewrite `scripts/check.sh` with parallel execution
  (clippy and tests run concurrently using separate CARGO_TARGET_DIR), CLI modes
  (`--quick`, `--sequential`, `--clean-cache`), structured report generation
  (`tmp/check-report.md`), and per-step log capture (`tmp/check-logs/`). Removes
  3 redundant steps and 4 stale package excludes. Also cleans stale excludes from
  `coverage.sh` and `coverage-all.sh`.

- **CI build cache fix (#588)**: Fix broken build cache in CI by using
  `cargo llvm-cov show-env --sh` in build steps to match RUSTC_WRAPPER fingerprints.
  Switch cache keys from `github.run_id` (never reused) to `hashFiles('Cargo.lock')`
  (cross-PR reuse). Replace `apt-get install protobuf-compiler` with
  `arduino/setup-protoc@v3`. Merge fmt into lint job, split MSRV to non-blocking
  parallel job checking key crates only (`reovim-app`, `reovim-kernel`, `reovim-server`).

- **CI pipeline parallelism (#588)**: Remove lint gate from build jobs so builds
  start immediately in parallel with lint (~70s critical path savings). Merge
  doc-tests into stable-checks job (shares MSRV toolchain, saves one runner).
  Extract `setup-coverage` composite action for DRY nightly+llvm-cov+protoc setup
  across 5 coverage jobs. Test jobs still require lint to pass before running.

- **CI test speed optimization (#588)**: Shard MC/DC unit tests across 2 runners
  using nextest `--partition count:N/2`, reducing critical path by ~40s. Add
  `.config/nextest.toml` with default and CI profiles (slow-timeout, fail-fast).
  Split monolithic test+report steps for timing visibility. Set
  `CARGO_PROFILE_DEV_DEBUG=line-tables-only` in CI for faster builds and smaller
  cache.

- **Enable notification integration tests (#578)**: Remove `#[ignore]` from 4
  notification E2E tests in `reovim-server`. Tests verify server connectivity,
  mode changes, buffer modification, and cursor movement via gRPC. CI already
  builds the server binary in `build-server` step so tests run automatically.

- **Mock clipboard provider (#576)**: Add `MockClipboardProvider` to
  `reovim-driver-clipboard` for headless CI testing. In-memory `ClipboardProvider`
  implementation using `RwLock<String>` that works without a display server.
  8 new driver tests, 5 new module-level mock tests. Display-dependent tests
  (1 unit, 6 integration) remain `#[ignore]` with tracking reference.

- **OptionSpec ownership tracking (#571)**: Add `owner: Option<ModuleId>` field to
  `OptionSpec` with `.with_owner()` builder, mirroring the `CommandId` ownership pattern.
  `OptionRegistry` gains `unregister_by_module()` for module lifecycle cleanup and
  `list_by_module()` for introspection. `OptionChanged`/`OptionReset` events extended
  with `scope: OptionScopeId` for runtime context. Vim module options now carry ownership.

### Removed

- **Orphaned OptionsModule (#575)**: Remove unused `reovim-module-options` crate. It was
  a pre-kernel leftover not loaded by `DefaultsModule` and conflicting with the kernel's
  `OptionRegistry` mechanism. If `virtualedit` support is needed, it should be registered
  as an `OptionSpec` in the vim module.

### Fixed

- **Markdown scroll crash (#565)**: Fix character/byte column mismatch in conceal
  rendering pipeline that caused panics when scrolling through markdown files.
  `apply_conceals()` was using character columns (from `byte_to_position()`) as
  byte indices for string slicing. Also fixes `ConcealedLine::identity()` sizing,
  `line_end_col()` return value, and background overlay column bounds — all were
  using byte lengths where character counts were expected.

### Added

- **Find-char repeat with extensible coordinator pattern (#563)**: Implement `;`
  (repeat last find-char same direction) and `,` (repeat reversed) using a two-command
  coordinator/execution split. `DISPATCH_FIND_CHAR` (motions module) records
  `FindCharState` then delegates to overridable `EXECUTE_FIND_CHAR` (vim module),
  enabling future providers (range-finder, easymotion) via `Priority::Override`.
  Policy-free `FindCharRecord` and `FindCharState` types in session driver follow
  `SearchState` precedent. Keybindings added to normal and all operator modes
  (delete, yank, change). `LastFind` removed from vim module (replaced by shared state).
- **Decoration system (#551)**: General-purpose decoration engine for markdown
  rendering with block and inline decorations. Language modules provide tree-sitter
  queries and declarative `DecorationRule` mappings; the `SyntaxDriver` trait
  exposes `decorations()` alongside `highlights()`.
  - Block decorations: heading icons, bullet concealment, checkbox replacement,
    blockquote bars, horizontal rules, code fence markers
  - Inline decorations: code span backtick concealment, link URL concealment,
    emphasis/bold/strikethrough highlighting
  - Dual-grammar support: `TreeSitterDriverBuilder::inline_decoration()` for
    secondary parsers (e.g., Markdown block + inline)
  - Session wiring: `decorations()` merged into `build_token_update()`,
    `notify_edit()`, and `send_full_refresh()`
  - Fixed `get_tokens()` and `stream_tokens()` gRPC handlers to transmit
    `AnnotationKind` (was hardcoding `kind: None`)
  - Insert mode cursor-line reveal: conceals bypassed on cursor line in insert
    mode (raw text visible while typing)
  - Cursor column remapping: visual cursor position accounts for conceal offsets
    via `source_to_display_col`
  - **Markdown extension extraction (Phase H)**: Extract ALL markdown rendering
    policy from `render_engine.rs` into `MarkdownRenderExtension` crate.
    Render engine is now pure mechanism — knows HOW to conceal/highlight/background
    but never WHICH categories trigger which behavior.
    - `RenderBehavior` enum moved to display driver as shared public type
    - 7 new `TuiExtension` trait methods: `classify_token()`, `on_buffer_update()`,
      `virtual_lines()`, `transform_line()`, `map_cursor_column()`,
      `on_cursor_update()`, `on_mode_change()`
    - New `reovim-tui-ext-markdown` crate: category-to-behavior mappings,
      table detection/layout/rendering, column mapping (68 tests)
    - Extension lifecycle hooks wired in notification handler
    - Hardware cursor accounts for virtual lines and extension column mapping
    - Strip inline markdown (`**`, `~~`, `` ` ``) from table cell display text
    - Fix `map_cursor_column()` to use stored buffer lines (was passing empty string)
    - Visual selection rendering maps columns through extensions for table alignment
- **TUI syntax highlighting (#548)**: Wire `AnnotationCacheManager` and `ThemeManager`
  into the TUI render engine for per-character syntax coloring. `render_line_content()`
  queries cached tokens for each line and resolves categories to styled colors via
  the theme system. Completes the last-mile gap: tree-sitter tokens now produce
  visible colored output in the terminal.
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

- **LSP hover, signature help, write + DidSave**: Three LSP features completing
  the core LSP integration on `reovim-lsp` branch.
  - **Hover command (K)**: `HoverCommand` handler in `lsp-navigation` module sends
    `LspRequest::Hover` with oneshot channel, formats all `HoverContents` variants
    (Scalar/String, Scalar/LanguageString, Array, Markup) via `format_hover_content()`,
    displays result through `NotificationState`. `has_hover_capability()` pure helper
    checks server capabilities. `K` keybinding in `vim:normal` mode via `vim-lsp` adapter.
  - **Signature help (`<C-k>`)**: `SignatureHelpCommand` handler sends
    `LspRequest::SignatureHelp`, formats active signature label via
    `format_signature_help()`. `has_signature_help_capability()` capability check.
    New `LspRequest::SignatureHelp` variant with oneshot response channel.
    `Client::signature_help()` async method. SignatureHelp client capability
    advertised with dynamic registration, documentation format, parameter info,
    and active parameter support. `<C-k>` keybinding in `vim:insert` mode.
  - **Write command (`:w`)**: Replace placeholder with actual VFS-backed
    implementation. Resolves target path (explicit arg or buffer's existing path),
    reads buffer content, writes via `VfsDriver::write_str()`, renames buffer on
    save-as, clears modified flag, emits `BufferSaved` kernel event.
  - **DidSave notification**: `LspRequest::DidSave` variant with URI and optional
    text. `Client::did_save()` sync notification method. Saturator `DidSave` handler.
    `LspModule` subscribes to `BufferSaved` kernel events and forwards `DidSave`
    notifications to all active LSP providers via `LspProviderRegistry`.
  - 433 tests across 5 affected crates, zero clippy warnings.

- **LSP extension pairs for hover and signature help (#550)**: Upgrade hover
  and signature help from toast notifications to dedicated TUI popup extensions
  using the `ExtensionStateBridge` + `TuiExtension` pattern.
  - **Server-side**: `HoverState` and `SignatureHelpState` session extensions
    store active content and origin position. `HoverBridge` and
    `SignatureHelpBridge` implement `ExtensionStateBridge` (client scope) with
    JSON serialization and auto-dismiss on mode change. Both registered via
    `BridgeProvider` in `lsp-navigation` module `init()`. `HoverCommand` and
    `SignatureHelpCommand` now set extension state instead of `notify_info()`.
    `hover_content_type()` detects PlainText vs Markdown from `HoverContents`.
  - **TUI hover extension**: `HoverExtension` renders multi-line bordered popup
    near cursor with content-type-aware border color (cyan=markdown,
    grey=plaintext). Popup width scales with terminal (max 60%, min 20 cols).
    Positioned below origin, falls back above or top.
  - **TUI signature help extension**: `SignatureHelpExtension` renders
    single-line bordered popup with yellow border above origin. Falls back
    below or top when no room above.
  - Both TUI extensions use typed `#[derive(Deserialize)]` for JSON parsing
    with `Origin::BufferPosition` deserialization.
  - 179 tests across 4 crates (118 lsp-navigation + 4 defaults + 37 hover
    + 24 signature-help), zero clippy warnings.

- **LSP diagnostics extension pair (#550)**: Inline diagnostic rendering via
  `DiagnosticBridge` (shared scope, tick-based) + `DiagnosticsExtension`.
  - **Server-side**: `DiagnosticSnapshot` and `DiagnosticPathIndex` (URI-to-buffer
    mapping) services. `DiagnosticBridge` reads `LspProviderRegistry` caches
    on `tick()`, resolves URIs to buffer IDs, and populates snapshot in shared
    `ExtensionMap`. `entries_eq` comparison avoids unnecessary notifications.
    `convert_severity` maps LSP severity enum to internal `DiagnosticSeverity`.
  - **TUI extension**: `DiagnosticsExtension` renders colored underlines on
    single-line diagnostics and right-aligned virtual text (severity prefix +
    message). Uses `ViewportContext::buffer_id` to filter for focused buffer.
    `ViewportContext` extended with `buffer_id: Option<u64>` field.
  - 118 tests across 3 crates (89 lsp + 4 defaults + 25 diagnostics), zero
    clippy warnings.

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
  directly. v2 enhancements: panic safety hardening (`ffi_catch_unwind` wraps
  all 36 `unsafe extern "C"` functions, returning `REOVIM_ERR_PANIC` on unwind).
  Clipboard API (4 functions: copy/paste for system clipboard and X11 primary
  selection, graceful degradation when unavailable). Register API (2 functions:
  get/set with `ReovimYankType` enum for characterwise/linewise distinction).
  Undo API (4 functions: undo, redo, can_undo, can_redo — edits auto-apply,
  FFI receives cursor position only; `record_edit` deferred to v3). Python
  bindings extended with 10 new methods covering clipboard, register, and undo.
  ABI version bumped to 1.2.0 (backward compatible). C header (`reovim.h`)
  updated with `ReovimYankType` enum and all new function declarations.
  333 unit tests (257 FFI + 76 Python), zero clippy warnings.
- **Range-finder client extensions and enhanced f/t motions (#535)**:
  - Phase 1: `ViewportContext` API on `TuiExtension` trait — `render_with_viewport()`
    provides scroll offset and content geometry for buffer-position-aware rendering.
    Default delegation to `render()` ensures backward compatibility.
  - Phase 2: TUI jump label extension renders labeled overlay at buffer positions
    with viewport-aware coordinate math (scroll subtraction, content offset).
    Two-char label styling with dim second character.
  - Phase 3: TUI fold extension with per-buffer fold state, hidden line range
    caching via `fold_hidden_lines()` trait method, and active-buffer context tracking.
  - Phase 4: Web client jump and fold extensions (`range-finder-jump.ts`,
    `range-finder-fold.ts`) mirror TUI implementations with DOM rendering.
  - Phase 5: Fold-aware render engine — `render_buffer_content` uses while-loop
    that queries `fold_hidden_lines()` to skip hidden lines, preserving screen
    real estate for visible content.
  - Phase 6: Enhanced f/t motions with multi-match jump label dispatch.
    `ExecuteFindChar` (vim module) detects multiple matches on a line and
    dispatches to `StartFindCharJumpCommand` (range-finder) via `CommandId`
    string — zero horizontal module coupling. Graceful fallback to first-match
    when range-finder module is not loaded. `start_with_matches()` on
    `JumpSessionState` bypasses two-char search, entering ShowingLabels directly.
    Match positions serialized as JSON to avoid type coupling between modules.
- **Polyblocks game module** (`reovim-module-tetromino`) - Loadable module architecture proof-of-concept (#537, #544)
  - Server-side: game engine, bridge, commands, modes (MENU/PLAY/PAUSED/LOBBY/ROOM/RESULT), key resolvers, keybindings
  - TUI extension (`reovim-tui-ext-tetromino`) - Renders board, active piece, next piece, hold piece, score, opponent view
  - Start with `<C-t>` in normal mode; hjkl/arrows for movement, Space for hard drop, p to pause, q/Esc to quit
  - Ghost piece (landing preview), hold piece (`c`), CCW rotation (`z`)
  - **Multiplayer** (#544): lobby system with room creation, ready-toggle countdown,
    per-client game state via `ExtensionMap`, shared `TetrominoLobbyState` for room tracking,
    opponent board snapshot in bridge context, tick-based countdown propagation to all players,
    forfeit detection on quit (surviving player wins), result screen with return-to-lobby
  - **Tick scheduler** (#544): `TickSchedulerHandle` in `ServiceRegistry` for gravity ticks,
    server-side `TickManager` spawns per-client tokio tasks, bridge `tick()` handles
    countdown, gravity, line clearing, and match-finished detection
  - Original game mechanics distinct from Tetris:
    - 8x16 board (not 10x20), 10 piece types including L-shapes and diagonals
    - Quadratic scoring `lines^2 * 50 * (level+1)`
    - Nudge rotation system (not SRS wall kicks)
    - History-4 randomizer with reroll (not 7-bag)
    - Exponential speed curve `900 * 0.85^level` (min 80ms)
    - Custom color palette: Amber/Teal/Rose/Sky/Lime/Violet/Coral/Sand/Mint/Slate
    - Soft drop 2pt/cell, hard drop 3pt/cell, no T-spin detection
  - **Bug fixes** (#544): countdown now propagates to all players via tick catch-up,
    quit during multiplayer match triggers forfeit (mark_dead + finish_match),
    mode stack preserved on quit by converting resolver ModeTransition::Set to
    ResolveResult::Execute (delegates to command handlers using safe set_mode)

### Changed

- **ArgValue::Bool semantic cleanup (#557)**: Add `Bool(bool)` variant to
  `ArgValue` and `ArgKind::Bool` to `ArgKind`, separating boolean flags
  (`linewise`, `find_inclusive`) from bang modifiers (`:q!`). Fix bridge
  mapping `InputArgValue::Bool` to `ArgValue::Bool` instead of `ArgValue::Bang`.
  Add `bool_flag()` accessor to `CommandContext`.

- **Error display pipeline (#558)**: Add `CmdlineMessage` enum (Error/Info) to
  `CmdlineState` for displaying ex-command errors and informational messages in
  the command-line area. Errors from `execute_ex_command()` (E492, command failures)
  now route through `CmdlineState.set_message()` instead of `tracing::warn!`.
  Messages are cleared on next normal-mode keypress. Bridge updated to serialize
  `message`/`message_kind` fields and report `is_active` when a message is present.

- **Tokenizer + ArgParser + ArgKind::Rest (#559)**: Add quote-aware `tokenize_args()`
  (double/single quotes, backslash escapes), `ArgError` enum with Display (E471/E488/E474),
  `bind_args()` for spec-driven argument binding, `raw_args` field on `ParsedCmdline`,
  and `ArgKind::Rest` variant for consuming all remaining text. Pure mechanism in the
  command driver layer.

- **Wire ArgSpec dispatch + update handlers (#560)**: Replace hardcoded argument
  population in `execute_ex_command()` with spec-driven `bind_args()` dispatch.
  Add `resolve_entry()` to `CommandNameIndex` for accessing command `ArgSpec`
  declarations at dispatch time. Add `args()` overrides to `EditCommand` (Rest),
  `WriteCommand` (Rest), `ColorschemeCommand` (Rest). Fix `ColorschemeCommand`
  handler to read `"theme"` instead of `"file"`. Resolve `WriteBufferCommand` /
  `WriteCommand` name collision by removing user-facing names from
  `WriteBufferCommand` (editor module) — `WriteCommand` (commands module) is now
  the canonical `:w`.

- **Prefix matching for ex-commands (#561)**: Add `resolve_prefix()` to
  `CommandNameIndex` for Vim-style prefix resolution (`:colo` resolves to
  `:colorscheme`, `:wri` to `:write`). Exact matches take priority over
  prefixes. Alias deduplication prevents false ambiguity (e.g., `w`/`write`
  are the same command). Returns `AmbiguousPrefix` error (E464) when a
  prefix matches multiple distinct commands. `execute_ex_command()` updated
  to use `resolve_prefix()` for all command resolution.
- **MC/DC coverage + E2E tests for ex-command system (#556)**: Achieve 100% MC/DC
  coverage on `name_index.rs` and `parse.rs` with targeted tests for tab separators,
  single-quote backslash handling, unclosed double-quote trailing backslash, and
  compound condition edge cases. Add 8 headless-capture integration tests for
  ex-command execution: `:write`/`:w`, prefix matching (`:wri`), unknown command
  error (E492), `:edit`, insert-then-write workflow, and sequential command execution.

- **Architecture**: Extract vim-mode coupling from feature modules into adapter crates.
  Five new `vim-*` adapter crates (`vim-microscope`, `vim-explorer`, `vim-completion`,
  `vim-range-finder`, `vim-snippet`) isolate vim-specific keybindings from feature
  modules. Feature modules (microscope, explorer, completion, range-finder, snippet)
  no longer hardcode `"vim:normal"` / `"vim:insert"` mode targets. Follows adapter
  pattern established by `vim-lsp` in #532. Defaults bundle updated to 33 modules.

- **Architecture**: Remove remaining vim-specific coupling from feature modules (#532)
  - **Adapter-injected parent modes**: `JumpParentMode` and `SnippetParentMode` config
    types let adapters inject parent mode IDs via `ServiceRegistry`. Feature modules
    (range-finder, snippet) read config at init instead of calling
    `find_by_name("vim", ...)`. Adapters init before their feature modules in defaults.
  - **Push/pop mode transitions**: Microscope, explorer, and tetromino use
    `push_mode`/`pop_mode` instead of hardcoding `set_mode(vim:normal)` for return
    transitions. Enables any editor personality to open/close these modules without
    vim dependency.
  - **Test-only cleanup**: Replace `ModuleId::new("vim")` with `ModuleId::new("test")`
    in test code across textobjects, completion, snippet, and range-finder.

- **Architecture**: Shared extension access and cross-client bridge context (#543)
  - Add `shared_ext()` / `shared_ext_mut()` to `ExtensionApi` trait (defaults return `None`)
  - Add `BridgeContext` struct for cross-client reads during bridge snapshot
  - Add `snapshot_with_context()` default method to `ExtensionStateBridge` (delegates to `snapshot()`)
  - Add `Session::with_bridge_context()` combined-lock helper (clients + state read locks)
  - Fix `ExtensionScope::Shared` snapshot TODO in notification builder

- **Architecture**: Unify command systems with Unix-style result and runtime signals (#547)
  - `CommandResult` simplified to `Success`/`Error` only (Unix exit code model)
  - Lifecycle side effects use `RuntimeSignal::Quit` through `SessionRuntime`
  - Ex-commands unified into `CommandHandler` (single command system)
  - `CommandNameIndex` provides name-based resolution with `complete()` delegation
  - `parse_cmdline()` pure function extracts parsing from mode dispatch
  - Re-entrant `CommandExecutor` with `get_handler()` + `Arc` pattern (max depth 16)
  - Signal queue on `SessionRuntime`: `signal()` to enqueue, `take_signals()` to drain
  - `should_quit` on `StateChanges` propagates quit signals through merge; `SendKeysResponse`
    protocol field signals client to disconnect; TUI stops event loop on quit response
  - `:q` checks `Buffer::is_modified()` on active buffer — returns E37 error for unsaved
    changes; `:q!` (bang) bypasses the check
  - Rename "ex-command" → "user command" in general system: `UserCommandEntry`,
    `COMMAND_SOURCE_USER`, `has_user_names()`, `list_user_commands()`; vim module
    retains "ex-command" as vim-specific concept
  - gRPC `CommandService` migrated from `ExCommandRegistry` to `CommandNameIndex`
  - FFI layers check signal queue after execution for backward-compatible quit codes
  - Deleted: `ExCommandHandler`, `ExCommandRegistry`, `ExCommandDispatcher`,
    `ExCommandHandlerStore`, `ExCommandQueryService`, `ExCommandInfo`

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

- **Undo corruption, missing f/F/t/T, broken r (#554)**: Three bugs fixed:
  (1) Undo after insert mode produced garbage content due to character-by-character
  position invalidation — fixed by collapsing batched edits into a single bulk edit
  at `end_batch()` time. (2) f/F/t/T find-char motions were non-functional because
  keybindings were never registered — added to normal and all operator modes, plus
  implemented resolver metadata transfer (`ResolveContext::ArgValue` to
  `CommandContext::ArgValue` bridge). (3) `r` (replace char) was a no-op due to wrong
  command ID binding and missing resolver interception — fixed binding, added
  `PendingCharOp::Replace` dispatch, implemented `ReplaceChar` handler.
  Also changed `CommandContext` args from `HashMap<&'static str, ArgValue>` to
  `HashMap<String, ArgValue>` to support dynamic metadata keys.

- **Cursor at invalid position after linewise delete (#552)**: Fix `dj`, `dk`, and
  `dd` on boundary lines leaving the cursor pointing to a deleted line. Added
  `cursor_after` field to `OperatorContext` so operators communicate their desired
  post-execution cursor position back to `execute_operator()`, with a clamped
  fallback for safety. Also fixed column off-by-one in `CursorUp`/`CursorDown`
  that allowed cursor one past the end of a line (now matches kernel `MotionEngine`).

- **DiagnosticBridge dead registry fix (#555)**: Fix `DiagnosticBridge` capturing
  `Arc<LspProviderRegistry>` and `Arc<DiagnosticPathIndex>` from a temporary
  `ServiceRegistry` created during `collect_bridges()` bootstrap, disconnected
  from the real session registries where LSP servers register. Made
  `DiagnosticBridge` a stateless unit struct that looks up services at tick time
  via a new `&ServiceRegistry` parameter on `ExtensionStateBridge::tick()`.
  `Session::with_tick_mut()` now passes the live session `ServiceRegistry` to
  tick callbacks. TetrominoBridge updated with the new signature (unused param).

- **LSP auto-start on file open and diagnostic tick (#564)**: Fix LSP servers
  only starting on `<C-Space>` completion trigger instead of when files are
  opened. `LspModule` now subscribes to `FileOpened` events: if an LSP provider
  is already active for the language, it sends `DidOpen`; otherwise it auto-starts
  the server via `LspLifecycle`. Diagnostic tick (`TickSchedulerHandle.start()`)
  is now called after successful LSP server registration in `LspAutoStarter`,
  enabling `DiagnosticBridge` to poll diagnostics on a 500ms interval. Shared
  helpers `find_project_root`, `language_id_from_path`, and `config_for_language`
  moved from completion module to `reovim-driver-lsp` for cross-module reuse.

- **Ex-command VFS propagation (#547)**: Fix `:e` command failing with "VFS not
  available" when invoked via keyboard (ex-command path). `execute_ex_command`
  in vim mode.rs was building a fresh `CommandContext` without propagating the
  VFS driver from the outer context. Now propagates both `buffer_id` and `vfs`.

- **LSP gd/gr deadlock (#532)**: Fix thread starvation that caused `gd` and `gr`
  commands to always fail with "LSP request failed". `recv_timeout` blocked the
  tokio worker thread, preventing the saturator's `tokio::select!` loop from
  processing the request. New `recv_response()` helper in `driver-lsp` wraps
  blocking waits in `tokio::task::block_in_place`, yielding the worker thread
  to other async tasks. Requests now complete in ~2ms instead of timing out.

- **Dedicated LSP traffic logger (#532)**: New `LspLogger` in `driver-lsp`
  writes LSP JSON-RPC traffic, server stderr, and lifecycle events to a
  dedicated log file. Activated via `REOVIM_LSP_LOG=1` (writes to
  `~/.local/share/reovim/lsp-{language}.log`) or `REOVIM_LSP_LOG=/path/to/dir`.
  Log format: `[HH:MM:SS.mmm] [lang] --> method params` for outgoing,
  `<--` for responses, `<-n` for notifications, `err` for stderr.


- **Client model: SemanticOrigin replaces Anchor (#549)**: Replace policy-violating
  `Anchor` positioning directive with informational `SemanticOrigin` metadata in
  `reovim-client-model`. `SemanticOrigin` (4 variants: `BufferPosition`, `BufferRange`,
  `Buffer`, `Session`) tells clients WHAT data relates to — each client independently
  decides WHERE to render. New `ExtensionCategory` enum (`Overlay`, `Inline`) classifies
  extensions informatively. `LogicalOverlay` field changed from `anchor: Anchor` to
  `origin: Option<SemanticOrigin>`. Web client's `positionOverlay()` recontracted to
  interpret origin informationally. WASM package regenerated with updated TypeScript types.
  Dead code removed: `Anchor` enum, `OverlayRenderer`/`OverlayManager` traits, TUI
  anchor/overlay adapter modules.


### Security

### Removed

- **`Anchor` enum removed (#549)**: `wire::Anchor` (5 variants: Cursor, Center, Buffer,
  Screen, Below) removed from client model. Was a policy-violating positioning directive.
  Replaced by informational `SemanticOrigin`.
- **`OverlayRenderer` and `OverlayManager` traits removed (#549)**: Policy traits
  removed from `traits::overlay`. Clients use the `ExtensionStateBridge` pattern instead.
- **TUI anchor/overlay adapter modules removed (#549)**: `adapter::anchor` and
  `adapter::overlay` were dead code (no extension used them).

---

## Version History

- v0.9.5 - Completion engine, snippet system, explorer, microscope, range-finder, notifications - see [CHANGELOG-0.9.5.md](changelog/CHANGELOG-0.9.5.md)
- v0.9.3 - Per-client architecture, TUI unification, presence rendering, integrated mode - see [CHANGELOG-0.9.3.md](changelog/CHANGELOG-0.9.3.md)
- v0.9.2 - Cmdline UI, themes, annotations, gRPC v2, web client, syntax service - see [CHANGELOG-0.9.2.md](changelog/CHANGELOG-0.9.2.md)
- v0.9.1 - Phase 7: E2E Test Suite & Mechanism/Policy Separation - see [CHANGELOG-0.9.1.md](changelog/CHANGELOG-0.9.1.md)
- v0.9.0 - New architecture (lib/arch, lib/kernel, lib/drivers/*) - see [CHANGELOG-0.9.0.md](changelog/CHANGELOG-0.9.0.md)
- v0.8.x and earlier - Legacy crates (lib/core, lib/sys, plugins) - see [CHANGELOG-archive.md](changelog/CHANGELOG-archive.md)
