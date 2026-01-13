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

use std::{borrow::Cow, collections::HashMap, fmt};

use reovim_arch::sync::RwLock;

use crate::{api::window_manager::WindowId, mm::BufferId};

// ============================================================================
// OptionValue - Type-safe option values
// ============================================================================

/// Type-safe option value.
///
/// This is the core value type for editor options. Each variant maps to
/// a common configuration pattern:
/// - `Bool`: Toggle settings (number, wrap, etc.)
/// - `Integer`: Numeric settings (tabwidth, scrolloff)
/// - `String`: Free-form text (theme name, paths)
/// - `Choice`: Enum-like selection from predefined values
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OptionValue {
    /// Boolean on/off option (e.g., `number`, `relativenumber`)
    Bool(bool),

    /// Integer option (e.g., `tabwidth`, `scrolloff`)
    Integer(i64),

    /// String option (e.g., `theme`, `signcolumn`)
    String(String),

    /// Choice option with predefined valid values.
    ///
    /// The `choices` field stores all valid values for validation.
    Choice {
        /// Current selected value
        value: String,
        /// All valid choices (for validation and completion)
        choices: Vec<String>,
    },
}

impl OptionValue {
    // ========================================================================
    // Constructors
    // ========================================================================

    /// Create a boolean value.
    #[must_use]
    pub const fn bool(value: bool) -> Self {
        Self::Bool(value)
    }

    /// Create an integer value.
    #[must_use]
    pub const fn int(value: i64) -> Self {
        Self::Integer(value)
    }

    /// Create a string value.
    #[must_use]
    pub fn string(value: impl Into<String>) -> Self {
        Self::String(value.into())
    }

    /// Create a choice value.
    ///
    /// # Panics
    ///
    /// Panics if `choices` is empty or if `value` is not in `choices`.
    #[must_use]
    pub fn choice(value: impl Into<String>, choices: Vec<String>) -> Self {
        let value = value.into();
        debug_assert!(!choices.is_empty(), "choices must not be empty");
        debug_assert!(choices.contains(&value), "value must be in choices");
        Self::Choice { value, choices }
    }

    // ========================================================================
    // Accessors
    // ========================================================================

    /// Try to get as boolean.
    #[must_use]
    pub const fn as_bool(&self) -> Option<bool> {
        match self {
            Self::Bool(b) => Some(*b),
            _ => None,
        }
    }

    /// Try to get as integer.
    #[must_use]
    pub const fn as_int(&self) -> Option<i64> {
        match self {
            Self::Integer(i) => Some(*i),
            _ => None,
        }
    }

    /// Try to get as string reference.
    #[must_use]
    pub fn as_str(&self) -> Option<&str> {
        match self {
            Self::String(s) => Some(s),
            Self::Choice { value, .. } => Some(value),
            _ => None,
        }
    }

    /// Get the type name for error messages.
    #[must_use]
    pub const fn type_name(&self) -> &'static str {
        match self {
            Self::Bool(_) => "bool",
            Self::Integer(_) => "integer",
            Self::String(_) => "string",
            Self::Choice { .. } => "choice",
        }
    }

    /// Check if this value has the same type as another.
    #[must_use]
    pub const fn same_type(&self, other: &Self) -> bool {
        matches!(
            (self, other),
            (Self::Bool(_), Self::Bool(_))
                | (Self::Integer(_), Self::Integer(_))
                | (Self::String(_), Self::String(_))
                | (Self::Choice { .. }, Self::Choice { .. })
        )
    }
}

impl fmt::Display for OptionValue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Bool(b) => write!(f, "{b}"),
            Self::Integer(i) => write!(f, "{i}"),
            Self::String(s) => write!(f, "{s}"),
            Self::Choice { value, .. } => write!(f, "{value}"),
        }
    }
}

// ============================================================================
// OptionScope - Where an option applies
// ============================================================================

