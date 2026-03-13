# Principles

## Non-Goals

- **Symmetry with server.** Different responsibilities, different shapes.
- **Speculative abstraction.** Each layer earns its existence through real need.
- **ABI stability.** Modules are Cargo crates, not binary plugins. Rebuild is OK.

## Core Principle: Semantic, Presence, Adoption, Specialization

Every client feature has four aspects:

**Semantic** — what the feature MEANS.
  A statusline shows editor state. A gutter shows per-line annotations.
  Semantics are universal. They don't change between platforms.

**Presence** — how the feature PRESENTS itself.
  Presence is NOT fixed. A module discovers what the platform can do and
  adapts its presence accordingly. On TUI: styled cells in a row. On Web:
  a DOM bar with CSS transitions. On Mobile: a native UILabel respecting
  safe area insets. Same semantic, different presence.

**Adoption** — the platform IS. It doesn't bend.
  The platform is ground truth. It provides concrete capabilities:
  cell grid, canvas, native views, true color, pointer events, haptic,
  safe area, smooth scroll. The platform declares what it IS.
  Modules discover this and adapt. Not the other way around.

**Specialization** — platform-unique capabilities that modules can leverage.
  Web: smooth pixel scrolling, CSS animations.
  Mobile: touch gestures, haptic feedback, safe area insets.
  TUI: terminal-specific escape sequences, sixel graphics.
  Modules that use specialization are platform-aware by choice.

## Modules adapt to the platform

A module is a visitor entering a venue. It doesn't demand the venue change
its acoustics. It listens to the room, discovers what the space offers,
and adapts its performance. The venue is stable. The performer is flexible.

```
PLATFORM (ground truth)
    |
    | declares capabilities
    v
MODULE (adaptive)
    |
    | queries capabilities
    | adapts presence
    | renders accordingly
    v
SCREEN
```

| Aspect | Where it lives | Direction |
|--------|---------------|-----------|
| Semantic | CLIENT MODULE | Module defines (universal) |
| Presence | CLIENT MODULE + DRIVER | Module adapts to platform capabilities |
| Adoption | PLATFORM ADAPTER | Platform declares (ground truth) |
| Specialization | PLATFORM ADAPTER | Platform offers, module may use |

## Capability-based, not identity-based

A module NEVER does this:
```rust
// WRONG: checking platform identity
if platform_name == "mobile" { ... }
```

A module DOES this:
```rust
// RIGHT: checking platform capabilities
if caps.safe_area().bottom > 0 {
    // adjust layout for bottom safe area
}
if caps.smooth_scroll() {
    // use sub-line scroll offset
} else {
    // snap to whole lines
}
```
