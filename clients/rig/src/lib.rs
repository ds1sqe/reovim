#![cfg_attr(not(feature = "std"), no_std)]
#![forbid(unsafe_code)]
//! What a client test stands on: a Kernel with fs over a directory of
//! its own, text, and a clipboard over a store of its own, Clients on
//! it through an in-process `Connection`, and the one stub Plugin a
//! test gives a region to. For tests only; no product crate links it.

#[cfg(feature = "std")]
mod rig;
#[cfg(feature = "std")]
mod stub;

pub use reovim_kernel_runtime::Limits;
#[cfg(feature = "std")]
pub use rig::Rig;
#[cfg(feature = "std")]
pub use stub::{Seen, Stub, Typed, buffer_over, shown};
