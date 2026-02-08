//! Service registry for cross-module service discovery.
//!
//! Provides two registry types:
//! - [`ServiceRegistry`]: For unique services (one provider per type)
//! - [`MultiServiceRegistry`]: For keyed services (multiple providers with typed keys)
//!
//! # Architecture
//!
//! This module implements the **generic mechanism** for service discovery.
//! Specific service traits and typed keys are defined in their respective drivers.
//!
//! ```text
//! server/lib/kernel/api/service/     → Generic ServiceRegistry (this module)
//! server/lib/drivers/vfs/            → VfsScheme enum + VfsProviderRegistry
//! server/lib/drivers/input/          → ModeProviderKey enum + ModeProviderRegistry
//! ```
//!
//! # Example
//!
//! ```ignore
//! use reovim_kernel::api::v1::{ServiceRegistry, MultiServiceRegistry, ServiceKey};
//!
//! // Unique service (one provider per type)
//! let registry = ServiceRegistry::new();
//! registry.register::<MyService>(Arc::new(my_service));
//! let service = registry.get::<MyService>();
//!
//! // Keyed service (multiple providers)
//! let vfs_registry = MultiServiceRegistry::<VfsScheme, dyn VfsDriver>::new();
//! vfs_registry.register(VfsScheme::File, Arc::new(local_fs));
//! let driver = vfs_registry.get(&VfsScheme::File);
//! ```

use std::{
    any::{Any, TypeId},
    collections::HashMap,
    fmt,
    hash::Hash,
    sync::Arc,
};

use reovim_arch::sync::RwLock;

// ============================================================================
// Service Trait
// ============================================================================

/// Marker trait for services that can be registered in [`ServiceRegistry`].
///
/// Implement this trait for types that should be discoverable via the
/// service registry. The trait bounds ensure thread-safety.
pub trait Service: Send + Sync + 'static {}

// ============================================================================
// ServiceKey Trait (typed key marker)
// ============================================================================

/// Marker trait for typed service keys.
///
/// Each driver defines its own key enum implementing this trait.
/// This ensures compile-time safety and provides metadata for error messages.
///
/// # Safety Contract
///
/// **DO NOT** use `service_name()` for:
/// - Runtime service routing or comparison
/// - Dynamic dispatch based on string matching
/// - Escape hatch to bypass typed key lookup
///
/// The `service_name()` method exists **ONLY** for human-readable error messages.
/// All service lookup MUST use the typed key directly via `get(key)`.
///
/// # Correct Usage
///
/// ```ignore
/// // GOOD: Direct typed key lookup
/// let vfs = registry.get(&VfsScheme::File);
/// ```
///
/// # Incorrect Usage
///
/// ```ignore
/// // BAD: Never compare service_name() for routing
/// if K::service_name() == "VFS" { ... }  // WRONG!
///
/// // BAD: Never use as escape hatch
/// fn get_any_service(name: &str) { ... }  // WRONG!
/// ```
///
/// # Example
///
/// ```ignore
/// use reovim_kernel::api::v1::ServiceKey;
///
/// #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
/// pub enum VfsScheme {
///     File,
///     Memory,
///     Ssh,
/// }
///
/// impl ServiceKey for VfsScheme {
///     fn service_name() -> &'static str { "VFS" }
/// }
/// ```
pub trait ServiceKey: Hash + Eq + Clone + Send + Sync + 'static {
    /// Human-readable service name for error messages ONLY.
    ///
    /// **NOT for comparison or routing.** See trait-level docs.
    fn service_name() -> &'static str;
}

// ============================================================================
// Unique Service Registry (one provider per type)
// ============================================================================

/// Registry for unique services - one provider wins per service type.
///
/// Uses `TypeId` internally for type-safe lookup. Each service type can have
/// at most one registered provider; registering a new provider replaces the
/// existing one.
///
/// # Thread Safety
///
/// All operations are protected by `RwLock`, allowing concurrent reads with
/// exclusive writes.
///
/// # Example
///
/// ```ignore
/// use reovim_kernel::api::v1::{Service, ServiceRegistry};
/// use std::sync::Arc;
///
/// struct MyCompositor;
/// impl Service for MyCompositor {}
///
/// let registry = ServiceRegistry::new();
/// registry.register(Arc::new(MyCompositor));
///
/// let compositor = registry.get::<MyCompositor>();
/// assert!(compositor.is_some());
/// ```
pub struct ServiceRegistry {
    services: RwLock<HashMap<TypeId, Arc<dyn Any + Send + Sync>>>,
}

