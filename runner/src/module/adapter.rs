//! Plugin adapter for bridging Module and Plugin systems.
//!
//! Provides `PluginFromModule` which wraps a kernel `Module` as a core `Plugin`,
//! allowing modules to integrate with the existing plugin infrastructure.

use std::sync::Arc;

use {
    reovim_core::plugin::{Plugin, PluginContext, PluginId, PluginStateRegistry},
    reovim_kernel::api::v1::ModuleId,
};

use super::handle::ModuleHandle;

/// Wraps a `ModuleHandle` as a `Plugin` for integration with existing system.
///
/// This adapter allows modules loaded via the module system to participate
/// in the plugin infrastructure. It bridges the kernel's `Module` trait with
/// the core's `Plugin` trait.
///
/// # Limitations
///
/// - Dynamic modules have limited adapter support (no direct trait access)
/// - Registration extraction only works for static modules
pub struct PluginFromModule {
    /// Shared handle to the loaded module.
    handle: Arc<ModuleHandle>,

    /// Cached plugin ID derived from module ID.
    plugin_id: PluginId,
}

impl PluginFromModule {
    /// Create a new plugin adapter from a module handle.
    ///
    /// The module ID is converted to a plugin ID with "module:" namespace.
    pub fn new(handle: Arc<ModuleHandle>) -> Self {
        // Convert module ID to plugin ID
        // Module: "lang-rust" -> Plugin: "module:lang-rust"
        let plugin_id_str = format!("module:{}", handle.id().as_str());
        // We need a &'static str for PluginId, so we leak the string
        // This is acceptable because modules are long-lived
        let plugin_id = PluginId::new(Box::leak(plugin_id_str.into_boxed_str()));

        Self { handle, plugin_id }
    }

    /// Get the underlying module ID.
    #[must_use]
    pub fn module_id(&self) -> &ModuleId {
        self.handle.id()
    }

    /// Get the underlying module handle.
    #[must_use]
    pub fn handle(&self) -> &Arc<ModuleHandle> {
        &self.handle
    }
}

impl Plugin for PluginFromModule {
    fn id(&self) -> PluginId {
        self.plugin_id.clone()
    }

    fn name(&self) -> &'static str {
        // Module name is not 'static, so we return a generic name
        // The actual name is available via handle.name()
        "Module Plugin"
    }

    fn description(&self) -> &'static str {
        "Plugin adapter for a loaded module"
    }

    fn build(&self, ctx: &mut PluginContext) {
        // For static modules, we can extract registrations
        if let Some(module) = self.handle.as_module() {
            // Register commands from module
            for cmd in module.commands() {
                tracing::debug!(
                    module = %self.handle.id(),
                    command = cmd.id,
                    "registering module command"
                );
                // Note: CommandRegistration from kernel needs to be adapted
                // to whatever core's PluginContext expects
                // For now, log that we would register it
                let _ = cmd; // Suppress unused warning
            }

            // Register keybindings from module
            for kb in module.keybindings() {
                tracing::debug!(
                    module = %self.handle.id(),
                    "registering module keybinding"
                );
                let _ = kb;
            }

            // Register event handlers from module
            for eh in module.event_handlers() {
                tracing::debug!(
                    module = %self.handle.id(),
                    "registering module event handler"
                );
                let _ = eh;
            }
        } else {
            // Dynamic modules: registration would need FFI support
            tracing::debug!(
                module = %self.handle.id(),
                "dynamic module - registration extraction not yet supported"
            );
        }

        let _ = ctx; // Suppress unused warning for now
    }

    fn init_state(&self, registry: &PluginStateRegistry) {
        // Modules manage their own state via ModuleContext
        // This is a no-op for the adapter
        let _ = registry;
    }
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        reovim_kernel::api::v1::{Module, ModuleContext, ModuleError, ProbeResult, Version},
    };

    struct TestModule;

    impl Module for TestModule {
        fn id(&self) -> ModuleId {
            ModuleId::new("test-module")
        }

        fn name(&self) -> &'static str {
            "Test Module"
        }

        fn version(&self) -> Version {
            Version::new(1, 0, 0)
        }

        fn init(&mut self, _ctx: &ModuleContext) -> ProbeResult {
            ProbeResult::Success
        }

        fn exit(&mut self) -> Result<(), ModuleError> {
            Ok(())
        }
    }

    #[test]
    fn test_plugin_id_conversion() {
        let handle = Arc::new(ModuleHandle::from_static(TestModule));
        let plugin = PluginFromModule::new(handle);

        assert!(plugin.id().as_str().starts_with("module:"));
        assert!(plugin.id().as_str().contains("test-module"));
    }

    #[test]
    fn test_module_id_access() {
        let handle = Arc::new(ModuleHandle::from_static(TestModule));
        let plugin = PluginFromModule::new(handle);

        assert_eq!(plugin.module_id().as_str(), "test-module");
    }
}
