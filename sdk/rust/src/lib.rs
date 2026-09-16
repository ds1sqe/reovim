#![no_std]
#![forbid(unsafe_code)]
//! The SDK: what a Module or a Client says in Rust and how it becomes
//! the bytes of Documentation/Design/00-core.md. Values in two forms,
//! typed (`Encode`, `Decode`, `Typed`) and dynamic (`Value`, by a
//! `Ty` at runtime). It links kernel/interface and nothing else.

extern crate alloc;

pub mod dynamic;
mod typed;

pub use reovim_kernel_interface::codec::Reader;
pub use typed::{Decode, Encode, Typed, decode, encode};
