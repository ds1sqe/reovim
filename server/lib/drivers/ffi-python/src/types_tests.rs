use super::*;

// ModuleId tests
#[test]
fn test_py_module_id_creation() {
    let id = PyModuleId::new("test-module");
    assert_eq!(id.as_str(), "test-module");
}

#[test]
fn test_py_module_id_repr() {
    let id = PyModuleId::new("my-module");
    assert_eq!(id.__repr__(), "ModuleId('my-module')");
    assert_eq!(id.__str__(), "my-module");
}

#[test]
fn test_py_module_id_equality() {
    let id1 = PyModuleId::new("test");
    let id2 = PyModuleId::new("test");
    let id3 = PyModuleId::new("other");
    assert!(id1.__eq__(&id2));
    assert!(!id1.__eq__(&id3));
}

#[test]
fn test_py_module_id_hash() {
    let id1 = PyModuleId::new("test");
    let id2 = PyModuleId::new("test");
    assert_eq!(id1.__hash__(), id2.__hash__());
}

#[test]
fn test_py_module_id_conversion() {
    let py_id = PyModuleId::new("test");
    let kernel_id: v1::ModuleId = py_id.clone().into();
    let back: PyModuleId = kernel_id.into();
    assert_eq!(py_id.as_str(), back.as_str());
}

// Version tests
#[test]
fn test_py_version_creation() {
    let ver = PyVersion::new(1, 2, 3);
    assert_eq!(ver.major(), 1);
    assert_eq!(ver.minor(), 2);
    assert_eq!(ver.patch(), 3);
}

#[test]
fn test_py_version_repr() {
    let ver = PyVersion::new(1, 2, 3);
    assert_eq!(ver.__repr__(), "Version(1, 2, 3)");
    assert_eq!(ver.__str__(), "1.2.3");
}

#[test]
fn test_py_version_equality() {
    let v1 = PyVersion::new(1, 0, 0);
    let v2 = PyVersion::new(1, 0, 0);
    let v3 = PyVersion::new(2, 0, 0);
    assert!(v1.__eq__(&v2));
    assert!(!v1.__eq__(&v3));
}

#[test]
fn test_py_version_ordering() {
    let v1 = PyVersion::new(1, 0, 0);
    let v2 = PyVersion::new(1, 1, 0);
    let v3 = PyVersion::new(2, 0, 0);

    assert!(v1.__lt__(&v2));
    assert!(v1.__le__(&v2));
    assert!(v2.__lt__(&v3));
    assert!(v3.__gt__(&v1));
    assert!(v3.__ge__(&v1));
}

#[test]
fn test_py_version_conversion() {
    let py_ver = PyVersion::new(1, 2, 3);
    let kernel_ver: v1::Version = py_ver.into();
    let back: PyVersion = kernel_ver.into();
    assert_eq!(py_ver.major(), back.major());
    assert_eq!(py_ver.minor(), back.minor());
    assert_eq!(py_ver.patch(), back.patch());
}

// ProbeResult tests
#[test]
fn test_py_probe_result_success() {
    let result = PyProbeResult::success();
    assert!(result.is_success());
    assert!(!result.is_defer());
    assert!(!result.is_failed());
    assert!(result.defer_reason().is_none());
    assert!(result.failure_reason().is_none());
}

#[test]
fn test_py_probe_result_defer() {
    let result = PyProbeResult::defer("waiting for dependency");
    assert!(!result.is_success());
    assert!(result.is_defer());
    assert!(!result.is_failed());
    assert_eq!(result.defer_reason(), Some("waiting for dependency".to_string()));
    assert!(result.failure_reason().is_none());
}

#[test]
fn test_py_probe_result_failed() {
    let result = PyProbeResult::failed("config error");
    assert!(!result.is_success());
    assert!(!result.is_defer());
    assert!(result.is_failed());
    assert!(result.defer_reason().is_none());
    assert_eq!(result.failure_reason(), Some("config error".to_string()));
}

#[test]
fn test_py_probe_result_repr() {
    assert_eq!(PyProbeResult::success().__repr__(), "ProbeResult.Success");
    assert_eq!(PyProbeResult::defer("reason").__repr__(), "ProbeResult.Defer('reason')");
    assert_eq!(PyProbeResult::failed("error").__repr__(), "ProbeResult.Failed('error')");
}

