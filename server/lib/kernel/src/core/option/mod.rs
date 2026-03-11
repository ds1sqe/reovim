//! Option registry mechanism for editor settings.
//!
//! Linux equivalent: `/proc/sys/` configuration interface
//!
//! This module provides the **MECHANISM** for option registration, storage,
//! and retrieval. **POLICY** (which options exist, what they do) is defined
//! by modules.
//!
//! # Design Philosophy
//!
//! Following "mechanism, not policy":
//! - Kernel provides `OptionRegistry` with type-safe value storage
//! - Modules register `OptionSpec` definitions during initialization
//! - Scope-aware storage (Global, Buffer, Window) with automatic fallback
//!
//! # Scope Resolution
//!
//! When getting an option value, the registry checks:
//! 1. Window-local value (if scope is Window)
//! 2. Buffer-local value (if scope is Buffer or Window)
//! 3. Global override value
//! 4. Default value from spec
//!
//! # Example
//!
//! ```ignore
//! use reovim_kernel::api::v1::*;
//!
//! let registry = OptionRegistry::new();
//!
//! // Register an option (done by modules)
//! registry.register(
//!     OptionSpec::new("number", "Show line numbers", OptionValue::Bool(false))
//!         .with_short("nu")
//!         .with_scope(OptionScope::Window)
//! )?;
//!
//! // Get/set values
//! registry.set("number", OptionValue::Bool(true), OptionScopeId::Global)?;
//! let value = registry.get("number", OptionScopeId::Global);
//! ```

mod constraint;
mod error;
mod scope;
mod spec;
mod value;

// Re-export all types for API compatibility
pub use {
    constraint::{ConstraintError, OptionConstraint},
    error::{OptionError, SetResult},
    scope::{OptionScope, OptionScopeId},
    spec::OptionSpec,
    value::OptionValue,
};

use std::collections::HashMap;

use reovim_arch::sync::RwLock;

use crate::{
    api::ModuleId,
    mm::{BufferId, WindowId},
};

// ============================================================================
// OptionRegistry - Thread-safe option storage
// ============================================================================

/// Thread-safe registry for all editor options.
///
/// This is the **MECHANISM** for option storage.
/// **POLICY** (which options to register) is in modules.
///
/// # Storage Model
///
/// ```text
/// OptionRegistry
/// ├── specs: HashMap<String, OptionSpec>              # Option definitions
/// ├── aliases: HashMap<String, String>                # Short -> Full name
/// ├── global_values: HashMap<String, OptionValue>     # Global overrides
/// ├── buffer_values: HashMap<(BufferId, String), OptionValue>   # Per-buffer
/// └── window_values: HashMap<(WindowId, String), OptionValue>   # Per-window
/// ```
///
/// # Thread Safety
///
/// All operations use `RwLock` for thread-safe access.
/// Multiple readers allowed, single writer for mutations.
#[derive(Debug, Default)]
pub struct OptionRegistry {
    /// Registered option specs indexed by full name.
    specs: RwLock<HashMap<String, OptionSpec>>,

    /// Short name -> full name mapping.
    aliases: RwLock<HashMap<String, String>>,

    /// Global option values (overrides from defaults).
    global_values: RwLock<HashMap<String, OptionValue>>,

    /// Buffer-local option values.
    buffer_values: RwLock<HashMap<(BufferId, String), OptionValue>>,

    /// Window-local option values.
    window_values: RwLock<HashMap<(WindowId, String), OptionValue>>,
}

impl OptionRegistry {
    /// Create a new empty option registry.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    // ========================================================================
    // Registration
    // ========================================================================

    /// Register an option specification.
    ///
    /// # Errors
    ///
    /// Returns error if:
    /// - Option with same name already exists
    /// - Short alias conflicts with existing name or alias
    pub fn register(&self, spec: OptionSpec) -> Result<(), OptionError> {
        let name = spec.name.to_string();

        // Check for duplicate name first
        if self.specs.read().contains_key(&name) {
            return Err(OptionError::AlreadyExists(name));
        }

        // Handle alias registration if present
        if let Some(ref short) = spec.short_form {
            let short_str = short.to_string();

            // Alias conflicts with existing full name
            if self.specs.read().contains_key(&short_str) {
                return Err(OptionError::AliasConflict(short_str));
            }

            // Alias conflicts with existing alias
            if self.aliases.read().contains_key(&short_str) {
                return Err(OptionError::AliasConflict(short_str));
            }

            // Register the alias
            self.aliases.write().insert(short_str, name.clone());
        }

        // Finally insert the spec
        self.specs.write().insert(name, spec);
        Ok(())
    }

    /// Unregister all options owned by a module.
    ///
    /// Removes all option specs with matching owner, their aliases,
    /// and any stored values (global, buffer-local, window-local).
    pub fn unregister_by_module(&self, module_id: &ModuleId) {
        // Collect names to remove
        let names_to_remove: Vec<String> = self
            .specs
            .read()
            .iter()
            .filter(|(_, spec)| spec.owner.as_ref() == Some(module_id))
            .map(|(name, _)| name.clone())
            .collect();

        if names_to_remove.is_empty() {
            return;
        }

        // Remove aliases pointing to these options
        let mut aliases = self.aliases.write();
        aliases.retain(|_, full_name| !names_to_remove.contains(full_name));
        drop(aliases);

        // Remove stored values
        let mut global_values = self.global_values.write();
        global_values.retain(|name, _| !names_to_remove.contains(name));
        drop(global_values);

        let mut buffer_values = self.buffer_values.write();
        buffer_values.retain(|(_, name), _| !names_to_remove.contains(name));
        drop(buffer_values);

        let mut window_values = self.window_values.write();
        window_values.retain(|(_, name), _| !names_to_remove.contains(name));
        drop(window_values);

        // Remove specs
        let mut specs = self.specs.write();
        specs.retain(|name, _| !names_to_remove.contains(name));
    }

