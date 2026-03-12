//! Module distribution registry for reovim (#621).
//!
//! Provides infrastructure for installing, updating, and removing third-party
//! modules from git repositories and local paths.
//!
//! # Architecture
//!
//! This is a **driver** (mechanism layer). It provides:
//! - `module.toml` manifest parsing
//! - Module source types (git, local path)
//! - Installed module metadata tracking
//! - Install/remove/update operations
//!
//! Policy decisions (which modules to install, UI) are in the app layer
//! and the module-manager TUI extension.
//!
//! # Module Manifest (`module.toml`)
//!
//! Third-party modules include a `module.toml` at their crate root:
//!
//! ```toml
//! [module]
//! id = "my-module"
//! name = "My Module"
//! version = "1.0.0"
//! description = "A cool module"
//! authors = ["Alice <alice@example.com>"]
//!
//! [capabilities]
//! provides = ["my-cap"]
//! requires = ["lsp-provider"]
//!
//! [dependencies]
//! vim = "^0.9.0"
//!
//! [build]
//! crate-name = "my-module"
//! ```

mod installed;
mod manifest;
mod source;

pub use {
    installed::{InstalledModule, InstalledModules},
    manifest::{ManifestError, ModuleManifest},
    source::ModuleSource,
};

#[cfg(test)]
#[path = "lib_tests.rs"]
mod tests;
