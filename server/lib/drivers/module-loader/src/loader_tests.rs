use std::path::PathBuf;

use reovim_kernel::api::v1::{Module, ModuleContext, ModuleError, ModuleId, ProbeResult, Version};

use super::{handle::ModuleHandle, loader::ModuleLoader};

// ============================================================================
// Test module
// ============================================================================

struct TestModule {
    initialized: bool,
}

impl Module for TestModule {
    fn id(&self) -> ModuleId {
        ModuleId::new("test-module")
    }
    fn name(&self) -> &'static str {
        "Test Module"
    }
    fn version(&self) -> Version {
        Version::new(1, 0, 0)
    }
    fn init(&mut self, _ctx: &ModuleContext) -> ProbeResult {
        self.initialized = true;
        ProbeResult::Success
    }
    fn exit(&mut self) -> Result<(), ModuleError> {
        self.initialized = false;
        Ok(())
    }
}

struct AnotherModule;

impl Module for AnotherModule {
    fn id(&self) -> ModuleId {
        ModuleId::new("another-module")
    }
    fn name(&self) -> &'static str {
        "Another Module"
    }
    fn version(&self) -> Version {
        Version::new(2, 0, 0)
    }
    fn init(&mut self, _ctx: &ModuleContext) -> ProbeResult {
        ProbeResult::Success
    }
    fn exit(&mut self) -> Result<(), ModuleError> {
        Ok(())
    }
}

// ============================================================================
// new / default
// ============================================================================

#[test]
fn new_loader_is_empty() {
    let loader = ModuleLoader::new();
    assert!(loader.is_empty());
    assert_eq!(loader.len(), 0);
}

#[test]
fn default_loader_is_empty() {
    let loader = ModuleLoader::default();
    assert!(loader.is_empty());
}

#[test]
fn new_loader_has_search_paths() {
    let loader = ModuleLoader::new();
    assert!(!loader.search_paths().is_empty());
}

// ============================================================================
// register_static
// ============================================================================

#[test]
fn register_static_success() {
    let mut loader = ModuleLoader::new();
    let id = loader
        .register_static(TestModule { initialized: false })
        .unwrap();
    assert_eq!(id.as_str(), "test-module");
    assert_eq!(loader.len(), 1);
    assert!(!loader.is_empty());
}

#[test]
fn register_static_duplicate_fails() {
    let mut loader = ModuleLoader::new();
    loader
        .register_static(TestModule { initialized: false })
        .unwrap();

    let result = loader.register_static(TestModule { initialized: false });
    assert!(matches!(result, Err(ModuleError::LoadFailed(_))));
}

#[test]
fn register_static_get_returns_handle() {
    let mut loader = ModuleLoader::new();
    let id = loader
        .register_static(TestModule { initialized: false })
        .unwrap();

    let handle = loader.get(&id);
    assert!(handle.is_some());
    assert_eq!(handle.unwrap().id().as_str(), "test-module");
}

#[test]
fn register_static_get_mut_returns_handle() {
    let mut loader = ModuleLoader::new();
    let id = loader
        .register_static(TestModule { initialized: false })
        .unwrap();

    let handle = loader.get_mut(&id);
    assert!(handle.is_some());
}

// ============================================================================
// register_static_boxed
// ============================================================================

#[test]
fn register_static_boxed_success() {
    let mut loader = ModuleLoader::new();
    let module: Box<dyn Module> = Box::new(TestModule { initialized: false });
    let id = loader.register_static_boxed(module).unwrap();
    assert_eq!(id.as_str(), "test-module");
}

#[test]
fn register_static_boxed_duplicate_fails() {
    let mut loader = ModuleLoader::new();
    loader
        .register_static(TestModule { initialized: false })
        .unwrap();

    let module: Box<dyn Module> = Box::new(TestModule { initialized: false });
    let result = loader.register_static_boxed(module);
    assert!(matches!(result, Err(ModuleError::LoadFailed(_))));
}

// ============================================================================
// unload
// ============================================================================

#[test]
fn unload_removes_module() {
    let mut loader = ModuleLoader::new();
    let id = loader
        .register_static(TestModule { initialized: false })
        .unwrap();

    let handle = loader.unload(&id).unwrap();
    assert_eq!(handle.id().as_str(), "test-module");
    assert!(loader.get(&id).is_none());
    assert!(loader.is_empty());
}

#[test]
fn unload_not_found() {
    let mut loader = ModuleLoader::new();
    let result = loader.unload(&ModuleId::new("nonexistent"));
    assert!(matches!(result, Err(ModuleError::NotLoaded(_))));
}

// ============================================================================
// get / get_mut
// ============================================================================

#[test]
fn get_nonexistent_returns_none() {
    let loader = ModuleLoader::new();
    assert!(loader.get(&ModuleId::new("nope")).is_none());
}

#[test]
fn get_mut_nonexistent_returns_none() {
    let mut loader = ModuleLoader::new();
    assert!(loader.get_mut(&ModuleId::new("nope")).is_none());
}

// ============================================================================
// loaded_ids
// ============================================================================

#[test]
fn loaded_ids_empty() {
    let loader = ModuleLoader::new();
    assert_eq!(loader.loaded_ids().count(), 0);
}

#[test]
fn loaded_ids_multiple() {
    let mut loader = ModuleLoader::new();
    loader
        .register_static(TestModule { initialized: false })
        .unwrap();
    loader.register_static(AnotherModule).unwrap();

    let mut ids: Vec<_> = loader.loaded_ids().map(ModuleId::as_str).collect();
    ids.sort_unstable();
    assert_eq!(ids, vec!["another-module", "test-module"]);
}

// ============================================================================
// add_search_path
// ============================================================================

#[test]
fn add_search_path_appends() {
    let mut loader = ModuleLoader::new();
    let initial_len = loader.search_paths().len();

    let new_path = PathBuf::from("/custom/modules");
    loader.add_search_path(new_path.clone());

    assert_eq!(loader.search_paths().len(), initial_len + 1);
    assert!(loader.search_paths().contains(&new_path));
}

#[test]
fn add_search_path_deduplicates() {
    let mut loader = ModuleLoader::new();
    let new_path = PathBuf::from("/custom/modules");
    loader.add_search_path(new_path.clone());
    let len_after_first = loader.search_paths().len();

    loader.add_search_path(new_path);
    assert_eq!(loader.search_paths().len(), len_after_first);
}

// ============================================================================
// discover
// ============================================================================

#[test]
fn discover_returns_vec() {
    let loader = ModuleLoader::new();
    // May be empty if no modules installed, but should not panic
    let _ = loader.discover();
}

// ============================================================================
// insert_handle
// ============================================================================

#[test]
fn insert_handle_duplicate_id_returns_load_failed() {
    let mut loader = ModuleLoader::new();

    let handle1 = ModuleHandle::from_static(TestModule { initialized: false });
    loader.insert_handle(handle1).unwrap();

    // Second handle with the same module ID must fail.
    let handle2 = ModuleHandle::from_static(TestModule { initialized: false });
    let result = loader.insert_handle(handle2);
    assert!(
        matches!(result, Err(ModuleError::LoadFailed(_))),
        "expected LoadFailed for duplicate id, got {result:?}"
    );
}
