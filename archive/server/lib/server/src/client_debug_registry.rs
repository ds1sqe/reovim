//! Server-side registry for client-debug driver instances (#770 Phase 1).
//!
//! The server hosts `LoadedClientDebug` instances in a
//! `ClientDebugRegistry` on behalf of a same-process client (TUI) in
//! embedded mode (#769 `transport_inproc`). A remote inspector (CLI,
//! test harness) drives the drivers via the bidirectional
//! `ClientDebugService::DebugStream` RPC.
//!
//! ## Cross-tree dependency rationale
//!
//! This is the first production dep from a `server/*` crate on
//! `clients/lib/subsys/driver-loader`. The semantic is "the server
//! embeds the client-debug driver-loader because the drivers live on
//! the client side of the ABI, but their runtime host is the server
//! process when the server and client share a process (embedded mode)."
//! Both dependencies (`reovim-client-subsys-debug`,
//! `reovim-client-subsys-driver-loader`) are subsys-tier core and are
//! permitted by the `core_ext_boundary.rs` depgraph probe.
//!
//! ## Concurrency model
//!
//! - The outer map (`RwLock`) is mutated only during composition-root
//!   setup and panic-eviction; the hot path is read-only lookup.
//! - Each driver entry is wrapped in `Arc<tokio::sync::Mutex<_>>`.
//!   `observe`/`drive` acquire this mutex and hold it for the
//!   duration of their operation.
//! - The Phase 0 `ClientDebugSurface` trait returns an observer that
//!   borrows `&mut self` of the driver. Consequently the mutex guard
//!   MUST be held for the observer's entire lifetime — releasing it
//!   between frames would invalidate the observer's `&mut` borrow.
//!   Concurrency consequence: concurrent probe/drive/observe on the
//!   same driver serialize. Concurrent operations on *different*
//!   drivers do not block each other.

use {
    parking_lot::RwLock,
    reovim_client_subsys_debug::{DebugError, DebugProbe},
    reovim_client_subsys_driver_loader::{LoadError, LoadedClientDebug, LoadedDebugObserver},
    std::{collections::HashMap, sync::Arc},
    tokio::sync::{Mutex as AsyncMutex, OwnedMutexGuard},
};

/// Error surfaced by `ClientDebugRegistry` operations.
///
/// Decouples callers from `LoadError`, which lives in the
/// driver-loader crate and carries loader-specific variants the
/// registry does not expose.
#[derive(Debug)]
pub enum RegistryError {
    /// No driver registered under this name.
    UnknownDriver(String),
    /// Driver returned a domain error (bad selector, unknown schema).
    /// Non-terminal: the stream stays open.
    DriverError(DebugError),
    /// Driver panicked across the FFI boundary (rc == -2). The entry
    /// has been evicted from the registry.
    DriverPanicked,
    /// Loader-level error (library open, ABI validation). Terminal
    /// for any driver operation but the registry itself remains
    /// usable.
    Loader(String),
}

impl std::fmt::Display for RegistryError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::UnknownDriver(name) => write!(f, "unknown driver: {name}"),
            Self::DriverError(e) => write!(f, "driver error: {e}"),
            Self::DriverPanicked => f.write_str("driver panicked"),
            Self::Loader(msg) => write!(f, "loader error: {msg}"),
        }
    }
}

impl std::error::Error for RegistryError {}

impl From<LoadError> for RegistryError {
    fn from(e: LoadError) -> Self {
        match e {
            LoadError::DriverError(msg) => Self::DriverError(DebugError(msg)),
            LoadError::DriverPanicked => Self::DriverPanicked,
            other => Self::Loader(format!("{other:?}")),
        }
    }
}

/// Object-safe adapter trait for driver implementations. Both
/// `LoadedClientDebug` (cdylib-backed) and in-tree test stubs impl
/// this trait so the registry can hold heterogeneous drivers in one
/// map.
pub trait DebugDriverHandle: Send {
    /// Return probe metadata. Must be cheap — the registry may call
    /// this under the read lock.
    fn probe(&self) -> DebugProbe;

