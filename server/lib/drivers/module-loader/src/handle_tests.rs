use reovim_kernel::api::v1::{Module, ModuleContext, ModuleError, ModuleId, ProbeResult, Version};

use super::handle::{InitResult, ModuleHandle};

// ============================================================================
// Test modules
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

struct DeferModule;

impl Module for DeferModule {
    fn id(&self) -> ModuleId {
        ModuleId::new("defer-module")
    }
    fn name(&self) -> &'static str {
        "Defer Module"
    }
    fn version(&self) -> Version {
        Version::new(1, 0, 0)
    }
    fn init(&mut self, _ctx: &ModuleContext) -> ProbeResult {
        ProbeResult::Defer("waiting for service".into())
    }
    fn exit(&mut self) -> Result<(), ModuleError> {
        Ok(())
    }
}

struct FailModule;

impl Module for FailModule {
    fn id(&self) -> ModuleId {
        ModuleId::new("fail-module")
    }
    fn name(&self) -> &'static str {
        "Fail Module"
    }
    fn version(&self) -> Version {
        Version::new(1, 0, 0)
    }
    fn init(&mut self, _ctx: &ModuleContext) -> ProbeResult {
        ProbeResult::Failed(ModuleError::InitFailed("deliberate failure".into()))
    }
    fn exit(&mut self) -> Result<(), ModuleError> {
        Ok(())
    }
}

struct DepsModule;

impl Module for DepsModule {
    fn id(&self) -> ModuleId {
        ModuleId::new("deps-module")
    }
    fn name(&self) -> &'static str {
        "Deps Module"
    }
    fn version(&self) -> Version {
        Version::new(2, 1, 0)
    }
    fn dependencies(&self) -> Vec<ModuleId> {
        vec![ModuleId::new("vim"), ModuleId::new("editor")]
    }
    fn optional_dependencies(&self) -> Vec<ModuleId> {
        vec![ModuleId::new("lsp")]
    }
    fn init(&mut self, _ctx: &ModuleContext) -> ProbeResult {
        ProbeResult::Success
    }
    fn exit(&mut self) -> Result<(), ModuleError> {
        Ok(())
    }
}

struct HotReloadModule {
    counter: u32,
}

impl Module for HotReloadModule {
    fn id(&self) -> ModuleId {
        ModuleId::new("hot-reload-module")
    }
    fn name(&self) -> &'static str {
        "Hot Reload Module"
    }
    fn version(&self) -> Version {
        Version::new(1, 0, 0)
    }
    fn init(&mut self, _ctx: &ModuleContext) -> ProbeResult {
        ProbeResult::Success
    }
    fn exit(&mut self) -> Result<(), ModuleError> {
        Ok(())
    }
    fn supports_hot_reload(&self) -> bool {
        true
    }
    fn save_state(&self) -> Option<Box<[u8]>> {
        Some(self.counter.to_le_bytes().to_vec().into_boxed_slice())
    }
    fn restore_state(&mut self, state: &[u8]) -> Result<(), ModuleError> {
        if state.len() != 4 {
            return Err(ModuleError::InitFailed("invalid state size".into()));
        }
        self.counter = u32::from_le_bytes([state[0], state[1], state[2], state[3]]);
        Ok(())
    }
}

// ============================================================================
// from_static / from_boxed
// ============================================================================

#[test]
fn static_handle_creation() {
    let handle = ModuleHandle::from_static(TestModule { initialized: false });
    assert!(handle.is_static());
    assert!(!handle.is_dynamic());
    assert_eq!(handle.id().as_str(), "test-module");
    assert_eq!(handle.name(), "Test Module");
    assert_eq!(handle.version(), Version::new(1, 0, 0));
    assert!(handle.path().is_none());
}

#[test]
fn boxed_handle_creation() {
    let module: Box<dyn Module> = Box::new(TestModule { initialized: false });
    let handle = ModuleHandle::from_boxed(module);
    assert!(handle.is_static());
    assert_eq!(handle.id().as_str(), "test-module");
}

#[test]
fn as_module_returns_reference() {
    let handle = ModuleHandle::from_static(TestModule { initialized: false });
    let module_ref = handle.as_module();
    assert!(module_ref.is_some());
    assert_eq!(module_ref.unwrap().id().as_str(), "test-module");
}

#[test]
fn as_module_mut_returns_mutable_reference() {
    let mut handle = ModuleHandle::from_static(TestModule { initialized: false });
    let module_ref = handle.as_module_mut();
    assert!(module_ref.is_some());
}

// ============================================================================
// init / exit
// ============================================================================

#[test]
fn static_handle_init_success() {
    let mut handle = ModuleHandle::from_static(TestModule { initialized: false });
    let ctx = ModuleContext::default();
    let result = handle.init(&ctx);
    assert!(matches!(result, Ok(InitResult::Success)));
}

