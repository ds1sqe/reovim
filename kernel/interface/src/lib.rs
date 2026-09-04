#![no_std]
#![forbid(unsafe_code)]
//! Portable contracts above Reovim Foundation.
//!
//! Kernel Interface owns semantic contract values only. Runtime mechanisms,
//! Host adapters, Event retention, rendering, and Assembly remain outside this
//! package.

extern crate alloc;

pub mod bound;
pub mod buffer;
pub mod cancellation;
pub mod carrier;
pub mod clock;
pub mod contract;
pub mod diagnostic;
pub mod effect;
pub mod event;
pub mod identity;
pub mod lifecycle;
pub mod operation;
pub mod output_health;
pub mod resource;
pub mod semantic;