#[test]
fn test_py_probe_result_str() {
    assert_eq!(PyProbeResult::success().__str__(), "Success");
    assert_eq!(PyProbeResult::defer("reason").__str__(), "Defer: reason");
    assert_eq!(PyProbeResult::failed("error").__str__(), "Failed: error");
}

#[test]
fn test_py_probe_result_equality() {
    assert!(PyProbeResult::success().__eq__(&PyProbeResult::success()));
    assert!(PyProbeResult::defer("a").__eq__(&PyProbeResult::defer("a")));
    assert!(PyProbeResult::failed("a").__eq__(&PyProbeResult::failed("a")));
    assert!(!PyProbeResult::success().__eq__(&PyProbeResult::defer("a")));
    assert!(!PyProbeResult::defer("a").__eq__(&PyProbeResult::defer("b")));
}

#[test]
fn test_py_probe_result_to_kernel() {
    let success = PyProbeResult::success();
    assert!(matches!(success.to_kernel(), v1::ProbeResult::Success));

    let defer = PyProbeResult::defer("reason");
    if let v1::ProbeResult::Defer(reason) = defer.to_kernel() {
        assert_eq!(reason, "reason");
    } else {
        panic!("expected Defer");
    }

    let failed = PyProbeResult::failed("error");
    if let v1::ProbeResult::Failed(err) = failed.to_kernel() {
        assert!(err.to_string().contains("error"));
    } else {
        panic!("expected Failed");
    }
}

#[test]
fn test_py_probe_result_from_kernel() {
    let success = PyProbeResult::from_kernel(&v1::ProbeResult::Success);
    assert!(success.is_success());

    let defer = PyProbeResult::from_kernel(&v1::ProbeResult::Defer("reason".to_string()));
    assert!(defer.is_defer());
    assert_eq!(defer.defer_reason(), Some("reason".to_string()));

    let failed = PyProbeResult::from_kernel(&v1::ProbeResult::Failed(
        v1::ModuleError::InitFailed("error".to_string()),
    ));
    assert!(failed.is_failed());
}

// ========================================================================
// CommandRegistration tests
// ========================================================================

#[test]
fn test_py_command_registration_creation() {
    let reg = PyCommandRegistration::new("my-command");
    assert_eq!(reg.id, "my-command");
    assert!(reg.name.is_empty());
    assert!(reg.description.is_empty());
    assert!(reg.category.is_none());
    assert!(!reg.accepts_count);
    assert!(!reg.accepts_motion);
    assert!(!reg.is_jump);
    assert!(!reg.is_text_modifying);
}

#[test]
fn test_py_command_registration_builder() {
    let reg = PyCommandRegistration::new("delete")
        .with_name("Delete")
        .with_description("Delete text")
        .with_category("edit")
        .with_count()
        .with_motion()
        .with_jump()
        .with_text_modifying();

    assert_eq!(reg.id, "delete");
    assert_eq!(reg.name, "Delete");
    assert_eq!(reg.description, "Delete text");
    assert_eq!(reg.category, Some("edit".to_string()));
    assert!(reg.accepts_count);
    assert!(reg.accepts_motion);
    assert!(reg.is_jump);
    assert!(reg.is_text_modifying);
}

#[test]
fn test_py_command_registration_repr() {
    let reg = PyCommandRegistration::new("test-cmd");
    assert_eq!(reg.__repr__(), "CommandRegistration('test-cmd')");
}

#[test]
fn test_py_command_registration_to_kernel() {
    let reg = PyCommandRegistration::new("yank")
        .with_name("Yank")
        .with_description("Copy text")
        .with_category("edit")
        .with_count()
        .with_motion();

    let kernel_reg = reg.to_kernel();
    assert_eq!(kernel_reg.id, "yank");
    assert_eq!(kernel_reg.name, "Yank");
    assert_eq!(kernel_reg.description, "Copy text");
    assert_eq!(kernel_reg.category, Some("edit"));
    assert!(kernel_reg.accepts_count());
    assert!(kernel_reg.accepts_motion());
    assert!(!kernel_reg.is_jump());
    assert!(!kernel_reg.is_text_modifying());
}