    /// List all options owned by a module.
    #[must_use]
    pub fn list_by_module(&self, module_id: &ModuleId) -> Vec<OptionSpec> {
        let specs = self.specs.read();
        specs
            .values()
            .filter(|s| s.owner.as_ref() == Some(module_id))
            .cloned()
            .collect()
    }

    // ========================================================================
    // Name Resolution
    // ========================================================================

    /// Resolve a name (which may be an alias) to the full option name.
    #[must_use]
    pub fn resolve_name(&self, name: &str) -> Option<String> {
        // Check if it's a direct spec name
        if self.specs.read().contains_key(name) {
            return Some(name.to_string());
        }

        // Check if it's an alias
        self.aliases.read().get(name).cloned()
    }

    /// Get an option specification by name (supports aliases).
    #[must_use]
    pub fn get_spec(&self, name: &str) -> Option<OptionSpec> {
        let full_name = self.resolve_name(name)?;
        let specs = self.specs.read();
        specs.get(&full_name).cloned()
    }

    /// Check if an option exists.
    #[must_use]
    pub fn contains(&self, name: &str) -> bool {
        self.resolve_name(name).is_some()
    }

    // ========================================================================
    // Value Access - Scope-aware
    // ========================================================================

    /// Get the effective value of an option for a given scope.
    ///
    /// Resolution order (most specific wins):
    /// 1. Window-local value (if `scope` is `Window`)
    /// 2. Buffer-local value (if `scope` is `Buffer` or has buffer context)
    /// 3. Global value (if set)
    /// 4. Default value from spec
    #[must_use]
    pub fn get(&self, name: &str, scope: OptionScopeId) -> Option<OptionValue> {
        let full_name = self.resolve_name(name)?;

        // Get default value from spec first, then release lock
        let default_value = self.specs.read().get(&full_name)?.default.clone();

        // Check scope-specific values
        match scope {
            OptionScopeId::Window(window_id) => {
                // Check window-local first
                if let Some(value) = self
                    .window_values
                    .read()
                    .get(&(window_id, full_name.clone()))
                {
                    return Some(value.clone());
                }
            }
            OptionScopeId::Buffer(buffer_id) => {
                // Check buffer-local
                if let Some(value) = self
                    .buffer_values
                    .read()
                    .get(&(buffer_id, full_name.clone()))
                {
                    return Some(value.clone());
                }
            }
            OptionScopeId::Global => {}
        }

        // Check global override
        if let Some(value) = self.global_values.read().get(&full_name) {
            return Some(value.clone());
        }

        // Return default
        Some(default_value)
    }

    /// Get global value (ignoring scope context).
    #[must_use]
    pub fn get_global(&self, name: &str) -> Option<OptionValue> {
        self.get(name, OptionScopeId::Global)
    }

    /// Get buffer-local value.
    #[must_use]
    pub fn get_for_buffer(&self, name: &str, buffer_id: BufferId) -> Option<OptionValue> {
        self.get(name, OptionScopeId::Buffer(buffer_id))
    }

    /// Get window-local value.
    #[must_use]
    pub fn get_for_window(&self, name: &str, window_id: WindowId) -> Option<OptionValue> {
        self.get(name, OptionScopeId::Window(window_id))
    }

    // ========================================================================
    // Value Setting
    // ========================================================================

    /// Set an option value at a specific scope.
    ///
    /// # Errors
    ///
    /// Returns error if:
    /// - Option not found
    /// - Value fails validation
    /// - Scope mismatch
    pub fn set(
        &self,
        name: &str,
        value: OptionValue,
        scope: OptionScopeId,
    ) -> Result<SetResult, OptionError> {
        let full_name = self
            .resolve_name(name)
            .ok_or_else(|| OptionError::NotFound(name.to_string()))?;

        let specs = self.specs.read();
        let spec = specs
            .get(&full_name)
            .ok_or_else(|| OptionError::NotFound(full_name.clone()))?;

        // Validate value
        spec.validate(&value)?;

        // Check scope compatibility: only global-scoped options cannot have local overrides
        if matches!(
            (&scope, &spec.scope),
            (OptionScopeId::Buffer(_) | OptionScopeId::Window(_), OptionScope::Global)
        ) {
            return Err(OptionError::ScopeMismatch {
                name: full_name,
                option_scope: spec.scope,
                requested: scope,
            });
        }

        drop(specs); // Release read lock before acquiring write lock

        // Set the value
        let old_value = match scope {
            OptionScopeId::Global => {
                let mut global_values = self.global_values.write();
                global_values.insert(full_name, value.clone())
            }
            OptionScopeId::Buffer(buffer_id) => {
                let mut buffer_values = self.buffer_values.write();
                buffer_values.insert((buffer_id, full_name), value.clone())
            }
            OptionScopeId::Window(window_id) => {
                let mut window_values = self.window_values.write();
                window_values.insert((window_id, full_name), value.clone())
            }
        };

        Ok(SetResult {
            old_value,
            new_value: value,
        })
    }

