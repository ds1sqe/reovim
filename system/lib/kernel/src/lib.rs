//! The World system kernel bridge (`reovim-system-kernel`).
//!
//! This crate carries the arch-free common half of the World boot surface:
//! console rendering policy, SGR/color/font handling, FDT parsing and
//! enumeration, `BootInfo` shaping, device-inventory shaping, and splash
//! content. It is not a platform provider and it does not read hardware facts
//! directly. The provider/composition layer below gathers raw arch/device facts
//! and hands them in as plain `kabi/*` data; layers above consume the shaped
//! `uapi/*`/kernel-facing surface rather than importing raw devices.
//!
//! No `arch*`, `platform*`, or direct `uapi/posix` dependency belongs here. Raw
//! mechanism such as `asm!`, MMIO, boot-pointer statics, page arenas, and write
//! sink installation stays below this bridge.

#![no_std]
// This bridge still forms process-lifetime slices from caller-provided static
// storage and stores callback contexts for the boot console.
#![allow(unsafe_code)]

pub mod color;
pub mod console;
pub mod escape;
pub mod fdt;
pub mod fonts;
pub mod boot_info_aarch64;
pub mod boot_info_x86;
pub mod inventory;
pub mod splash;

// ── device-neutral boot-surface re-exports ───────────────────────────────────
//
// Keep the old target-selected root names for freestanding composition roots.
// The functions now take caller-supplied facts/storage rather than reaching
// below this crate themselves.
#[cfg(all(target_os = "none", target_arch = "aarch64"))]
pub use boot_info_aarch64::collect_boot_info;
#[cfg(all(target_os = "none", target_arch = "x86_64"))]
pub use boot_info_x86::collect_boot_info;

#[cfg(all(target_os = "none", target_arch = "x86_64"))]
pub use boot_info_x86::collect_device_inventory;
#[cfg(all(target_os = "none", target_arch = "aarch64"))]
pub use inventory::collect_device_inventory;
