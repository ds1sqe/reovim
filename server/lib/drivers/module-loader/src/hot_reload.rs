//! Hot reload manager for development.
//!
//! Provides file watching and automatic module reloading when source files
//! change. This module is feature-gated behind the `hot-reload` feature.
//!
//! # Usage
//!
//! Enable with `--features hot-reload` at compile time. At runtime, watch
//! dynamic module `.so` files for changes:
//!
//! ```ignore
//! let manager = HotReloadManager::new(registry, ctx)?;
//! manager.watch(Path::new("/path/to/module.so"), &module_id)?;
//!
//! // In event loop:
//! let reloaded = manager.process_events();
//! ```

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
    file_watcher: Mutex<RecommendedWatcher>,

    /// Watched paths to module IDs.
    watch_map: Mutex<HashMap<PathBuf, ModuleId>>,

    /// Module registry (shared via Arc).
    registry: Arc<ModuleRegistry>,

    /// Module context for reinit.
    ctx: ModuleContext,

    /// Event receiver.
    rx: Mutex<mpsc::Receiver<Result<Event, notify::Error>>>,
}

impl HotReloadManager {
    /// Create a new hot reload manager.
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

        let file_watcher = notify::recommended_watcher(move |res| {
            // Use blocking send since we're in a sync callback
            let _ = tx.blocking_send(res);
        })
        .map_err(|e| ModuleError::InitFailed(format!("failed to create watcher: {e}")))?;

        Ok(Self {
            file_watcher: Mutex::new(file_watcher),
            watch_map: Mutex::new(HashMap::new()),
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
    pub fn watch(&self, path: &Path, id: &ModuleId) -> Result<(), ModuleError> {
        self.file_watcher
            .lock()
            .watch(path, RecursiveMode::NonRecursive)
            .map_err(|e| ModuleError::LoadFailed(format!("failed to watch: {e}")))?;

        self.watch_map
            .lock()
            .insert(path.to_path_buf(), id.clone());
        tracing::info!(module = %id, path = %path.display(), "watching for changes");
        Ok(())
    }

    /// Stop watching a file.
    ///
    /// # Errors
    ///
    /// Returns error if unwatch fails.
    pub fn unwatch(&self, path: &Path) -> Result<(), ModuleError> {
        self.file_watcher
            .lock()
            .unwatch(path)
            .map_err(|e| ModuleError::LoadFailed(format!("failed to unwatch: {e}")))?;

        let removed = self.watch_map.lock().remove(path);
        if let Some(id) = removed {
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
            let mut collected = Vec::new();
            while let Ok(event) = rx.try_recv() {
                collected.push(event);
            }
            collected
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
                    let id = self.watch_map.lock().get(&path).cloned();

                    if let Some(id) = id
                        && !to_reload.contains(&id)
                    {
                        to_reload.push(id);
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
    pub fn reload(&self, id: &ModuleId) -> Result<(), ModuleError> {
        // Delegate to registry's atomic reload which:
        // 1. Checks dependents
        // 2. Gets path BEFORE unloading
        // 3. Unloads old module
        // 4. Loads new module from same path
        // 5. Reinitializes
        // All under a single lock to prevent TOCTOU races.
        #[allow(unsafe_code)]
        // SAFETY: Hot reload assumes the recompiled .so is ABI-compatible with the
        // running binary. This is valid for development builds where both host and
        // module are compiled from the same workspace with the same rustc version.
        unsafe {
            self.registry.reload_atomic(id, &self.ctx)
        }
    }

    /// Get list of watched module IDs.
    #[must_use]
    pub fn watched_modules(&self) -> Vec<ModuleId> {
        self.watch_map.lock().values().cloned().collect()
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

    #[test]
    fn test_process_events_empty() {
        let registry = Arc::new(ModuleRegistry::new());
        let ctx = create_test_context();
        let manager = HotReloadManager::new(registry, ctx).unwrap();

        // No events to process
        let reloaded = manager.process_events();
        assert!(reloaded.is_empty());
    }

    #[test]
    fn test_watch_nonexistent_path() {
        let registry = Arc::new(ModuleRegistry::new());
        let ctx = create_test_context();
        let manager = HotReloadManager::new(registry, ctx).unwrap();

        // Watching a nonexistent path should fail
        let result = manager.watch(
            Path::new("/nonexistent/module.so"),
            &ModuleId::new("test"),
        );
        assert!(result.is_err());
    }

    #[test]
    fn test_watch_and_unwatch() {
        let registry = Arc::new(ModuleRegistry::new());
        let ctx = create_test_context();
        let manager = HotReloadManager::new(registry, ctx).unwrap();

        // Create a temp file to watch
        let dir = std::env::temp_dir().join(format!(
            "reovim-hot-reload-test-{}",
            std::process::id()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let file = dir.join("test.so");
        std::fs::write(&file, b"fake module").unwrap();

        // Watch
        let id = ModuleId::new("test-mod");
        assert!(manager.watch(&file, &id).is_ok());
        assert_eq!(manager.watched_modules().len(), 1);

        // Unwatch
        assert!(manager.unwatch(&file).is_ok());
        assert!(manager.watched_modules().is_empty());

        // Cleanup
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn test_reload_static_module_fails() {
        use reovim_kernel::api::v1::{Module, ProbeResult, Version};

        struct TestMod;
        impl Module for TestMod {
            fn id(&self) -> ModuleId {
                ModuleId::new("test-static")
            }
            fn name(&self) -> &'static str {
                "Test"
            }
            fn version(&self) -> Version {
                Version::new(1, 0, 0)
            }
            fn init(&mut self, _: &ModuleContext) -> ProbeResult {
                ProbeResult::Success
            }
            fn exit(&mut self) -> Result<(), ModuleError> {
                Ok(())
            }
        }

        let registry = Arc::new(ModuleRegistry::new());
        registry.register(TestMod).unwrap();
        let ctx = create_test_context();
        let manager = HotReloadManager::new(Arc::clone(&registry), ctx).unwrap();

        // Reloading a static module should fail (no path)
        let result = manager.reload(&ModuleId::new("test-static"));
        assert!(result.is_err());
    }

    #[test]
    fn test_reload_nonexistent_module_fails() {
        let registry = Arc::new(ModuleRegistry::new());
        let ctx = create_test_context();
        let manager = HotReloadManager::new(registry, ctx).unwrap();

        let result = manager.reload(&ModuleId::new("nonexistent"));
        assert!(result.is_err());
    }
}
