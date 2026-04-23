//! End-to-end Phase 0 driver-ABI round-trip test.
//!
//! `dlopen`s the `PoC` cdylib built by the `reovim-driver-abi-poc`
//! sibling crate, validates the vtable header, constructs a driver
//! instance, borrows a render target, submits a payload, and drops.
//! Covers acceptance criteria 8-10 of `01-abi-spec-and-poc.md`.

#![allow(unsafe_code)]

use {
    reovim_client_subsys_driver_loader::LoadedClientRender,
    reovim_client_subsys_render::target::RenderTarget,
    std::{env, path::PathBuf},
};

fn cdylib_path(crate_underscore_name: &str) -> PathBuf {
    // CARGO_MANIFEST_DIR is the driver-loader crate; workspace target
    // dir lives four levels up.
    let manifest = env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR");
    let target_dir = env::var("CARGO_TARGET_DIR").unwrap_or_else(|_| {
        let workspace = PathBuf::from(&manifest)
            .ancestors()
            .nth(4) // crate → subsys → lib → clients → workspace-root
            .expect("workspace root")
            .to_path_buf();
        workspace.join("target").display().to_string()
    });
    let mut p = PathBuf::from(target_dir);
    p.push("debug");
    let filename = if cfg!(target_os = "windows") {
        format!("{crate_underscore_name}.dll")
    } else if cfg!(target_os = "macos") {
        format!("lib{crate_underscore_name}.dylib")
    } else {
        format!("lib{crate_underscore_name}.so")
    };
    p.push(filename);
    p
}

#[test]
fn dlopen_construct_submit_drop_roundtrip() {
    use libloading::{Library, Symbol};

    const FLAG_SHUTDOWN: u32 = 1 << 0;

    let path = cdylib_path("reovim_driver_abi_poc");
    assert!(
        path.exists(),
        "`PoC` cdylib not found at {}; run `cargo build -p reovim-driver-abi-poc` first",
        path.display()
    );

    // Hold a second refcount on the cdylib so we can read the driver's
    // `LIFECYCLE_FLAGS` atomic after the `LoadedClientRender`'s Drop
    // runs (which calls shutdown + destroy and unmaps the first map).
    // SAFETY: loading the same cdylib twice is sound; dlopen
    // refcounts the mapping.
    let sentinel = unsafe { Library::new(&path) }.expect("sentinel dlopen");

    let mut driver = LoadedClientRender::load_from_path(&path).expect("load `PoC` cdylib");
    {
        let mut target = driver.target().expect("borrow target");
        target.submit(b"frame one").expect("submit frame 1");
        target.submit(b"frame two").expect("submit frame 2");
    }
    drop(driver);

    // SAFETY: the sentinel holds the cdylib mapped; the exported
    // `reovim_driver_abi_poc_lifecycle_flags` symbol is an
    // `extern "C" fn() -> u32`.
    let flags_fn: Symbol<'_, unsafe extern "C" fn() -> u32> = unsafe {
        sentinel.get(b"reovim_driver_abi_poc_lifecycle_flags")
    }
    .expect("find lifecycle flags accessor");
    let flags = unsafe { flags_fn() };
    assert_eq!(
        flags & FLAG_SHUTDOWN,
        FLAG_SHUTDOWN,
        "shutdown flag was not set by driver Drop; expected bit 0 of \
         lifecycle flags to be set, got {flags:b}"
    );

    drop(sentinel);
}

#[test]
fn panicking_driver_is_caught_at_trampoline_boundary() {
    use reovim_client_subsys_render::target::RenderError;

    let path = cdylib_path("reovim_driver_abi_poc_panic");
    assert!(
        path.exists(),
        "panic `PoC` cdylib not found at {}; run `cargo build -p reovim-driver-abi-poc-panic` first",
        path.display()
    );

    let mut driver =
        LoadedClientRender::load_from_path(&path).expect("load panic `PoC` cdylib");
    let mut target = driver.target().expect("borrow target");
    let err = target.submit(b"triggers panic").unwrap_err();
    // Trampoline caught the panic, returned -2, host surfaced it as an
    // `InvalidData` with the "panicked" marker. If `catch_unwind` was
    // missing, this call would abort the test process instead.
    match err {
        RenderError::InvalidData(ref m) => assert!(
            m.contains("panicked"),
            "expected panic-marked error message, got: {m}"
        ),
        RenderError::NotReady => panic!("expected InvalidData error, got NotReady"),
    }
}

