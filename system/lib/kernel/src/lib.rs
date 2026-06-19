//! The World system kernel (`reovim-system-kernel`).
//!
//! The device-neutral bare-metal library lifted out of the `arch-sys-none-*`
//! raw-mechanism crates (SP04 04a): the framebuffer console device model (SGR /
//! blit / grid / blend), its ANSI/VT escape parser, the SGR color model, the
//! embedded coverage fonts, the Flattened-Device-Tree reader + device
//! enumerator, and the boot-info / device-inventory *assembly* for both
//! freestanding arches.
//!
//! This crate is a contract *implementor* — bare-metal it plays the
//! `kabi/platform` provider role (`editor → kabi/platform → SYSTEM KERNEL →
//! arch-sys-none`), structurally peer to the hosted `platform-*` providers
//! (master-plan §11, invariant #2). Its only floor edges are the two
//! freestanding raw-mechanism crates it implements over: it consumes the
//! `VideoCore` `Framebuffer` *type* (raw MMIO, stays below — the Q2 seam) and
//! reads the register / mailbox / Multiboot / DTB raw facts through their
//! crate-root accessors. The raw mechanism (all `asm!`, the MMIO, the static
//! arena, the boot-pointer statics) STAYS below the seam (invariant #4); this
//! crate carries only the device-neutral assembly and decode.

#![no_std]
// The migrated console / device-inventory assembly reach the framebuffer MMIO
// surface and form `'static` slices over arena hand-outs through `unsafe`, the
// same as in their previous arch-sys-none home; this crate opts out of the
// workspace `-D unsafe-code` gate exactly as the floor crates do.
#![allow(unsafe_code)]

// Pure device-neutral data + parsers: buildable on every target (no MMIO, no
// asm, no per-arch surface). The console that consumes them is freestanding-only
// because it renders through the aarch64 `Framebuffer` type (the Q2 seam).
pub mod color;
pub mod escape;
pub mod fonts;

// The FDT reader is pure, but the device enumerator backs its `'static` entry
// storage with the aarch64 floor's static arena accessor (raw mechanism, stays
// below), so the module is gated to the freestanding aarch64 target — the only
// one that passes a DTB and defines that accessor.
#[cfg(all(target_os = "none", target_arch = "aarch64"))]
mod fdt;

// The framebuffer console consumes the arch-sys-none-aarch64 `Framebuffer` type
// (raw VideoCore MMIO, freestanding-only — the Q2 seam), so it is gated to the
// target that defines that type. The `install_console` boot action builds the
// standing console and registers its `fn(&[u8])` trampoline into the floor's
// write-sink registry, so fd 1/2 standingly fans to the framebuffer console with
// no fixture-side install glue (SP04 04b).
#[cfg(all(target_os = "none", target_arch = "aarch64"))]
pub mod console;

// The boot-info assembly is split per-arch (aarch64 register/mailbox facts vs
// x86 Multiboot mmap), each reading its facts from the matching arch-sys-none
// crate. Gated to the freestanding target it serves so a hosted build of this
// crate (the host `cargo test` of the migrated unit tests) does not pull a
// per-arch boot path it cannot satisfy.
#[cfg(all(target_os = "none", target_arch = "aarch64"))]
pub mod boot_info_aarch64;
#[cfg(all(target_os = "none", target_arch = "x86_64"))]
pub mod boot_info_x86;

// The device-inventory assembly (DTB parse + enumerate) — aarch64 only; x86
// freestanding has no device tree (its mirror lives in `boot_info_x86`).
#[cfg(all(target_os = "none", target_arch = "aarch64"))]
pub mod inventory;

// ── device-neutral boot-surface re-exports ───────────────────────────────────
//
// The bootcore / splash fixtures name `collect_boot_info` / `collect_device_inventory`
// at the crate root (mirroring the old `arch::sys::*` facade shape), so the
// per-arch assembly is re-exported here under the neutral name. Same target gate
// as the assembly each re-export sources.
#[cfg(all(target_os = "none", target_arch = "aarch64"))]
pub use boot_info_aarch64::collect_boot_info;
#[cfg(all(target_os = "none", target_arch = "x86_64"))]
pub use boot_info_x86::collect_boot_info;

#[cfg(all(target_os = "none", target_arch = "x86_64"))]
pub use boot_info_x86::collect_device_inventory;
#[cfg(all(target_os = "none", target_arch = "aarch64"))]
pub use inventory::collect_device_inventory;
