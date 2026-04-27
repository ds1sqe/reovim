//! Client-side service registry for cross-module communication.
//!
//! `ClientServiceRegistry` provides `TypeId`-based service lookup, matching the
//! pattern used by the kernel's `ServiceRegistry` but living entirely in the
//! client subsys-module crate (no kernel dependency).
//!
//! Modules register services during `init()` or `on_all_loaded()`, and other
//! modules query them by type.

use std::{
    any::{Any, TypeId},
    collections::HashMap,
    sync::{Arc, RwLock},
};

/// Client-side type-safe service registry.
///
/// Modules register services by type, and other modules query them. Thread-safe
/// via `RwLock` (required for `Send + Sync` on `ModuleContext`).
///
/// # Example
///
/// ```ignore
/// registry.register(Arc::new(MyService::new()));
/// let svc = registry.get::<MyService>();
/// ```
#[derive(Debug, Default)]
pub struct ClientServiceRegistry {
    services: RwLock<HashMap<TypeId, Arc<dyn Any + Send + Sync>>>,
}

impl ClientServiceRegistry {
    /// Create an empty service registry.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Register a service. Replaces any existing service of the same type.
    ///
    /// # Panics
    ///
    /// Panics if the internal `RwLock` is poisoned.
    pub fn register<T: Send + Sync + 'static>(&self, service: Arc<T>) {
        let type_id = TypeId::of::<T>();
        self.services
            .write()
            .expect("service registry poisoned")
            .insert(type_id, service);
    }

    /// Get a service by type. Returns `None` if not registered.
    ///
    /// # Panics
    ///
    /// Panics if the internal `RwLock` is poisoned.
    #[must_use]
    pub fn get<T: Send + Sync + 'static>(&self) -> Option<Arc<T>> {
        let type_id = TypeId::of::<T>();
        self.services
            .read()
            .expect("service registry poisoned")
            .get(&type_id)
            .and_then(|any| any.clone().downcast::<T>().ok())
    }

    /// Check if a service of the given type is registered.
    ///
    /// # Panics
    ///
    /// Panics if the internal `RwLock` is poisoned.
    #[must_use]
    pub fn contains<T: Send + Sync + 'static>(&self) -> bool {
        let type_id = TypeId::of::<T>();
        self.services
            .read()
            .expect("service registry poisoned")
            .contains_key(&type_id)
    }

    /// Number of registered services.
    ///
    /// # Panics
    ///
    /// Panics if the internal `RwLock` is poisoned.
    #[must_use]
    pub fn len(&self) -> usize {
        self.services
            .read()
            .expect("service registry poisoned")
            .len()
    }

    /// Whether the registry is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

#[cfg(test)]
#[path = "services_tests.rs"]
mod tests;
