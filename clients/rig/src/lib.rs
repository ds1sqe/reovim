#![cfg_attr(not(feature = "std"), no_std)]
#![forbid(unsafe_code)]
//! What a client test stands on: a Kernel with fs over a directory of
//! its own and text, Clients on it through an in-process `Connection`,
//! a clipboard store a Client takes as its machine's, the one stub
//! Plugin a test gives a region to, what a case over a Driver under
//! `/bin` spells in `bins`, and what a case over an editor spells.
//! For tests only; no product crate links it.

#[cfg(feature = "std")]
pub mod bins;
#[cfg(feature = "std")]
mod editor;
#[cfg(feature = "std")]
mod rig;
#[cfg(feature = "std")]
mod stub;

#[cfg(feature = "std")]
pub use editor::{
    frame, lines, listed, opened, pressed, said, screen, status, typed,
};
pub use reovim_kernel_runtime::Limits;
#[cfg(feature = "std")]
pub use rig::Rig;
#[cfg(feature = "std")]
pub use stub::{Seen, Stub, Typed, buffer_over, shown};
