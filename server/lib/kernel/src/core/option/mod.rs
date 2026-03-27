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
#[path = "mod_tests.rs"]
mod tests;
