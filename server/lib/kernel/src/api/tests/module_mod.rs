use super::*;

use crate::api::{context::ModuleContext, version::API_VERSION};
use crate::mm::BufferId;

/// Verify Module trait is object-safe (can be used as `dyn Module`).
/// This is critical for dynamic module loading via libloading.
#[test]
#[cfg_attr(coverage_nightly, coverage(off))]
fn test_module_trait_object_safety() {
    // These function signatures compile = trait is object safe
    fn _accepts_module_ref(_: &dyn Module) {}
    fn _accepts_boxed_module(_: Box<dyn Module>) {}

    // Trait is object safe if this compiles
}

#[test]
#[cfg_attr(coverage_nightly, coverage(off))]
fn test_module_deferred_probing() {
    // Test module that uses deferred probing pattern
    struct DeferredModule {
        ready: bool,
    }

    impl Module for DeferredModule {
        fn id(&self) -> ModuleId {
            ModuleId::new("deferred")
        }
        fn name(&self) -> &'static str {
            "Deferred Module"
        }
        fn version(&self) -> Version {
            Version::new(1, 0, 0)
        }
        fn init(&mut self, _ctx: &ModuleContext) -> ProbeResult {
            if self.ready {
                ProbeResult::Success
            } else {
                ProbeResult::Defer("waiting for dependency".into())
            }
        }
        fn exit(&mut self) -> Result<(), ModuleError> {
            Ok(())
        }
    }

    // Verify module implements the trait correctly with both states
    let ready_module = DeferredModule { ready: true };
    assert_eq!(ready_module.id(), ModuleId::new("deferred"));
    assert_eq!(ready_module.name(), "Deferred Module");

    let not_ready = DeferredModule { ready: false };
    assert_eq!(not_ready.id(), ModuleId::new("deferred"));
    // Both ready and not-ready modules have the same identity
}

// ========== Module default impls ==========

#[test]
#[cfg_attr(coverage_nightly, coverage(off))]
fn test_module_default_impls() {
    struct MinimalModule;

    impl Module for MinimalModule {
        fn id(&self) -> ModuleId {
            ModuleId::new("minimal")
        }
        fn name(&self) -> &'static str {
            "Minimal"
        }
        fn version(&self) -> Version {
            Version::new(0, 1, 0)
        }
        fn init(&mut self, _ctx: &ModuleContext) -> ProbeResult {
            ProbeResult::Success
        }
        fn exit(&mut self) -> Result<(), ModuleError> {
            Ok(())
        }
    }

    let mut m = MinimalModule;

    // Test all default impls
    assert!(m.dependencies().is_empty());
    assert!(m.optional_dependencies().is_empty());
    assert!(m.commands().is_empty());
    assert!(m.keybindings().is_empty());
    assert!(m.event_handlers().is_empty());
    assert!(!m.supports_hot_reload());
    assert!(m.save_state().is_none());

    // restore_state default returns error
    let result = m.restore_state(&[1, 2, 3]);
    assert!(result.is_err());

    // on_unload default returns Ok
    assert!(m.on_unload().is_ok());

    // api_version returns current API version
    assert_eq!(m.api_version(), API_VERSION);
}

// ========== Lifecycle hooks via trait object ==========

#[test]
#[cfg_attr(coverage_nightly, coverage(off))]
fn test_module_lifecycle_hooks_via_dyn() {
    use std::sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    };

    struct HookModule {
        all_loaded: Arc<AtomicBool>,
        buffer_focus: Arc<AtomicBool>,
        on_unload: Arc<AtomicBool>,
    }

    impl Module for HookModule {
        fn id(&self) -> ModuleId {
            ModuleId::new("hook-test")
        }
        fn name(&self) -> &'static str {
            "Hook Test"
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
        fn on_all_loaded(&mut self, _ctx: &ModuleContext) {
            self.all_loaded.store(true, Ordering::SeqCst);
        }
        fn on_buffer_focus(&mut self, _buffer_id: BufferId, _ctx: &ModuleContext) {
            self.buffer_focus.store(true, Ordering::SeqCst);
        }
        fn on_unload(&mut self) -> Result<(), ModuleError> {
            self.on_unload.store(true, Ordering::SeqCst);
            Ok(())
        }
    }

    let all_loaded = Arc::new(AtomicBool::new(false));
    let buffer_focus = Arc::new(AtomicBool::new(false));
    let unload = Arc::new(AtomicBool::new(false));

    let mut module: Box<dyn Module> = Box::new(HookModule {
        all_loaded: Arc::clone(&all_loaded),
        buffer_focus: Arc::clone(&buffer_focus),
        on_unload: Arc::clone(&unload),
    });

    // Call lifecycle hooks through trait object
    let ctx = ModuleContext::default();
    module.on_all_loaded(&ctx);
    assert!(all_loaded.load(Ordering::SeqCst));

    module.on_buffer_focus(BufferId::from_raw(1), &ctx);
    assert!(buffer_focus.load(Ordering::SeqCst));

    assert!(module.on_unload().is_ok());
    assert!(unload.load(Ordering::SeqCst));
}

