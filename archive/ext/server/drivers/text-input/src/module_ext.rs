//! Module extension traits for default mode provider capability.

use std::sync::Arc;

use crate::DefaultModeProvider;

/// Companion trait for modules that declare entry modes.
pub trait DefaultModeProviderModule {
    /// Return the default mode provider if this module has one.
    fn default_mode_provider(&self) -> Option<Arc<dyn DefaultModeProvider>>;
}
