//! `Display` and round-trip coverage for the loader error variants.
//!
//! Mirrors the client-side coverage: every variant of [`LoadError`],
//! [`ValidationError`], and [`ScanEntryError`] gets a Display test plus
//! a `From`/transparent-conversion check where applicable. Keeps the
//! 100% line-coverage target on the error module without depending on
//! the rest of the crate.

use {super::*, reovim_dylib_loader::ScanEntryError as DylibScanError, std::path::PathBuf};

#[test]
fn load_error_library_open_displays_payload() {
    let e = LoadError::LibraryOpen("dlopen refused".to_owned());
    assert_eq!(format!("{e}"), "failed to open driver cdylib: dlopen refused");
}

#[test]
fn load_error_validation_round_trips_inner_display() {
    let v = ValidationError::VtablePointerNull;
    let inner_display = format!("{v}");
    let e = LoadError::from(v);
    assert!(matches!(e, LoadError::Validation(_)));
    // The `#[error(transparent)]` attribute forwards Display to inner.
    assert_eq!(format!("{e}"), inner_display);
}

#[test]
fn load_error_driver_error_displays_payload() {
    let e = LoadError::DriverError("construct failed".to_owned());
    assert_eq!(format!("{e}"), "driver returned error: construct failed");
}

#[test]
fn load_error_driver_panicked_displays_constant_message() {
    let e = LoadError::DriverPanicked;
    assert_eq!(format!("{e}"), "driver panicked at FFI boundary");
}

#[test]
fn validation_error_vtable_pointer_null_displays() {
    let e = ValidationError::VtablePointerNull;
    assert_eq!(format!("{e}"), "vtable pointer is null");
}

#[test]
fn validation_error_abi_version_mismatch_displays() {
    let e = ValidationError::AbiVersionMismatch {
        found: 2,
        expected: 1,
    };
    assert_eq!(format!("{e}"), "ABI version mismatch: driver=2, host=1");
}

#[test]
fn validation_error_api_version_incompatible_displays() {
    let e = ValidationError::ApiVersionIncompatible {
        found_major: 2,
        found_minor: 0,
        found_patch: 1,
        expected_major: 1,
        expected_minor: 3,
    };
    assert_eq!(format!("{e}"), "API version incompatible: driver=2.0.1, host requires 1.3+");
}

#[test]
fn validation_error_size_of_self_mismatch_displays() {
    let e = ValidationError::SizeOfSelfMismatch {
        found: 64,
        expected: 72,
    };
    assert_eq!(format!("{e}"), "size_of_self mismatch: driver=64, host=72");
}

#[test]
fn scan_entry_error_loader_round_trips_dylib_error() {
    let inner = DylibScanError::DlopenFailed {
        path: PathBuf::from("/broken.so"),
        source_text: "missing dependency".to_owned(),
    };
    let inner_display = format!("{inner}");
    let e = ScanEntryError::from(inner);
    assert!(matches!(e, ScanEntryError::Loader(_)));
    assert_eq!(format!("{e}"), inner_display);
}

#[test]
fn scan_entry_error_abi_mismatch_round_trips_validation() {
    let v = ValidationError::SizeOfSelfMismatch {
        found: 0,
        expected: 8,
    };
    let inner_display = format!("{v}");
    let e = ScanEntryError::from(v);
    assert!(matches!(e, ScanEntryError::AbiMismatch(_)));
    assert_eq!(format!("{e}"), inner_display);
}

#[test]
fn scan_entry_error_driver_error_displays_payload() {
    let e = ScanEntryError::DriverError("ctor failed".to_owned());
    assert_eq!(format!("{e}"), "driver returned error: ctor failed");
}

#[test]
fn scan_entry_error_driver_panicked_displays_constant_message() {
    let e = ScanEntryError::DriverPanicked;
    assert_eq!(format!("{e}"), "driver panicked at FFI boundary");
}