#[test]
#[cfg_attr(coverage_nightly, coverage(off))]
fn test_module_default_lifecycle_hooks_via_dyn() {
    struct DefaultModule;

    impl Module for DefaultModule {
        fn id(&self) -> ModuleId {
            ModuleId::new("default-hooks")
        }
        fn name(&self) -> &'static str {
            "Default Hooks"
        }
        fn version(&self) -> Version {
            Version::new(0, 1, 0)
        }
        fn init(&mut self, _ctx: &ModuleContext) -> ProbeResult {
            ProbeResult::Success
        }
        fn exit(&mut self) -> Result<(), ModuleError> {
            Ok(())
        }
    }

    let mut module: Box<dyn Module> = Box::new(DefaultModule);
    let ctx = ModuleContext::default();

    // Default on_all_loaded is no-op (should not panic)
    module.on_all_loaded(&ctx);

    // Default on_buffer_focus is no-op (should not panic)
    module.on_buffer_focus(BufferId::from_raw(42), &ctx);

    // Default on_unload returns Ok
    assert!(module.on_unload().is_ok());

    // Test all registration methods via dyn
    assert!(module.commands().is_empty());
    assert!(module.keybindings().is_empty());
    assert!(module.event_handlers().is_empty());
    assert!(module.dependencies().is_empty());
    assert!(module.optional_dependencies().is_empty());
    assert!(!module.supports_hot_reload());
    assert!(module.save_state().is_none());
    assert!(module.restore_state(&[1, 2]).is_err());
}

#[test]
#[cfg_attr(coverage_nightly, coverage(off))]
fn test_module_init_exit_via_dyn() {
    struct InitExitModule {
        initialized: bool,
    }

    impl Module for InitExitModule {
        fn id(&self) -> ModuleId {
            ModuleId::new("init-exit")
        }
        fn name(&self) -> &'static str {
            "Init Exit"
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

    let mut module: Box<dyn Module> = Box::new(InitExitModule { initialized: false });
    let ctx = ModuleContext::default();

    // Test init
    let result = module.init(&ctx);
    assert!(matches!(result, ProbeResult::Success));

    // Test exit
    assert!(module.exit().is_ok());

    // Test identity via dyn
    assert_eq!(module.id(), ModuleId::new("init-exit"));
    assert_eq!(module.name(), "Init Exit");
    assert_eq!(module.version(), Version::new(1, 0, 0));
    assert_eq!(module.api_version(), API_VERSION);
}

#[test]
#[cfg_attr(coverage_nightly, coverage(off))]
fn test_module_hot_reload_via_dyn() {
    struct HotReloadModule {
        state: u32,
    }

    impl Module for HotReloadModule {
        fn id(&self) -> ModuleId {
            ModuleId::new("hot-reload")
        }
        fn name(&self) -> &'static str {
            "Hot Reload"
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
            let bytes = self.state.to_le_bytes();
            Some(bytes.to_vec().into_boxed_slice())
        }
        fn restore_state(&mut self, state: &[u8]) -> Result<(), ModuleError> {
            if state.len() < 4 {
                return Err(ModuleError::InitFailed("state too short".into()));
            }
            let bytes: [u8; 4] = state[..4]
                .try_into()
                .map_err(|_| ModuleError::InitFailed("invalid state".into()))?;
            self.state = u32::from_le_bytes(bytes);
            Ok(())
        }
    }

    let mut module: Box<dyn Module> = Box::new(HotReloadModule { state: 42 });

    assert!(module.supports_hot_reload());

    // Save state via dyn
    let saved = module.save_state().unwrap();
    assert_eq!(saved.len(), 4);

    // Restore state via dyn
    assert!(module.restore_state(&saved).is_ok());

    // Restore with bad state
    assert!(module.restore_state(&[1]).is_err());
}

