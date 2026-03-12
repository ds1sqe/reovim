use super::*;

#[test]
fn test_module_probe_new() {
    let probe =
        ModuleProbe::new("test-id", "Test Name", Version::new(1, 2, 3), Version::new(1, 0, 0));

    assert_eq!(probe.id_str(), "test-id");
    assert_eq!(probe.name_str(), "Test Name");
    assert_eq!(probe.version, Version::new(1, 2, 3));
    assert_eq!(probe.api_version, Version::new(1, 0, 0));
}

#[test]
fn test_module_probe_truncation() {
    // Long strings should be truncated, not overflow
    let long_id = "a".repeat(100);
    let probe =
        ModuleProbe::new(&long_id, "name", Version::new(1, 0, 0), Version::new(1, 0, 0));

    // Should be truncated to 63 chars (64 - 1 for null)
    assert_eq!(probe.id_str().len(), 63);
}

#[test]
fn test_module_probe_name_truncation() {
    // Long name should be truncated
    let long_name = "b".repeat(200);
    let probe =
        ModuleProbe::new("id", &long_name, Version::new(1, 0, 0), Version::new(1, 0, 0));

    // Should be truncated to 127 chars (128 - 1 for null)
    assert_eq!(probe.name_str().len(), 127);
}

#[test]
fn test_module_probe_is_copy() {
    let probe1 = ModuleProbe::new("test", "Test", Version::new(1, 0, 0), Version::new(1, 0, 0));
    let probe2 = probe1; // Copy
    assert_eq!(probe1.id_str(), probe2.id_str());
    assert_eq!(probe1.name_str(), probe2.name_str());
}

#[test]
fn test_module_probe_empty_strings() {
    let probe = ModuleProbe::new("", "", Version::new(0, 0, 0), Version::new(0, 0, 0));

    assert_eq!(probe.id_str(), "");
    assert_eq!(probe.name_str(), "");
}

#[test]
fn test_module_probe_const_fn() {
    // Verify ModuleProbe::new can be used in const context
    const PROBE: ModuleProbe = ModuleProbe::new(
        "const-module",
        "Const Module",
        Version::new(1, 0, 0),
        Version::new(1, 0, 0),
    );

    assert_eq!(PROBE.id_str(), "const-module");
    assert_eq!(PROBE.name_str(), "Const Module");
}

#[test]
fn test_module_probe_rustc_version() {
    let probe = ModuleProbe::new("test", "Test", Version::new(1, 0, 0), Version::new(0, 2, 0))
        .with_rustc_version("1.92.0");
    assert_eq!(probe.rustc_version_str(), "1.92.0");
}

#[test]
fn test_module_probe_rustc_version_empty() {
    let probe = ModuleProbe::new("test", "Test", Version::new(1, 0, 0), Version::new(0, 2, 0));
    assert_eq!(probe.rustc_version_str(), "");
}

#[test]
fn test_module_probe_required_deps() {
    let probe = ModuleProbe::new("test", "Test", Version::new(1, 0, 0), Version::new(0, 2, 0))
        .with_required_dep(0, "core")
        .with_required_dep(1, "treesitter");

    let deps = probe.required_deps();
    assert_eq!(deps.len(), 2);
    assert_eq!(deps[0].as_str(), "core");
    assert_eq!(deps[1].as_str(), "treesitter");
}

#[test]
fn test_module_probe_optional_deps() {
    let probe = ModuleProbe::new("test", "Test", Version::new(1, 0, 0), Version::new(0, 2, 0))
        .with_optional_dep(0, "lsp")
        .with_optional_dep(1, "completion");

    let deps = probe.optional_deps();
    assert_eq!(deps.len(), 2);
    assert_eq!(deps[0].as_str(), "lsp");
    assert_eq!(deps[1].as_str(), "completion");
}

#[test]
fn test_module_probe_max_deps() {
    let mut probe =
        ModuleProbe::new("test", "Test", Version::new(1, 0, 0), Version::new(0, 2, 0));
    for i in 0..8 {
        probe = probe.with_required_dep(i, &format!("dep{i}"));
    }

    let deps = probe.required_deps();
    assert_eq!(deps.len(), 8);
    assert_eq!(deps[7].as_str(), "dep7");
}

#[test]
fn test_module_probe_deps_overflow_ignored() {
    // Index 8 should be silently ignored
    let probe = ModuleProbe::new("test", "Test", Version::new(1, 0, 0), Version::new(0, 2, 0))
        .with_required_dep(8, "overflow");

    let deps = probe.required_deps();
    assert_eq!(deps.len(), 0);
}

#[test]
fn test_module_probe_no_deps_by_default() {
    let probe = ModuleProbe::new("test", "Test", Version::new(1, 0, 0), Version::new(0, 2, 0));
    assert!(probe.required_deps().is_empty());
    assert!(probe.optional_deps().is_empty());
}
