#![no_std]
#![forbid(unsafe_code)]
//! Kernel runtime: the Resource table, dispatch, Pending, watch and log.
//!
//! One `Kernel`, in-process and single-threaded. It enforces what
//! Documentation/Design/00-core.md and 01-core-runtime.md say and
//! nothing else.

extern crate alloc;

mod calls;
mod connection;
mod dispatch;
mod kernel;
mod log;
mod pending;
mod table;
mod watch;

pub use calls::ModuleCalls;
pub use connection::{Connection, Loader, NoLoader};
pub use kernel::{Heap, Kernel, Limits};
pub use table::{CORE, LIMITS, LOG, LOGLEVEL, MEM, Param, ROOT, SYS};
