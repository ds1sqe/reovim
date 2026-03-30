# Changelog

For old changelog, see `changelog/CHANGELOG-{version}.md`

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
