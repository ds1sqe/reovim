//! Hot reload manager for development.
//!
//! Provides file watching and automatic module reloading when source files change.
//! This module is feature-gated behind the `hot-reload` feature.

use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    sync::Arc,
};

use {
    notify::{Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher},
    reovim_arch::sync::Mutex,
    reovim_kernel::api::v1::{ModuleContext, ModuleError, ModuleId},
    tokio::sync::mpsc,
};

use super::registry::ModuleRegistry;

/// Hot reload manager for development.
///
/// Watches module files for changes and automatically reloads them.
/// All fields are mutex-protected for thread-safe concurrent access.
pub struct HotReloadManager {
    /// File watcher.
    watcher: Mutex<RecommendedWatcher>,

    /// Watched paths to module IDs.
    watched: Mutex<HashMap<PathBuf, ModuleId>>,

    /// Module registry (shared via Arc).
    registry: Arc<ModuleRegistry>,

    /// Module context for reinit.
    ctx: ModuleContext,

    /// Event receiver.
    rx: Mutex<mpsc::Receiver<Result<Event, notify::Error>>>,
}

impl HotReloadManager {
    /// Create new hot reload manager.
    ///
    /// # Arguments
    ///
    /// * `registry` - Shared module registry
    /// * `ctx` - Module context for reinitialization
    ///
    /// # Errors
    ///
    /// Returns error if file watcher fails to initialize.
    pub fn new(registry: Arc<ModuleRegistry>, ctx: ModuleContext) -> Result<Self, ModuleError> {
        let (tx, rx) = mpsc::channel(100);

        let watcher = notify::recommended_watcher(move |res| {
            // Use blocking send since we're in a sync callback
            let _ = tx.blocking_send(res);
        })
        .map_err(|e| ModuleError::InitFailed(format!("failed to create watcher: {e}")))?;

        Ok(Self {
            watcher: Mutex::new(watcher),
            watched: Mutex::new(HashMap::new()),
            registry,
            ctx,
            rx: Mutex::new(rx),
        })
    }

    /// Watch a module file for changes.
    ///
    /// # Arguments
    ///
    /// * `path` - Path to the module shared library
    /// * `id` - Module ID to associate with this path
    ///
    /// # Errors
    ///
    /// Returns error if watch registration fails.
    pub fn watch(&self, path: &Path, id: ModuleId) -> Result<(), ModuleError> {
        let mut watcher = self.watcher.lock();
        let mut watched = self.watched.lock();

        watcher
            .watch(path, RecursiveMode::NonRecursive)
            .map_err(|e| ModuleError::LoadFailed(format!("failed to watch: {e}")))?;

        watched.insert(path.to_path_buf(), id.clone());
        tracing::info!(module = %id, path = %path.display(), "watching for changes");
        Ok(())
    }

    /// Stop watching a file.
    ///
    /// # Errors
    ///
    /// Returns error if unwatch fails.
    pub fn unwatch(&self, path: &Path) -> Result<(), ModuleError> {
        let mut watcher = self.watcher.lock();
        let mut watched = self.watched.lock();

        watcher
            .unwatch(path)
            .map_err(|e| ModuleError::LoadFailed(format!("failed to unwatch: {e}")))?;

        if let Some(id) = watched.remove(path) {
            tracing::info!(module = %id, path = %path.display(), "stopped watching");
        }
        Ok(())
    }

    /// Process pending file change events.
    ///
    /// Returns list of reloaded module IDs.
    ///
    /// # Implementation Note
    ///
    /// Events are collected first, then processed. This prevents holding the
    /// channel lock during reload operations, which could block the file watcher.
    pub fn process_events(&self) -> Vec<ModuleId> {
        // 1. Collect events without holding the lock during processing
        let events: Vec<_> = {
            let mut rx = self.rx.lock();
            let mut events = Vec::new();
            while let Ok(event) = rx.try_recv() {
                events.push(event);
            }
            events
            // Lock released here
        };

        // 2. Collect module IDs that need reloading (deduplicate)
        let mut to_reload = Vec::new();
        for event in events {
            if let Ok(Event {
                kind: EventKind::Modify(_),
                paths,
                ..
            }) = event
            {
                for path in paths {
                    let id = {
                        let watched = self.watched.lock();
                        watched.get(&path).cloned()
                    };

                    if let Some(id) = id {
                        if !to_reload.contains(&id) {
                            to_reload.push(id);
                        }
                    }
                }
            }
        }

        // 3. Process reloads
        let mut reloaded = Vec::new();
        for id in to_reload {
            match self.reload(&id) {
                Ok(()) => {
                    tracing::info!(module = %id, "hot reload successful");
                    reloaded.push(id);
                }
                Err(e) => {
                    tracing::error!(module = %id, error = %e, "hot reload failed");
                }
            }
        }

        reloaded
    }

    /// Manually trigger reload for a module.
    ///
    /// Uses atomic reload from registry to prevent TOCTOU races.
    /// The registry holds its lock for the entire operation, ensuring
    /// no other thread can modify state between checks and reload.
    ///
    /// # Errors
    ///
    /// Returns error if:
    /// - Module has active dependents
    /// - Module is static (cannot reload)
    /// - Module path is unknown
    /// - Reload fails
    ///
    /// # Safety
    ///
    /// This method calls unsafe code internally because reloading
    /// a dynamic module requires loading a new shared library.
    /// The safety requirements are the same as for `load_dynamic`:
    /// the module must be ABI-compatible.
    pub fn reload(&self, id: &ModuleId) -> Result<(), ModuleError> {
        // Delegate to registry's atomic reload which:
        // 1. Checks dependents
        // 2. Gets path BEFORE unloading
        // 3. Unloads old module
        // 4. Loads new module from same path
        // 5. Reinitializes
        // All under a single lock to prevent TOCTOU races.
        //
        // SAFETY: We trust that the recompiled module maintains ABI compatibility.
        // This is a development feature - in production, version checks should be stricter.
        unsafe { self.registry.reload_atomic(id, &self.ctx) }
    }

    /// Get list of watched module IDs.
    #[must_use]
    pub fn watched_modules(&self) -> Vec<ModuleId> {
        let watched = self.watched.lock();
        watched.values().cloned().collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_context() -> ModuleContext {
        ModuleContext::default()
    }

    #[test]
    fn test_manager_creation() {
        let registry = Arc::new(ModuleRegistry::new());
        let ctx = create_test_context();

        let manager = HotReloadManager::new(registry, ctx);
        assert!(manager.is_ok());
    }

    #[test]
    fn test_watched_modules_empty() {
        let registry = Arc::new(ModuleRegistry::new());
        let ctx = create_test_context();
        let manager = HotReloadManager::new(registry, ctx).unwrap();

        assert!(manager.watched_modules().is_empty());
    }
}
