# `kabi/` — the down-face contract tier

`uapi/` is the up-face (what the kernel exposes upward to apps, clients, and
modules); `kabi/` is the down-face (what the kernel requires downward from a
platform provider, devices, and drivers). Both faces are sealed, versioned,
`#[repr(C)]`, `no_std` declarations — not HAL *layers*. A `kabi/*` crate
declares a mechanism and is a **leaf**: it depends on nothing impl-side and
never names its implementor (the cardinal `kabi → arch` forbidden edge).

## Crates

- **`platform/`** (`reovim-kabi-platform`) — the `#[repr(C)]` platform handle
  vtable, the process-global write-once handle install, and provider-facing
  error/value types. The vtable is append-only: it starts with clock,
  allocation, and park/unpark primitives, then adds fd/socket, detached-thread,
  wall-clock, file, terminal raw-mode, and path-operation slots. Platform
  provider crates implement it (`provider/arch → kabi`); `kabi/platform`
  never names an implementor.

## Placeholders (no crate yet — rule of three)

- **`device/`** — the device contract. **Not a crate this flight.** Only the
  client kernel needs devices and no concrete device consumer exists yet
  (rule of three: the seam shape is best learned from the first real
  consumer, not guessed). `kabi/device` becomes a crate when that consumer
  arrives; until then it is this documented placeholder, with no empty
  workspace member.