#[test]
#[cfg_attr(coverage_nightly, coverage(off))]
fn test_module_with_dependencies_via_dyn() {
    struct DepModule;

    impl Module for DepModule {
        fn id(&self) -> ModuleId {
            ModuleId::new("dep-module")
        }
        fn name(&self) -> &'static str {
            "Dep Module"
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
        fn dependencies(&self) -> Vec<ModuleId> {
            vec![ModuleId::new("core-module")]
        }
        fn optional_dependencies(&self) -> Vec<ModuleId> {
            vec![ModuleId::new("feat-lsp")]
        }
    }

    let mut module: Box<dyn Module> = Box::new(DepModule);
    let deps = module.dependencies();
    assert_eq!(deps.len(), 1);
    assert_eq!(deps[0], ModuleId::new("core-module"));

    let opt_deps = module.optional_dependencies();
    assert_eq!(opt_deps.len(), 1);
    assert_eq!(opt_deps[0], ModuleId::new("feat-lsp"));

    // Exercise init/exit on DepModule
    let ctx = ModuleContext::default();
    let result = module.init(&ctx);
    assert!(matches!(result, ProbeResult::Success));
    assert!(module.exit().is_ok());
}

// ========================================================================
// Additional coverage tests
// ========================================================================

#[test]
#[cfg_attr(coverage_nightly, coverage(off))]
fn test_deferred_module_init_both_paths() {
    struct DeferredModule {
        ready: bool,
    }

    impl Module for DeferredModule {
        fn id(&self) -> ModuleId {
            ModuleId::new("deferred")
        }
        fn name(&self) -> &'static str {
            "Deferred Module"
        }
        fn version(&self) -> Version {
            Version::new(1, 0, 0)
        }
        fn init(&mut self, _ctx: &ModuleContext) -> ProbeResult {
            if self.ready {
                ProbeResult::Success
            } else {
                ProbeResult::Defer("waiting for dependency".into())
            }
        }
        fn exit(&mut self) -> Result<(), ModuleError> {
            Ok(())
        }
    }

    let ctx = ModuleContext::default();

    // Not ready path - should defer
    let mut not_ready = DeferredModule { ready: false };
    assert_eq!(not_ready.id(), ModuleId::new("deferred"));
    assert_eq!(not_ready.name(), "Deferred Module");
    assert_eq!(not_ready.version(), Version::new(1, 0, 0));
    let result = not_ready.init(&ctx);
    assert!(matches!(result, ProbeResult::Defer(_)));
    assert!(not_ready.exit().is_ok());

    // Ready path - should succeed
    let mut ready = DeferredModule { ready: true };
    let result = ready.init(&ctx);
    assert!(matches!(result, ProbeResult::Success));
    assert!(ready.exit().is_ok());
}

#[test]
#[cfg_attr(coverage_nightly, coverage(off))]
fn test_minimal_module_full_lifecycle() {
    struct MinimalModule;

    impl Module for MinimalModule {
        fn id(&self) -> ModuleId {
            ModuleId::new("minimal")
        }
        fn name(&self) -> &'static str {
            "Minimal"
        }
        fn version(&self) -> Version {
            Version::new(0, 1, 0)
        }
        fn init(&mut self, _ctx: &ModuleContext) -> ProbeResult {
            ProbeResult::Success
        }
        fn exit(&mut self) -> Result<(), ModuleError> {
            Ok(())
        }
    }

    let mut m = MinimalModule;
    let ctx = ModuleContext::default();

    // Exercise identity methods
    assert_eq!(m.id(), ModuleId::new("minimal"));
    assert_eq!(m.name(), "Minimal");
    assert_eq!(m.version(), Version::new(0, 1, 0));
    assert_eq!(m.api_version(), API_VERSION);

    // Exercise lifecycle
    let result = m.init(&ctx);
    assert!(matches!(result, ProbeResult::Success));

    m.on_all_loaded(&ctx);
    m.on_buffer_focus(BufferId::from_raw(1), &ctx);

    assert!(m.on_unload().is_ok());
    assert!(m.exit().is_ok());

    // Exercise registration defaults
    assert!(m.dependencies().is_empty());
    assert!(m.optional_dependencies().is_empty());
    assert!(m.commands().is_empty());
    assert!(m.keybindings().is_empty());
    assert!(m.event_handlers().is_empty());

    // Exercise hot reload defaults
    assert!(!m.supports_hot_reload());
    assert!(m.save_state().is_none());
    assert!(m.restore_state(&[1, 2, 3]).is_err());
}

