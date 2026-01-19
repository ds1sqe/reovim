//! End-to-end tests for Python module loading.
//!
//! These tests verify that Python modules can actually be loaded and executed,
//! testing the full integration path:
//! 1. Python runtime initialization
//! 2. reovim module registration
//! 3. Python file loading and execution
//! 4. Module trait method invocation
//!
//! # Requirements
//!
//! These tests require:
//! - Python 3.11+ installed on the system
//! - The `python` feature enabled
//!
//! Run with: `cargo test -p reovim --features python python_e2e`

#![cfg(feature = "python")]

use std::{io::Write, sync::Once};

use {
    reovim_driver_ffi_python::init_python,
    reovim_kernel::api::v1::{ModuleContext, ModuleError},
    runner::module::{InitResult, ModuleLoader},
    tempfile::NamedTempFile,
};

/// Initialize Python with reovim module registered.
/// This must be called before any Python operations.
static INIT: Once = Once::new();

fn ensure_python_initialized() {
    INIT.call_once(|| {
        init_python();
    });
}

// ============================================================================
// E2E Tests: Full Python Module Loading
// ============================================================================

/// Test loading a minimal Python module that implements all required methods.
#[test]
fn test_e2e_load_minimal_python_module() {
    ensure_python_initialized();

    let mut file = NamedTempFile::with_suffix(".py").unwrap();
    writeln!(
        file,
        r#"
from reovim import Module, ModuleId, ProbeResult

class MinimalModule(Module):
    def id(self):
        return ModuleId("minimal-python-module")

    def name(self):
        return "Minimal Python Module"

    def version(self):
        return (1, 0, 0)

    def init(self, ctx):
        return ProbeResult.Success

    def exit(self):
        pass
"#
    )
    .unwrap();

    let mut loader = ModuleLoader::new();
    let result = loader.load_python(file.path());

    assert!(result.is_ok(), "Failed to load module: {:?}", result.err());
    let id = result.unwrap();
    assert_eq!(id.as_str(), "minimal-python-module");

    // Verify module is registered and accessible
    let handle = loader.get(&id);
    assert!(handle.is_some());
}

/// Test that module trait methods work correctly through FFI.
#[test]
fn test_e2e_python_module_trait_methods() {
    ensure_python_initialized();

    let mut file = NamedTempFile::with_suffix(".py").unwrap();
    writeln!(
        file,
        r#"
from reovim import Module, ModuleId, ProbeResult, Version

class FeatureModule(Module):
    def id(self):
        return ModuleId("feature-module")

    def name(self):
        return "Feature Module"

    def version(self):
        return (2, 3, 4)

    def api_version(self):
        return Version(1, 0, 0)

    def dependencies(self):
        return [ModuleId("dep-one"), ModuleId("dep-two")]

    def optional_dependencies(self):
        return [ModuleId("optional-dep")]

    def init(self, ctx):
        return ProbeResult.Success

    def exit(self):
        pass

    def supports_hot_reload(self):
        return True
"#
    )
    .unwrap();

    let mut loader = ModuleLoader::new();
    let id = loader.load_python(file.path()).unwrap();
    let handle = loader.get(&id).unwrap();

    // Test identity methods
    assert_eq!(handle.id().as_str(), "feature-module");
    assert_eq!(handle.name(), "Feature Module");

    let version = handle.version();
    assert_eq!(version.major, 2);
    assert_eq!(version.minor, 3);
    assert_eq!(version.patch, 4);

    // Test dependencies
    let deps = handle.dependencies();
    assert_eq!(deps.len(), 2);
    assert_eq!(deps[0].as_str(), "dep-one");
    assert_eq!(deps[1].as_str(), "dep-two");

    let opt_deps = handle.optional_dependencies();
    assert_eq!(opt_deps.len(), 1);
    assert_eq!(opt_deps[0].as_str(), "optional-dep");

    // Test hot reload support
    assert!(handle.supports_hot_reload());
}

