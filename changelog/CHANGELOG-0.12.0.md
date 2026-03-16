# Changelog - v0.12.0-dev

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

