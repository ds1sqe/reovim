//! Tests for `error.rs` — `RuntimeError` variant and display coverage.
//!
//! Registered under the `selftest` feature; runs on the arch no_std
//! selftest runner (arch_test! + testrt::run). The server-selftest bin in
//! tests/fixtures/ runs these.

use reovim_arch::arch_test;

use crate::error::RuntimeError;

arch_test!(runtime_error_io_is_io_variant, {
    assert!(matches!(RuntimeError::Io(42), RuntimeError::Io(_)));
});

arch_test!(runtime_error_protocol_is_protocol_variant, {
    assert!(matches!(RuntimeError::Protocol(3), RuntimeError::Protocol(_)));
});

arch_test!(runtime_error_dispatch_eq, {
    assert_eq!(RuntimeError::Dispatch, RuntimeError::Dispatch);
});

arch_test!(runtime_error_alloc_eq, {
    assert_eq!(RuntimeError::Alloc, RuntimeError::Alloc);
});

arch_test!(runtime_error_invalid_path_eq, {
    assert_eq!(RuntimeError::InvalidPath, RuntimeError::InvalidPath);
});

arch_test!(runtime_error_already_attached_eq, {
    assert_eq!(RuntimeError::AlreadyAttached, RuntimeError::AlreadyAttached);
});

arch_test!(runtime_error_distinct_variants_ne, {
    assert_ne!(RuntimeError::Dispatch, RuntimeError::Alloc);
    assert_ne!(RuntimeError::InvalidPath, RuntimeError::AlreadyAttached);
});

arch_test!(runtime_error_io_inner_distinguishes_variants, {
    let a = RuntimeError::Io(1);
    let b = RuntimeError::Io(2);
    assert_ne!(a, b);
});

// ── Display coverage (error.rs line 35-43) ────────────────────────────────────
//
// The `fmt` impl is a zero-dependency pure fn over the six variants. One test
// per variant exercises every `write!` / `write_str` arm, closing the 0% Display
// region. Format strings are pinned to the spec text so a typo in the impl is
// caught.

arch_test!(runtime_error_display_io, {
    use reovim_lib_ds::{Bytes, BytesWriter};
    // RuntimeError::Io(errno) → "io error (errno <N>)"
    let e = RuntimeError::Io(5);
    let mut buf = Bytes::new();
    let _ = core::fmt::write(&mut BytesWriter::new(&mut buf), format_args!("{e}"));
    let got = core::str::from_utf8(buf.as_slice()).unwrap_or("");
    assert_eq!(got, "io error (errno 5)", "Io Display mismatch: {got:?}");
});

arch_test!(runtime_error_display_protocol, {
    use reovim_lib_ds::{Bytes, BytesWriter};
    // RuntimeError::Protocol(code) → "protocol error (code <N>)"
    let e = RuntimeError::Protocol(7);
    let mut buf = Bytes::new();
    let _ = core::fmt::write(&mut BytesWriter::new(&mut buf), format_args!("{e}"));
    let got = core::str::from_utf8(buf.as_slice()).unwrap_or("");
    assert_eq!(got, "protocol error (code 7)", "Protocol Display mismatch: {got:?}");
});

arch_test!(runtime_error_display_dispatch, {
    use reovim_lib_ds::{Bytes, BytesWriter};
    let e = RuntimeError::Dispatch;
    let mut buf = Bytes::new();
    let _ = core::fmt::write(&mut BytesWriter::new(&mut buf), format_args!("{e}"));
    let got = core::str::from_utf8(buf.as_slice()).unwrap_or("");
    assert_eq!(got, "editor core dispatch failed", "Dispatch Display mismatch: {got:?}");
});

arch_test!(runtime_error_display_alloc, {
    use reovim_lib_ds::{Bytes, BytesWriter};
    let e = RuntimeError::Alloc;
    let mut buf = Bytes::new();
    let _ = core::fmt::write(&mut BytesWriter::new(&mut buf), format_args!("{e}"));
    let got = core::str::from_utf8(buf.as_slice()).unwrap_or("");
    assert_eq!(got, "allocation failed", "Alloc Display mismatch: {got:?}");
});

arch_test!(runtime_error_display_invalid_path, {
    use reovim_lib_ds::{Bytes, BytesWriter};
    let e = RuntimeError::InvalidPath;
    let mut buf = Bytes::new();
    let _ = core::fmt::write(&mut BytesWriter::new(&mut buf), format_args!("{e}"));
    let got = core::str::from_utf8(buf.as_slice()).unwrap_or("");
    assert_eq!(got, "listener path invalid", "InvalidPath Display mismatch: {got:?}");
});

arch_test!(runtime_error_display_already_attached, {
    use reovim_lib_ds::{Bytes, BytesWriter};
    let e = RuntimeError::AlreadyAttached;
    let mut buf = Bytes::new();
    let _ = core::fmt::write(&mut BytesWriter::new(&mut buf), format_args!("{e}"));
    let got = core::str::from_utf8(buf.as_slice()).unwrap_or("");
    assert_eq!(
        got, "second Attach on one connection (SP1)",
        "AlreadyAttached Display mismatch: {got:?}"
    );
});