/// Test the full module lifecycle: init and exit.
#[test]
fn test_e2e_python_module_lifecycle() {
    ensure_python_initialized();

    let mut file = NamedTempFile::with_suffix(".py").unwrap();
    writeln!(
        file,
        r#"
from reovim import Module, ModuleId, ProbeResult

class LifecycleModule(Module):
    def __init__(self):
        super().__init__()
        self.initialized = False
        self.exited = False

    def id(self):
        return ModuleId("lifecycle-module")

    def name(self):
        return "Lifecycle Module"

    def version(self):
        return (1, 0, 0)

    def init(self, ctx):
        self.initialized = True
        return ProbeResult.Success

    def exit(self):
        self.exited = True
"#
    )
    .unwrap();

    let mut loader = ModuleLoader::new();
    let id = loader.load_python(file.path()).unwrap();
    let handle = loader.get_mut(&id).unwrap();

    // Initialize the module
    let ctx = ModuleContext::default();
    let result = handle.init(&ctx);
    assert!(matches!(result, Ok(InitResult::Success)));

    // Exit the module
    let exit_result = handle.exit();
    assert!(exit_result.is_ok());
}

/// Test deferred initialization.
#[test]
fn test_e2e_python_module_deferred_init() {
    ensure_python_initialized();

    let mut file = NamedTempFile::with_suffix(".py").unwrap();
    writeln!(
        file,
        r#"
from reovim import Module, ModuleId, ProbeResult

class DeferredModule(Module):
    def __init__(self):
        super().__init__()
        self.init_count = 0

    def id(self):
        return ModuleId("deferred-module")

    def name(self):
        return "Deferred Module"

    def version(self):
        return (1, 0, 0)

    def init(self, ctx):
        self.init_count += 1
        if self.init_count < 3:
            return ProbeResult.Defer("waiting for dependency")
        return ProbeResult.Success

    def exit(self):
        pass
"#
    )
    .unwrap();

    let mut loader = ModuleLoader::new();
    let id = loader.load_python(file.path()).unwrap();
    let handle = loader.get_mut(&id).unwrap();

    let ctx = ModuleContext::default();

    // First call should defer
    let result1 = handle.init(&ctx);
    assert!(matches!(result1, Ok(InitResult::Defer(_))));

    // Second call should also defer
    let result2 = handle.init(&ctx);
    assert!(matches!(result2, Ok(InitResult::Defer(_))));

    // Third call should succeed
    let result3 = handle.init(&ctx);
    assert!(matches!(result3, Ok(InitResult::Success)));
}

/// Test failed initialization.
#[test]
fn test_e2e_python_module_failed_init() {
    ensure_python_initialized();

    let mut file = NamedTempFile::with_suffix(".py").unwrap();
    writeln!(
        file,
        r#"
from reovim import Module, ModuleId, ProbeResult

class FailingModule(Module):
    def id(self):
        return ModuleId("failing-module")

    def name(self):
        return "Failing Module"

    def version(self):
        return (1, 0, 0)

    def init(self, ctx):
        return ProbeResult.Failed("intentional failure for testing")

    def exit(self):
        pass
"#
    )
    .unwrap();

    let mut loader = ModuleLoader::new();
    let id = loader.load_python(file.path()).unwrap();
    let handle = loader.get_mut(&id).unwrap();

    let ctx = ModuleContext::default();
    let result = handle.init(&ctx);
    assert!(result.is_err(), "Expected init to fail: {result:?}");
}

