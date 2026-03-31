# Changelog

For old changelog, see `changelog/CHANGELOG-{version}.md`

## [Unreleased] - v0.14.4-dev

### Added

- **kernel**: rope data structure (`mm/rope.rs`) — custom B-tree with O(log n) insert/delete, O(1) clone via `Arc` structural sharing, and O(log n) position conversion. Zero external dependencies. (#711)

### Changed

- **kernel**: `Buffer` internal storage replaced from `Vec<String>` to `Rope` — all public API signatures unchanged, `Buffer::clone()` is now O(1) (~16ns vs ~8ms for 100K-line buffer) (#711)
- **kernel**: `BufferSnapshot` and `block::Snapshot` now use `Rope` clone instead of `Vec<String>` deep copy — snapshot creation is O(1) (#711)
- **kernel**: rope uses line-separator semantics — trailing `\n` produces an empty line in `line_count()` and `line()`, matching editor expectations for delete+insert patterns (#732)
- **search**: `position_to_byte`/`byte_to_position` now use Buffer's O(log n) rope methods instead of local O(n) helpers — eliminates 2-4 content() materializations per search (#711)
- **session**: `buffer_text_range()` uses byte-indexed `&str` slicing instead of `Vec<char>` per-line allocation (#711)

### Fixed

- **input**: remove gratuitous `Box::leak` in `handle_pop_result` — `CommandContext::set()` accepts `&str`, no static lifetime needed (#714)
- **motions**: `ge` and `gE` now correctly distinguish Word vs BigWord boundaries — `word_end_backward` was ignoring the `WordBoundary` parameter (#720)
- **session**: `close_window` and compositor close now use `WindowLayout::remove()` instead of bypassing it — fixes stale `active_index` returning wrong window after close (#712)
- **editor**: `display_line_count` now uses Unicode display width instead of char count — CJK characters correctly occupy 2 terminal columns (#717)
- **kernel**: `CommandId.name` and `OperatorId.name` changed from `&'static str` to `Cow<'static, str>` — `from_qualified()` no longer leaks memory via `Box::leak` on every call (#713)
- **vfs**: replace lossy `From<io::Error>` with `VfsError::from_io(err, path)` — error messages now include the file path instead of showing empty string (#715)
- **vim**: `load_personality_manifest` now propagates parse errors instead of swallowing them — module correctly fails to init if vim.toml is corrupted (#718)
- **lsp**: monitor LSP saturator task with watcher — if the task panics, `active` flag is cleared and user is notified instead of silently losing all LSP features (#719)
- **trace**: `init_profiling_with_filter` now uses the filter argument via `TracingProfiler::with_filter` instead of discarding it (#716)
- **kernel**: `OptionRegistry::register` holds write lock for entire check-and-insert, fixing TOCTOU race between read and write locks (#721)
- **testing**: `IntegrationTest` and `StepTest` now query registers via gRPC instead of returning empty HashMap — `assert_register!` macro is usable (#722)

## [0.14.3] - 2026-03-30

### Added

- **statusline**: diagnostic counts section (Section X) with Nerd Font error/warning/info/hint icons — only shown when counts > 0
- **statusline**: git branch icon (), modified icon (●), readonly icon (󰌾) replace plain text indicators
- **statusline**: theme-aware colors via `statusline_bg`/`statusline_fg` highlight groups cached in `init()`
- **bufferline**: Nerd Font pin icon (󰐃) and modified dot (●) replace ASCII `*` and `[+]`
- **bufferline**: theme-aware colors via highlight groups cached in `init()`
- **microscope**: file type icons rendered next to picker results
- **display**: `DiagnosticPresenter` registered in bootstrap — diagnostic gutter icons now appear
- **display**: `mode_replace` highlight group added to all 3 builtin themes (Dark, Light, TokyoNight)
- **illuminate**: word reference highlighting on cursor-hold — after 300ms idle, all occurrences of the word under cursor are highlighted with `]]`/`[[` navigation
- **syntax-treesitter**: injection decorations — doc comments (`///`, `//!`) now inherit markdown decorations from child drivers (#696)
- **hover**: markdown rendering in hover popups — bold, italic, code spans, headings, fenced code blocks, and horizontal rules are styled instead of shown as raw syntax (#697)
- **microscope**: syntax highlighting in preview panel — file previews show tree-sitter-based syntax colors for all supported languages (#698)
- **syntax-treesitter**: bare fenced code blocks in doc comments inherit parent language — ` ``` ` without a language tag in Rust doc comments defaults to Rust highlighting (#701)
- **keybindings**: `LeaderKeyProvider` service and in-crate personality adapters — feature modules (git-signs, diagnostics-panel, bufferline, etc.) register vim-aware keybindings with qualified `"vim:normal"` mode strings and `<leader>` expansion at bootstrap (#700)
- **lsp**: integration tests for hover (`K`) and goto-definition (`gd`) against real rust-analyzer (#692)

### Fixed

- **viewport**: overlapping syntax tokens now resolve to the most specific (narrowest) match — fixes injection highlights being hidden by broader parent tokens (#696)
- **session**: split window inherits cursor and viewport from source — `<C-w>v` / `<C-w>s` no longer reset new pane to line 0
- **range-finder**: `;`/`,` repeat after label-selected `f`/`F`/`t`/`T` now advances to the next match instead of re-entering label mode (#663)
- **server**: operator-pending `ds`/`cs`/`ys` with leap motions now completes — `on_command_complete` fires after pop-result command execution (#663)
- **gutter**: add generic column priority for ordering — columns now sort by `priority` field (lower = further left) instead of Vec insertion order
- **tui**: dispatch `OptionChanged` notifications to client modules — `:set nu` and `:set rnu` now toggle line numbers in real-time
- **lsp**: non-blocking hover popup — `K` (hover) returns immediately via fire-and-forget async + ArcSwap cache + tick polling, no longer freezes editor for up to 5 seconds
- **hover**: dismiss popup on cursor movement in normal mode via `on_cursor_update()`
- **hover**: fix popup requiring two `K` presses — async task no longer kills the tick consumer before it can deliver the cached LSP result
- **hover**: fix crash on multi-byte characters (box-drawing `─` from markdown horizontal rules) — truncation now uses character-aware slicing instead of byte offsets (#692)
- **hover**: `display_width()` returns character count instead of byte length for correct popup sizing (#692)
- **lsp**: stabilize flaky diagnostic count assertion — relaxed to `>= 1` with at least one error check (#692)
- **cmdline**: `:s/pattern/replacement/` now dispatches correctly — ex-command parser splits name at first non-alpha character instead of whitespace (#705)
- **bufferline**: `H`/`L` buffer prev/next, `:bnext`/`:bprevious`/`:bd`/`:qa` ex-commands, `<Space>bd` binding (#699)
- **motions**: screen-high/low relocated from `H`/`L` to `gH`/`gL` (#699)
- **window**: `<C-h/j/k/l>` direct window focus navigation in normal mode (#699)
- **lsp-navigation**: diagnostic navigation `]d`/`[d`, `]e`/`[e`, `]w`/`[w` for next/prev diagnostic/error/warning (#699)