    /// Set global value.
    ///
    /// # Errors
    ///
    /// Returns error if option not found or validation fails.
    pub fn set_global(&self, name: &str, value: OptionValue) -> Result<SetResult, OptionError> {
        self.set(name, value, OptionScopeId::Global)
    }

    /// Set buffer-local value.
    ///
    /// # Errors
    ///
    /// Returns error if option not found, validation fails, or scope mismatch.
    pub fn set_for_buffer(
        &self,
        name: &str,
        value: OptionValue,
        buffer_id: BufferId,
    ) -> Result<SetResult, OptionError> {
        self.set(name, value, OptionScopeId::Buffer(buffer_id))
    }

    /// Set window-local value.
    ///
    /// # Errors
    ///
    /// Returns error if option not found, validation fails, or scope mismatch.
    pub fn set_for_window(
        &self,
        name: &str,
        value: OptionValue,
        window_id: WindowId,
    ) -> Result<SetResult, OptionError> {
        self.set(name, value, OptionScopeId::Window(window_id))
    }

    // ========================================================================
    // Reset
    // ========================================================================

    /// Reset an option to its default value at a specific scope.
    ///
    /// # Errors
    ///
    /// Returns error if option not found.
    pub fn reset(
        &self,
        name: &str,
        scope: OptionScopeId,
    ) -> Result<Option<OptionValue>, OptionError> {
        let full_name = self
            .resolve_name(name)
            .ok_or_else(|| OptionError::NotFound(name.to_string()))?;

        let removed = match scope {
            OptionScopeId::Global => {
                let mut global_values = self.global_values.write();
                global_values.remove(&full_name)
            }
            OptionScopeId::Buffer(buffer_id) => {
                let mut buffer_values = self.buffer_values.write();
                buffer_values.remove(&(buffer_id, full_name))
            }
            OptionScopeId::Window(window_id) => {
                let mut window_values = self.window_values.write();
                window_values.remove(&(window_id, full_name))
            }
        };

        Ok(removed)
    }

    /// Reset all buffer-local values for a buffer (called when buffer closes).
    pub fn clear_buffer(&self, buffer_id: BufferId) {
        let mut buffer_values = self.buffer_values.write();
        buffer_values.retain(|(bid, _), _| *bid != buffer_id);
    }

    /// Reset all window-local values for a window (called when window closes).
    pub fn clear_window(&self, window_id: WindowId) {
        let mut window_values = self.window_values.write();
        window_values.retain(|(wid, _), _| *wid != window_id);
    }

    // ========================================================================
    // Toggle
    // ========================================================================

    /// Toggle a boolean option.
    ///
    /// # Errors
    ///
    /// Returns error if option not found or is not boolean.
    pub fn toggle(&self, name: &str, scope: OptionScopeId) -> Result<bool, OptionError> {
        let current = self
            .get(name, scope)
            .ok_or_else(|| OptionError::NotFound(name.to_string()))?;

        let current_bool = current.as_bool().ok_or_else(|| OptionError::TypeMismatch {
            name: name.to_string(),
            expected: "bool",
            got: current.type_name(),
        })?;

        let new_value = !current_bool;
        self.set(name, OptionValue::Bool(new_value), scope)?;
        Ok(new_value)
    }

    // ========================================================================
    // Query
    // ========================================================================

    /// List all registered option names.
    #[must_use]
    pub fn list_all(&self) -> Vec<String> {
        let specs = self.specs.read();
        specs.keys().cloned().collect()
    }

    /// List options matching a prefix (for tab completion).
    #[must_use]
    #[cfg_attr(coverage_nightly, coverage(off))]
    pub fn list_matching(&self, prefix: &str) -> Vec<OptionSpec> {
        let specs = self.specs.read();

        let mut results = Vec::new();

        for (name, spec) in specs.iter() {
            if name.starts_with(prefix) {
                results.push(spec.clone());
            }
        }

        // Also check aliases
        let aliases = self.aliases.read();
        for (alias, full_name) in aliases.iter() {
            if alias.starts_with(prefix)
                && let Some(spec) = specs.get(full_name)
                && !results.iter().any(|s| s.name == spec.name)
            {
                results.push(spec.clone());
            }
        }
        drop(aliases);

        results
    }

    /// List options by scope.
    #[must_use]
    pub fn list_by_scope(&self, scope: OptionScope) -> Vec<OptionSpec> {
        let specs = self.specs.read();
        specs
            .values()
            .filter(|s| s.scope == scope)
            .cloned()
            .collect()
    }

    /// Get the number of registered options.
    #[must_use]
    pub fn len(&self) -> usize {
        let specs = self.specs.read();
        specs.len()
    }

    /// Check if the registry is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        let specs = self.specs.read();
        specs.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_registry_register() {
        let registry = OptionRegistry::new();

        let result = registry.register(OptionSpec::new(
            "number",
            "Show line numbers",
            OptionValue::bool(false),
        ));
        assert!(result.is_ok());
        assert!(registry.contains("number"));
    }

    #[test]
    fn test_registry_register_duplicate() {
        let registry = OptionRegistry::new();

        assert!(
            registry
                .register(OptionSpec::new("number", "desc", OptionValue::bool(false)))
                .is_ok()
        );

        let result = registry.register(OptionSpec::new("number", "desc2", OptionValue::bool(true)));
        assert!(matches!(result, Err(OptionError::AlreadyExists(_))));
    }

