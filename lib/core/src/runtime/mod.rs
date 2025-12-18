//! Runtime module - the main editor event loop and state management

mod core;
mod event_loop;
mod handlers;

pub mod test;

pub use core::Runtime;
pub use test::{TestResult, TestRuntime, TestRuntimeBuilder};
