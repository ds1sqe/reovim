#![no_std]
#![forbid(unsafe_code)]
//! The SDK: what a Module or a Client says in Rust and how it becomes
//! the bytes of Documentation/Design/00-core.md. Values in two forms,
//! typed (`Encode`, `Decode`, `Typed`) and dynamic (`Value`, by a
//! `Ty` at runtime), `declare!`, an api as the design writes it, the
//! one `walk` from a path to an rd, a path resolved by spelling
//! alone, `drivers!`, the Module a set of Drivers under one `bin` is,
//! and the output cell one of them answers a run with. Beside them
//! what both sides of a Resource read the same way: the log's
//! entries, the ops anything makes over its own `Calls`, a look and
//! the meanings a theme resolves to one, a Kernel instant as a
//! calendar reads it, and `progress`, a long fill's two numbers and
//! the arithmetic of `eta`, caring for no render policy of its own.
//! It links kernel/interface and nothing else.

extern crate alloc;

pub mod cell;
pub mod date;
mod declare;
mod drivers;
pub mod dynamic;
pub mod log;
pub mod look;
pub mod ops;
pub mod path;
pub mod progress;
pub mod sgr;
mod typed;
pub mod walk;

pub use declare::Call;
pub use reovim_kernel_interface::codec::Reader;
pub use typed::{Decode, Encode, Typed, decode, encode};

/// What the macros name; not for callers.
#[doc(hidden)]
pub mod __interface {
    pub use alloc::boxed::Box;
    pub use alloc::rc::Rc;
    pub use alloc::string::{String, ToString};
    pub use alloc::vec;
    pub use alloc::vec::Vec;
    pub use reovim_kernel_interface::{
        Booted, Calls, Decl, Driver, DriverId, Error, Field, Flags, Handle,
        Hash, Method, Module, Rd, Reply, Rights, Ty,
    };
}
