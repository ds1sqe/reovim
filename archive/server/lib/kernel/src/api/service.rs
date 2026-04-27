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
//! ext/server/drivers/vfs/            → VfsScheme enum + VfsProviderRegistry
//! ext/server/drivers/input/          → ModeProviderKey enum + ModeProviderRegistry
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

use std::{any::Any, collections::HashMap, fmt, hash::Hash, sync::Arc};

use reovim_arch::sync::RwLock;

// ============================================================================
// Service Trait
// ============================================================================

/// Marker trait for services that can be registered in [`ServiceRegistry`].
///
/// Implement this trait for types that should be discoverable via the
/// service registry. The trait bounds ensure thread-safety.
///
/// # Cross-cdylib Safety Contract
///
/// `ServiceRegistry` uses [`std::any::type_name`] (not `TypeId`) for lookup,
/// enabling stable service discovery across cdylib boundaries. This imposes
/// two constraints on implementing types:
///
/// 1. **Same crate version**: All compilation units (host binary and dynamic
///    `.so` modules) must link against the **same version** of the crate that
///    defines the service type. Different versions produce the same `type_name`
///    but potentially different layouts, causing unsound pointer casts.
///
/// 2. **Stable type path**: The type must have a unique, fully-qualified path.
///    Named type aliases (e.g., `CommandHandlerStore`) are safe. Generic
///    monomorphizations (e.g., `MultiServiceRegistry<K, T>`) are safe when
///    all cdylibs share the same crate version, but carry higher risk under
///    version skew since the expanded `type_name` would still match.
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
/// Uses `type_name` as the primary lookup key for cross-cdylib stability.
/// Rust's `TypeId` is not guaranteed stable across separate compilation units
/// (cdylib modules), so dynamic `.so` modules would silently create duplicate
/// service instances if `TypeId` were the only key. By keying on `type_name`
/// (the fully-qualified type path, stable for the same crate version), both
/// the host binary and dynamically loaded modules resolve to the same entry.
///
/// # Safety
///
/// The `get` method uses an unsafe `Arc` pointer cast (identical to the
/// implementation inside `Arc::downcast`) instead of `Any::downcast`, because
/// `downcast` relies on `TypeId` which may differ across cdylib boundaries.
/// Correctness depends on `type_name` being unique per concrete type within
/// a single crate version — a property that holds in practice for all
/// non-generic, non-closure types.
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
    services: RwLock<HashMap<String, Arc<dyn Any + Send + Sync>>>,
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
        let key = std::any::type_name::<T>().to_string();
        self.services.write().insert(key, service);
    }

    /// Get a unique service by type.
    ///
    /// Uses `type_name` for lookup (stable across cdylib boundaries) and an
    /// unsafe `Arc` pointer cast instead of `Any::downcast` (which relies on
    /// `TypeId`, unstable across cdylib boundaries).
    ///
    /// Returns `None` if no service of type `T` is registered.
    ///
    /// # Safety (internal)
    ///
    /// The pointer cast is sound because `type_name` uniquely identifies the
    /// concrete type within a crate version. This is the same cast that
    /// `Arc::downcast` performs internally, minus the `TypeId` check.
    ///
    /// **Invariant**: correctness requires `T` to be a concrete named type with
    /// a stable fully-qualified path — not a generic monomorphization or a type
    /// re-exported under a different module path. All current call sites use
    /// named type aliases from a single crate (e.g., `CommandHandlerStore`,
    /// `ModeInfoStore`), satisfying this requirement.
    #[must_use]
    #[allow(unsafe_code)]
    pub fn get<T: Service>(&self) -> Option<Arc<T>> {
        let key = std::any::type_name::<T>();
        self.services.read().get(key).map(|any| {
            // SAFETY: The entry was inserted by `register::<T>()` which stored
            // an `Arc<T>` erased to `Arc<dyn Any>`. We look up by type_name
            // which is stable across cdylib boundaries (unlike TypeId).
            // This cast is identical to what `Arc::downcast` does internally:
            // extract the data pointer from the fat pointer and reconstruct Arc<T>.
            let raw: *const (dyn Any + Send + Sync) = Arc::into_raw(Arc::clone(any));
            unsafe { Arc::from_raw(raw.cast::<T>()) }
        })
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

    /// List all registered providers.
    #[must_use]
    pub fn values(&self) -> Vec<Arc<T>> {
        self.providers.read().values().cloned().collect()
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
