#![no_std]
#![forbid(unsafe_code)]
//! The SDK: what a Module or a Client says in Rust and how it becomes
//! the bytes of Documentation/Design/00-core.md. Values in two forms,
//! typed (`Encode`, `Decode`, `Typed`) and dynamic (`Value`, by a
//! `Ty` at runtime), and `declare!`, an api as the design writes it.
//! It links kernel/interface and nothing else.

extern crate alloc;

mod declare;
pub mod dynamic;
mod typed;

pub use declare::Call;
pub use reovim_kernel_interface::codec::Reader;
pub use typed::{Decode, Encode, Typed, decode, encode};

/// What the macros name; not for callers.
#[doc(hidden)]
pub mod __interface {
    pub use reovim_kernel_interface::{
        Calls, Decl, Error, Field, Handle, Hash, Method, Rd, Rights, Ty,
    };
}
