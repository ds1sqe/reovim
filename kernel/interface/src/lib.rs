#![forbid(unsafe_code)]
//! Portable contracts above Reovim Foundation.
//!
//! Kernel Interface owns semantic contract values only. Runtime mechanisms,
//! Host adapters, Event retention, rendering, and Assembly remain outside this
//! package.

pub mod cancellation;
pub mod clock;
pub mod event;
pub mod lifecycle;
pub mod operation;
