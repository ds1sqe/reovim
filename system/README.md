# `system/` — the World system-kernel root

The settled tree carries three kernel roots: `editor/` (the Math editor
kernel), `client/` (the Math client kernel), and `system/` (the World system
kernel — KERNEL 1).

## Crates

- **`lib/kernel/`** (`reovim-system-kernel`) — arch-free common console, FDT,
  splash, boot-info, inventory, and service-bridge policy. Raw facts,
  framebuffer storage, and provider installs stay in the lower
  composition/provider layer.

The depgraph `system/*` → Foundation category row is active for this crate.
