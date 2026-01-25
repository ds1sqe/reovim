//! Startup module loading from configuration.
//!
//! Loads modules based on `~/.config/reovim/config.toml` settings.
//! Supports both dynamic loading from XDG paths and static fallback.

use std::path::PathBuf;

use reovim_kernel::api::v1::{Module, ModuleId};

use {
    super::{ModuleLoader, discovery::find_module},
    crate::server::module::config::ModuleConfig,
};

/// Result of loading a single module.
#[derive(Debug)]
pub enum LoadResult {
    /// Module loaded dynamically from path.
    Dynamic { id: ModuleId, path: PathBuf },
    /// Module registered statically (fallback).
    Static { id: ModuleId },
    /// Module not found in search paths and no static fallback.
    NotFound { name: String },
    /// Module loading failed.
    Failed { name: String, error: String },
}

/// Statistics from startup loading.
#[derive(Debug, Default)]
pub struct StartupLoadStats {
    /// Modules loaded dynamically.
    pub dynamic_loaded: Vec<ModuleId>,
    /// Modules registered statically.
    pub static_loaded: Vec<ModuleId>,
    /// Modules not found.
    pub not_found: Vec<String>,
    /// Modules that failed to load.
    pub failed: Vec<(String, String)>,
}

impl StartupLoadStats {
    /// Check if all requested modules were loaded successfully.
    #[must_use]
    pub const fn all_loaded(&self) -> bool {
        self.not_found.is_empty() && self.failed.is_empty()
    }

    /// Total modules loaded (dynamic + static).
    #[must_use]
    pub const fn total_loaded(&self) -> usize {
        self.dynamic_loaded.len() + self.static_loaded.len()
    }
}

/// Static module factory function type.
///
/// Maps module name to a boxed Module instance.
pub type StaticModuleFactory = fn(&str) -> Option<Box<dyn Module>>;

/// Load modules based on configuration.
///
/// For each module in `config.effective_modules()`:
/// 1. Try to find the module in search paths
/// 2. If found, load dynamically
/// 3. If not found, try static factory fallback
///
/// # Safety
///
/// Loading dynamic modules crosses the FFI boundary. Caller must ensure
/// modules in search paths are ABI-compatible.
///
/// # Arguments
///
/// * `loader` - Module loader to register modules with
/// * `config` - Module configuration with search paths and module list
/// * `static_factory` - Optional fallback for modules not found in search paths
///
/// # Returns
///
/// Statistics about what was loaded.
#[allow(unsafe_code)]
pub fn load_from_config(
    loader: &mut ModuleLoader,
    config: &ModuleConfig,
    static_factory: Option<StaticModuleFactory>,
) -> StartupLoadStats {
    let mut stats = StartupLoadStats::default();
    let search_paths = config.all_search_paths_with_env();
    let modules_to_load = config.effective_modules();

    // Log search paths with priority indication (#433)
    let env_module_path = std::env::var("REOVIM_MODULE_PATH").ok();
    if let Some(ref env_path) = env_module_path {
        tracing::info!(
            env_path = %env_path,
            "REOVIM_MODULE_PATH set (highest priority)"
        );
    }

    tracing::info!(
        first_path = ?search_paths.first(),
        total_paths = search_paths.len(),
        "Module search paths (highest priority first)"
    );
    tracing::debug!(all_paths = ?search_paths, "Full module search path list");

    tracing::info!(
        count = modules_to_load.len(),
        modules = ?modules_to_load,
        "Loading modules"
    );

    for module_name in modules_to_load {
        match load_single_module(loader, &module_name, &search_paths, static_factory) {
            LoadResult::Dynamic { id, path } => {
                tracing::info!(module = %id, path = ?path, "loaded module dynamically");
                stats.dynamic_loaded.push(id);
            }
            LoadResult::Static { id } => {
                tracing::info!(module = %id, "registered module statically");
                stats.static_loaded.push(id);
            }
            LoadResult::NotFound { name } => {
                tracing::warn!(module = %name, "module not found");
                stats.not_found.push(name);
            }
            LoadResult::Failed { name, error } => {
                tracing::error!(module = %name, error = %error, "failed to load module");
                stats.failed.push((name, error));
            }
        }
    }

    stats
}

/// Load a single module by name.
///
/// # Safety
///
/// Dynamic loading crosses FFI boundary.
#[allow(unsafe_code)]
fn load_single_module(
    loader: &mut ModuleLoader,
    name: &str,
    search_paths: &[PathBuf],
    static_factory: Option<StaticModuleFactory>,
) -> LoadResult {
    // First, try to find in search paths
    if let Some(path) = find_module(search_paths, &module_lib_name(name)) {
        // SAFETY: Caller ensures ABI compatibility
        match unsafe { loader.load_dynamic(&path) } {
            Ok(id) => return LoadResult::Dynamic { id, path },
            Err(e) => {
                tracing::warn!(
                    module = %name,
                    path = ?path,
                    error = %e,
                    "dynamic load failed, trying static fallback"
                );
            }
        }
    }

    // Fallback to static factory
    if let Some(factory) = static_factory
        && let Some(module) = factory(name)
    {
        match loader.register_static_boxed(module) {
            Ok(id) => return LoadResult::Static { id },
            Err(e) => {
                return LoadResult::Failed {
                    name: name.to_string(),
                    error: format!("static registration failed: {e}"),
                };
            }
        }
    }

    LoadResult::NotFound {
        name: name.to_string(),
    }
}

/// Convert module name to library name.
///
/// Maps kebab-case module names to underscore library names:
/// - `hot-reload-demo` → `reovim_module_hot_reload_demo`
fn module_lib_name(name: &str) -> String {
    format!("reovim_module_{}", name.replace('-', "_"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_module_lib_name() {
        assert_eq!(module_lib_name("vim"), "reovim_module_vim");
        assert_eq!(module_lib_name("hot-reload-demo"), "reovim_module_hot_reload_demo");
        assert_eq!(module_lib_name("mode-manager"), "reovim_module_mode_manager");
    }

    #[test]
    fn test_startup_load_stats_default() {
        let stats = StartupLoadStats::default();
        assert!(stats.all_loaded());
        assert_eq!(stats.total_loaded(), 0);
    }

    #[test]
    fn test_startup_load_stats_with_data() {
        let mut stats = StartupLoadStats::default();
        stats.dynamic_loaded.push(ModuleId::new("vim"));
        stats.static_loaded.push(ModuleId::new("keymap"));

        assert!(stats.all_loaded());
        assert_eq!(stats.total_loaded(), 2);
    }

    #[test]
    fn test_startup_load_stats_with_failures() {
        let mut stats = StartupLoadStats::default();
        stats.not_found.push("missing".to_string());

        assert!(!stats.all_loaded());
    }
}
