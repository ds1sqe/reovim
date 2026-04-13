use std::{path::Path, sync::Arc};

use reovim_kernel::api::v1::{ModuleContext, ModuleId};

use super::{hot_reload::HotReloadManager, registry::ModuleRegistry};

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
    let result = manager.watch(Path::new("/nonexistent/module.so"), &ModuleId::new("test"));
    assert!(result.is_err());
}

#[test]
fn test_watch_and_unwatch() {
    let registry = Arc::new(ModuleRegistry::new());
    let ctx = create_test_context();
    let manager = HotReloadManager::new(registry, ctx).unwrap();

    // Create a temp file to watch
    let dir = std::env::temp_dir().join(format!("reovim-hot-reload-test-{}", std::process::id()));
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
    use reovim_kernel::api::v1::{Module, ModuleError, ProbeResult, Version};

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
