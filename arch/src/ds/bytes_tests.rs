//! Tests for `ds/bytes.rs`, compiled into the lib under `selftest` (#785 Phase 5).
//!
//! L12 layout: declared inside `bytes.rs` as
//! `#[cfg(feature = "selftest")] #[path = "bytes_tests.rs"] mod tests;`.
//! All items under test are public so `super::` private access is not needed.

use {
    crate::{arch_test, testrt},
    core::fmt::Write,
};

use super::{Bytes, BytesWriter, Str};

arch_test!(bytes_new_is_empty, {
    let b = Bytes::new();
    testrt::check(b.is_empty(), "new bytes is empty");
    testrt::check_eq(b.len(), 0);
    testrt::check(b.as_slice().is_empty(), "slice is empty");
});

arch_test!(bytes_default_is_empty, {
    let b = Bytes::default();
    testrt::check(b.is_empty(), "default bytes is empty");
});

arch_test!(bytes_push_and_extend, {
    let mut b = Bytes::new();
    b.try_push(1).unwrap();
    b.try_extend_from_slice(&[2, 3, 4]).unwrap();
    testrt::check(b.as_slice() == &[1u8, 2, 3, 4], "bytes content matches");
    testrt::check_eq(b.len(), 4);
    testrt::check(!b.is_empty(), "bytes is not empty");
});

arch_test!(bytes_from_slice_and_deref, {
    let b = Bytes::try_from_slice(b"hello").unwrap();
    // Deref to [u8].
    testrt::check(&*b == b"hello", "deref matches slice");
    testrt::check_eq(b.first(), Some(&b'h'));
});

arch_test!(bytes_extend_empty_is_ok, {
    let mut b = Bytes::new();
    b.try_extend_from_slice(&[]).unwrap();
    testrt::check(b.is_empty(), "extending with empty leaves empty");
});

arch_test!(bytes_writer_writes_and_grows, {
    let mut b = Bytes::new();
    {
        let mut w = BytesWriter::new(&mut b);
        write!(w, "n={}", 42).unwrap();
        w.write_str(" tail").unwrap();
    }
    testrt::check(b.as_slice() == b"n=42 tail", "writer output matches");
});

arch_test!(str_from_and_push, {
    let mut s = Str::try_from_str("ab").unwrap();
    s.try_push_str("cd").unwrap();
    testrt::check(s.as_str() == "abcd", "str content matches");
    testrt::check(&*s == "abcd", "deref matches");
    testrt::check_eq(s.len(), 4);
    testrt::check(!s.is_empty(), "str is not empty");
});

arch_test!(str_new_and_default_empty, {
    let s = Str::new();
    testrt::check(s.is_empty(), "new str is empty");
    testrt::check(s.as_str().is_empty(), "as_str is empty");
    let d = Str::default();
    testrt::check(d.is_empty(), "default str is empty");
});

arch_test!(str_preserves_multibyte_utf8, {
    // A multibyte char proves the invariant survives push.
    let mut s = Str::try_from_str("é").unwrap();
    s.try_push_str("中").unwrap();
    testrt::check(s.as_str() == "é中", "multibyte content preserved");
});

arch_test!(bytes_try_from_slice_oom_propagates, {
    // `Bytes::try_from_slice` calls `try_extend_from_slice(slice)?`, which
    // calls `try_push(byte)?`, which calls `Seq::try_push` → first `alloc`.
    // Forcing that alloc to fail drives the `?` arms in both `try_from_slice`
    // and `try_extend_from_slice` (the byte loop's `try_push(byte)?`).
    crate::alloc::fault::fail_after(0);
    let r = Bytes::try_from_slice(b"hello");
    crate::alloc::fault::reset();
    testrt::check(r.is_err(), "try_from_slice surfaces AllocError under fault");
});

arch_test!(bytes_try_extend_partial_then_oom, {
    // `try_extend_from_slice`'s `try_push(byte)?` arm: seed one byte so the
    // backing `Seq` is allocated, then arm the fault so the grow needed to
    // append past capacity fails mid-loop.
    let mut b = Bytes::new();
    // Fill to the Seq's first capacity (4) so the next push grows.
    b.try_extend_from_slice(&[1, 2, 3, 4]).unwrap();
    crate::alloc::fault::fail_after(0);
    let r = b.try_extend_from_slice(&[5, 6]);
    crate::alloc::fault::reset();
    testrt::check(r.is_err(), "extend surfaces AllocError when the grow fails");
    // The bytes pushed before the failure remain (partial-append contract).
    testrt::check(b.as_slice() == &[1u8, 2, 3, 4], "prefix preserved after failed grow");
});

arch_test!(str_try_from_str_oom_propagates, {
    // `Str::try_from_str` calls `Bytes::try_from_slice(s.as_bytes())?` — its
    // own `?` arm.
    crate::alloc::fault::fail_after(0);
    let r = Str::try_from_str("abc");
    crate::alloc::fault::reset();
    testrt::check(r.is_err(), "try_from_str surfaces AllocError under fault");
});