/// Scope where an option applies.
///
/// Determines the granularity of option storage:
/// - `Global`: Single value for the entire editor
/// - `Buffer`: Per-buffer values (e.g., `tabwidth`)
/// - `Window`: Per-window values (e.g., `number`)
///
/// Window-scoped options can also have buffer-local defaults.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum OptionScope {
    /// Applies globally to the entire editor.
    #[default]
    Global,

    /// Per-buffer setting (e.g., `filetype`, `tabwidth`, `expandtab`).
    Buffer,

    /// Per-window setting (e.g., `number`, `relativenumber`, `wrap`).
    Window,
}

impl OptionScope {
    /// Get display name for this scope.
    #[must_use]
    pub const fn display_name(self) -> &'static str {
        match self {
            Self::Global => "global",
            Self::Buffer => "buffer",
            Self::Window => "window",
        }
    }
}

impl fmt::Display for OptionScope {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.display_name())
    }
}

// ============================================================================
// OptionScopeId - Runtime scope identifier
// ============================================================================

/// Runtime scope identifier for option access.
///
/// Used when getting or setting option values to specify the exact
/// scope context (which buffer, which window, or global).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum OptionScopeId {
    /// Global scope (no buffer/window context).
    #[default]
    Global,
    /// Buffer-local scope with specific buffer ID.
    Buffer(BufferId),
    /// Window-local scope with specific window ID.
    Window(WindowId),
}

impl fmt::Display for OptionScopeId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Global => write!(f, "global"),
            Self::Buffer(id) => write!(f, "buffer({id:?})"),
            Self::Window(id) => write!(f, "window({id:?})"),
        }
    }
}

// ============================================================================
// OptionConstraint - Validation constraints
// ============================================================================

/// Constraints for option value validation.
///
/// These constraints are checked when setting option values.
/// Not all constraints apply to all value types.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct OptionConstraint {
    /// For Integer: minimum value (inclusive).
    pub min: Option<i64>,
    /// For Integer: maximum value (inclusive).
    pub max: Option<i64>,
    /// For String: minimum length.
    pub min_length: Option<usize>,
    /// For String: maximum length.
    pub max_length: Option<usize>,
}

impl OptionConstraint {
    /// Create a constraint with no restrictions.
    #[must_use]
    pub const fn none() -> Self {
        Self {
            min: None,
            max: None,
            min_length: None,
            max_length: None,
        }
    }

    /// Create a constraint with integer range (inclusive).
    #[must_use]
    pub const fn range(min: i64, max: i64) -> Self {
        Self {
            min: Some(min),
            max: Some(max),
            min_length: None,
            max_length: None,
        }
    }

    /// Create a constraint with minimum integer value.
    #[must_use]
    pub const fn min(min: i64) -> Self {
        Self {
            min: Some(min),
            max: None,
            min_length: None,
            max_length: None,
        }
    }

    /// Create a constraint with maximum integer value.
    #[must_use]
    pub const fn max(max: i64) -> Self {
        Self {
            min: None,
            max: Some(max),
            min_length: None,
            max_length: None,
        }
    }

    /// Create a constraint with string length range.
    #[must_use]
    pub const fn string_length(min_length: usize, max_length: usize) -> Self {
        Self {
            min: None,
            max: None,
            min_length: Some(min_length),
            max_length: Some(max_length),
        }
    }

