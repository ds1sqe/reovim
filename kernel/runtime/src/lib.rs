#![no_std]
#![forbid(unsafe_code)]
//! Resource admission and execution with Runtime-owned request custody.
//!
//! Example: assembly registers providers, grants a Caller, then drives advance
//! independently of that caller's result polling.

extern crate alloc;

mod engine;
pub use engine::{
    Activation, Binding, Grant, Limits, ModuleProbe, ModuleSelection,
    PublicationGrant, Runtime, SelectedModule,
};