#[test]
#[cfg_attr(coverage_nightly, coverage(off))]
fn test_hook_module_init_exit_exercised() {
    use std::sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    };

    struct HookModule {
        all_loaded: Arc<AtomicBool>,
        buffer_focus: Arc<AtomicBool>,
        on_unload: Arc<AtomicBool>,
    }

    impl Module for HookModule {
        fn id(&self) -> ModuleId {
            ModuleId::new("hook-test")
        }
        fn name(&self) -> &'static str {
            "Hook Test"
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
        fn on_all_loaded(&mut self, _ctx: &ModuleContext) {
            self.all_loaded.store(true, Ordering::SeqCst);
        }
        fn on_buffer_focus(&mut self, _buffer_id: BufferId, _ctx: &ModuleContext) {
            self.buffer_focus.store(true, Ordering::SeqCst);
        }
        fn on_unload(&mut self) -> Result<(), ModuleError> {
            self.on_unload.store(true, Ordering::SeqCst);
            Ok(())
        }
    }

    let all_loaded = Arc::new(AtomicBool::new(false));
    let buffer_focus = Arc::new(AtomicBool::new(false));
    let unload = Arc::new(AtomicBool::new(false));

    let mut module: Box<dyn Module> = Box::new(HookModule {
        all_loaded: Arc::clone(&all_loaded),
        buffer_focus: Arc::clone(&buffer_focus),
        on_unload: Arc::clone(&unload),
    });

    // Exercise identity
    assert_eq!(module.id(), ModuleId::new("hook-test"));
    assert_eq!(module.name(), "Hook Test");
    assert_eq!(module.version(), Version::new(1, 0, 0));

    // Exercise init/exit
    let ctx = ModuleContext::default();
    let result = module.init(&ctx);
    assert!(matches!(result, ProbeResult::Success));

    module.on_all_loaded(&ctx);
    assert!(all_loaded.load(Ordering::SeqCst));

    module.on_buffer_focus(BufferId::from_raw(1), &ctx);
    assert!(buffer_focus.load(Ordering::SeqCst));

    assert!(module.on_unload().is_ok());
    assert!(unload.load(Ordering::SeqCst));

    assert!(module.exit().is_ok());
}

#[test]
#[cfg_attr(coverage_nightly, coverage(off))]
fn test_default_module_full_exercise() {
    struct DefaultModule;

    impl Module for DefaultModule {
        fn id(&self) -> ModuleId {
            ModuleId::new("default-hooks")
        }
        fn name(&self) -> &'static str {
            "Default Hooks"
        }
        fn version(&self) -> Version {
            Version::new(0, 1, 0)
        }
        fn init(&mut self, _ctx: &ModuleContext) -> ProbeResult {
            ProbeResult::Success
        }
        fn exit(&mut self) -> Result<(), ModuleError> {
            Ok(())
        }
    }

    let mut module: Box<dyn Module> = Box::new(DefaultModule);
    let ctx = ModuleContext::default();

    // Identity
    assert_eq!(module.id(), ModuleId::new("default-hooks"));
    assert_eq!(module.name(), "Default Hooks");
    assert_eq!(module.version(), Version::new(0, 1, 0));
    assert_eq!(module.api_version(), API_VERSION);

    // Full lifecycle
    let result = module.init(&ctx);
    assert!(matches!(result, ProbeResult::Success));

    module.on_all_loaded(&ctx);
    module.on_buffer_focus(BufferId::from_raw(42), &ctx);
    assert!(module.on_unload().is_ok());
    assert!(module.exit().is_ok());

    // Registration defaults
    assert!(module.commands().is_empty());
    assert!(module.keybindings().is_empty());
    assert!(module.event_handlers().is_empty());
    assert!(module.dependencies().is_empty());
    assert!(module.optional_dependencies().is_empty());

    // Hot reload defaults
    assert!(!module.supports_hot_reload());
    assert!(module.save_state().is_none());
    assert!(module.restore_state(&[1, 2]).is_err());
}

