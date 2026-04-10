# Event Layer Model

Reovim events are organized into three layers. Each layer has distinct
ownership, emission rules, and subscriber expectations.

## Three Layers

| Layer | Home | Rate | Transport |
|-------|------|------|-----------|
| Kernel substrate | `server/lib/kernel/src/ipc/events/` | Low-moderate | `EventBus::emit` |
| Per-domain transition | Per-domain events crate (e.g. `shared/domains/text/events/`) | Low-moderate | `EventBus::emit` |
| Per-domain streaming state | Per-provider (not centralized) | High (Hz-level) | Shared atomic / watch channel |

### Kernel substrate (domain-free)

Events that apply regardless of content domain. Payloads carry only
opaque identifiers (`BufferId`, `WindowId`), byte data (`ByteEdit`),
or kernel-owned enums (`ModeId`, `ChangeSource`).

Post-decoupling kernel events:

- `BufferCreated`, `BufferClosed`, `BufferSwitched`
- `BufferBytesEdited`, `BufferWillSave`, `BufferSaved`, `FileOpened`
- `WindowCreated`, `WindowClosed`, `WindowFocused`
- `LayoutChanged` (with `LayoutChangeKind`, `SplitDirection`)
- `ModeChanged`
- `OptionChanged`, `OptionReset` (with `ChangeSource`)
- `Shutdown`

### Per-domain transition (semantic, low-rate)

Events that carry domain-specific types. Each content domain owns its
events in a dedicated crate. Subscribers opt in by depending on the
domain events crate.

Text domain (`reovim-domain-text-events`):

- `TextBufferModified` — carries `TextEdit`, `TextPosition`, and
  byte-range fields for tree-sitter correlation
- `CursorMoved` — carries `TextPosition`
- `ViewportScrolled` — carries `top_line`, `bottom_line`

Codec driver (`reovim-driver-codec`):

- `FileTypeChanged` — file type is a codec-factory concern, not
  a text concern

### Per-domain streaming state (high-rate, NOT on EventBus)

High-frequency state that would overwhelm the event bus. Examples:
audio play head, render frame position, mouse drag coordinates.

Rule: if emission rate could exceed the slowest subscriber's handler
time, do not use `EventBus::emit`. Use shared atomic state, `arc-swap`,
`watch` channels, or similar non-queuing mechanisms. Emit transitions
(start/stop/seek/mode-change) on the bus, not per-frame values.

This layer does not yet exist in reovim. Design exploration for stream
support is captured in `tmp/740/09-stream-architecture-exploration.md`.

## Rules for Future Events

1. **Kernel events carry NO domain-specific payload.** No `(line, col)`
   positions, no `text: String`, no domain enums.
2. **Domain events live in per-domain crates**, not in kernel.
3. **Domain events `impl Event`** via a `reovim-kernel` dependency.
4. **High-rate state does NOT use EventBus.** See streaming layer above.
5. **The `Domain` trait stays minimal.** Adding bounds excludes valid
   future domains. `Position`, `Edit`, `Content` are opaque associated
   types used by the codec layer, not by kernel events.

## Anti-Patterns

- Adding `CameraMoved` (60 Hz) to EventBus
- Adding `CursorMoved` with hardcoded `(u32, u32)` to kernel
- Adding `D::Cursor` or `D::Viewport` to the `Domain` trait
- Provider crate depending on another provider to share cursor types
- Creating a `DomainView` lifecycle trait that pretends view state is
  unifiable across domains

## Extensibility Example: Adding `reovim-domain-hex`

1. Create `shared/domains/hex/events/` with `HexCursorMoved`,
   `HexSelectionChanged`, etc.
2. Depend on `reovim-kernel` for `Event` trait + `BufferId`.
3. Depend on hex-specific types crate for position/selection types.
4. Zero kernel changes required. Zero text domain changes required.
