//! Passive configured artifacts; composition supplies registration authority.

#[path = "boot/config.rs"]
mod config;
pub use config::{Config, Source};
#[path = "boot/startup.rs"]
mod startup;
pub use startup::Startup;

use alloc::{rc::Rc, vec::Vec};
use reovim_kernel_interface::{
    Error, ResourceId, declaration::ValueLimits, module::Definition,
};
use reovim_kernel_runtime::{Grant, Runtime};
#[cfg(target_os = "linux")]
use std::os::unix::ffi::OsStrExt;

use crate::loading::{Admit, Target};

/// Captured native definitions, selected by explicit configuration labels.
/// Discovery runs Host admission but never constructs a module instance.
pub struct Modules {
    config: Config,
    definitions: Vec<Rc<Definition>>,
}

impl Modules {
    /// Captures configured artifacts through the supplied Host trust policy.
    /// For example, an allowlisted `Admit` can call the native Host loader.
    /// # Errors
    /// Invalid configuration, denied artifacts or invalid metadata reject
    /// before any Runtime registration or module construction.
    pub fn discover(
        config: Config,
        admit: &Admit,
        values: ValueLimits,
        capacity: usize,
    ) -> Result<Self, Error> {
        config.validate()?;
        let mut definitions = Vec::new();
        definitions
            .try_reserve_exact(config.sources.len())
            .map_err(|_| Error::PRESSURE)?;
        for source in &config.sources {
            let native = admit_source(source, admit)?;
            definitions.push(Rc::new(native.definition(values, capacity)?));
        }
        Ok(Self { config, definitions })
    }

    /// Returns the captured configuration, e.g. required startup labels.
    #[must_use]
    pub const fn config(&self) -> &Config {
        &self.config
    }

    /// Registers an exact configured definition and grants its recipient.
    /// The Target supplies actual registry, mount and immutable import owners.
    /// This admits a passive package; startup uses its ordinary activation API.
    /// # Errors
    /// Unknown labels and registration failures reject. A failed recipient
    /// grant retains the registered package identity in the error. Retrying
    /// uses the same captured definition, even if the artifact was replaced.
    pub fn register(
        &self,
        label: &str,
        runtime: &mut Runtime,
        target: &Target,
    ) -> Result<ResourceId, Error> {
        let index = self
            .config
            .sources
            .iter()
            .position(|source| source.label == label)
            .ok_or(Error::UNAVAILABLE)?;
        let package = crate::loading::register(
            runtime,
            target,
            self.definitions[index].clone(),
        )?;
        runtime
            .grant(
                &target.recipient,
                Grant {
                    resource: package,
                    access: 5,
                    create_access: 0,
                    retain_loans: false,
                },
            )
            .map_err(|error| Error::with_id(error.code.0, package))?;
        Ok(package)
    }
}

#[cfg(target_os = "linux")]
fn admit_source(
    source: &Source,
    admit: &Admit,
) -> Result<reovim_sdk::NativeModule, Error> {
    admit(source.library.as_os_str().as_bytes())
}

#[cfg(not(target_os = "linux"))]
fn admit_source(
    _: &Source,
    _: &Admit,
) -> Result<reovim_sdk::NativeModule, Error> {
    Err(Error::UNSUPPORTED)
}

#[cfg(all(test, target_os = "linux"))]
#[path = "boot/tests.rs"]
mod tests;
