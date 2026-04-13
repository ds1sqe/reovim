//! Module extension traits for VFS provider capability.
//!
//! This module defines companion traits that modules can implement
//! alongside the core `Module` trait to provide VFS capabilities.
//!
//! # Design
//!
//! Following the kernel's pattern (e.g., `CommandProvider` companion trait),
//! these traits extend module capabilities without modifying the core
//! `Module` trait.
//!
//! # Example
//!
//! ```ignore
//! use reovim_subsys_vfs::{VfsProvider, VfsProviderModule};
//! use std::sync::Arc;
//!
//! struct MyModule;
//!
//! impl VfsProviderModule for MyModule {
//!     fn vfs_providers(&self) -> Vec<Arc<dyn VfsProvider>> {
//!         vec![Arc::new(MyVfsProvider::new())]
//!     }
//! }
//! ```

use std::sync::Arc;

use crate::VfsProvider;

/// Companion trait for modules that provide VFS implementations.
///
/// Modules implement this alongside the `Module` trait to declare
/// VFS providers during initialization.
///
/// # Usage
///
/// The runner checks if loaded modules implement this trait and
/// collects their VFS providers into the `VfsProviderRegistry`.
pub trait VfsProviderModule {
    /// Return VFS providers this module offers.
    ///
    /// Called during module loading to collect all VFS providers.
    /// Multiple providers can be returned (e.g., for different schemes).
    fn vfs_providers(&self) -> Vec<Arc<dyn VfsProvider>>;
}