#[test]
fn static_handle_init_defer() {
    let mut handle = ModuleHandle::from_static(DeferModule);
    let ctx = ModuleContext::default();
    let result = handle.init(&ctx);
    assert!(matches!(result, Ok(InitResult::Defer(_))));
}

#[test]
fn static_handle_init_failed() {
    let mut handle = ModuleHandle::from_static(FailModule);
    let ctx = ModuleContext::default();
    let result = handle.init(&ctx);
    assert!(result.is_err());
}

#[test]
fn static_handle_exit() {
    let mut handle = ModuleHandle::from_static(TestModule { initialized: true });
    let result = handle.exit();
    assert!(result.is_ok());
}

// ============================================================================
// on_all_loaded
// ============================================================================

#[test]
fn on_all_loaded_does_not_panic() {
    let mut handle = ModuleHandle::from_static(TestModule { initialized: true });
    let ctx = ModuleContext::default();
    handle.on_all_loaded(&ctx);
    // No panic = success
}

// ============================================================================
// dependencies
// ============================================================================

#[test]
fn dependencies_returns_module_deps() {
    let handle = ModuleHandle::from_static(DepsModule);
    let deps = handle.dependencies();
    assert_eq!(deps.len(), 2);
    assert_eq!(deps[0].as_str(), "vim");
    assert_eq!(deps[1].as_str(), "editor");
}

#[test]
fn optional_dependencies_returns_module_optional_deps() {
    let handle = ModuleHandle::from_static(DepsModule);
    let deps = handle.optional_dependencies();
    assert_eq!(deps.len(), 1);
    assert_eq!(deps[0].as_str(), "lsp");
}

#[test]
fn no_dependencies_returns_empty() {
    let handle = ModuleHandle::from_static(TestModule { initialized: false });
    assert!(handle.dependencies().is_empty());
    assert!(handle.optional_dependencies().is_empty());
}

// ============================================================================
// extension_kinds
// ============================================================================

#[test]
fn extension_kinds_empty_by_default() {
    let handle = ModuleHandle::from_static(TestModule { initialized: false });
    assert!(handle.extension_kinds().is_empty());
}

// ============================================================================
// hot reload
// ============================================================================

#[test]
fn supports_hot_reload_true() {
    let handle = ModuleHandle::from_static(HotReloadModule { counter: 0 });
    assert!(handle.supports_hot_reload());
}

#[test]
fn supports_hot_reload_false() {
    let handle = ModuleHandle::from_static(TestModule { initialized: false });
    assert!(!handle.supports_hot_reload());
}

#[test]
fn save_state_returns_data() {
    let handle = ModuleHandle::from_static(HotReloadModule { counter: 12345 });
    let state = handle.save_state();
    assert!(state.is_some());
    let state = state.unwrap();
    assert_eq!(state.len(), 4);
    assert_eq!(u32::from_le_bytes([state[0], state[1], state[2], state[3]]), 12345);
}

#[test]
fn save_state_returns_none_for_non_hot_reload_module() {
    let handle = ModuleHandle::from_static(TestModule { initialized: false });
    assert!(handle.save_state().is_none());
}

#[test]
fn restore_state_success() {
    let mut handle = ModuleHandle::from_static(HotReloadModule { counter: 0 });
    let state: Box<[u8]> = 99999_u32.to_le_bytes().to_vec().into_boxed_slice();
    let result = handle.restore_state(&state);
    assert!(result.is_ok());

    // Verify by saving
    let saved = handle.save_state().unwrap();
    assert_eq!(u32::from_le_bytes([saved[0], saved[1], saved[2], saved[3]]), 99999);
}

#[test]
fn restore_state_invalid_data() {
    let mut handle = ModuleHandle::from_static(HotReloadModule { counter: 42 });
    let invalid: Box<[u8]> = vec![1, 2, 3].into_boxed_slice();
    let result = handle.restore_state(&invalid);
    assert!(result.is_err());
}

#[test]
fn full_hot_reload_cycle() {
    // 1. Module with state
    let handle = ModuleHandle::from_static(HotReloadModule { counter: 777 });

    // 2. Save state
    let saved = handle.save_state().unwrap();

    // 3. Drop old handle
    drop(handle);

    // 4. Create new handle
    let mut new_handle = ModuleHandle::from_static(HotReloadModule { counter: 0 });

    // 5. Restore state
    assert!(new_handle.restore_state(&saved).is_ok());

    // Verify
    let final_state = new_handle.save_state().unwrap();
    assert_eq!(
        u32::from_le_bytes([
            final_state[0],
            final_state[1],
            final_state[2],
            final_state[3]
        ]),
        777
    );
}

// ============================================================================
// InitResult Display
// ============================================================================

#[test]
fn init_result_debug() {
    let success = InitResult::Success;
    let defer = InitResult::Defer("waiting".into());
    assert!(format!("{success:?}").contains("Success"));
    assert!(format!("{defer:?}").contains("waiting"));
}
