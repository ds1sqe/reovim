# 8.2 — Platform Runtimes

**Scope.** Per-platform client runners: TUI, web, native (future).
What a platform runtime owns, the launcher's `run()` entry point,
and the platform-specific config surface.

**Locked rules.** None new; references CL3.

---

## 1. Platform runtime contract

A platform runtime crate at `ext/client/platforms/<p>/` exports:

```rust
pub fn run(args: PlatformArgs) -> Result<(), PlatformError>;
```

This is the entry point the launcher (`apps/reovim`) dispatches to.
The runtime owns:

- the `ClientRuntime` (per-platform registry of drivers, modules,
  capabilities discovered from the library root),
- input acquisition (TUI: terminal events; web: DOM events; ...),
- output emission (TUI: raw `termios` + ANSI escape sequences via
  `arch/`-owned FFI; web: DOM/SVG; ...),
- transport client (framed-protocol client or in-memory adapter
  from embedded mode),
- platform-specific debug surface (8.4).

## 2. Platforms

| Platform | Crate | Status |
|---|---|---|
| TUI | `ext/client/platforms/tui/` | Active (#753) |
| Web | `ext/client/platforms/web/` | Active (#753 Phase E.1) |
| Native | `ext/client/platforms/native/` | Out of target |

The first realization (#797) ships the TUI as a single thin crate at
`ext/client/platforms/tui/`: the UDS carrier, the framed handshake,
and the raw-`termios`/ANSI paint live together until the second
platform or driver consumer arrives; the 8.1 `clients/lib/subsys/*`
split and the 8.3 cdylib tiers grow out of that crate, not beside it.
The thin crate is the documented predecessor, not a competing design.

The TUI runtime registers a terminal-restore callback in the `arch/`
pre-exit seam before entering raw mode: the panic path (6.2 §5)
invokes registered pre-exit callbacks before rendering its final
output, so an aborting client never leaves the controlling terminal
in raw mode and the panic line prints in cooked mode.

## 3. Module/driver/capability discovery

The platform runtime calls `ClientRuntime::discover()` over the
library root's `client/` subdir. Discovered cdylibs:

- modules → `ClientModuleRegistry`,
- drivers → `ClientDriverRegistry`,
- capabilities → `CapabilityRegistry`.

All registrations are tracked with `owner_cdylib_id` (LF8 mirror
on the client side).

## 4. Embedded vs subprocess vs external

Per 1.3 §2:

- **Embedded.** Launcher process holds the Kernel and the platform
  runtime side-by-side; transport is in-memory adapter.
- **Subprocess.** Launcher forks `reovim-server` and the platform
  binary. Transport is the configured framed-protocol profile.
- **External.** Launcher only calls into the platform runtime;
  server is a long-running external process.

In all three, the platform runtime contract is the same `run(args)`
entry. The runtime selects the transport implementation based on
the args.

## 5. Platform manifest (TODO)

Each platform may ship its own per-platform config schema (1.5):

- `[platform.tui]` — terminal capabilities, scrollback (a sub-section
  of `kernel.shell.[ui]` perhaps; or a participant `platform.tui`
  with its own schema).
- `[platform.web]` — bundle preferences, DOM target IDs.

The cleanest model: each platform is its own config participant
under `platform.<p>`. The README leaves the exact placement
open.

## Open items

1. Platform-as-config-participant decision (§5).
2. Whether the platform runtime can be hot-swapped during a session
   (e.g. attach via TUI then switch to web). Default no.
3. Native platform runner — out of target.

## Conformance

This chapter has no rules. Platform-specific behaviours are
fixtured per-platform in their own crates' tests.