/// Test command registrations from Python module.
#[test]
fn test_e2e_python_module_commands() {
    ensure_python_initialized();

    let mut file = NamedTempFile::with_suffix(".py").unwrap();
    writeln!(
        file,
        r#"
from reovim import Module, ModuleId, ProbeResult, CommandRegistration

class CommandModule(Module):
    def id(self):
        return ModuleId("command-module")

    def name(self):
        return "Command Module"

    def version(self):
        return (1, 0, 0)

    def init(self, ctx):
        return ProbeResult.Success

    def exit(self):
        pass

    def commands(self):
        return [
            CommandRegistration("my-command")
                .with_name("My Command")
                .with_description("Does something useful"),
            CommandRegistration("another-command")
        ]
"#
    )
    .unwrap();

    let mut loader = ModuleLoader::new();
    let id = loader.load_python(file.path()).unwrap();
    let handle = loader.get(&id).unwrap();

    // Access module trait methods via as_module()
    let module = handle.as_module().expect("Should be a static module");
    let commands = module.commands();
    assert_eq!(commands.len(), 2);
    assert_eq!(commands[0].id, "my-command");
    assert_eq!(commands[0].name, "My Command");
    assert_eq!(commands[0].description, "Does something useful");
    assert_eq!(commands[1].id, "another-command");
}

/// Test keybinding registrations from Python module.
#[test]
fn test_e2e_python_module_keybindings() {
    ensure_python_initialized();

    let mut file = NamedTempFile::with_suffix(".py").unwrap();
    writeln!(
        file,
        r#"
from reovim import Module, ModuleId, ProbeResult, KeybindingRegistration

class KeybindingModule(Module):
    def id(self):
        return ModuleId("keybinding-module")

    def name(self):
        return "Keybinding Module"

    def version(self):
        return (1, 0, 0)

    def init(self, ctx):
        return ProbeResult.Success

    def exit(self):
        pass

    def keybindings(self):
        return [
            KeybindingRegistration("<C-m>", "my-command")
                .with_modes(["normal", "visual"])
                .with_description("Trigger my command"),
            KeybindingRegistration("<leader>x", "exit-command")
        ]
"#
    )
    .unwrap();

    let mut loader = ModuleLoader::new();
    let id = loader.load_python(file.path()).unwrap();
    let handle = loader.get(&id).unwrap();

    // Access module trait methods via as_module()
    let module = handle.as_module().expect("Should be a static module");
    let keybindings = module.keybindings();
    assert_eq!(keybindings.len(), 2);
    assert_eq!(keybindings[0].keys, "<C-m>");
    assert_eq!(keybindings[0].command_id.module().as_str(), "unknown");
    assert_eq!(keybindings[0].command_id.name(), "my-command");
    assert_eq!(keybindings[0].modes, &["normal", "visual"]);
    assert_eq!(keybindings[0].description, "Trigger my command");
}

/// Test event handler registrations from Python module.
#[test]
fn test_e2e_python_module_event_handlers() {
    ensure_python_initialized();

    let mut file = NamedTempFile::with_suffix(".py").unwrap();
    writeln!(
        file,
        r#"
from reovim import Module, ModuleId, ProbeResult, EventHandlerRegistration

class EventModule(Module):
    def id(self):
        return ModuleId("event-module")

    def name(self):
        return "Event Module"

    def version(self):
        return (1, 0, 0)

    def init(self, ctx):
        return ProbeResult.Success

    def exit(self):
        pass

    def event_handlers(self):
        return [
            EventHandlerRegistration("BufferChanged")
                .with_description("React to buffer changes"),
            EventHandlerRegistration("CursorMoved")
        ]
"#
    )
    .unwrap();

    let mut loader = ModuleLoader::new();
    let id = loader.load_python(file.path()).unwrap();
    let handle = loader.get(&id).unwrap();

    // Access module trait methods via as_module()
    let module = handle.as_module().expect("Should be a static module");
    let handlers = module.event_handlers();
    assert_eq!(handlers.len(), 2);
    assert_eq!(handlers[0].event_type, "BufferChanged");
    assert_eq!(handlers[0].description, "React to buffer changes");
    assert_eq!(handlers[1].event_type, "CursorMoved");
}

