use super::*;

#[test]
#[cfg_attr(coverage_nightly, coverage(off))]
fn test_module_error_display() {
    let err = ModuleError::NotFound("test-module".into());
    assert_eq!(format!("{err}"), "module 'test-module' not found");

    let err = ModuleError::IncompatibleVersion {
        module: (2, 0),
        kernel: (1, 5),
    };
    assert!(format!("{err}").contains("2.0"));
    assert!(format!("{err}").contains("1.5"));
}

#[test]
#[cfg_attr(coverage_nightly, coverage(off))]
fn test_module_error_variants() {
    let err = ModuleError::LoadFailed("dlopen failed".into());
    assert!(format!("{err}").contains("load"));

    let err = ModuleError::NoEntryPoint("reovim_module".into());
    assert!(format!("{err}").contains("entry point"));

    let err = ModuleError::InitFailed("config missing".into());
    assert!(format!("{err}").contains("initialization failed"));

    let err = ModuleError::InUse {
        module: ModuleId::new("core"),
        by: ModuleId::new("lang-rust"),
    };
    assert!(format!("{err}").contains("in use"));

    let err = ModuleError::NotLoaded(ModuleId::new("missing"));
    assert!(format!("{err}").contains("not loaded"));
}

#[test]
fn test_probe_result_success() {
    let result = ProbeResult::Success;
    assert_eq!(result, ProbeResult::Success);
}

#[test]
#[cfg_attr(coverage_nightly, coverage(off))]
fn test_probe_result_defer() {
    let result = ProbeResult::Defer("waiting for treesitter".into());
    if let ProbeResult::Defer(reason) = &result {
        assert!(reason.contains("treesitter"));
    } else {
        panic!("expected Defer variant");
    }
}

#[test]
#[cfg_attr(coverage_nightly, coverage(off))]
fn test_probe_result_failed() {
    let err = ModuleError::InitFailed("config missing".into());
    let result = ProbeResult::Failed(err.clone());
    if let ProbeResult::Failed(e) = result {
        assert_eq!(e, err);
    } else {
        panic!("expected Failed variant");
    }
}
