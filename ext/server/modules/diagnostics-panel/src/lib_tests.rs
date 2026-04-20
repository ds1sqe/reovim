use super::*;

#[test]
fn module_id() {
    let module = DiagnosticsPanelModule::new();
    assert_eq!(module.id(), ids::MODULE);
}

#[test]
fn module_name() {
    let module = DiagnosticsPanelModule::new();
    assert_eq!(module.name(), "Diagnostics Panel");
}

#[test]
fn module_version() {
    let module = DiagnosticsPanelModule::new();
    assert_eq!(module.version(), Version::new(0, 1, 0));
}

#[test]
fn module_default() {
    let module = DiagnosticsPanelModule;
    assert_eq!(module.id(), ids::MODULE);
}

#[test]
fn extension_kinds() {
    let module = DiagnosticsPanelModule::new();
    assert_eq!(module.extension_kinds(), &[KIND]);
}

#[test]
fn exit_succeeds() {
    let mut module = DiagnosticsPanelModule::new();
    assert!(module.exit().is_ok());
}

#[test]
fn keybindings_count() {
    let module = DiagnosticsPanelModule::new();
    let bindings = module.keybindings();
    // 1 normal mode + 12 panel mode bindings
    assert_eq!(bindings.len(), 13);
}

#[test]
fn keybindings_have_descriptions() {
    let module = DiagnosticsPanelModule::new();
    for binding in module.keybindings() {
        assert!(!binding.description.is_empty(), "Keybinding missing description");
    }
}
