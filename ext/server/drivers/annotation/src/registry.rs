//! Annotation source registry for service lookup.
//!
//! Provides [`AnnotationSourceKey`] and [`AnnotationSourceRegistry`] for
//! registering and looking up annotation sources in a service registry.

use reovim_kernel::api::v1::{MultiServiceRegistry, ServiceKey};

use super::source::AnnotationSource;

/// Key for annotation source lookup.
///
/// Used to register and lookup annotation sources in a registry.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct AnnotationSourceKey(pub &'static str);

impl ServiceKey for AnnotationSourceKey {
    fn service_name() -> &'static str {
        "AnnotationSource"
    }
}

impl AnnotationSourceKey {
    /// Create a new source key.
    #[must_use]
    pub const fn new(name: &'static str) -> Self {
        Self(name)
    }
}

/// Registry for annotation sources.
///
/// Uses the kernel's `MultiServiceRegistry` pattern for type-safe lookup.
pub type AnnotationSourceRegistry = MultiServiceRegistry<AnnotationSourceKey, dyn AnnotationSource>;

#[cfg(test)]
#[path = "registry_tests.rs"]
mod tests;
