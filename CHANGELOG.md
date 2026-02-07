# Changelog

For old changelog, see `changelog/CHANGELOG-{version}.md`

## [Unreleased] - v0.9.4-dev

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
