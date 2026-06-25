//! The World system kernel bridge (`reovim-system-kernel`).
//!
//! This crate carries the arch-free common half of the World boot surface:
//! console rendering policy, SGR/color/font handling, FDT parsing and
//! enumeration, up-face `BootInfo` shaping, device-inventory shaping, and splash
//! content. It is not a platform provider and it does not read hardware facts
//! directly. The provider/composition layer below gathers raw arch/device facts
//! and hands them in as plain lower data; layers above consume the shaped
//! `uapi/*`/kernel-facing surface rather than importing raw devices.
//!
//! No `arch*`, `platform*`, or direct `uapi/posix` dependency belongs here. Raw
//! mechanism such as `asm!`, MMIO, boot-pointer statics, page arenas, and write
//! sink installation stays below this bridge.

#![no_std]
// This bridge still forms process-lifetime slices from caller-provided static
// storage and stores callback contexts for the boot console.
#![allow(unsafe_code)]

pub mod block;
pub mod boot;
pub mod boot_info;
pub mod color;
pub mod console;
pub mod console_io;
pub mod dump;
pub mod escape;
pub mod exec;
pub mod fdt;
pub mod fonts;
pub mod fs;
pub mod input;
pub mod inventory;
pub mod klog;
pub mod log;
pub mod mm;
pub mod net;
pub mod panic;
pub mod proc;
pub mod program;
pub mod root_shell;
pub mod rootd;
pub mod sched;
pub mod source_media;
pub mod source_store;
pub mod splash;
pub mod syscall;
pub mod terminal;
pub mod vfs;

#[cfg(feature = "selftest")]
pub(crate) mod bin_fixture;

// ── device-neutral boot-surface re-exports ───────────────────────────────────
//
// These functions take caller-supplied neutral facts/storage rather than
// reaching below this crate themselves.
pub use {boot_info::collect_boot_info, inventory::collect_device_inventory};

#[cfg(feature = "selftest")]
#[path = "panic_tests.rs"]
mod panic_tests;

#[cfg(feature = "selftest")]
#[path = "terminal_tests.rs"]
mod terminal_tests;