    #[test]
    fn test_registry_alias() {
        let registry = OptionRegistry::new();

        assert!(
            registry
                .register(
                    OptionSpec::new("number", "Show line numbers", OptionValue::bool(false))
                        .with_short("nu"),
                )
                .is_ok()
        );

        assert!(registry.contains("number"));
        assert!(registry.contains("nu"));
        assert_eq!(registry.resolve_name("nu"), Some("number".to_string()));
    }

    #[test]
    fn test_registry_get_default() {
        let registry = OptionRegistry::new();

        assert!(
            registry
                .register(OptionSpec::new("tabwidth", "Tab width", OptionValue::int(4)))
                .is_ok()
        );

        let value = registry.get("tabwidth", OptionScopeId::Global);
        assert_eq!(value, Some(OptionValue::int(4)));
    }

    #[test]
    fn test_registry_set_and_get() {
        let registry = OptionRegistry::new();

        assert!(
            registry
                .register(OptionSpec::new("tabwidth", "Tab width", OptionValue::int(4)))
                .is_ok()
        );

        assert!(
            registry
                .set("tabwidth", OptionValue::int(8), OptionScopeId::Global)
                .is_ok()
        );

        let value = registry.get("tabwidth", OptionScopeId::Global);
        assert_eq!(value, Some(OptionValue::int(8)));
    }

    #[test]
    fn test_registry_buffer_local() {
        let registry = OptionRegistry::new();

        assert!(
            registry
                .register(
                    OptionSpec::new("tabwidth", "Tab width", OptionValue::int(4))
                        .with_scope(OptionScope::Buffer),
                )
                .is_ok()
        );

        let buffer1 = BufferId::new();
        let buffer2 = BufferId::new();

        // Set buffer-local value for buffer1
        assert!(
            registry
                .set_for_buffer("tabwidth", OptionValue::int(2), buffer1)
                .is_ok()
        );

        // buffer1 should have 2, buffer2 should have default (4)
        assert_eq!(registry.get_for_buffer("tabwidth", buffer1), Some(OptionValue::int(2)));
        assert_eq!(registry.get_for_buffer("tabwidth", buffer2), Some(OptionValue::int(4)));
    }

    #[test]
    fn test_registry_window_local() {
        let registry = OptionRegistry::new();

        assert!(
            registry
                .register(
                    OptionSpec::new("number", "Show line numbers", OptionValue::bool(false))
                        .with_scope(OptionScope::Window),
                )
                .is_ok()
        );

        let window1 = WindowId::new();
        let window2 = WindowId::new();

        assert!(
            registry
                .set_for_window("number", OptionValue::bool(true), window1)
                .is_ok()
        );

        assert_eq!(registry.get_for_window("number", window1), Some(OptionValue::bool(true)));
        assert_eq!(registry.get_for_window("number", window2), Some(OptionValue::bool(false)));
    }

    #[test]
    fn test_registry_scope_mismatch() {
        let registry = OptionRegistry::new();

        assert!(
            registry
                .register(
                    OptionSpec::new("theme", "Color theme", OptionValue::string("default"))
                        .with_scope(OptionScope::Global),
                )
                .is_ok()
        );

        let result = registry.set_for_buffer("theme", OptionValue::string("dark"), BufferId::new());
        assert!(matches!(result, Err(OptionError::ScopeMismatch { .. })));
    }

    #[test]
    fn test_registry_reset() {
        let registry = OptionRegistry::new();

        assert!(
            registry
                .register(OptionSpec::new("tabwidth", "Tab width", OptionValue::int(4)))
                .is_ok()
        );

        assert!(
            registry
                .set("tabwidth", OptionValue::int(8), OptionScopeId::Global)
                .is_ok()
        );
        assert_eq!(registry.get("tabwidth", OptionScopeId::Global), Some(OptionValue::int(8)));

        assert!(registry.reset("tabwidth", OptionScopeId::Global).is_ok());
        assert_eq!(registry.get("tabwidth", OptionScopeId::Global), Some(OptionValue::int(4)));
    }

    #[test]
    fn test_registry_toggle() {
        let registry = OptionRegistry::new();

        assert!(
            registry
                .register(OptionSpec::new("number", "Show line numbers", OptionValue::bool(false)))
                .is_ok()
        );

        let result = registry.toggle("number", OptionScopeId::Global);
        assert!(result.is_ok());
        assert!(result.is_ok_and(|v| v));

        let result = registry.toggle("number", OptionScopeId::Global);
        assert!(result.is_ok());
        assert!(result.is_ok_and(|v| !v));
    }

    #[test]
    fn test_registry_list_matching() {
        let registry = OptionRegistry::new();

        assert!(
            registry
                .register(OptionSpec::new("number", "desc", OptionValue::bool(false)))
                .is_ok()
        );
        assert!(
            registry
                .register(OptionSpec::new("numberwidth", "desc", OptionValue::int(4)))
                .is_ok()
        );
        assert!(
            registry
                .register(OptionSpec::new("wrap", "desc", OptionValue::bool(true)))
                .is_ok()
        );

        let matches = registry.list_matching("num");
        assert_eq!(matches.len(), 2);
    }

