//! Module extension traits for default mode provider capability.
//!
//! This module defines companion traits that modules can implement
//! alongside the core `Module` trait to provide default mode capabilities.
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
//! use reovim_driver_input::{DefaultModeProvider, DefaultModeProviderModule};
//! use std::sync::Arc;
//!
//! struct VimModule;
//!
//! impl DefaultModeProviderModule for VimModule {
//!     fn default_mode_provider(&self) -> Option<Arc<dyn DefaultModeProvider>> {
//!         Some(Arc::new(VimDefaultModeProvider))
//!     }
//! }
//! ```

use std::sync::Arc;

use crate::DefaultModeProvider;

/// Companion trait for modules that declare entry modes.
///
/// Modules implement this alongside the `Module` trait to declare
/// themselves as the default mode provider.
///
/// # Usage
///
/// The runner checks if loaded modules implement this trait and
/// collects their mode providers into the `DefaultModeProviderRegistry`.
///
/// # Single Entry Mode
///
/// Unlike VFS providers (which can provide multiple schemes), a module
/// typically provides only one default mode provider. Returns `None`
/// if the module doesn't want to provide a default mode.
pub trait DefaultModeProviderModule {
    /// Return the default mode provider if this module has one.
    ///
    /// Called during module loading to collect mode providers.
    /// Returns `None` if the module doesn't declare an entry mode.
    fn default_mode_provider(&self) -> Option<Arc<dyn DefaultModeProvider>>;
}