    /// Validate a value against this constraint.
    ///
    /// # Errors
    ///
    /// Returns `ConstraintError` if the value violates the constraint.
    pub fn validate(&self, value: &OptionValue) -> Result<(), ConstraintError> {
        match value {
            OptionValue::Integer(i) => {
                if let Some(min) = self.min
                    && *i < min
                {
                    return Err(ConstraintError::BelowMinimum { value: *i, min });
                }
                if let Some(max) = self.max
                    && *i > max
                {
                    return Err(ConstraintError::AboveMaximum { value: *i, max });
                }
            }
            OptionValue::String(s) => {
                if let Some(min_len) = self.min_length
                    && s.len() < min_len
                {
                    return Err(ConstraintError::StringTooShort {
                        len: s.len(),
                        min: min_len,
                    });
                }
                if let Some(max_len) = self.max_length
                    && s.len() > max_len
                {
                    return Err(ConstraintError::StringTooLong {
                        len: s.len(),
                        max: max_len,
                    });
                }
            }
            OptionValue::Choice { value, choices } => {
                if !choices.contains(value) {
                    return Err(ConstraintError::InvalidChoice {
                        value: value.clone(),
                        choices: choices.clone(),
                    });
                }
            }
            OptionValue::Bool(_) => {
                // No constraints for boolean values
            }
        }
        Ok(())
    }
}

/// Constraint validation error.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConstraintError {
    /// Integer value is below minimum.
    BelowMinimum {
        /// The actual value
        value: i64,
        /// The minimum allowed
        min: i64,
    },
    /// Integer value is above maximum.
    AboveMaximum {
        /// The actual value
        value: i64,
        /// The maximum allowed
        max: i64,
    },
    /// String is too short.
    StringTooShort {
        /// Actual length
        len: usize,
        /// Minimum required length
        min: usize,
    },
    /// String is too long.
    StringTooLong {
        /// Actual length
        len: usize,
        /// Maximum allowed length
        max: usize,
    },
    /// Choice value is not in the allowed choices.
    InvalidChoice {
        /// The invalid value
        value: String,
        /// The valid choices
        choices: Vec<String>,
    },
}

impl fmt::Display for ConstraintError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::BelowMinimum { value, min } => {
                write!(f, "value {value} is below minimum {min}")
            }
            Self::AboveMaximum { value, max } => {
                write!(f, "value {value} is above maximum {max}")
            }
            Self::StringTooShort { len, min } => {
                write!(f, "string length {len} is below minimum {min}")
            }
            Self::StringTooLong { len, max } => {
                write!(f, "string length {len} is above maximum {max}")
            }
            Self::InvalidChoice { value, choices } => {
                write!(f, "'{value}' is not a valid choice (valid: {choices:?})")
            }
        }
    }
}

impl std::error::Error for ConstraintError {}

// ============================================================================
// OptionSpec - Complete option specification
// ============================================================================

/// Complete specification for an editor option.
///
/// This is the **MECHANISM** type - it defines the structure of options.
/// **POLICY** (which options exist and what they do) is in modules.
///
/// # Example
///
/// ```ignore
/// use reovim_kernel::api::v1::*;
///
/// let spec = OptionSpec::new("tabwidth", "Tab width in spaces", OptionValue::int(4))
///     .with_short("tw")
///     .with_constraint(OptionConstraint::range(1, 32))
///     .with_scope(OptionScope::Buffer);
/// ```
#[derive(Debug, Clone)]
pub struct OptionSpec {
    /// Full option name (e.g., "number", "tabwidth").
    pub name: Cow<'static, str>,
    /// Short alias (e.g., "nu" for "number").
    pub short_form: Option<Cow<'static, str>>,
    /// Human-readable description.
    pub description: Cow<'static, str>,
    /// Default value (also defines the type).
    pub default: OptionValue,
    /// Constraints for validation.
    pub constraint: OptionConstraint,
    /// Scope (global, buffer-local, window-local).
    pub scope: OptionScope,
}

impl OptionSpec {
    /// Create a new option specification.
    #[must_use]
    pub fn new(
        name: impl Into<Cow<'static, str>>,
        description: impl Into<Cow<'static, str>>,
        default: OptionValue,
    ) -> Self {
        Self {
            name: name.into(),
            description: description.into(),
            default,
            short_form: None,
            constraint: OptionConstraint::none(),
            scope: OptionScope::default(),
        }
    }

    /// Add a short alias.
    #[must_use]
    pub fn with_short(mut self, short: impl Into<Cow<'static, str>>) -> Self {
        self.short_form = Some(short.into());
        self
    }