    #[test]
    fn test_registry_clear_buffer() {
        let registry = OptionRegistry::new();

        assert!(
            registry
                .register(
                    OptionSpec::new("tabwidth", "Tab width", OptionValue::int(4))
                        .with_scope(OptionScope::Buffer),
                )
                .is_ok()
        );

        let buffer = BufferId::new();
        assert!(
            registry
                .set_for_buffer("tabwidth", OptionValue::int(2), buffer)
                .is_ok()
        );

        assert_eq!(registry.get_for_buffer("tabwidth", buffer), Some(OptionValue::int(2)));

        registry.clear_buffer(buffer);

        // Should be back to default
        assert_eq!(registry.get_for_buffer("tabwidth", buffer), Some(OptionValue::int(4)));
    }

    #[test]
    fn test_registry_validation_on_set() {
        let registry = OptionRegistry::new();

        assert!(
            registry
                .register(
                    OptionSpec::new("tabwidth", "Tab width", OptionValue::int(4))
                        .with_constraint(OptionConstraint::range(1, 32)),
                )
                .is_ok()
        );

        // Valid value
        assert!(
            registry
                .set("tabwidth", OptionValue::int(8), OptionScopeId::Global)
                .is_ok()
        );

        // Invalid value
        assert!(
            registry
                .set("tabwidth", OptionValue::int(100), OptionScopeId::Global)
                .is_err()
        );
    }

    // === toggle non-boolean ===

    #[test]
    fn test_registry_toggle_non_boolean() {
        let registry = OptionRegistry::new();
        assert!(
            registry
                .register(OptionSpec::new("tabwidth", "desc", OptionValue::int(4)))
                .is_ok()
        );
        let result = registry.toggle("tabwidth", OptionScopeId::Global);
        assert!(matches!(result, Err(OptionError::TypeMismatch { .. })));
    }

    // === list_matching with alias ===

