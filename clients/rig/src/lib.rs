#![cfg_attr(not(feature = "std"), no_std)]
#![forbid(unsafe_code)]
//! What a client test stands on: a Kernel with fs over a directory of
//! its own and text, and Clients on it through an in-process
//! `Connection`. For tests only; no product crate links it.

#[cfg(feature = "std")]
mod rig;

pub use reovim_kernel_runtime::Limits;
#[cfg(feature = "std")]
pub use rig::Rig;