#[test]
#[cfg_attr(coverage_nightly, coverage(off))]
fn test_hot_reload_module_full_exercise() {
    struct HotReloadModule {
        state: u32,
    }

    impl Module for HotReloadModule {
        fn id(&self) -> ModuleId {
            ModuleId::new("hot-reload")
        }
        fn name(&self) -> &'static str {
            "Hot Reload"
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
            let bytes = self.state.to_le_bytes();
            Some(bytes.to_vec().into_boxed_slice())
        }
        fn restore_state(&mut self, state: &[u8]) -> Result<(), ModuleError> {
            if state.len() < 4 {
                return Err(ModuleError::InitFailed("state too short".into()));
            }
            let bytes: [u8; 4] = state[..4]
                .try_into()
                .map_err(|_| ModuleError::InitFailed("invalid state".into()))?;
            self.state = u32::from_le_bytes(bytes);
            Ok(())
        }
    }

    let mut module: Box<dyn Module> = Box::new(HotReloadModule { state: 42 });
    let ctx = ModuleContext::default();

    // Identity
    assert_eq!(module.id(), ModuleId::new("hot-reload"));
    assert_eq!(module.name(), "Hot Reload");
    assert_eq!(module.version(), Version::new(1, 0, 0));

    // Lifecycle
    let result = module.init(&ctx);
    assert!(matches!(result, ProbeResult::Success));

    // Hot reload
    assert!(module.supports_hot_reload());
    let saved = module.save_state().unwrap();
    assert_eq!(saved.len(), 4);
    assert!(module.restore_state(&saved).is_ok());

    // Restore with too-short state
    let short_result = module.restore_state(&[1]);
    assert!(short_result.is_err());

    // Restore with empty state
    let empty_result = module.restore_state(&[]);
    assert!(empty_result.is_err());

    // Restore with exactly 4 bytes
    assert!(module.restore_state(&[0, 0, 0, 0]).is_ok());

    assert!(module.exit().is_ok());
}

#[test]
#[cfg_attr(coverage_nightly, coverage(off))]
fn test_module_failed_init() {
    struct FailingModule;

    impl Module for FailingModule {
        fn id(&self) -> ModuleId {
            ModuleId::new("failing")
        }
        fn name(&self) -> &'static str {
            "Failing Module"
        }
        fn version(&self) -> Version {
            Version::new(0, 1, 0)
        }
        fn init(&mut self, _ctx: &ModuleContext) -> ProbeResult {
            ProbeResult::Failed(ModuleError::InitFailed("config missing".into()))
        }
        fn exit(&mut self) -> Result<(), ModuleError> {
            Err(ModuleError::InitFailed("never initialized".into()))
        }
    }

    let mut module = FailingModule;
    let ctx = ModuleContext::default();

    assert_eq!(module.id(), ModuleId::new("failing"));
    assert_eq!(module.name(), "Failing Module");
    assert_eq!(module.version(), Version::new(0, 1, 0));

    let result = module.init(&ctx);
    assert!(matches!(result, ProbeResult::Failed(_)));

    let exit_result = module.exit();
    assert!(exit_result.is_err());
}

#[test]
#[cfg_attr(coverage_nightly, coverage(off))]
fn test_module_on_unload_error() {
    struct UnloadFailModule;

    impl Module for UnloadFailModule {
        fn id(&self) -> ModuleId {
            ModuleId::new("unload-fail")
        }
        fn name(&self) -> &'static str {
            "Unload Fail"
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
        fn on_unload(&mut self) -> Result<(), ModuleError> {
            Err(ModuleError::InitFailed("resource leak".into()))
        }
    }

    let mut module = UnloadFailModule;
    let ctx = ModuleContext::default();

    assert_eq!(module.id(), ModuleId::new("unload-fail"));
    assert_eq!(module.name(), "Unload Fail");
    assert_eq!(module.version(), Version::new(1, 0, 0));

    let result = module.init(&ctx);
    assert!(matches!(result, ProbeResult::Success));

    let unload_result = module.on_unload();
    assert!(unload_result.is_err());

    assert!(module.exit().is_ok());
}

