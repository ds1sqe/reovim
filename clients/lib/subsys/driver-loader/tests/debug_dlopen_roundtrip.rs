//! End-to-end Phase 0 client-debug driver-ABI round-trip test.
//!
//! `dlopen`s the `driver-debug-poc` cdylib, validates the vtable
//! header, reads the probe both with and without constructing an
//! instance, observes frames, drives an echo command, and verifies
//! the lifecycle-flags sentinel was set before the library was
//! unmapped. A second `PoC` (`driver-debug-poc-observer-panic`) proves
//! the observer trampoline's `catch_unwind` catches a panic on the
//! novel sub-vtable `next_frame` slot.
//!
//! Covers Phase 0.E acceptance of
//! `~/docs/plans/reovim/770-debug-surface/01-trait-and-macro.md`.

#![allow(unsafe_code)]

use {
    reovim_client_subsys_driver_loader::{LoadError, LoadedClientDebug},
    std::{env, path::PathBuf},
};

fn cdylib_path(crate_underscore_name: &str) -> PathBuf {
    // CARGO_MANIFEST_DIR is the driver-loader crate; workspace target
    // dir lives four levels up.
    let manifest = env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR");
    let target_dir = env::var("CARGO_TARGET_DIR").unwrap_or_else(|_| {
        let workspace = PathBuf::from(&manifest)
            .ancestors()
            .nth(4)
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

fn cstr_to_string(buf: &[u8]) -> String {
    let end = buf.iter().position(|&b| b == 0).unwrap_or(buf.len());
    String::from_utf8_lossy(&buf[..end]).into_owned()
}

#[test]
fn debug_probe_from_path_reads_metadata_without_constructing() {
    let path = cdylib_path("reovim_driver_debug_poc");
    assert!(
        path.exists(),
        "debug PoC cdylib not found at {}; run `cargo build -p reovim-driver-debug-poc` first",
        path.display()
    );

    let probe = LoadedClientDebug::probe_from_path(&path).expect("probe_from_path");
    assert_eq!(cstr_to_string(&probe.driver_name), "debug-poc");
    assert_eq!(cstr_to_string(&probe.description), "Phase 0 debug-surface PoC");
    assert_eq!(probe.observe_schemas_count, 1);
    assert_eq!(cstr_to_string(&probe.observe_schemas[0]), "poc-frames");
    assert_eq!(probe.drive_schemas_count, 1);
    assert_eq!(cstr_to_string(&probe.drive_schemas[0]), "poc-echo");
}

#[test]
fn debug_dlopen_construct_observe_drive_drop_roundtrip() {
    use libloading::{Library, Symbol};

    const FLAG_SHUTDOWN: u32 = 1 << 0;

    let path = cdylib_path("reovim_driver_debug_poc");
    assert!(path.exists(), "debug PoC cdylib not found at {}", path.display());

    // Hold a second refcount on the cdylib so we can read the driver's
    // `LIFECYCLE_FLAGS` atomic after the `LoadedClientDebug`'s Drop
    // runs (which calls shutdown + destroy and unmaps the first map).
    // SAFETY: loading the same cdylib twice is sound; dlopen refcounts.
    let sentinel = unsafe { Library::new(&path) }.expect("sentinel dlopen");

    let mut driver = LoadedClientDebug::load_from_path(&path).expect("load debug PoC cdylib");

    // Probe on the constructed instance matches the pre-construct probe.
    let probe = driver.probe();
    assert_eq!(cstr_to_string(&probe.driver_name), "debug-poc");

    // Observe: pump three frames, then end-of-stream.
    {
        let mut obs = driver.observe(b"poc-frames").expect("observe");
        let f0 = obs.next_frame().expect("frame 0").expect("Some");
        let f1 = obs.next_frame().expect("frame 1").expect("Some");
        let f2 = obs.next_frame().expect("frame 2").expect("Some");
        assert_eq!(f0, b"frame-2");
        assert_eq!(f1, b"frame-1");
        assert_eq!(f2, b"frame-0");
        assert!(obs.next_frame().expect("eos").is_none());
    }

    // Drive: echoes the command bytes.
    let response = driver.drive(b"hello").expect("drive");
    assert_eq!(response, b"hello");

    drop(driver);

    // SAFETY: the sentinel holds the cdylib mapped; the exported
    // `reovim_driver_debug_poc_lifecycle_flags` symbol is an
    // `extern "C" fn() -> u32`.
    let flags_fn: Symbol<'_, unsafe extern "C" fn() -> u32> =
        unsafe { sentinel.get(b"reovim_driver_debug_poc_lifecycle_flags") }
            .expect("find lifecycle flags accessor");
    let flags = unsafe { flags_fn() };
    assert_eq!(
        flags & FLAG_SHUTDOWN,
        FLAG_SHUTDOWN,
        "shutdown flag was not set by driver Drop; expected bit 0, got {flags:b}"
    );

    drop(sentinel);
}

#[test]
fn debug_load_from_missing_path_reports_library_open_error() {
    let missing = PathBuf::from("/definitely/not/a/real/path.so");
    match LoadedClientDebug::load_from_path(&missing) {
        Err(LoadError::LibraryOpen(_)) => (),
        Ok(_) => panic!("expected LibraryOpen error on missing path"),
        Err(other) => panic!("unexpected error kind: {other:?}"),
    }
}

#[test]
fn debug_observe_unknown_selector_reports_driver_error() {
    let path = cdylib_path("reovim_driver_debug_poc");
    assert!(path.exists(), "debug PoC cdylib not found at {}", path.display());

    let mut driver = LoadedClientDebug::load_from_path(&path).expect("load");
    match driver.observe(b"unknown-schema") {
        Err(LoadError::DriverError(msg)) => {
            assert!(
                msg.contains("unknown selector"),
                "expected driver-supplied error message to survive round trip, got: {msg}"
            );
        }
        Ok(_) => panic!("expected DriverError on unknown selector"),
        Err(other) => panic!("unexpected error kind: {other:?}"),
    }
}

#[test]
fn debug_observer_drop_without_exhausting_calls_close_slot() {
    let path = cdylib_path("reovim_driver_debug_poc");
    assert!(path.exists(), "debug PoC cdylib not found at {}", path.display());

    let mut driver = LoadedClientDebug::load_from_path(&path).expect("load");
    {
        let mut obs = driver.observe(b"poc-frames").expect("observe");
        let _first = obs.next_frame().expect("frame 0").expect("Some");
        // Drop without pumping the remaining frames — compile-level
        // assertion that `Drop for LoadedDebugObserver` is wired. If
        // the close slot weren't called, valgrind would show a leak;
        // at unit level the test passes by exiting cleanly.
    }
    // Driver still usable after observer drop.
    let resp = driver.drive(b"ping").expect("drive after observer drop");
    assert_eq!(resp, b"ping");
}

#[test]
fn debug_observer_next_frame_panic_is_caught_at_trampoline() {
    let path = cdylib_path("reovim_driver_debug_poc_observer_panic");
    assert!(
        path.exists(),
        "observer-panic PoC cdylib not found at {}; run \
         `cargo build -p reovim-driver-debug-poc-observer-panic` first",
        path.display()
    );

    let mut driver =
        LoadedClientDebug::load_from_path(&path).expect("load observer-panic PoC cdylib");
    let mut obs = driver.observe(b"panic-frames").expect("observe");

    // First call: returns a frame successfully.
    let first = obs.next_frame().expect("first frame").expect("Some");
    assert_eq!(first, b"first-frame");

    // Second call: panics inside the observer's next_frame. The
    // generated trampoline's `catch_unwind` MUST catch it and return
    // rc = -2, which we surface as `LoadError::DriverPanicked`. If
    // `catch_unwind` were missing on this slot, this call would abort
    // the test process instead of returning an error.
    match obs.next_frame() {
        Err(LoadError::DriverPanicked) => (),
        Ok(frame) => panic!("expected DriverPanicked, got frame: {frame:?}"),
        Err(other) => panic!("expected DriverPanicked, got: {other:?}"),
    }
}