    /// Add validation constraints.
    #[must_use]
    pub const fn with_constraint(mut self, constraint: OptionConstraint) -> Self {
        self.constraint = constraint;
        self
    }

    /// Set the scope.
    #[must_use]
    pub const fn with_scope(mut self, scope: OptionScope) -> Self {
        self.scope = scope;
        self
    }

    /// Check if this option matches a name (full name or alias).
    #[must_use]
    pub fn matches_name(&self, query: &str) -> bool {
        self.name == query
            || self
                .short_form
                .as_ref()
                .is_some_and(|s: &Cow<'static, str>| s.as_ref() == query)
    }

    /// Validate a value against this option's type and constraints.
    ///
    /// # Errors
    ///
    /// Returns error if value type doesn't match or violates constraints.
    pub fn validate(&self, value: &OptionValue) -> Result<(), OptionError> {
        // Type check
        if !self.default.same_type(value) {
            return Err(OptionError::TypeMismatch {
                name: self.name.to_string(),
                expected: self.default.type_name(),
                got: value.type_name(),
            });
        }

        // Constraint check
        self.constraint
            .validate(value)
            .map_err(|e| OptionError::ValidationFailed {
                name: self.name.to_string(),
                reason: e.to_string(),
            })
    }
}

// ============================================================================
// OptionError - Error type for option operations
// ============================================================================

/// Error type for option operations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OptionError {
    /// Option not found in registry.
    NotFound(String),

    /// Option already registered.
    AlreadyExists(String),

    /// Validation failed.
    ValidationFailed {
        /// Option name
        name: String,
        /// Reason for failure
        reason: String,
    },

    /// Type mismatch.
    TypeMismatch {
        /// Option name
        name: String,
        /// Expected type
        expected: &'static str,
        /// Actual type
        got: &'static str,
    },

    /// Alias conflicts with existing name.
    AliasConflict(String),

    /// Scope mismatch (e.g., setting buffer-local for global-only option).
    ScopeMismatch {
        /// Option name
        name: String,
        /// Option's declared scope
        option_scope: OptionScope,
        /// Requested scope
        requested: OptionScopeId,
    },
}

impl fmt::Display for OptionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotFound(name) => write!(f, "option not found: {name}"),
            Self::AlreadyExists(name) => write!(f, "option already exists: {name}"),
            Self::ValidationFailed { name, reason } => {
                write!(f, "validation failed for '{name}': {reason}")
            }
            Self::TypeMismatch {
                name,
                expected,
                got,
            } => {
                write!(f, "type mismatch for '{name}': expected {expected}, got {got}")
            }
            Self::AliasConflict(alias) => {
                write!(f, "alias conflicts with existing name: {alias}")
            }
            Self::ScopeMismatch {
                name,
                option_scope,
                requested,
            } => {
                write!(
                    f,
                    "scope mismatch for '{name}': option is {option_scope}, requested {requested}"
                )
            }
        }
    }
}

impl std::error::Error for OptionError {}

// ============================================================================
// SetResult - Result of a set operation
// ============================================================================