// ========================================================================
// KeybindingRegistration tests
// ========================================================================

#[test]
fn test_py_keybinding_registration_creation() {
    let reg = PyKeybindingRegistration::new("dd", "delete-line");
    assert_eq!(reg.keys, "dd");
    assert_eq!(reg.command_id, "delete-line");
    assert!(reg.modes.is_empty());
    assert!(reg.description.is_empty());
    assert!(reg.category.is_none());
    assert!(reg.enabled);
    assert_eq!(reg.priority, 100);
}

#[test]
fn test_py_keybinding_registration_builder() {
    let reg = PyKeybindingRegistration::new("<C-w>h", "window-left")
        .with_modes(vec!["normal".to_string()])
        .with_description("Move to left window")
        .with_category("window")
        .with_priority(50)
        .with_disabled();

    assert_eq!(reg.keys, "<C-w>h");
    assert_eq!(reg.command_id, "window-left");
    assert_eq!(reg.modes, vec!["normal"]);
    assert_eq!(reg.description, "Move to left window");
    assert_eq!(reg.category, Some("window".to_string()));
    assert!(!reg.enabled);
    assert_eq!(reg.priority, 50);
}

#[test]
fn test_py_keybinding_registration_repr() {
    let reg = PyKeybindingRegistration::new("j", "move-down");
    assert_eq!(reg.__repr__(), "KeybindingRegistration('j', 'move-down')");
}

#[test]
fn test_py_keybinding_registration_to_kernel() {
    let reg = PyKeybindingRegistration::new("dd", "editor:delete-line")
        .with_modes(vec!["normal".to_string(), "visual".to_string()])
        .with_description("Delete entire line")
        .with_priority(10);

    let kernel_reg = reg.to_kernel();
    assert_eq!(kernel_reg.keys, "dd");
    assert_eq!(kernel_reg.command_id.module().as_str(), "editor");
    assert_eq!(kernel_reg.command_id.name(), "delete-line");
    assert_eq!(kernel_reg.modes.len(), 2);
    assert_eq!(kernel_reg.modes[0], "normal");
    assert_eq!(kernel_reg.modes[1], "visual");
    assert_eq!(kernel_reg.description, "Delete entire line");
    assert!(kernel_reg.enabled);
    assert_eq!(kernel_reg.priority, 10);
}

// ========================================================================
// EventHandlerRegistration tests
// ========================================================================

#[test]
fn test_py_event_handler_registration_creation() {
    let reg = PyEventHandlerRegistration::new("BufferChanged");
    assert_eq!(reg.event_type, "BufferChanged");
    assert_eq!(reg.priority, 100);
    assert!(reg.description.is_empty());
    assert!(!reg.once);
    assert!(reg.target_component.is_none());
}

#[test]
fn test_py_event_handler_registration_builder() {
    let reg = PyEventHandlerRegistration::new("CursorMoved")
        .with_priority(50)
        .with_description("Track cursor movement")
        .with_once()
        .with_target("editor");

    assert_eq!(reg.event_type, "CursorMoved");
    assert_eq!(reg.priority, 50);
    assert_eq!(reg.description, "Track cursor movement");
    assert!(reg.once);
    assert_eq!(reg.target_component, Some("editor".to_string()));
}

#[test]
fn test_py_event_handler_registration_repr() {
    let reg = PyEventHandlerRegistration::new("ModeChanged");
    assert_eq!(reg.__repr__(), "EventHandlerRegistration('ModeChanged')");
}

#[test]
fn test_py_event_handler_registration_to_kernel() {
    let reg = PyEventHandlerRegistration::new("BufferWrite")
        .with_priority(25)
        .with_description("Auto-save handler")
        .with_once()
        .with_target("buffer");

    let kernel_reg = reg.to_kernel();
    assert_eq!(kernel_reg.event_type, "BufferWrite");
    assert_eq!(kernel_reg.priority, 25);
    assert_eq!(kernel_reg.description, "Auto-save handler");
    assert!(kernel_reg.once);
    assert_eq!(kernel_reg.target_component, Some("buffer"));
}
