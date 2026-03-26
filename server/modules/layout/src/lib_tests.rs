use super::*;

#[test]
fn module_id() {
    let m = LayoutModule::new();
    assert_eq!(m.id().as_str(), "layout");
}

#[test]
fn module_name() {
    let m = LayoutModule::new();
    assert_eq!(m.name(), "Layout");
}

#[test]
fn module_version() {
    let m = LayoutModule::new();
    assert_eq!(m.version(), Version::new(0, 1, 0));
}

#[test]
fn module_default() {
    let m = LayoutModule;
    assert_eq!(m.id().as_str(), "layout");
}

#[test]
fn exit_succeeds() {
    let mut m = LayoutModule::new();
    assert!(m.exit().is_ok());
}