    /// Open an observer for the given selector. The returned observer
    /// borrows `&mut self`; the registry keeps the driver's mutex
    /// guard alive for the observer's lifetime.
    ///
    /// # Errors
    /// `RegistryError::DriverError` on unknown selector;
    /// `RegistryError::DriverPanicked` if the driver panicked;
    /// `RegistryError::Loader` on loader-level failures.
    fn observe<'a>(
        &'a mut self,
        selector: &[u8],
    ) -> Result<Box<dyn DebugObserverHandle + Send + 'a>, RegistryError>;

    /// Execute a one-shot drive command.
    ///
    /// # Errors
    /// Same as [`observe`](Self::observe).
    fn drive(&mut self, command: &[u8]) -> Result<Vec<u8>, RegistryError>;
}

/// Object-safe observer handle. Returned by
/// [`DebugDriverHandle::observe`].
pub trait DebugObserverHandle: Send {
    /// Fetch the next frame. `Ok(None)` is end-of-stream.
    ///
    /// # Errors
    /// `RegistryError::DriverError` / `DriverPanicked` / `Loader` as
    /// appropriate.
    fn next_frame(&mut self) -> Result<Option<Vec<u8>>, RegistryError>;
}

impl DebugDriverHandle for LoadedClientDebug {
    fn probe(&self) -> DebugProbe {
        Self::probe(self)
    }

    fn observe<'a>(
        &'a mut self,
        selector: &[u8],
    ) -> Result<Box<dyn DebugObserverHandle + Send + 'a>, RegistryError> {
        let observer = Self::observe(self, selector).map_err(RegistryError::from)?;
        Ok(Box::new(observer))
    }

    fn drive(&mut self, command: &[u8]) -> Result<Vec<u8>, RegistryError> {
        Self::drive(self, command).map_err(RegistryError::from)
    }
}

impl DebugObserverHandle for LoadedDebugObserver<'_> {
    fn next_frame(&mut self) -> Result<Option<Vec<u8>>, RegistryError> {
        Self::next_frame(self).map_err(RegistryError::from)
    }
}

/// Thread-safe shared handle to one driver entry in the registry.
type DriverSlot = Arc<AsyncMutex<Box<dyn DebugDriverHandle>>>;

/// Server-side store for client-debug drivers. Thread-safe; clone the
/// `Arc<ClientDebugRegistry>` returned by composition roots.
pub struct ClientDebugRegistry {
    drivers: RwLock<HashMap<String, DriverSlot>>,
}

impl ClientDebugRegistry {
    /// Create an empty registry. Composition roots populate it before
    /// serving RPCs.
    #[must_use]
    pub fn new() -> Self {
        Self {
            drivers: RwLock::new(HashMap::new()),
        }
    }

    /// Register a driver under a name. Replaces any prior registration
    /// under the same name.
    pub fn register(&self, name: impl Into<String>, driver: Box<dyn DebugDriverHandle>) {
        let mut map = self.drivers.write();
        map.insert(name.into(), Arc::new(AsyncMutex::new(driver)));
    }

    /// List the names of registered drivers, in unspecified order.
    #[must_use]
    pub fn list_drivers(&self) -> Vec<String> {
        self.drivers.read().keys().cloned().collect()
    }

    /// Read probe metadata for the named driver without opening a
    /// session. Returns `None` if the driver is not registered.
    ///
    /// This call synchronously acquires the driver mutex's
    /// [`try_lock`](AsyncMutex::try_lock) to avoid blocking the outer
    /// `RwLock`. If the mutex is contended (another operation in
    /// flight), returns `Some(Err(...))` with a
    /// `RegistryError::Loader` indicating contention — the caller can
    /// retry or use [`probe_async`](Self::probe_async).
    #[must_use]
    pub fn probe(&self, name: &str) -> Option<Result<DebugProbe, RegistryError>> {
        let arc = self.drivers.read().get(name)?.clone();
        Some(arc.try_lock().map_or_else(
            |_| Err(RegistryError::Loader("driver busy (try_lock contended)".into())),
            |guard| Ok(guard.probe()),
        ))
    }