    #[test]
    fn test_registry_list_matching_alias() {
        let registry = OptionRegistry::new();
        assert!(
            registry
                .register(
                    OptionSpec::new("number", "desc", OptionValue::bool(false)).with_short("nu"),
                )
                .is_ok()
        );
        // Prefix "nu" should match via alias
        let matches = registry.list_matching("nu");
        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].name, "number");
    }

    // === list_by_scope ===

    #[test]
    fn test_registry_list_by_scope() {
        let registry = OptionRegistry::new();
        assert!(
            registry
                .register(
                    OptionSpec::new("number", "desc", OptionValue::bool(false))
                        .with_scope(OptionScope::Window),
                )
                .is_ok()
        );
        assert!(
            registry
                .register(
                    OptionSpec::new("tabwidth", "desc", OptionValue::int(4))
                        .with_scope(OptionScope::Buffer),
                )
                .is_ok()
        );
        assert!(
            registry
                .register(
                    OptionSpec::new("theme", "desc", OptionValue::string("dark"))
                        .with_scope(OptionScope::Global),
                )
                .is_ok()
        );

        let window_opts = registry.list_by_scope(OptionScope::Window);
        assert_eq!(window_opts.len(), 1);
        assert_eq!(window_opts[0].name, "number");

        let buffer_opts = registry.list_by_scope(OptionScope::Buffer);
        assert_eq!(buffer_opts.len(), 1);

        let global_opts = registry.list_by_scope(OptionScope::Global);
        assert_eq!(global_opts.len(), 1);
    }

    // === clear_window ===

    #[test]
    fn test_registry_clear_window() {
        let registry = OptionRegistry::new();
        assert!(
            registry
                .register(
                    OptionSpec::new("number", "desc", OptionValue::bool(false))
                        .with_scope(OptionScope::Window),
                )
                .is_ok()
        );

        let window = WindowId::new();
        assert!(
            registry
                .set_for_window("number", OptionValue::bool(true), window)
                .is_ok()
        );
        assert_eq!(registry.get_for_window("number", window), Some(OptionValue::bool(true)));

        registry.clear_window(window);
        // Should fall back to default
        assert_eq!(registry.get_for_window("number", window), Some(OptionValue::bool(false)));
    }

    // === reset buffer/window scopes ===

    #[test]
    fn test_registry_reset_buffer_scope() {
        let registry = OptionRegistry::new();
        assert!(
            registry
                .register(
                    OptionSpec::new("tabwidth", "desc", OptionValue::int(4))
                        .with_scope(OptionScope::Buffer),
                )
                .is_ok()
        );

        let buffer = BufferId::new();
        assert!(
            registry
                .set_for_buffer("tabwidth", OptionValue::int(8), buffer)
                .is_ok()
        );

        let removed = registry.reset("tabwidth", OptionScopeId::Buffer(buffer));
        assert!(removed.is_ok());
        assert_eq!(removed.unwrap(), Some(OptionValue::int(8)));

        // Now back to default
        assert_eq!(registry.get_for_buffer("tabwidth", buffer), Some(OptionValue::int(4)));
    }

    #[test]
    fn test_registry_reset_window_scope() {
        let registry = OptionRegistry::new();
        assert!(
            registry
                .register(
                    OptionSpec::new("number", "desc", OptionValue::bool(false))
                        .with_scope(OptionScope::Window),
                )
                .is_ok()
        );

        let window = WindowId::new();
        assert!(
            registry
                .set_for_window("number", OptionValue::bool(true), window)
                .is_ok()
        );

        let removed = registry.reset("number", OptionScopeId::Window(window));
        assert!(removed.is_ok());
        assert_eq!(removed.unwrap(), Some(OptionValue::bool(true)));
    }

    // === alias conflicts ===

    #[test]
    fn test_registry_alias_conflicts_with_name() {
        let registry = OptionRegistry::new();
        // Register "nu" as a full option name
        assert!(
            registry
                .register(OptionSpec::new("nu", "desc", OptionValue::bool(false)))
                .is_ok()
        );
        // Now try to register "number" with short "nu" - should conflict
        let result = registry
            .register(OptionSpec::new("number", "desc", OptionValue::bool(false)).with_short("nu"));
        assert!(matches!(result, Err(OptionError::AliasConflict(_))));
    }

    #[test]
    fn test_registry_alias_conflicts_with_alias() {
        let registry = OptionRegistry::new();
        assert!(
            registry
                .register(
                    OptionSpec::new("number", "desc", OptionValue::bool(false)).with_short("nu"),
                )
                .is_ok()
        );
        // Another option with the same alias should conflict
        let result = registry.register(
            OptionSpec::new("numbers", "desc", OptionValue::bool(false)).with_short("nu"),
        );
        assert!(matches!(result, Err(OptionError::AliasConflict(_))));
    }

    // === scope mismatch window on global ===

    #[test]
    fn test_registry_scope_mismatch_window_on_global() {
        let registry = OptionRegistry::new();
        assert!(
            registry
                .register(
                    OptionSpec::new("theme", "desc", OptionValue::string("dark"))
                        .with_scope(OptionScope::Global),
                )
                .is_ok()
        );

        let result =
            registry.set_for_window("theme", OptionValue::string("light"), WindowId::new());
        assert!(matches!(result, Err(OptionError::ScopeMismatch { .. })));
    }

    // === Coverage: get_spec via alias ===

    #[test]
    fn test_registry_get_spec_via_alias() {
        let registry = OptionRegistry::new();
        assert!(
            registry
                .register(
                    OptionSpec::new("number", "desc", OptionValue::bool(false)).with_short("nu"),
                )
                .is_ok()
        );

        let spec = registry.get_spec("nu");
        assert!(spec.is_some());
        assert_eq!(spec.unwrap().name, "number");

        // Also get_spec by full name
        let spec = registry.get_spec("number");
        assert!(spec.is_some());
    }

    #[test]
    fn test_registry_get_spec_nonexistent() {
        let registry = OptionRegistry::new();
        assert!(registry.get_spec("nope").is_none());
    }

    // === Coverage: get_global convenience ===

    #[test]
    fn test_registry_get_global_convenience() {
        let registry = OptionRegistry::new();
        assert!(
            registry
                .register(OptionSpec::new("tabwidth", "desc", OptionValue::int(4)))
                .is_ok()
        );

        let val = registry.get_global("tabwidth");
        assert_eq!(val, Some(OptionValue::int(4)));

        let val = registry.get_global("nonexistent");
        assert_eq!(val, None);
    }

    // === Coverage: set_global convenience ===

    #[test]
    fn test_registry_set_global_convenience() {
        let registry = OptionRegistry::new();
        assert!(
            registry
                .register(OptionSpec::new("tabwidth", "desc", OptionValue::int(4)))
                .is_ok()
        );

        assert!(registry.set_global("tabwidth", OptionValue::int(8)).is_ok());
        assert_eq!(registry.get_global("tabwidth"), Some(OptionValue::int(8)));
    }

    // === Coverage: set not found ===

    #[test]
    fn test_registry_set_not_found() {
        let registry = OptionRegistry::new();
        let result = registry.set("nonexistent", OptionValue::int(1), OptionScopeId::Global);
        assert!(matches!(result, Err(OptionError::NotFound(_))));
    }

    // === Coverage: toggle not found ===

    #[test]
    fn test_registry_toggle_not_found() {
        let registry = OptionRegistry::new();
        let result = registry.toggle("nonexistent", OptionScopeId::Global);
        assert!(matches!(result, Err(OptionError::NotFound(_))));
    }

    // === Coverage: reset not found ===

    #[test]
    fn test_registry_reset_not_found() {
        let registry = OptionRegistry::new();
        let result = registry.reset("nonexistent", OptionScopeId::Global);
        assert!(matches!(result, Err(OptionError::NotFound(_))));
    }

    // === Coverage: list_matching via alias prefix ===

    #[test]
    fn test_registry_list_matching_alias_prefix() {
        let registry = OptionRegistry::new();
        assert!(
            registry
                .register(
                    OptionSpec::new("number", "desc", OptionValue::bool(false)).with_short("nu"),
                )
                .is_ok()
        );
        assert!(
            registry
                .register(OptionSpec::new("numberwidth", "desc", OptionValue::int(4)))
                .is_ok()
        );

        // "nu" prefix matches alias AND "number" and "numberwidth"
        let matches = registry.list_matching("nu");
        assert!(matches.len() >= 2);
    }

    // === Coverage: len and is_empty ===

    #[test]
    fn test_registry_len_and_is_empty() {
        let registry = OptionRegistry::new();
        assert!(registry.is_empty());
        assert_eq!(registry.len(), 0);

        assert!(
            registry
                .register(OptionSpec::new("number", "desc", OptionValue::bool(false)))
                .is_ok()
        );
        assert!(!registry.is_empty());
        assert_eq!(registry.len(), 1);
    }

    // === Coverage: set via alias ===

    #[test]
    fn test_registry_set_via_alias() {
        let registry = OptionRegistry::new();
        assert!(
            registry
                .register(
                    OptionSpec::new("number", "desc", OptionValue::bool(false)).with_short("nu"),
                )
                .is_ok()
        );

        assert!(
            registry
                .set("nu", OptionValue::bool(true), OptionScopeId::Global)
                .is_ok()
        );
        assert_eq!(registry.get("number", OptionScopeId::Global), Some(OptionValue::bool(true)));
    }

    // === Coverage: list_all ===

    #[test]
    fn test_registry_list_all() {
        let registry = OptionRegistry::new();
        assert!(
            registry
                .register(OptionSpec::new("a", "desc", OptionValue::bool(false)))
                .is_ok()
        );
        assert!(
            registry
                .register(OptionSpec::new("b", "desc", OptionValue::int(1)))
                .is_ok()
        );

        let all = registry.list_all();
        assert_eq!(all.len(), 2);
        assert!(all.contains(&"a".to_string()));
        assert!(all.contains(&"b".to_string()));
    }

    // === MC/DC: register() - AlreadyExists branch (line ~128-129) ===
    // Exercises the duplicate name check returning AlreadyExists error.

    #[test]
    fn test_registry_register_already_exists_error() {
        let registry = OptionRegistry::new();
        let spec = OptionSpec::new("tabstop", "Tab stop width", OptionValue::int(8));
        assert!(registry.register(spec).is_ok());

        // Second registration of the same name must return AlreadyExists
        let dup = OptionSpec::new("tabstop", "Duplicate", OptionValue::int(4));
        let result = registry.register(dup);
        assert!(matches!(result, Err(OptionError::AlreadyExists(name)) if name == "tabstop"));
    }

    // === MC/DC: register() - AliasConflict when alias matches existing spec name ===
    // Exercises the check at line ~137: alias == an existing full spec name.

    #[test]
    fn test_registry_register_alias_conflicts_with_existing_spec_name() {
        let registry = OptionRegistry::new();
        // Register "ts" as a full spec name
        assert!(
            registry
                .register(OptionSpec::new("ts", "Existing spec", OptionValue::int(4)))
                .is_ok()
        );
        // Try to register "tabstop" with short alias "ts" - should fail because "ts" is a spec name
        let result = registry
            .register(OptionSpec::new("tabstop", "desc", OptionValue::int(8)).with_short("ts"));
        assert!(matches!(result, Err(OptionError::AliasConflict(alias)) if alias == "ts"));
    }

    // === MC/DC: register() - AliasConflict when alias matches existing alias ===
    // Exercises the check at line ~142: alias already registered as an alias.

    #[test]
    fn test_registry_register_alias_conflicts_with_existing_alias() {
        let registry = OptionRegistry::new();
        // Register "number" with alias "nu"
        assert!(
            registry
                .register(
                    OptionSpec::new("number", "Line numbers", OptionValue::bool(false))
                        .with_short("nu"),
                )
                .is_ok()
        );
        // Try to register "numberwidth" with the same alias "nu"
        let result = registry.register(
            OptionSpec::new("numberwidth", "Number column width", OptionValue::int(4))
                .with_short("nu"),
        );
        assert!(matches!(result, Err(OptionError::AliasConflict(alias)) if alias == "nu"));
    }

    // === MC/DC: resolve_name() - TRUE branch: name is a direct spec name (line ~163) ===
    // Exercises `if self.specs.read().contains_key(name)` returning true.

    #[test]
    fn test_resolve_name_returns_direct_spec_name() {
        let registry = OptionRegistry::new();
        assert!(
            registry
                .register(OptionSpec::new("wrap", "Line wrap", OptionValue::bool(true)))
                .is_ok()
        );

        // "wrap" is a registered spec name - resolve_name should return it directly
        let resolved = registry.resolve_name("wrap");
        assert_eq!(resolved, Some("wrap".to_string()));
    }

    // === MC/DC: get() - Window scope value found (line ~207) ===
    // Exercises `if let Some(value) = self.window_values.read().get(...)` TRUE branch.

    #[test]
    fn test_get_returns_window_local_value_when_set() {
        let registry = OptionRegistry::new();
        assert!(
            registry
                .register(
                    OptionSpec::new("number", "Line numbers", OptionValue::bool(false))
                        .with_scope(OptionScope::Window),
                )
                .is_ok()
        );

        let window_id = WindowId::new();
        // Set a window-local value
        assert!(
            registry
                .set_for_window("number", OptionValue::bool(true), window_id)
                .is_ok()
        );

        // get() with Window scope must return the window-local value (TRUE branch at ~207)
        let value = registry.get("number", OptionScopeId::Window(window_id));
        assert_eq!(value, Some(OptionValue::bool(true)));
    }

    // === MC/DC: get() - Buffer scope value found (line ~217) ===
    // Exercises `if let Some(value) = self.buffer_values.read().get(...)` TRUE branch.

    #[test]
    fn test_get_returns_buffer_local_value_when_set() {
        let registry = OptionRegistry::new();
        assert!(
            registry
                .register(
                    OptionSpec::new("tabstop", "Tab width", OptionValue::int(4))
                        .with_scope(OptionScope::Buffer),
                )
                .is_ok()
        );

        let buffer_id = BufferId::new();
        // Set a buffer-local value
        assert!(
            registry
                .set_for_buffer("tabstop", OptionValue::int(2), buffer_id)
                .is_ok()
        );

        // get() with Buffer scope must return the buffer-local value (TRUE branch at ~217)
        let value = registry.get("tabstop", OptionScopeId::Buffer(buffer_id));
        assert_eq!(value, Some(OptionValue::int(2)));
    }

    // === MC/DC: get() - global override value present (line ~229) ===
    // Exercises `if let Some(value) = self.global_values.read().get(&full_name)` TRUE branch.

    #[test]
    fn test_get_returns_global_override_when_set() {
        let registry = OptionRegistry::new();
        assert!(
            registry
                .register(OptionSpec::new("tabstop", "Tab width", OptionValue::int(4)))
                .is_ok()
        );

        // Set a global override (different from default)
        assert!(registry.set_global("tabstop", OptionValue::int(8)).is_ok());

        // get() with Global scope must return the global override (TRUE branch at ~229)
        let value = registry.get("tabstop", OptionScopeId::Global);
        assert_eq!(value, Some(OptionValue::int(8)));
    }

    // === Module ownership: unregister_by_module ===

    #[test]
    fn test_unregister_by_module_removes_specs_and_aliases() {
        let registry = OptionRegistry::new();
        let vim = ModuleId::new("vim");

        assert!(
            registry
                .register(
                    OptionSpec::new("number", "desc", OptionValue::bool(false))
                        .with_short("nu")
                        .with_scope(OptionScope::Window)
                        .with_owner(vim.clone()),
                )
                .is_ok()
        );
        assert!(
            registry
                .register(
                    OptionSpec::new("relativenumber", "desc", OptionValue::bool(false))
                        .with_short("rnu")
                        .with_scope(OptionScope::Window)
                        .with_owner(vim.clone()),
                )
                .is_ok()
        );
        // Unowned option should survive
        assert!(
            registry
                .register(OptionSpec::new("tabwidth", "desc", OptionValue::int(4)))
                .is_ok()
        );

        assert_eq!(registry.len(), 3);
        registry.unregister_by_module(&vim);
        assert_eq!(registry.len(), 1);
        assert!(!registry.contains("number"));
        assert!(!registry.contains("nu"));
        assert!(!registry.contains("relativenumber"));
        assert!(!registry.contains("rnu"));
        assert!(registry.contains("tabwidth"));
    }

    #[test]
    fn test_unregister_by_module_cleans_stored_values() {
        let registry = OptionRegistry::new();
        let vim = ModuleId::new("vim");

        assert!(
            registry
                .register(
                    OptionSpec::new("number", "desc", OptionValue::bool(false))
                        .with_scope(OptionScope::Window)
                        .with_owner(vim.clone()),
                )
                .is_ok()
        );

        // Set values at all scope levels
        assert!(
            registry
                .set("number", OptionValue::bool(true), OptionScopeId::Global)
                .is_ok()
        );
        let window = WindowId::new();
        assert!(
            registry
                .set_for_window("number", OptionValue::bool(true), window)
                .is_ok()
        );

        registry.unregister_by_module(&vim);

        // Option should be completely gone
        assert!(registry.get("number", OptionScopeId::Global).is_none());
    }

    #[test]
    fn test_unregister_by_module_noop_for_unknown() {
        let registry = OptionRegistry::new();
        assert!(
            registry
                .register(OptionSpec::new("number", "desc", OptionValue::bool(false)))
                .is_ok()
        );

        let unknown = ModuleId::new("unknown");
        registry.unregister_by_module(&unknown);
        assert_eq!(registry.len(), 1);
    }

    #[test]
    fn test_unregister_by_module_cleans_buffer_values() {
        let registry = OptionRegistry::new();
        let editor = ModuleId::new("editor");

        assert!(
            registry
                .register(
                    OptionSpec::new("tabwidth", "desc", OptionValue::int(4))
                        .with_scope(OptionScope::Buffer)
                        .with_owner(editor.clone()),
                )
                .is_ok()
        );

        let buffer = BufferId::new();
        assert!(
            registry
                .set_for_buffer("tabwidth", OptionValue::int(8), buffer)
                .is_ok()
        );

        registry.unregister_by_module(&editor);
        assert!(registry.get_for_buffer("tabwidth", buffer).is_none());
    }

    // === Module ownership: list_by_module ===

    #[test]
    fn test_list_by_module() {
        let registry = OptionRegistry::new();
        let vim = ModuleId::new("vim");
        let editor = ModuleId::new("editor");

        assert!(
            registry
                .register(
                    OptionSpec::new("number", "desc", OptionValue::bool(false))
                        .with_owner(vim.clone()),
                )
                .is_ok()
        );
        assert!(
            registry
                .register(
                    OptionSpec::new("relativenumber", "desc", OptionValue::bool(false))
                        .with_owner(vim.clone()),
                )
                .is_ok()
        );
        assert!(
            registry
                .register(
                    OptionSpec::new("tabwidth", "desc", OptionValue::int(4))
                        .with_owner(editor.clone()),
                )
                .is_ok()
        );
        // Unowned option
        assert!(
            registry
                .register(OptionSpec::new("wrap", "desc", OptionValue::bool(true)))
                .is_ok()
        );

        let vim_opts = registry.list_by_module(&vim);
        assert_eq!(vim_opts.len(), 2);

        let editor_opts = registry.list_by_module(&editor);
        assert_eq!(editor_opts.len(), 1);
        assert_eq!(editor_opts[0].name, "tabwidth");

        let unknown = ModuleId::new("unknown");
        assert!(registry.list_by_module(&unknown).is_empty());
    }
}
