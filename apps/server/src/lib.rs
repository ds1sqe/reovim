#![no_std]
#![deny(unsafe_code)]
//! Server composition; backend implementations enter through module activation.

extern crate alloc;
#[cfg(feature = "std")]
extern crate std;

#[cfg(feature = "std")]
pub mod boot;
#[path = "service/clients.rs"]
pub mod clients;
pub mod loading;
#[cfg(feature = "std")]
pub mod service;

use reovim_kernel_interface::Error;
use reovim_kernel_runtime::{Limits, Runtime};

/// Starts the server's execution owner without selecting any Resource backend.
///
/// Composition supplies a distinct Kernel epoch and explicit budgets, then
/// admits modules and grants Caller access. For example, a new empty server:
/// ```
/// let mut runtime = reovim_server::assemble(
///     [1; 16], reovim_kernel_runtime::Limits::default(),
/// )?;
/// let client = runtime.bind(&[])?.caller();
/// // Client semantics use this Caller; only composition retains Runtime.
/// runtime.close()?;
/// # Ok::<(), reovim_kernel_interface::Error>(())
/// ```
/// # Errors
/// Invalid identity/budgets or allocation pressure leave no running server.
pub fn assemble(epoch: [u8; 16], limits: Limits) -> Result<Runtime, Error> {
    let mut runtime = Runtime::new(epoch, limits)?;
    runtime.start()?;
    Ok(runtime)
}