    /// Async probe that waits for the driver mutex.
    ///
    /// # Errors
    /// Returns `RegistryError::UnknownDriver` if the name is not
    /// registered.
    pub async fn probe_async(&self, name: &str) -> Result<DebugProbe, RegistryError> {
        let arc = self.handle_arc(name)?;
        let guard = arc.lock_owned().await;
        Ok(guard.probe())
    }

    /// Drive the named driver with a one-shot command.
    ///
    /// # Errors
    /// See [`RegistryError`].
    pub async fn drive(&self, name: &str, command: &[u8]) -> Result<Vec<u8>, RegistryError> {
        let arc = self.handle_arc(name)?;
        let mut guard = arc.lock_owned().await;
        guard.drive(command)
    }

    /// Open an observer. The returned [`ObserverPump`] owns the
    /// driver's mutex guard; it will be released when the pump is
    /// dropped (typically at EOS or when the client disconnects).
    ///
    /// # Errors
    /// See [`RegistryError`].
    pub async fn observe(
        &self,
        name: &str,
        selector: &[u8],
    ) -> Result<ObserverPump, RegistryError> {
        let arc = self.handle_arc(name)?;
        let mut guard = arc.lock_owned().await;
        let observer = guard.observe(selector)?;

        // SAFETY: the observer borrows `guard` for lifetime `'_`.
        // `ObserverPump` stores both so the guard outlives the
        // observer. Drop order is enforced by field declaration order
        // (observer dropped first, then guard). This mirrors the
        // Phase 0 macro pattern for crossing FFI lifetime boundaries.
        #[allow(unsafe_code)]
        let observer: Box<dyn DebugObserverHandle + Send + 'static> = unsafe {
            std::mem::transmute::<
                Box<dyn DebugObserverHandle + Send + '_>,
                Box<dyn DebugObserverHandle + Send + 'static>,
            >(observer)
        };

        Ok(ObserverPump {
            observer,
            _guard: guard,
        })
    }

    /// Remove a driver from the registry. Used after a driver panic
    /// (`rc == -2`) to prevent reuse of a corrupted instance.
    /// No-op if the name is not registered.
    pub fn evict_on_panic(&self, name: &str) {
        let mut map = self.drivers.write();
        map.remove(name);
    }

    fn handle_arc(&self, name: &str) -> Result<DriverSlot, RegistryError> {
        self.drivers
            .read()
            .get(name)
            .cloned()
            .ok_or_else(|| RegistryError::UnknownDriver(name.to_owned()))
    }
}

impl Default for ClientDebugRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Debug for ClientDebugRegistry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ClientDebugRegistry")
            .field("drivers", &self.list_drivers())
            .finish()
    }
}

/// Guard returned by [`ClientDebugRegistry::observe`]. Owns the
/// driver's mutex guard so the observer's `&mut Driver` borrow stays
/// valid. Dropped in field order: observer first, then guard.
pub struct ObserverPump {
    // Declared first so it drops first. Do not reorder.
    observer: Box<dyn DebugObserverHandle + Send + 'static>,
    _guard: OwnedMutexGuard<Box<dyn DebugDriverHandle>>,
}

impl ObserverPump {
    /// Fetch the next frame. `Ok(None)` is end-of-stream.
    ///
    /// # Errors
    /// See [`RegistryError`].
    pub fn next_frame(&mut self) -> Result<Option<Vec<u8>>, RegistryError> {
        self.observer.next_frame()
    }
}

impl std::fmt::Debug for ObserverPump {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ObserverPump").finish_non_exhaustive()
    }
}

#[cfg(test)]
#[path = "client_debug_registry_tests.rs"]
mod tests;
