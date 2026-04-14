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
- **drivers**: Session contract types extracted from `reovim-driver-session` into new `reovim-subsys-session` crate. Moved items: ClientId, Viewport, CursorSnapshot, KeySequence, SessionExtension, ExtensionMap, SessionMode, EmptySessionHandler, SessionHandlerKey/Registry, InitialModeProvider, LeaderKeyProvider, PendingNotificationQueue, StaleCheck, TickScheduler, ClipboardApi, ExtensionApi, FindCharRecord, CompositorApi, BridgeRegistry/Provider. Driver-session re-exports all items for backwards compatibility
- **drivers**: Syntax contract types extracted from `reovim-driver-syntax` into new `reovim-subsys-syntax` crate. All core traits and types moved (SyntaxDriver, SyntaxDriverFactory, LanguageRegistry, SyntaxCache, SyntaxEdit, HighlightCategory, Annotation, FoldRange, etc.). Only the domain-specific bridge function `text_event_to_syntax_edit` remains in driver-syntax. Driver-syntax re-exports all items for backwards compatibility
- **drivers**: `reovim-driver-command-types` renamed to `reovim-subsys-command-types` and moved to `server/lib/subsys/command-types/`. Domain dependency removed: `CommandContext::cursor_position()` now returns `Option<(usize, usize)>` instead of `Option<Position>`, `set_cursor_position()` now takes `(line: usize, col: usize)` instead of `Position`. All `use reovim_driver_command_types` imports must change to `use reovim_subsys_command_types`
- **drivers**: Command metadata traits extracted from `reovim-driver-command` into new `reovim-subsys-command` crate. Moved items: Command, CommandPriority, CommandInfo, CommandQueryProvider, CommandQueryService, AmbiguousPrefix, CommandNameIndex, bind_args, parse_cmdline, tokenize_args. CommandHandler, CommandHandlerStore, and CommandProvider remain in driver-command (they reference SessionRuntime). Driver-command re-exports all items for backwards compatibility
- **drivers**: Domain-free input contract types extracted from `reovim-driver-input` into new `reovim-subsys-input` crate. Moved items: KeyCode, KeyEvent, KeyEventKind, KeymapResult, Modifiers, MouseButton, MouseEvent, MouseEventKind, ClipboardError, InputError, ClipboardProvider, KeySequence, Keybinding, KeybindingTarget, BindingLayer, EagerLookupPolicy, KeyLookupPolicy, KeyLookupResult, KeyLookupState, KeymapQuery, ModeLifecycleHandler, NopLifecycleHandler, DefaultModeProvider, ProviderPriority, DefaultModeProviderModule, ModeProviderKey, ModeProviderRegistry, ModeInfo, ModeInfoStore, KeybindingStore, LookupPolicyStore, PendingBindings, BindingInfo. Driver-input re-exports all moved items for backwards compatibility; resolver, fallback, and resolver_registry remain in driver-input

- **server**: `reovim-driver-undo` and `reovim-driver-search` removed from server dependencies (zero production usage). Server imports for subsys-available types switched from driver crates to subsys crates (subsys-session, subsys-command, subsys-command-types, subsys-input, subsys-syntax). Remaining driver deps (session, input, command, buffer, codec, syntax) are domain-locked and documented for Tier 2 decoupling
- **drivers**: `reovim-subsys-syntax` dissolved back into `reovim-driver-syntax`. Syntax is a text-domain concept and does not belong in the domain-neutral subsys layer. All source modules moved from `server/lib/subsys/syntax/` into `server/lib/drivers/syntax/`. All `use reovim_subsys_syntax` imports must change to `use reovim_driver_syntax`

### Added

- **depgraph**: Layer enforcement tests — `subsys_no_domain`, `subsys_no_driver`, `subsys_no_name_leak`, `server_no_domain`, `server_no_driver` guard the subsys/driver layer separation
- **subsys**: New `reovim-subsys-coordination` crate — domain-neutral Position and Cursor traits with `[u8; 8]` header codec pattern (`domain_id(u32) + inner_id(u16) + flags(u16)`), PositionCodec/CursorCodec decode traits, CoordinationRegistry for dynamic driver enlistment. Zero dependencies. Enables multi-domain editing (text, mesh, image) without server changes

### Changed

### Fixed
