//! End-to-end SP03 Phase 5 buffer-driver ABI lifecycle round-trip.
//!
//! `dlopen`s the `reovim-driver-text-buffer` cdylib produced by the
//! workspace, validates the vtable header, constructs a driver
//! instance, exercises `BufferDriver::create_buffer` and the
//! per-buffer `Buffer` trait surface (`id`, `size`, `read_bytes`, `apply_edit`,
//! list/close), and drops the loader (which calls
//! `vtable.buffer_destroy` then `vtable.destroy` through the FFI
//! boundary).
//!
//! The codec-attachment surface is intentionally not exercised here:
//! the host-side FFI adapter that pins a `Box<dyn ByteNotifiable>` for
//! the cdylib's lifetime is deferred (see the notes in
//! `buffer_instance::LoadedBufferInstance::attach_codec`). The
//! integration test below covers the byte-level lifecycle that is in
//! Phase 5 scope.

#![cfg(unix)]
#![allow(unsafe_code)]

use {
    reovim_kernel::api::v1::ByteEdit,
    reovim_subsys_buffer::BufferDriver,
    reovim_subsys_driver_loader::LoadedBuffer,
    std::{env, path::PathBuf},
};

/// Resolve the workspace target dir → `target/<profile>/lib<name>.so`.
///
/// The release dir takes precedence (release builds stage the cdylib
/// for production); falls back to debug if release is missing.
fn cdylib_path(crate_underscore_name: &str) -> Option<PathBuf> {
    let manifest = env::var("CARGO_MANIFEST_DIR").ok()?;
    let target_dir = env::var("CARGO_TARGET_DIR").unwrap_or_else(|_| {
        // crate → driver-loader → subsys → lib → server → workspace-root
        let workspace = PathBuf::from(&manifest)
            .ancestors()
            .nth(4)
            .expect("workspace root")
            .to_path_buf();
        workspace.join("target").display().to_string()
    });
    let filename = if cfg!(target_os = "windows") {
        format!("{crate_underscore_name}.dll")
    } else if cfg!(target_os = "macos") {
        format!("lib{crate_underscore_name}.dylib")
    } else {
        format!("lib{crate_underscore_name}.so")
    };
    for profile in ["release", "debug"] {
        let mut p = PathBuf::from(&target_dir);
        p.push(profile);
        p.push(&filename);
        if p.exists() {
            return Some(p);
        }
    }
    None
}

#[test]
fn lifecycle_roundtrip_create_edit_close() {
    let Some(path) = cdylib_path("reovim_driver_text_buffer") else {
        eprintln!("text-buffer cdylib not found in target/{{release,debug}}; skipping");
        return;
    };

    let driver = LoadedBuffer::load_from_path(&path).expect("load text-buffer cdylib");

    // create_buffer → returns Arc<dyn Buffer>; route through the trait.
    let buf = driver
        .create_buffer(b"hello", None)
        .expect("create_buffer ok");
    let id = buf.id();
    assert_eq!(buf.size(), 5);

    let bytes = buf.read_bytes(0..5).expect("read_bytes ok");
    assert_eq!(bytes, b"hello".to_vec());

    // apply_edit: insert " world" at offset 5 → size becomes 11.
    buf.apply_edit(ByteEdit::insert(5, b" world"))
        .expect("apply_edit ok");
    assert_eq!(buf.size(), 11);
    let bytes = buf.read_bytes(0..11).expect("read_bytes ok");
    assert_eq!(bytes, b"hello world".to_vec());

    // list_buffers contains our id.
    let listed = driver.list_buffers();
    assert!(listed.contains(&id), "expected buffer id {id:?} in list, got {listed:?}");

    // Drop the buffer wrapper before closing in the registry, so the
    // driver's `close_buffer` slot can free its own bookkeeping.
    drop(buf);

    driver.close_buffer(id).expect("close_buffer ok");

    // After close, get_buffer returns None.
    assert!(
        driver.get_buffer(id).is_none(),
        "expected get_buffer(id) to return None after close",
    );

    // Drop the driver — vtable.destroy runs through the FFI boundary.
    drop(driver);
}

#[test]
fn load_from_missing_path_reports_library_open_error() {
    use reovim_subsys_driver_loader::LoadError;
    let missing = PathBuf::from("/definitely/not/a/real/path-text-buffer.so");
    match LoadedBuffer::load_from_path(&missing) {
        Err(LoadError::LibraryOpen(_)) => (),
        Ok(_) => panic!("expected LibraryOpen error on missing path"),
        Err(other) => panic!("unexpected error kind: {other:?}"),
    }
}

#[test]
fn from_path_scan_on_missing_driver_dir_returns_empty_vec() {
    let root = tempfile::tempdir().expect("tempdir");
    let results = LoadedBuffer::from_path_scan(root.path());
    assert!(results.is_empty(), "expected empty; got {} entries", results.len());
}
