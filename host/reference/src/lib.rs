#![no_std]
#![deny(unsafe_code)]
//! Bounded embedded driver for local examples; no hidden provider executor.

extern crate alloc;

#[cfg(feature = "std")]
extern crate std;

#[cfg(all(feature = "std", target_os = "linux"))]
#[allow(unsafe_code)]
pub mod native;

#[cfg(all(feature = "std", target_os = "linux"))]
#[allow(unsafe_code)]
pub mod process;

use alloc::boxed::Box;
use core::{
    future::Future,
    task::{Context, Poll, Waker},
};
use reovim_kernel_interface::{Error, driver::DriverOps};

/// Drives an Interface implementation while observing one caller result.
/// # Errors
/// Exhaustion or driver failure leaves cleanup with the execution owner.
pub fn run_with<T, State>(
    future: impl Future<Output = T>,
    state: &mut State,
    driver: &DriverOps<State>,
) -> Result<T, Error> {
    run(future, |context| (driver.advance)(state, context).map(|_| ()))
}

/// # Errors
/// Budget exhaustion leaves admitted work owned by its Runtime and scope.
pub fn run<T>(
    future: impl Future<Output = T>,
    mut advance: impl FnMut(&mut Context<'_>) -> Result<(), Error>,
) -> Result<T, Error> {
    let mut future = Box::pin(future);
    let mut context = Context::from_waker(Waker::noop());
    for _ in 0..4096 {
        if let Poll::Ready(result) = future.as_mut().poll(&mut context) {
            return Ok(result);
        }
        advance(&mut context)?;
    }
    Err(Error::BUSY)
}
