//! Tests for `mem.rs`, compiled into the lib under `selftest` + `runtime`
//! (#785 Phase 5).
//!
//! L12 layout: declared in `lib.rs` as
//! `#[cfg(all(feature = "selftest", feature = "runtime"))] pub mod mem_tests;`.
//!
//! The mem intrinsics (`memset`, `memcpy`, `memmove`, `memcmp`, `bcmp`) are
//! runtime-gated (they would clash with libc in libtest builds). These tests
//! run only inside the arch-selftest binary (features "selftest" + "runtime"),
//! which is exactly the binary the coverage measurement is taken from.
//!
//! Coverage targets:
//! - `memmove` forward branch (dest < src)
//! - `memmove` backward branch (dest >= src; overlap case)
//! - `memcmp` diff arm (returns non-zero)
//! - `memcmp` equal arm (returns 0)
//! - `bcmp` (delegates to `memcmp`)

use crate::{arch_test, testrt};

// Pull in the intrinsics from the runtime module. The `extern "C"` names are
// the canonical symbols; call through the module path so clippy does not flag
// them as unresolved.
use crate::mem::{bcmp, memcmp, memcpy, memmove, memset};

arch_test!(mem_memset_fills_all_bytes, {
    let mut buf = [0u8; 8];
    // SAFETY: `buf` is valid for 8 writes; exclusive in this thread.
    let ret = unsafe { memset(buf.as_mut_ptr(), 0xAB, buf.len()) };
    testrt::check_eq(ret, buf.as_mut_ptr());
    for b in buf {
        testrt::check_eq(b, 0xABu8);
    }
    // Ensure zero-length call is a no-op (no UB, no panic).
    let _ = unsafe { memset(buf.as_mut_ptr(), 0, 0) };
});

arch_test!(mem_memcpy_copies_disjoint_ranges, {
    let src = [1u8, 2, 3, 4, 5];
    let mut dst = [0u8; 5];
    // SAFETY: `src` valid for 5 reads, `dst` valid for 5 writes, disjoint.
    let ret = unsafe { memcpy(dst.as_mut_ptr(), src.as_ptr(), src.len()) };
    testrt::check_eq(ret, dst.as_mut_ptr());
    testrt::check(dst == src, "memcpy produced identical bytes");
});

arch_test!(mem_memmove_forward_copy_lower_dest, {
    // The forward branch requires `dest < src` to be guaranteed, not left to
    // stack layout: copy WITHIN one buffer from a higher index (src) down to a
    // lower index (dest), so `dest as usize < src as usize` holds by
    // construction and the forward loop body runs.
    let mut buf = [0u8, 0, 10, 20, 30, 40];
    //              ^dest (index 0)  ^src (index 2)
    // SAFETY: `buf` is valid for the overlapping copy; dest is strictly below
    // src, so the forward copy reads ahead of the write cursor safely.
    unsafe {
        let dst_ptr: *mut u8 = buf.as_mut_ptr();
        let src_ptr: *const u8 = buf.as_ptr().add(2);
        let ret = memmove(dst_ptr, src_ptr, 4);
        testrt::check_eq(ret, dst_ptr);
    }
    // After: buf[0..4] == original buf[2..6] == [10,20,30,40].
    testrt::check_eq(buf[0], 10u8);
    testrt::check_eq(buf[1], 20u8);
    testrt::check_eq(buf[2], 30u8);
    testrt::check_eq(buf[3], 40u8);
});

arch_test!(mem_memmove_backward_copy_overlap, {
    // dest > src: overlap case — backward branch (dest as usize >= src as usize).
    // We use a single buffer shifted right by 2 to create the overlap.
    let mut buf = [1u8, 2, 3, 4, 5, 6, 0, 0];
    //                                  ^dest (index 2) overlaps src (index 0)
    // Copy bytes [0..4] into [2..6]: overlap forces the backward-copy branch.
    // SAFETY: `buf` is valid and properly overlapping; memmove handles this.
    unsafe {
        let src_ptr: *const u8 = buf.as_ptr();
        let dst_ptr: *mut u8 = buf.as_mut_ptr().add(2);
        memmove(dst_ptr, src_ptr, 4);
    }
    // After memmove: buf[2..6] == original buf[0..4] == [1,2,3,4]
    testrt::check_eq(buf[2], 1u8);
    testrt::check_eq(buf[3], 2u8);
    testrt::check_eq(buf[4], 3u8);
    testrt::check_eq(buf[5], 4u8);
});

arch_test!(mem_memcmp_equal_returns_zero, {
    let a = [1u8, 2, 3];
    let b = [1u8, 2, 3];
    // SAFETY: both valid for 3 reads.
    let r = unsafe { memcmp(a.as_ptr(), b.as_ptr(), a.len()) };
    testrt::check_eq(r, 0i32);
});

arch_test!(mem_memcmp_diff_returns_nonzero, {
    let a = [1u8, 2, 5];
    let b = [1u8, 2, 3];
    // SAFETY: both valid for 3 reads.
    let r = unsafe { memcmp(a.as_ptr(), b.as_ptr(), a.len()) };
    testrt::check(r != 0, "different bytes produce non-zero result");
    // a[2]=5 > b[2]=3, so result should be positive.
    testrt::check(r > 0, "a > b produces positive result");
});

arch_test!(mem_bcmp_equal_is_zero_diff_is_nonzero, {
    let a = [7u8, 8];
    let b = [7u8, 8];
    let c = [7u8, 9];
    // SAFETY: all valid for 2 reads.
    testrt::check_eq(unsafe { bcmp(a.as_ptr(), b.as_ptr(), 2) }, 0i32);
    testrt::check(unsafe { bcmp(a.as_ptr(), c.as_ptr(), 2) } != 0, "bcmp diff non-zero");
});

arch_test!(mem_zero_length_memcmp_is_equal, {
    let a = [0u8; 0];
    let b = [0u8; 0];
    // SAFETY: zero-length; no bytes read.
    testrt::check_eq(unsafe { memcmp(a.as_ptr(), b.as_ptr(), 0) }, 0i32);
});