impl ServiceRegistry {
    /// Create a new empty service registry.
    #[must_use]
    pub fn new() -> Self {
        Self {
            services: RwLock::new(HashMap::new()),
        }
    }

    /// Register a unique service (replaces existing if any).
    ///
    /// # Arguments
    ///
    /// * `service` - The service instance wrapped in `Arc`
    pub fn register<T: Service>(&self, service: Arc<T>) {
        let type_id = TypeId::of::<T>();
        self.services.write().insert(type_id, service);
    }

    /// Get a unique service by type.
    ///
    /// Returns `None` if no service of type `T` is registered.
    #[must_use]
    pub fn get<T: Service>(&self) -> Option<Arc<T>> {
        let type_id = TypeId::of::<T>();
        self.services
            .read()
            .get(&type_id)
            .and_then(|any| any.clone().downcast::<T>().ok())
    }

    /// Get required service or panic.
    ///
    /// # Panics
    ///
    /// Panics with `FATAL: No provider for {name}` if no service is registered.
    ///
    /// # Arguments
    ///
    /// * `name` - Human-readable service name for the error message
    #[must_use]
    pub fn get_required<T: Service>(&self, name: &str) -> Arc<T> {
        self.get::<T>()
            .unwrap_or_else(|| panic!("FATAL: No provider for {name}"))
    }

    /// Get or create a keyed service registry.
    ///
    /// This is a convenience method for managing [`MultiServiceRegistry`] instances.
    /// If a registry of type `R` doesn't exist, creates and registers a new one.
    ///
    /// # Type Parameters
    ///
    /// * `R` - The `MultiServiceRegistry` type (must implement `Service + Default`)
    #[must_use]
    pub fn get_or_create<R: Service + Default>(&self) -> Arc<R> {
        if let Some(registry) = self.get::<R>() {
            return registry;
        }

        // Create new registry
        let registry = Arc::new(R::default());
        self.register(registry.clone());
        registry
    }
}

impl Default for ServiceRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Debug for ServiceRegistry {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let count = self.services.read().len();
        f.debug_struct("ServiceRegistry")
            .field("registered_services", &count)
            .finish()
    }
}

// ============================================================================
// Multi Service Registry (multiple providers with typed key lookup)
// ============================================================================

/// Registry for keyed services - multiple providers with typed key-based lookup.
///
/// Generic mechanism in kernel, typed keys defined in drivers.
///
/// # Type Safety
///
/// Uses `K: ServiceKey` bound to ensure only proper service keys can be used.
/// The [`ServiceKey`] trait provides:
/// - Compile-time enforcement (arbitrary types rejected)
/// - Built-in service name for error messages
/// - Self-documenting API
///
/// # Example
///
/// ```ignore
/// use reovim_kernel::api::v1::{MultiServiceRegistry, ServiceKey};
///
/// // In driver: define typed key implementing ServiceKey
/// #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
/// pub enum VfsScheme { File, Memory, Ssh }
///
/// impl ServiceKey for VfsScheme {
///     fn service_name() -> &'static str { "VFS" }
/// }
///
/// // In driver: define type alias with typed key
/// pub type VfsProviderRegistry = MultiServiceRegistry<VfsScheme, dyn VfsDriver>;
///
/// // Usage
/// let registry = VfsProviderRegistry::new();
/// registry.register(VfsScheme::File, Arc::new(local_fs_provider));
/// let provider = registry.get(&VfsScheme::File);
/// ```
pub struct MultiServiceRegistry<K: ServiceKey, T: ?Sized> {
    providers: RwLock<HashMap<K, Arc<T>>>,
}

