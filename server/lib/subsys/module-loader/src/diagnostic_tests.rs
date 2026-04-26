//! Tests for [`super::LoadDiagnostic`].
//!
//! `LoadDiagnostic` is a thin `thiserror`-derived wrapper around
//! [`ModuleError`]. The interesting behaviour — package-name
//! enrichment via [`reovim_pkg_runtime_loader::enrich_validation_error`]
//! — is exercised by
//! [`crate::loader::loader_tests::from_path_scan_filtered_diag`*]
//! integration tests in Phase 4.F. The unit tests below pin only the
//! `Display` and `From` shapes so a future variant reorder doesn't
//! silently regress the user-facing diagnostic.

use reovim_kernel::api::v1::ModuleError;

use super::LoadDiagnostic;

#[test]
fn bare_variant_is_constructed_via_from() {
    let err = ModuleError::NotFound("foo".into());
    let diag: LoadDiagnostic = err.into();
    match diag {
        LoadDiagnostic::Bare(ModuleError::NotFound(name)) => assert_eq!(name, "foo"),
        other => panic!("expected Bare(NotFound), got {other:?}"),
    }
}

#[test]
fn at_package_display_carries_package_name() {
    let diag = LoadDiagnostic::AbiMismatchAtPackage {
        package: "vim-snippet".to_owned(),
        source: ModuleError::IncompatibleVersion {
            module: (2, 0),
            kernel: (3, 0),
        },
    };
    let text = diag.to_string();
    assert!(text.contains("vim-snippet"), "missing package: {text}");
    assert!(text.contains("ABI mismatch"), "missing prefix: {text}");
}

#[test]
fn bare_display_transparent_through_module_error() {
    let inner = ModuleError::LoadFailed("disk read".into());
    let diag = LoadDiagnostic::Bare(inner);
    assert_eq!(diag.to_string(), ModuleError::LoadFailed("disk read".into()).to_string());
}
