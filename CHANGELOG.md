# Changelog

For old changelog, see `changelog/CHANGELOG-{version}.md`

## [Unreleased] - v0.14.1-dev

### Fixed

- **gutter**: add generic column priority for ordering — columns now sort by `priority` field (lower = further left) instead of Vec insertion order
- **tui**: dispatch `OptionChanged` notifications to client modules — `:set nu` and `:set rnu` now toggle line numbers in real-time
- **lsp**: non-blocking hover popup — `K` (hover) returns immediately via fire-and-forget async + ArcSwap cache + tick polling, no longer freezes editor for up to 5 seconds
- **hover**: dismiss popup on cursor movement in normal mode via `on_cursor_update()`