#[test]
fn load_from_missing_path_reports_library_open_error() {
    use reovim_client_subsys_driver_loader::LoadError;
    let missing = PathBuf::from("/definitely/not/a/real/path.so");
    match LoadedClientRender::load_from_path(&missing) {
        Err(LoadError::LibraryOpen(_)) => (),
        Ok(_) => panic!("expected LibraryOpen error on missing path"),
        Err(other) => panic!("unexpected error kind: {other:?}"),
    }
}

#[test]
fn load_from_cdylib_whose_construct_errors_reports_driver_error() {
    use reovim_client_subsys_driver_loader::LoadError;
    let path = cdylib_path("reovim_driver_abi_poc_construct_err");
    assert!(
        path.exists(),
        "construct-err `PoC` cdylib not found at {}; run \
         `cargo build -p reovim-driver-abi-poc-construct-err` first",
        path.display()
    );
    match LoadedClientRender::load_from_path(&path) {
        Err(LoadError::DriverError(msg)) => {
            assert!(
                msg.contains("construct failure"),
                "expected driver-supplied error message to survive the round trip, got: {msg}"
            );
        }
        Ok(_) => panic!("expected DriverError on construct-failing cdylib"),
        Err(other) => panic!("unexpected error kind: {other:?}"),
    }
}

#[test]
fn load_from_cdylib_without_vtable_symbol_reports_library_open_error() {
    use reovim_client_subsys_driver_loader::LoadError;
    let path = cdylib_path("reovim_driver_abi_poc_empty");
    assert!(
        path.exists(),
        "empty `PoC` cdylib not found at {}; run \
         `cargo build -p reovim-driver-abi-poc-empty` first",
        path.display()
    );
    match LoadedClientRender::load_from_path(&path) {
        Err(LoadError::LibraryOpen(_)) => (),
        Ok(_) => panic!("expected LibraryOpen error on cdylib missing vtable symbol"),
        Err(other) => panic!("unexpected error kind: {other:?}"),
    }
}

#[test]
fn from_path_scan_stages_all_four_poc_cdylibs_and_surfaces_per_entry_results() {
    use {
        reovim_client_subsys_driver_loader::ScanEntryError,
        std::fs,
    };

    // Stage all four Phase 0 PoC cdylibs into a synthetic driver root.
    let root = tempfile::tempdir().unwrap();
    let driver_dir = root.path().join("driver");
    fs::create_dir_all(&driver_dir).unwrap();

    for stem in [
        "reovim_driver_abi_poc",
        "reovim_driver_abi_poc_panic",
        "reovim_driver_abi_poc_construct_err",
        "reovim_driver_abi_poc_empty",
    ] {
        let src = cdylib_path(stem);
        assert!(src.exists(), "missing PoC at {}", src.display());
        let dest = driver_dir.join(src.file_name().unwrap());
        fs::copy(&src, &dest).unwrap_or_else(|e| panic!("copy {stem}: {e}"));
    }

    let results = LoadedClientRender::from_path_scan(root.path());
    assert_eq!(results.len(), 4, "expected 4 entries, got {}", results.len());

    let successes = results.iter().filter(|r| r.is_ok()).count();
    let errors: Vec<&ScanEntryError> = results.iter().filter_map(|r| r.as_ref().err()).collect();

    // Happy-path and panic PoCs both construct cleanly (panic PoC
    // only panics inside submit, which from_path_scan does not call).
    // construct-err surfaces as DriverError, empty cdylib as
    // AbiMismatch (missing vtable symbol).
    assert_eq!(successes, 2, "expected 2 successes (happy + panic), got {successes}");
    assert_eq!(errors.len(), 2, "expected 2 per-entry errors");

    let variant_names: Vec<&'static str> = errors
        .iter()
        .map(|e| match e {
            ScanEntryError::Loader(_) => "Loader",
            ScanEntryError::AbiMismatch(_) => "AbiMismatch",
            ScanEntryError::DriverError(_) => "DriverError",
            ScanEntryError::DriverPanicked => "DriverPanicked",
        })
        .collect();
    assert!(variant_names.contains(&"DriverError"), "missing DriverError: {variant_names:?}");
    assert!(variant_names.contains(&"AbiMismatch"), "missing AbiMismatch: {variant_names:?}");
}

#[test]
fn from_path_scan_on_missing_driver_dir_returns_empty_vec() {
    let root = tempfile::tempdir().unwrap();
    // root/driver/ does not exist — scan layer warns and returns empty.
    let results = LoadedClientRender::from_path_scan(root.path());
    assert!(results.is_empty(), "expected empty; got {} entries", results.len());
}