#[test]
#[cfg_attr(coverage_nightly, coverage(off))]
fn test_module_with_custom_api_version() {
    struct CustomApiModule;

    impl Module for CustomApiModule {
        fn id(&self) -> ModuleId {
            ModuleId::new("custom-api")
        }
        fn name(&self) -> &'static str {
            "Custom API"
        }
        fn version(&self) -> Version {
            Version::new(1, 0, 0)
        }
        fn api_version(&self) -> Version {
            Version::new(0, 1, 0)
        }
        fn init(&mut self, _ctx: &ModuleContext) -> ProbeResult {
            ProbeResult::Success
        }
        fn exit(&mut self) -> Result<(), ModuleError> {
            Ok(())
        }
    }

    let mut module = CustomApiModule;
    let ctx = ModuleContext::default();

    assert_eq!(module.api_version(), Version::new(0, 1, 0));
    assert_ne!(module.api_version(), API_VERSION);

    let result = module.init(&ctx);
    assert!(matches!(result, ProbeResult::Success));
    assert!(module.exit().is_ok());
}

#[test]
#[cfg_attr(coverage_nightly, coverage(off))]
fn test_module_with_registrations() {
    struct RegistrationModule;

    impl Module for RegistrationModule {
        fn id(&self) -> ModuleId {
            ModuleId::new("reg-module")
        }
        fn name(&self) -> &'static str {
            "Registration Module"
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
        fn commands(&self) -> Vec<CommandRegistration> {
            vec![CommandRegistration::new("test-cmd").with_name("Test Command")]
        }
        fn keybindings(&self) -> Vec<KeybindingRegistration> {
            let cmd_id = CommandId::new(ModuleId::new("reg-module"), "test-cmd");
            vec![KeybindingRegistration::new("t", cmd_id).with_modes(&["normal"])]
        }
        fn event_handlers(&self) -> Vec<EventHandlerRegistration> {
            vec![EventHandlerRegistration::new("BufferChanged").with_priority(50)]
        }
    }

    let mut module: Box<dyn Module> = Box::new(RegistrationModule);
    let ctx = ModuleContext::default();

    // Identity
    assert_eq!(module.id(), ModuleId::new("reg-module"));
    assert_eq!(module.name(), "Registration Module");
    assert_eq!(module.version(), Version::new(1, 0, 0));

    // Lifecycle
    let result = module.init(&ctx);
    assert!(matches!(result, ProbeResult::Success));

    // Registrations
    let cmds = module.commands();
    assert_eq!(cmds.len(), 1);
    assert_eq!(cmds[0].id, "test-cmd");
    assert_eq!(cmds[0].name, "Test Command");

    let keys = module.keybindings();
    assert_eq!(keys.len(), 1);
    assert_eq!(keys[0].keys, "t");

    let events = module.event_handlers();
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].event_type, "BufferChanged");
    assert_eq!(events[0].priority, 50);

    assert!(module.exit().is_ok());
}

#[test]
#[cfg_attr(coverage_nightly, coverage(off))]
fn test_module_restore_state_default_error_message() {
    struct NoHotReload;

    impl Module for NoHotReload {
        fn id(&self) -> ModuleId {
            ModuleId::new("no-hot-reload")
        }
        fn name(&self) -> &'static str {
            "No Hot Reload"
        }
        fn version(&self) -> Version {
            Version::new(0, 1, 0)
        }
        fn init(&mut self, _ctx: &ModuleContext) -> ProbeResult {
            ProbeResult::Success
        }
        fn exit(&mut self) -> Result<(), ModuleError> {
            Ok(())
        }
    }

    let mut module = NoHotReload;
    let result = module.restore_state(&[1, 2, 3, 4]);
    assert!(result.is_err());
    if let Err(ModuleError::InitFailed(msg)) = result {
        assert!(msg.contains("hot reload not supported"));
    } else {
        panic!("expected InitFailed error");
    }
}

#[test]
#[cfg_attr(coverage_nightly, coverage(off))]
fn test_module_multiple_buffer_focus_calls() {
    struct CountingModule {
        focus_count: u32,
    }

    impl Module for CountingModule {
        fn id(&self) -> ModuleId {
            ModuleId::new("counting")
        }
        fn name(&self) -> &'static str {
            "Counting"
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
        fn on_buffer_focus(&mut self, _buffer_id: BufferId, _ctx: &ModuleContext) {
            self.focus_count += 1;
        }
    }

    let mut module = CountingModule { focus_count: 0 };
    let ctx = ModuleContext::default();

    let result = module.init(&ctx);
    assert!(matches!(result, ProbeResult::Success));

    module.on_buffer_focus(BufferId::from_raw(1), &ctx);
    module.on_buffer_focus(BufferId::from_raw(2), &ctx);
    module.on_buffer_focus(BufferId::from_raw(3), &ctx);
    assert_eq!(module.focus_count, 3);

    assert!(module.exit().is_ok());
}
