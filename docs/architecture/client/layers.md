# Architecture

## Layer Diagram

```
┌──────────────────────────────────────────────────────────────────────┐
│                                                                      │
│  PLATFORM ADAPTER                          GROUND TRUTH / ADOPTION   │
│                                                                      │
│  The platform IS. It declares capabilities. It doesn't bend.         │
│                                                                      │
│  Implements:                                                         │
│    PlatformCapabilities — what this platform can do                   │
│    ChromeSurface        — how to put content on screen               │
│    InputSource          — how to receive user input                   │
│                                                                      │
│  TUI:  crossterm, ANSI, cell grid, terminal resize                   │
│  Web:  Canvas/WebGL, DOM events, WASM                                │
│  iOS:  UIKit/SwiftUI, virtual keyboard, safe area                    │
│  Android: Compose/Views, IME, display cutout                         │
│                                                                      │
├──────────────────────────────────────────────────────────────────────┤
│                                                                      │
│  COMMON CLIENT                                     PLATFORM-AGNOSTIC │
│                                                                      │
│  ┌────────────────────────────────────────────────────────────────┐  │
│  │  CLIENT CORE                                     MECHANISM     │  │
│  │                                                                │  │
│  │  Compositor. Routes events, allocates regions, composites.     │  │
│  │  Zero knowledge of any specific UI feature.                    │  │
│  │  Talks to platform ONLY through abstract traits.               │  │
│  │                                                                │  │
│  │  Owns:                                                         │  │
│  │    - Protocol adapter (gRPC connection, notification decode)   │  │
│  │    - State cache (buffer content, cursor, mode)                │  │
│  │    - Event dispatcher (broadcast on_* to all modules)          │  │
│  │    - Region allocator (Top/Bottom/Left/Right/Overlay)          │  │
│  │    - Frame compositor (delegates to ViewportRenderer)          │  │
│  │    - Module lifecycle (load, init, shutdown)                   │  │
│  │    - Input forwarding (PlatformEvent -> gRPC -> server)           │  │
│  │                                                                │  │
│  │  grep test: search CORE for "statusline", "gutter",           │  │
│  │  "line_number", "diagnostic", "git_branch", "breadcrumb",     │  │
│  │  "colorscheme", "number", "relativenumber", "crossterm",      │  │
│  │  "terminal", "canvas", "dom", "UIKit" — you find NOTHING.     │  │
│  ├────────────────────────────────────────────────────────────────┤  │
│  │  CLIENT DRIVER                                   CONTRACT      │  │
│  │                                                                │  │
│  │  Platform-agnostic trait contracts and rendering primitives.   │  │
│  │  The stable API surface for module authors.                    │  │
│  │                                                                │  │
│  │  Module contract:                                              │  │
│  │    ClientModule      — lifecycle, events, role dispatch        │  │
│  │                                                                │  │
│  │  Rendering:                                                    │  │
│  │    ViewportRenderer  — buffer rendering pipeline               │  │
│  │    LayoutPolicy      — window arrangement                     │  │
│  │    ChromeSurface     — abstract output target                  │  │
│  │    PlatformCapabilities — what the platform offers             │  │
│  │    ThemeProvider     — color scheme and highlight groups        │  │
│  │                                                                │  │
│  │  Atoms:                                                        │  │
│  │    Style, Rect, Insets, PlatformEvent                             │  │
│  │                                                                │  │
│  │  Registries:                                                   │  │
│  │    ServiceRegistry — cross-module communication                │  │
│  ├────────────────────────────────────────────────────────────────┤  │
│  │  CLIENT MODULE                                   POLICY        │  │
│  │                                                                │  │
│  │  Feature implementations that adapt to the platform.           │  │
│  │  One crate per module. Compiled-in via Cargo dependency.       │  │
│  │                                                                │  │
│  │  Examples:                                                     │  │
│  │    statusline, line-numbers, cmdline, explorer, whichkey,      │  │
│  │    bracket-pair, diagnostics, indent-guide, sticky-context,    │  │
│  │    git-signs, scrollbar, minimap                               │  │
│  └────────────────────────────────────────────────────────────────┘  │
│                                                                      │
└──────────────────────────────────────────────────────────────────────┘
```

## Layer Rules

### PLATFORM ADAPTER

- IS the ground truth. Declares capabilities honestly.
- Implements `PlatformCapabilities`, `ChromeSurface`. Creates input channel.
- Never changes to accommodate modules. Modules adapt to it.
- One adapter per platform binary (TUI binary, Web binary, Mobile binary).

