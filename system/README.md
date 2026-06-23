# `system/` — the World system-kernel root

The settled vocabulary reserves **kernel** for this World-layer root.
Product-side mechanisms are cores: `editor/` carries the editor core and
`client/` carries client-side platform/core pieces. The system kernel is the
only normal bridge from product-facing `uapi/*` to machine-facing `kabi/*`.

## Crates

- **`lib/kernel/`** (`reovim-system-kernel`) — arch-free common console, FDT,
  splash, boot-info, inventory, and service-bridge policy. Raw facts,
  framebuffer storage, and provider installs stay in the lower
  composition/provider layer.

The depgraph `system/*` → Foundation category row is active for this crate.
