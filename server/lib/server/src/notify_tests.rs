//! Tests for `notify.rs` — `push_projection` and `WIRE_MAX_FRAME_BYTES`.
//!
//! Registered under the `selftest` feature; runs on the arch no_std
//! selftest runner (arch_test! + testrt::run). The server-selftest bin in
//! tests/fixtures/ runs these.
//!
//! Note: `push_projection` requires a live UDS connection; integration-level
//! coverage is provided by the carrier-smoke tests in `carrier_tests.rs`.
//! Unit-level tests here cover the constant and error-path invariants.

use reovim_arch::arch_test;

use crate::notify::WIRE_MAX_FRAME_BYTES;

arch_test!(wire_max_frame_bytes_is_4mib, {
    assert_eq!(WIRE_MAX_FRAME_BYTES, 4 * 1024 * 1024);
});

arch_test!(wire_max_frame_bytes_exceeds_u16_max, {
    // 4 MiB > u16::MAX: confirms the constant is not accidentally a small value.
    assert!(WIRE_MAX_FRAME_BYTES > u16::MAX as usize);
});

arch_test!(ensure_frame_cap_refuses_oversize_and_accepts_cap, {
    use crate::{
        error::RuntimeError,
        notify::{WIRE_MAX_FRAME_BYTES, ensure_frame_cap},
    };

    assert!(ensure_frame_cap(WIRE_MAX_FRAME_BYTES).is_ok());
    assert!(matches!(
        ensure_frame_cap(WIRE_MAX_FRAME_BYTES + 1),
        Err(RuntimeError::Protocol(_))
    ));
});
