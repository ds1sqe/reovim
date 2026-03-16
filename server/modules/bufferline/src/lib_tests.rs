use super::*;

#[test]
fn module_id() {
    let module = BufferlineModule::new();
    assert_eq!(module.id(), ids::MODULE);
}

#[test]
fn module_name() {
    let module = BufferlineModule::new();
    assert_eq!(module.name(), "Bufferline");
}

#[test]
fn module_version() {
    let module = BufferlineModule::new();
    assert_eq!(module.version(), Version::new(0, 1, 0));
}

#[test]
fn module_default() {
    let module = BufferlineModule::default();
    assert_eq!(module.id(), ids::MODULE);
}

#[test]
fn extension_kinds() {
    let module = BufferlineModule::new();
    assert_eq!(module.extension_kinds(), &[KIND]);
}

#[test]
fn exit_succeeds() {
    let mut module = BufferlineModule::new();
    assert!(module.exit().is_ok());
}

#[test]
fn keybindings_count() {
    let module = BufferlineModule::new();
    let bindings = module.keybindings();
    assert_eq!(bindings.len(), 3);
}

#[test]
fn keybindings_have_descriptions() {
    let module = BufferlineModule::new();
    for binding in module.keybindings() {
        assert!(!binding.description.is_empty(), "Keybinding missing description");
    }
}
