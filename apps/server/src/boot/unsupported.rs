//! Hosted service remains available where the native adapter is unavailable.

#[path = "config.rs"]
mod config;
pub use config::{Config, Scope, Session, Source};

use alloc::boxed::Box;
use reovim_kernel_interface::{Error, RequestError};
use reovim_kernel_runtime::{Binding, Runtime};

pub struct Modules;
pub type Startup = core::pin::Pin<
    Box<dyn core::future::Future<Output = Result<(), RequestError>>>,
>;

impl Modules {
    /// # Errors
    /// Native artifacts and contract startup need a supported Host adapter.
    pub fn discover(config: Config) -> Result<Self, Error> {
        if !config.sources.is_empty()
            || !config.start.is_empty()
            || !config.client_start.is_empty()
        {
            return Err(Error::UNSUPPORTED);
        }
        Ok(Self)
    }

    /// # Errors
    /// An empty boot configuration has no startup work to reject.
    pub fn prepare(
        &self,
        _: &mut Runtime,
        _: &Binding,
        _: Scope,
    ) -> Result<Startup, RequestError> {
        Ok(Box::pin(core::future::ready(Ok(()))))
    }
}
