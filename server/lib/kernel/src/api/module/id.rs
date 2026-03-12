//! Module identifier type.

use std::{borrow::Cow, fmt};

/// Unique identifier for a loadable module.
///
/// Convention: Use kebab-case names like "lang-rust", "feat-completion".
///
/// # Static vs Dynamic IDs
///
/// Module IDs can be either:
/// - **Static** (`&'static str`): For compile-time known modules, use `ModuleId::new()`
/// - **Dynamic** (`String`): For runtime-generated modules, use `ModuleId::from_string()`
///
/// Static IDs are preferred for performance (no allocation), but dynamic IDs
/// allow for user-defined or plugin-loaded modules with arbitrary names.
///
/// # Example
///
/// ```
/// use reovim_kernel::api::v1::ModuleId;
///
/// // Static ID (compile-time known)
/// let static_id = ModuleId::new("lang-rust");
///
/// // Dynamic ID (runtime generated)
/// let name = format!("user-plugin-{}", 42);
/// let dynamic_id = ModuleId::from_string(name);
///
/// // Both work the same way
/// assert_eq!(static_id.as_str(), "lang-rust");
/// assert_eq!(dynamic_id.as_str(), "user-plugin-42");
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ModuleId(Cow<'static, str>);

impl ModuleId {
    /// Create a new module identifier from a static string.
    ///
    /// This is the preferred way to create module IDs for statically-known modules.
    /// It's a const fn and involves no allocation.
    #[must_use]
    pub const fn new(id: &'static str) -> Self {
        Self(Cow::Borrowed(id))
    }

    /// Create a module identifier from an owned String.
    ///
    /// Use this for dynamically-generated module IDs (e.g., user plugins,
    /// runtime-loaded modules with user-provided names).
    #[must_use]
    #[allow(clippy::missing_const_for_fn)] // String operations aren't const-stable
    pub fn from_string(id: String) -> Self {
        Self(Cow::Owned(id))
    }

    /// Get the identifier string.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Check if this is a static (borrowed) ID.
    #[must_use]
    pub const fn is_static(&self) -> bool {
        matches!(self.0, Cow::Borrowed(_))
    }

    /// Check if this is a dynamic (owned) ID.
    #[must_use]
    pub const fn is_dynamic(&self) -> bool {
        matches!(self.0, Cow::Owned(_))
    }
}

impl fmt::Display for ModuleId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl From<&'static str> for ModuleId {
    fn from(s: &'static str) -> Self {
        Self::new(s)
    }
}

impl From<String> for ModuleId {
    fn from(s: String) -> Self {
        Self::from_string(s)
    }
}