### CLIENT CORE

- ZERO knowledge of any specific UI feature.
- ZERO knowledge of any specific platform.
- Receives `PlatformCapabilities` at startup, passes to modules.
- Outputs to `ChromeSurface` (abstract), reads from input channel (concrete).
- Depends on CLIENT DRIVER for trait definitions.
- Delegates buffer rendering to `ViewportRenderer` — never handles folds,
  virtual lines, transforms, or token classification directly.

### CLIENT DRIVER

- Defines contracts, never instantiates modules.
- All types are platform-agnostic.
- `PlatformCapabilities` defined here, implemented by adapters.
- `ChromeSurface` defined here, implemented by adapters.
- `ViewportRenderer` defined here with a default implementation.
- `ThemeProvider` defined here.
- May provide default implementations for bootstrapping.

### CLIENT MODULE

- Depends ONLY on CLIENT DRIVER.
- No cross-module imports. No CORE internals. No platform APIs.
- Receives `PlatformCapabilities` at init and render. Adapts accordingly.
- NEVER checks platform identity. ALWAYS checks capabilities.
- Receives state through `on_*` events. Caches what it needs internally.

## Dependency Graph

```
                        ┌──────────────┐
                        │ reovim-kernel │  (ServiceRegistry, BufferId,
                        │              │   WindowId — SSOT for both sides)
                        └──────┬───────┘
                               │
                    ┌──────────┴──────────┐
                    │   CLIENT DRIVER     │  ClientModule, ViewportRenderer,
                    │ (reovim-client-     │  PlatformCapabilities, ChromeSurface,
                    │  driver)            │  ThemeProvider, PlatformEvent, Style
                    └──┬─────┬────────┬──┘
                       │     │        │
            ┌──────────┘     │        └──────────┐
            │                │                   │
     ┌──────┴──────┐  ┌─────┴──────┐  ┌─────────┴───┐
     │ statusline  │  │ line-nums  │  │  cmdline     │  CLIENT MODULEs
     │  module     │  │  module    │  │  module      │
     └─────────────┘  └────────────┘  └──────────────┘
                               │
     ┌─────────────────────────┴──────────────────────┐
     │           PLATFORM BINARY                       │
     │  (e.g., reovim-tui)                             │
     │                                                 │
     │  Cargo.toml: common-client-core + modules +     │
     │  platform adapter                               │
     │                                                 │
     │  main.rs: register_modules(), start CORE        │
     └─────────────────────────┬──────────────────────┘
                               │
     ┌─────────────────────────┴──────────────────────┐
     │           COMMON CLIENT CORE                    │
     │  (protocol, compositor, event dispatch,         │
     │   module lifecycle, ViewportRenderer dispatch)  │
     └─────────────────────────┬──────────────────────┘
                               │
            ┌──────────────────┼──────────────────┐
            │                  │                  │
     ┌──────┴──────┐   ┌──────┴──────┐   ┌───────┴─────┐
     │ TUI adapter │   │ Web adapter │   │Mobile adapt. │
     │ (crossterm) │   │ (Canvas)    │   │ (UIKit)      │
     └─────────────┘   └─────────────┘   └──────────────┘
```

### Enforcement (Cargo dependency rules)

```
common-client-core  depends on  client-driver       (for contracts)
common-client-core  does NOT depend on  any platform adapter
common-client-core  does NOT depend on  any module crate

module-X            depends on  client-driver       (ONLY)
module-X            does NOT depend on  common-client-core
module-X            does NOT depend on  module-Y
module-X            does NOT depend on  any platform adapter

platform-binary     depends on  common-client-core  (to run it)
platform-binary     depends on  platform adapter    (for this platform)
platform-binary     depends on  module crates       (its distribution)
platform-binary     calls       register_modules()  (explicit, no magic)

client-driver       depends on  reovim-kernel       (ServiceRegistry, IDs)
client-driver       does NOT depend on  any platform
```

### Enforcement: Known Limits

Cargo prevents modules from importing CORE, other modules, or platform adapters.
Two blind spots exist as **design-time covenants**, not compile-time constraints:

1. **`unsafe` FFI** — a module can `dlopen` arbitrary libraries or call platform
   C symbols without a Cargo dependency.
2. **`std` access** — `std::env::var("TERM")`, `/proc`, `SystemTime`, etc. are
   available without Cargo trace. Any `std` facility that leaks platform identity
   is a bypass.

These apply to any `std`-accessible system resource, not just the two examples.
Compiled-in modules from trusted authors follow the model by convention.
