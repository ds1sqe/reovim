#![no_std]
#![forbid(unsafe_code)]
//! Explicit Host adapters, with no ambient services in portable consumers.

extern crate alloc;
#[cfg(feature = "std")]
extern crate std;

mod clock;
mod diagnostic;

pub use clock::DeterministicClock;
#[cfg(feature = "std")]
pub use clock::StdClock;
pub use diagnostic::{DiagnosticQueue, QueueError};
