# Changelog

For old changelog, see `changelog/CHANGELOG-{version}.md`

## [Unreleased] - v0.9.4-dev

### Added

- **Code coverage infrastructure**: Added `scripts/coverage.sh` for local
  coverage with four modes (`line`, `branch`, `mcdc`, `server`) using
  `cargo-llvm-cov`. CI generates MC/DC coverage for non-server crates and
  line coverage for `reovim-server`, merged into a single Codecov upload.
  Codecov PR annotations show coverage impact (informational, non-blocking).
  `reovim-server` excluded from branch/MC/DC due to LLVM bug
  [#119558](https://github.com/llvm/llvm-project/issues/119558)
  (`getInstantiationGroups` SIGSEGV on branch coverage data from
  `#[tonic::async_trait]` service implementations). Use `server` mode for
  text-only branch coverage of the server crate.

- **Floating cursor labels for remote clients (#474)**: TUI and Web clients now
  show a colored background overlay above each remote client's cursor, making it
  easy to identify who is editing where. TUI labels use `apply_style` to preserve
  buffer content underneath (colored background without hiding text). Labels use
  the CBF-8 colorblind-friendly palette and truncate long names via Unicode-safe
  `truncate_end()`. Web labels replace the previous browser-native tooltip with
  a persistent floating `<div>`.

### Fixed

- **One-way presence visibility (#474)**: When Client B joined after Client A,
  A could not see B's cursor. Root cause: `ClientPresence::new()` initialized
  `buffer_id: None`, so the `PresenceJoined` notification lacked the buffer_id.
  A's render engine skipped B (different-buffer filter). Fix: server now reads
  the new client's active window buffer_id before broadcasting the notification.

- **Resize propagation to all TUIs (#474)**: `ResizeRequestPayload` had no
  `target_client_id` field, causing all TUI clients to resize when any one
  client resized. Fix: added `target_client_id` to the proto, server sets it
  from the authenticated token, TUI handler filters by target. Also reordered
  `connect_common()` to call `resize()` after `join()` so the token is attached.

### Security

- **Connection-bound client identity (#483)**: Replaced self-reported `client_id`
  request fields with server-side token-based authentication. `Join()` returns a
  `session_token`; clients send it via `x-reovim-token` metadata header; the
  `AuthInterceptor` resolves tokens to `ClientId` server-side. Caller-identity
  RPCs (`send_keys`, `leave`, `update_presence`, `set_sync_mode`) now require
  token authentication — the body `client_id` field is removed and reserved.
  State-query RPCs (`get_mode`, `get_cursor`, `get_layout`) use the token for
  caller authentication while the body `client_id` selects the target (0 = self).

### Removed

- **Dead `CmdlineBuffer` from `AppState` (#452)**: Removed unused `CmdlineBuffer`
  field from server-level `AppState` and deleted `app/cmdline.rs`. The cmdline
  buffer was superseded by `CmdlineState` in the driver layer during v0.9.2.
  This completes the cmdline state consolidation started in #452.

---

## Version History

- v0.9.3 - Per-client architecture, TUI unification, presence rendering, integrated mode - see [CHANGELOG-0.9.3.md](changelog/CHANGELOG-0.9.3.md)
- v0.9.2 - Cmdline UI, themes, annotations, gRPC v2, web client, syntax service - see [CHANGELOG-0.9.2.md](changelog/CHANGELOG-0.9.2.md)
- v0.9.1 - Phase 7: E2E Test Suite & Mechanism/Policy Separation - see [CHANGELOG-0.9.1.md](changelog/CHANGELOG-0.9.1.md)
- v0.9.0 - New architecture (lib/arch, lib/kernel, lib/drivers/*) - see [CHANGELOG-0.9.0.md](changelog/CHANGELOG-0.9.0.md)
- v0.8.x and earlier - Legacy crates (lib/core, lib/sys, plugins) - see [CHANGELOG-archive.md](changelog/CHANGELOG-archive.md)
