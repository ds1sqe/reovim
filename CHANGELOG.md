# Changelog

For old changelog, see `changelog/CHANGELOG-{version}.md`

## [0.15.0-dev] - Unreleased

### Breaking Changes (Internal)

- **server**: `reovim-domain-text`, `reovim-domain-text-events`, and `reovim-provider-text` removed from `reovim-server` entirely (both `[dependencies]` and `[dev-dependencies]`). Server is now a domain-agnostic dispatch layer with zero domain crate imports in production and test code. All text-domain types accessed via driver re-exports (`reovim-driver-session`, `reovim-driver-buffer`, `reovim-driver-codec`). Depgraph guard test `server_no_domain_text` checks all dependency kinds (normal + dev)
- **server**: `ResolveResult::InsertChar` no longer handled by server dispatch. Resolvers now insert characters directly through `SessionApiDyn`. Server `InsertChar` arm is defensive no-op with error logging
- **server**: `FallbackContext` impl removed from `AppState`
- **kernel**: `reovim-domain-text` removed from kernel dependency graph entirely. ~5,190 lines of text-domain tests relocated
- **kernel**: `UnsavedBuffer.line_count: usize` replaced with `content_size: u64`
- **kernel**: `CommandRegistration` capability flags renamed to domain-neutral vocabulary
- **drivers**: 12 pure-contract driver crates renamed from `reovim-driver-*` to `reovim-subsys-*` and moved from `server/lib/drivers/` to `server/lib/subsys/`. Affected crates: annotation, clipboard, completion, formatter, git, layout, manifest, module-config, module-loader, module-registry, statusline, vfs. All `use reovim_driver_{name}` imports must change to `use reovim_subsys_{name}`

### Added

### Changed

### Fixed
