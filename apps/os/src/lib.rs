//! `reovim-os` composition boot helpers.
//!
//! This crate owns the RTOS composition entry path: installs provider and
//! sysroot services, gathers neutral boot facts/device inventory, renders the
//! splash, and enters the system-kernel root daemon.
//!
//! `main.rs` wires target-appropriate floor crates to this module through the
//! `entry!` macro; `boot.rs` keeps the profile entry implementation.

#![no_std]

pub mod boot;
