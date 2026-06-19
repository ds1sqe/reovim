//! Flattened Device Tree (FDT v17 / DTB) support.
//!
//! Only the aarch64 bare-metal floor passes a DTB — x86 learns the machine
//! from Multiboot / E820 tags and never sees an FDT. The reader itself is
//! consumer-agnostic (pure `&[u8]` in, typed values out); the system kernel's
//! device-inventory assembly ([`crate::inventory`]) feeds it the firmware blob
//! the floor captured and turns its output into a neutral inventory. It lifted
//! up here with that assembly out of the arch-sys-none raw-mechanism crate
//! (SP04 04a).
//!
//! Phase 1 exposes the raw reader only. Phase 2 adds `enumerate.rs`, the
//! caller that walks the device tree and hands structured `DeviceEntry` records
//! to the kernel via [`reovim_kabi_platform::DeviceInventory`].

pub mod enumerate;
pub mod reader;