impl<K: ServiceKey, T: ?Sized + Send + Sync + 'static> MultiServiceRegistry<K, T> {
    /// Create a new empty keyed service registry.
    #[must_use]
    pub fn new() -> Self {
        Self {
            providers: RwLock::new(HashMap::new()),
        }
    }

    /// Register a provider for a typed key.
    ///
    /// Replaces any existing provider for the same key.
    ///
    /// # Arguments
    ///
    /// * `key` - The typed service key
    /// * `provider` - The provider instance wrapped in `Arc`
    pub fn register(&self, key: K, provider: Arc<T>) {
        self.providers.write().insert(key, provider);
    }

    /// Get provider by typed key.
    ///
    /// Returns `None` if no provider is registered for the key.
    #[must_use]
    pub fn get(&self, key: &K) -> Option<Arc<T>> {
        self.providers.read().get(key).cloned()
    }

    /// Get required provider or panic.
    ///
    /// Uses `K::service_name()` for the error message automatically.
    ///
    /// # Panics
    ///
    /// Panics with `FATAL: No {service_name} provider for {key:?}` if not found.
    #[must_use]
    pub fn get_required(&self, key: &K) -> Arc<T>
    where
        K: fmt::Debug,
    {
        self.get(key)
            .unwrap_or_else(|| panic!("FATAL: No {} provider for {:?}", K::service_name(), key))
    }

    /// List all registered keys.
    #[must_use]
    pub fn keys(&self) -> Vec<K> {
        self.providers.read().keys().cloned().collect()
    }

    /// Check if a provider is registered for the given key.
    #[must_use]
    pub fn contains(&self, key: &K) -> bool {
        self.providers.read().contains_key(key)
    }

    /// Get the number of registered providers.
    #[must_use]
    pub fn len(&self) -> usize {
        self.providers.read().len()
    }

    /// Check if the registry is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.providers.read().is_empty()
    }
}

impl<K: ServiceKey, T: ?Sized + Send + Sync + 'static> Default for MultiServiceRegistry<K, T> {
    fn default() -> Self {
        Self::new()
    }
}

// Implement Service so MultiServiceRegistry can be stored in ServiceRegistry
impl<K: ServiceKey, T: ?Sized + Send + Sync + 'static> Service for MultiServiceRegistry<K, T> {}

