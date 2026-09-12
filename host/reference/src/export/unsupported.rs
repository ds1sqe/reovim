//! Filesystem output needs a Host adapter with equivalent containment rules.

use reovim_kernel_interface::provider::ResourceProvider;
use std::{io, path::Path};

pub struct Directory(core::convert::Infallible);

impl Directory {
    /// # Errors
    /// This target has no admitted filesystem export adapter.
    pub fn open(_: &Path) -> io::Result<Self> {
        Err(io::Error::new(io::ErrorKind::Unsupported, "filesystem export"))
    }

    /// # Errors
    /// No Directory can be constructed on this target.
    pub fn create(&self, _: &str) -> io::Result<Self> {
        match self.0 {}
    }

    #[must_use]
    pub fn provider(&self) -> ResourceProvider {
        match self.0 {}
    }

    /// # Errors
    /// No Directory can be constructed on this target.
    pub fn private_provider(&self, _: &str) -> io::Result<ResourceProvider> {
        match self.0 {}
    }
}
