//! Module load report — populated during bootstrap to expose module status
//! to health-check diagnostics (#610).

use std::path::PathBuf;

use reovim_kernel::api::v1::{ModuleId, Service};

/// Report of module loading results, registered in `ServiceRegistry`.
///
/// Populated by bootstrap after the module initialization loop.
/// Health-check reads this to display module status, dependency issues,
/// and configuration paths.
pub struct ModuleLoadReport {
    /// Modules that initialized successfully.
    pub loaded: Vec<ModuleId>,
    /// Modules explicitly disabled by user config.
    pub disabled: Vec<ModuleId>,
    /// Modules that failed to initialize, with error messages.
    pub failed: Vec<(ModuleId, String)>,
    /// Required dependency not available: `(module, missing_dep)`.
    pub missing_deps: Vec<(ModuleId, ModuleId)>,
    /// Path to `modules.toml` (if found).
    pub config_path: Option<PathBuf>,
    /// Module search paths used for `.so` discovery.
    pub search_paths: Vec<PathBuf>,
    /// Whether env var isolation is active.
    pub isolation_active: bool,
}

impl ModuleLoadReport {
    /// Create a new empty report.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            loaded: Vec::new(),
            disabled: Vec::new(),
            failed: Vec::new(),
            missing_deps: Vec::new(),
            config_path: None,
            search_paths: Vec::new(),
            isolation_active: false,
        }
    }

    /// Total number of modules attempted (loaded + failed + disabled).
    #[must_use]
    pub const fn total_count(&self) -> usize {
        self.loaded.len() + self.failed.len() + self.disabled.len()
    }
}

impl Default for ModuleLoadReport {
    fn default() -> Self {
        Self::new()
    }
}

impl Service for ModuleLoadReport {}

#[allow(clippy::missing_fields_in_debug)]
impl std::fmt::Debug for ModuleLoadReport {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ModuleLoadReport")
            .field("loaded", &self.loaded.len())
            .field("disabled", &self.disabled.len())
            .field("failed", &self.failed.len())
            .finish()
    }
}

#[cfg(test)]
#[path = "report_tests.rs"]
mod tests;