/// Result of a successful set operation.
#[derive(Debug, Clone)]
pub struct SetResult {
    /// Previous value (None if was default).
    pub old_value: Option<OptionValue>,
    /// New value that was set.
    pub new_value: OptionValue,
}

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

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // ========================================================================
    // OptionValue tests
    // ========================================================================

    #[test]
    fn test_option_value_bool() {
        let value = OptionValue::bool(true);
        assert_eq!(value.as_bool(), Some(true));
        assert_eq!(value.as_int(), None);
        assert_eq!(value.as_str(), None);
        assert_eq!(value.type_name(), "bool");
    }

    #[test]
    fn test_option_value_int() {
        let value = OptionValue::int(42);
        assert_eq!(value.as_int(), Some(42));
        assert_eq!(value.as_bool(), None);
        assert_eq!(value.type_name(), "integer");
    }

    #[test]
    fn test_option_value_string() {
        let value = OptionValue::string("hello");
        assert_eq!(value.as_str(), Some("hello"));
        assert_eq!(value.as_bool(), None);
        assert_eq!(value.type_name(), "string");
    }

    #[test]
    fn test_option_value_choice() {
        let value = OptionValue::choice("dark", vec!["light".into(), "dark".into()]);
        assert_eq!(value.as_str(), Some("dark"));
        assert_eq!(value.type_name(), "choice");
    }

    #[test]
    fn test_option_value_same_type() {
        assert!(OptionValue::bool(true).same_type(&OptionValue::bool(false)));
        assert!(OptionValue::int(1).same_type(&OptionValue::int(2)));
        assert!(!OptionValue::bool(true).same_type(&OptionValue::int(1)));
    }

    #[test]
    fn test_option_value_display() {
        assert_eq!(OptionValue::bool(true).to_string(), "true");
        assert_eq!(OptionValue::int(42).to_string(), "42");
        assert_eq!(OptionValue::string("hello").to_string(), "hello");
    }

    // ========================================================================
    // OptionConstraint tests
    // ========================================================================

    #[test]
    fn test_constraint_range() {
        let constraint = OptionConstraint::range(1, 10);

        // Valid values
        assert!(constraint.validate(&OptionValue::int(1)).is_ok());
        assert!(constraint.validate(&OptionValue::int(5)).is_ok());
        assert!(constraint.validate(&OptionValue::int(10)).is_ok());

        // Invalid values
        assert!(constraint.validate(&OptionValue::int(0)).is_err());
        assert!(constraint.validate(&OptionValue::int(11)).is_err());
    }

    #[test]
    fn test_constraint_string_length() {
        let constraint = OptionConstraint::string_length(2, 5);

        assert!(constraint.validate(&OptionValue::string("ab")).is_ok());
        assert!(constraint.validate(&OptionValue::string("abcde")).is_ok());
        assert!(constraint.validate(&OptionValue::string("a")).is_err());
        assert!(constraint.validate(&OptionValue::string("abcdef")).is_err());
    }

    #[test]
    fn test_constraint_choice() {
        let value = OptionValue::choice("valid", vec!["valid".into(), "other".into()]);
        let constraint = OptionConstraint::none();
        assert!(constraint.validate(&value).is_ok());
    }

    // ========================================================================
    // OptionSpec tests
    // ========================================================================

    #[test]
    fn test_option_spec_builder() {
        let spec = OptionSpec::new("number", "Show line numbers", OptionValue::bool(false))
            .with_short("nu")
            .with_scope(OptionScope::Window);

        assert_eq!(spec.name, "number");
        assert_eq!(spec.short_form.as_deref(), Some("nu"));
        assert_eq!(spec.scope, OptionScope::Window);
    }

    #[test]
    fn test_option_spec_matches_name() {
        let spec = OptionSpec::new("number", "desc", OptionValue::bool(false)).with_short("nu");

        assert!(spec.matches_name("number"));
        assert!(spec.matches_name("nu"));
        assert!(!spec.matches_name("other"));
    }

    #[test]
    fn test_option_spec_validate() {
        let spec = OptionSpec::new("tabwidth", "Tab width", OptionValue::int(4))
            .with_constraint(OptionConstraint::range(1, 32));

        assert!(spec.validate(&OptionValue::int(4)).is_ok());
        assert!(spec.validate(&OptionValue::int(1)).is_ok());
        assert!(spec.validate(&OptionValue::int(32)).is_ok());
        assert!(spec.validate(&OptionValue::int(0)).is_err());
        assert!(spec.validate(&OptionValue::int(33)).is_err());
        assert!(spec.validate(&OptionValue::bool(true)).is_err()); // Type mismatch
    }

    // ========================================================================
    // OptionRegistry tests
    // ========================================================================

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
}
