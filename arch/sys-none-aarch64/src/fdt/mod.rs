//! Arch-local Flattened Device Tree (FDT v17 / DTB) support.
//!
//! Only the aarch64 bare-metal floor consumes a DTB — x86 learns the machine
//! from Multiboot / E820 tags and never sees an FDT. That single-consumer
//! reality is the reason this lives under `sys/none_aarch64/` rather than at
//! the crate root. If a second architecture ever needs it, the extraction is a
//! file-move and a `mod fdt;` addition in the new target directory; the reader
//! itself is consumer-agnostic (pure `&[u8]` in, typed values out).
//!
//! Phase 1 exposes the raw reader only. Phase 2 adds `enumerate.rs`, the
//! caller that walks the device tree and hands structured `DeviceEntry` records
//! to the kernel via [`reovim_kabi_platform::DeviceInventory`].

pub mod enumerate;
pub mod reader;