/// Test hot reload state preservation.
#[test]
fn test_e2e_python_module_hot_reload() {
    ensure_python_initialized();

    let mut file = NamedTempFile::with_suffix(".py").unwrap();
    writeln!(
        file,
        r#"
import pickle
from reovim import Module, ModuleId, ProbeResult

class HotReloadModule(Module):
    def __init__(self):
        super().__init__()
        self.counter = 0

    def id(self):
        return ModuleId("hot-reload-module")

    def name(self):
        return "Hot Reload Module"

    def version(self):
        return (1, 0, 0)

    def init(self, ctx):
        self.counter += 1
        return ProbeResult.Success

    def exit(self):
        pass

    def supports_hot_reload(self):
        return True

    def save_state(self):
        return pickle.dumps({{"counter": self.counter}})

    def restore_state(self, state):
        data = pickle.loads(state)
        self.counter = data.get("counter", 0)
"#
    )
    .unwrap();

    let mut loader = ModuleLoader::new();
    let id = loader.load_python(file.path()).unwrap();
    let handle = loader.get_mut(&id).unwrap();

    // Verify hot reload is supported
    assert!(handle.supports_hot_reload());

    // Initialize
    let ctx = ModuleContext::default();
    let init_result = handle.init(&ctx);
    assert!(init_result.is_ok());

    // Save state
    let state = handle.save_state();
    assert!(state.is_some());
    let state_bytes = state.unwrap();
    assert!(!state_bytes.is_empty());

    // Restore state (simulates reload)
    let restore_result = handle.restore_state(&state_bytes);
    assert!(restore_result.is_ok());
}

// ============================================================================
// Error Handling Tests
// ============================================================================

/// Test that loading a non-existent file fails gracefully.
#[test]
fn test_e2e_python_file_not_found() {
    ensure_python_initialized();

    let mut loader = ModuleLoader::new();
    let result = loader.load_python(std::path::Path::new("/nonexistent/module.py"));

    assert!(matches!(result, Err(ModuleError::LoadFailed(_))));
}

/// Test that loading a file without Module subclass fails.
#[test]
fn test_e2e_python_no_module_subclass() {
    ensure_python_initialized();

    let mut file = NamedTempFile::with_suffix(".py").unwrap();
    writeln!(
        file,
        r"
# A Python file without a Module subclass
def some_function():
    return 42

class NotAModule:
    pass
"
    )
    .unwrap();

    let mut loader = ModuleLoader::new();
    let result = loader.load_python(file.path());

    assert!(
        matches!(result, Err(ModuleError::LoadFailed(msg)) if msg.contains("no Module subclass"))
    );
}

/// Test that syntax errors are reported correctly.
#[test]
fn test_e2e_python_syntax_error() {
    ensure_python_initialized();

    let mut file = NamedTempFile::with_suffix(".py").unwrap();
    writeln!(file, "def invalid syntax {{}}").unwrap();

    let mut loader = ModuleLoader::new();
    let result = loader.load_python(file.path());

    assert!(matches!(result, Err(ModuleError::LoadFailed(_))));
}

/// Test that duplicate module registration fails.
#[test]
fn test_e2e_python_duplicate_registration() {
    ensure_python_initialized();

    let mut file = NamedTempFile::with_suffix(".py").unwrap();
    writeln!(
        file,
        r#"
from reovim import Module, ModuleId, ProbeResult

class DuplicateModule(Module):
    def id(self):
        return ModuleId("duplicate-test-module")

    def name(self):
        return "Duplicate Test"

    def version(self):
        return (1, 0, 0)

    def init(self, ctx):
        return ProbeResult.Success

    def exit(self):
        pass
"#
    )
    .unwrap();

    let mut loader = ModuleLoader::new();

    // First load should succeed
    let result1 = loader.load_python(file.path());
    assert!(result1.is_ok());

    // Second load should fail (duplicate ID)
    let result2 = loader.load_python(file.path());
    assert!(matches!(result2, Err(ModuleError::LoadFailed(msg)) if msg.contains("already loaded")));
}
