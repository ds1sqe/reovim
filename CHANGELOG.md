# Changelog

For old changelog, see `changelog/CHANGELOG-{version}.md`

## [Unreleased] - v0.14.2-dev

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

### Fixed

- **session**: split window inherits cursor and viewport from source — `<C-w>v` / `<C-w>s` no longer reset new pane to line 0
- **range-finder**: `;`/`,` repeat after label-selected `f`/`F`/`t`/`T` now advances to the next match instead of re-entering label mode (#663)
- **server**: operator-pending `ds`/`cs`/`ys` with leap motions now completes — `on_command_complete` fires after pop-result command execution (#663)
- **gutter**: add generic column priority for ordering — columns now sort by `priority` field (lower = further left) instead of Vec insertion order
- **tui**: dispatch `OptionChanged` notifications to client modules — `:set nu` and `:set rnu` now toggle line numbers in real-time
- **lsp**: non-blocking hover popup — `K` (hover) returns immediately via fire-and-forget async + ArcSwap cache + tick polling, no longer freezes editor for up to 5 seconds
- **hover**: dismiss popup on cursor movement in normal mode via `on_cursor_update()`
- **hover**: fix popup requiring two `K` presses — async task no longer kills the tick consumer before it can deliver the cached LSP result
