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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_module_id() {
        let id = ModuleId::new("lang-rust");
        assert_eq!(id.as_str(), "lang-rust");
        assert_eq!(format!("{id}"), "lang-rust");
    }

    #[test]
    fn test_module_id_equality() {
        let id1 = ModuleId::new("lang-rust");
        let id2 = ModuleId::new("lang-rust");
        let id3 = ModuleId::new("lang-python");
        assert_eq!(id1, id2);
        assert_ne!(id1, id3);
    }

    #[test]
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn test_module_id_dynamic() {
        let name = format!("user-plugin-{}", 42);
        let id = ModuleId::from_string(name);
        assert_eq!(id.as_str(), "user-plugin-42");
        assert!(id.is_dynamic());
        assert!(!id.is_static());
    }

    #[test]
    fn test_module_id_static() {
        let id = ModuleId::new("lang-rust");
        assert!(id.is_static());
        assert!(!id.is_dynamic());
    }

    #[test]
    fn test_module_id_static_dynamic_equality() {
        // Static and dynamic IDs with same content should be equal
        let static_id = ModuleId::new("test-module");
        let dynamic_id = ModuleId::from_string("test-module".to_string());

        assert_eq!(static_id, dynamic_id);
        assert_eq!(static_id.as_str(), dynamic_id.as_str());
    }

    #[test]
    fn test_module_id_from_traits() {
        // Test From<&'static str>
        let id1: ModuleId = "lang-rust".into();
        assert_eq!(id1.as_str(), "lang-rust");

        // Test From<String>
        let id2: ModuleId = String::from("lang-python").into();
        assert_eq!(id2.as_str(), "lang-python");
    }
}
