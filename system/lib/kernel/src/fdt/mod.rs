//! Flattened Device Tree (FDT v17 / DTB) support.
//!
//! The reader is consumer-agnostic (pure `&[u8]` in, typed values out). The
//! device-inventory bridge ([`crate::inventory`]) receives a process-lifetime
//! DTB byte slice plus a caller-supplied compatible-string classifier from the
//! composition root and enumerates it into neutral
//! [`reovim_kabi_platform::DeviceInventory`] records.

pub mod enumerate;
pub mod reader;
