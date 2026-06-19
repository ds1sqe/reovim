# 8.2 — Platform Runtimes

**Scope.** Per-platform client runners: TUI, web, native (future).
What a platform runtime owns, the launcher's `run()` entry point,
and the platform-specific config surface.

**Locked rules.** None new; references CL3.

---

## 1. Platform runtime contract

A platform runtime crate at `client/platforms/<p>/` exports:

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
| TUI | `client/platforms/tui/` | Active (#753) |
| Web | `client/platforms/web/` | Active (#753 Phase E.1) |
| Native | `client/platforms/native/` | Out of target |

The first realization (#797) ships the TUI as a single thin crate at
`client/platforms/tui/`: the UDS carrier, the framed handshake,
and the raw-`termios`/ANSI paint live together until the second
platform or driver consumer arrives; the 8.1 `client/lib/subsys/*`
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

## 6. Client I/O — design direction (deferred)

This section records the settled vocabulary and the deferred capability
split. It is a **design direction, not an asserted contract**; the
formal capability seam earns its formalization at the second concrete
source/surface (bare-metal HID + framebuffer, or web DOM + canvas).

### 6.1 Capability mediation

The client reaches display, input, and output through
**client-specific capability contracts** — not the platform handle
(`06-ABI/05-Platform-Contract.md`) and not `kabi/device` directly.
Same reasoning as the device bridge (§4.6 of the kernel model): I/O
is a different axis, only the client needs it, and folding it into the
platform handle would bloat the Math-layer core.

### 6.2 Two mode-invariant vocabularies (settled)

These two vocabularies are settled. Everything below them is
mode-varying.

- **Input = raw-input records.** `RawInput{kind, payload}` over USB
  HID keycodes + extension range. HID is the canonical shape; every
  source normalizes into it. See `05-View/02-Raw-Input.md`.
- **Output = the frame/cell model.** A grid of cells (glyph + attrs
  + cursor). The client states "cursor at (r,c), shape X"; the
  surface driver translates to hardware cursor (terminal), drawn cell
  (framebuffer), or DOM element. See `05-View/03-Projections.md`.

Input and output are **independent axes with independent device
lifecycles.** A keyboard unplug does not imply display mode-loss;
the contracts stay distinct even when one platform driver
co-implements both.

### 6.3 Mode-varying layers

```
        client kernel  [Math, mode-invariant]
        raw-input pipeline  +  frame/cell render  +  module host
   ┌──────────────┴───────────────┐
   ▼ INPUT capability             ▼ OUTPUT capability
   yields raw-input records       accepts a frame/cell model
   ┌──────────────┐               ┌──────────────┐
   │ source driver │ (mode-vary)  │ surface driver│ (mode-vary)
   └──────┬───────┘               └──────┬───────┘
   bare metal: HID-decode          bare metal: cell→framebuffer blit
   terminal:   termios-parse       terminal:   cell→ANSI compose
   web:        DOM KeyboardEvent   web:        cell→DOM/canvas
          │ bottoms out on…               │ bottoms out on…
   bare metal → kabi/device (HID / fb) → SYSTEM KERNEL
   hosted     → platform vtable fd-I/O  → host terminal/window
```

On bare metal the client is **machine-affected too**: its I/O
capability drivers bottom out on the system kernel's devices (HID,
framebuffer) via `kabi/device`, so the system kernel sits below
both the editor and the client. On hosted targets, drivers bottom out
on platform-handle fd-I/O.

### 6.4 Rule-of-three status (honest)

| Side | Shipped contract | Live implementors |
|---|---|---|
| Output | `RenderProjector` (`editor/lib/subsys/domain/src/contract.rs`) | 1 — `TextProjector` (others are test mocks) |
| Input (server-side) | `OnRawInputHandler` | 1 — terminal/termios source |
| Input (client-side) | **none yet** | — |

Count: output = 1, input = 1. The formal **client-side input-source
capability** (the dual of output) is **deferred to its second concrete
source** — bare-metal HID or web DOM. It is a design note, not a
contract to assert now. The settled part is the *vocabularies*; the
capability *split* is deferred per §7.4 of the kernel model.

### 6.5 Honest cost: terminal as a lossy input source

Terminal input is a lossy source — no key-release events, limited
modifier chords. The terminal source driver reverse-maps byte
sequences → HID keycodes approximately. Capabilities needing precise
key-up or rich chords work fully only on bare-metal HID and rich
GUI/web, and are degraded on a terminal. HID-as-canonical is the
right call (the invariant vocabulary should be the richest source,
with others degraded, not the poorest), but the degradation is
permanent and worth stating.

## Open items

1. Platform-as-config-participant decision (§5).
2. Whether the platform runtime can be hot-swapped during a session
   (e.g. attach via TUI then switch to web). Default no.
3. Native platform runner — out of target.
4. Client-side input-source capability contract (§6.4): deferred to
   second concrete source (bare-metal HID or web DOM).

## Conformance

This chapter has no rules. Platform-specific behaviours are
fixtured per-platform in their own crates' tests.