impl<K: ServiceKey + fmt::Debug, T: ?Sized + Send + Sync + 'static> fmt::Debug
    for MultiServiceRegistry<K, T>
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let keys: Vec<_> = self.keys();
        f.debug_struct("MultiServiceRegistry")
            .field("service", &K::service_name())
            .field("keys", &keys)
            .finish()
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // Test service type
    struct TestService {
        value: i32,
    }

    impl Service for TestService {}

    #[test]
    fn test_service_registry_register_and_get() {
        let registry = ServiceRegistry::new();

        registry.register(Arc::new(TestService { value: 42 }));

        let service = registry.get::<TestService>();
        assert!(service.is_some());
        assert_eq!(service.unwrap().value, 42);
    }

    #[test]
    fn test_service_registry_get_nonexistent() {
        let registry = ServiceRegistry::new();

        let service = registry.get::<TestService>();
        assert!(service.is_none());
    }

    #[test]
    fn test_service_registry_replace() {
        let registry = ServiceRegistry::new();

        registry.register(Arc::new(TestService { value: 1 }));
        registry.register(Arc::new(TestService { value: 2 }));

        let service = registry.get::<TestService>().unwrap();
        assert_eq!(service.value, 2);
    }

    #[test]
    #[should_panic(expected = "FATAL: No provider for TestService")]
    fn test_service_registry_get_required_panics() {
        let registry = ServiceRegistry::new();
        let _: Arc<TestService> = registry.get_required("TestService");
    }

    // Test service key
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
    enum TestKey {
        Primary,
        Secondary,
    }

    impl ServiceKey for TestKey {
        fn service_name() -> &'static str {
            "Test"
        }
    }

    // Test provider trait
    trait TestProvider: Send + Sync {
        fn name(&self) -> &'static str;
    }

    struct PrimaryProvider;
    impl TestProvider for PrimaryProvider {
        fn name(&self) -> &'static str {
            "primary"
        }
    }

    struct SecondaryProvider;
    impl TestProvider for SecondaryProvider {
        fn name(&self) -> &'static str {
            "secondary"
        }
    }

    #[test]
    fn test_multi_registry_register_and_get() {
        let registry = MultiServiceRegistry::<TestKey, dyn TestProvider>::new();

        registry.register(TestKey::Primary, Arc::new(PrimaryProvider));
        registry.register(TestKey::Secondary, Arc::new(SecondaryProvider));

        let primary = registry.get(&TestKey::Primary).unwrap();
        assert_eq!(primary.name(), "primary");

        let secondary = registry.get(&TestKey::Secondary).unwrap();
        assert_eq!(secondary.name(), "secondary");
    }

    #[test]
    fn test_multi_registry_get_nonexistent() {
        let registry = MultiServiceRegistry::<TestKey, dyn TestProvider>::new();

        let provider = registry.get(&TestKey::Primary);
        assert!(provider.is_none());
    }

    #[test]
    fn test_multi_registry_keys() {
        let registry = MultiServiceRegistry::<TestKey, dyn TestProvider>::new();

        registry.register(TestKey::Primary, Arc::new(PrimaryProvider));
        registry.register(TestKey::Secondary, Arc::new(SecondaryProvider));

        let keys = registry.keys();
        assert_eq!(keys.len(), 2);
        assert!(keys.contains(&TestKey::Primary));
        assert!(keys.contains(&TestKey::Secondary));
    }

    #[test]
    fn test_multi_registry_contains() {
        let registry = MultiServiceRegistry::<TestKey, dyn TestProvider>::new();

        registry.register(TestKey::Primary, Arc::new(PrimaryProvider));

        assert!(registry.contains(&TestKey::Primary));
        assert!(!registry.contains(&TestKey::Secondary));
    }

    #[test]
    fn test_multi_registry_len_and_is_empty() {
        let registry = MultiServiceRegistry::<TestKey, dyn TestProvider>::new();

        assert!(registry.is_empty());
        assert_eq!(registry.len(), 0);

        registry.register(TestKey::Primary, Arc::new(PrimaryProvider));

        assert!(!registry.is_empty());
        assert_eq!(registry.len(), 1);
    }

    #[test]
    #[should_panic(expected = "FATAL: No Test provider for Primary")]
    fn test_multi_registry_get_required_panics() {
        let registry = MultiServiceRegistry::<TestKey, dyn TestProvider>::new();
        let _: Arc<dyn TestProvider> = registry.get_required(&TestKey::Primary);
    }

    #[test]
    fn test_service_key_service_name() {
        assert_eq!(TestKey::service_name(), "Test");
    }

    // ========== ServiceRegistry Debug ==========

    #[test]
    fn test_service_registry_debug() {
        let registry = ServiceRegistry::new();
        registry.register(Arc::new(TestService { value: 1 }));

        let debug_str = format!("{registry:?}");
        assert!(debug_str.contains("ServiceRegistry"));
        assert!(debug_str.contains("registered_services"));
    }

    // ========== ServiceRegistry Default ==========

    #[test]
    fn test_service_registry_default() {
        let registry = ServiceRegistry::default();
        let debug_str = format!("{registry:?}");
        assert!(debug_str.contains('0')); // 0 registered services
    }

    // ========== get_or_create ==========

    #[test]
    fn test_service_registry_get_or_create() {
        #[derive(Default)]
        struct DefaultableService {
            value: i32,
        }
        impl Service for DefaultableService {}

        let registry = ServiceRegistry::new();

        // First call creates the service
        let svc1 = registry.get_or_create::<DefaultableService>();
        assert_eq!(svc1.value, 0); // Default value

        // Second call returns the cached service
        let svc2 = registry.get_or_create::<DefaultableService>();
        assert_eq!(Arc::as_ptr(&svc1), Arc::as_ptr(&svc2));
    }

    // ========== MultiServiceRegistry Debug ==========

    #[test]
    fn test_multi_service_registry_debug() {
        let registry = MultiServiceRegistry::<TestKey, dyn TestProvider>::new();
        registry.register(TestKey::Primary, Arc::new(PrimaryProvider));

        let debug_str = format!("{registry:?}");
        assert!(debug_str.contains("MultiServiceRegistry"));
        assert!(debug_str.contains("Test")); // service_name()
    }

    // ========== MultiServiceRegistry Default ==========

    #[test]
    fn test_multi_service_registry_default() {
        let registry = MultiServiceRegistry::<TestKey, dyn TestProvider>::default();
        assert!(registry.is_empty());
    }
}
