//! Freestanding memory intrinsics (`memset`/`memcpy`/`memmove`/`memcmp`/
//! `bcmp`).
//!
//! A `#![no_std] #![no_main]` binary links no libc, but the compiler still
//! lowers core operations (slice fills, `core::fmt` buffer writes, struct
//! copies) to calls on these five C symbols. Someone must define them; under
//! DAG6 (1.2 §10) the platform floor does. Gated on the `runtime` feature:
//! in bootstrap-state-1 libtest builds libc already provides the symbols,
//! and a second strong definition would be a duplicate-symbol link error.
//!
//! ## Why volatile byte loops
//!
//! LLVM's loop-idiom pass recognizes a plain byte-fill/byte-copy loop and
//! rewrites it into a call to `memset`/`memcpy` — inside the very function
//! defining that symbol, producing infinite recursion. Volatile accesses are
//! exempt from idiom recognition, so each function below is immune by
//! construction. Byte-at-a-time volatile is the slow-but-right floor; a
//! `rep stosb`/`rep movsb` fast path is added when a real consumer profiles
//! a need (rule of three), not speculatively.

/// Fills `n` bytes at `dest` with the byte value `c`. Returns `dest`.
///
/// # Safety
///
/// `dest` must be valid for `n` writes. The caller is the compiler's
/// lowering machinery (or equivalent), which guarantees exclusive access to
/// the range for the duration of the call.
///
/// ```ignore
/// // memset is called by the compiler's code-generation machinery; it requires
/// // the runtime feature (gated against the std libtest harness's own libc copy).
/// // Direct invocation from user code is not the intended interface.
/// ```
#[unsafe(no_mangle)]
pub unsafe extern "C" fn memset(dest: *mut u8, c: i32, n: usize) -> *mut u8 {
    // The C contract passes the fill byte as an `int`; only the low 8 bits
    // are the value.
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    let byte = c as u8;
    let mut i = 0;
    while i < n {
        // SAFETY: the caller guarantees `dest..dest+n` is valid for writes;
        // `i < n` keeps the access in bounds. Volatile defeats loop-idiom
        // recognition (module doc).
        unsafe { dest.add(i).write_volatile(byte) };
        i += 1;
    }
    dest
}

/// Copies `n` bytes from `src` to `dest`; the ranges must not overlap.
/// Returns `dest`.
///
/// # Safety
///
/// `src` must be valid for `n` reads, `dest` for `n` writes, and the ranges
/// must be disjoint (the C `memcpy` contract).
///
/// ```ignore
/// // memcpy is a compiler intrinsic — runtime feature required; not called directly.
/// ```
#[unsafe(no_mangle)]
pub unsafe extern "C" fn memcpy(dest: *mut u8, src: *const u8, n: usize) -> *mut u8 {
    let mut i = 0;
    while i < n {
        // SAFETY: the caller guarantees both ranges are valid for `n`
        // accesses and disjoint; `i < n` keeps both in bounds. Volatile
        // defeats loop-idiom recognition (module doc).
        unsafe { dest.add(i).write_volatile(src.add(i).read_volatile()) };
        i += 1;
    }
    dest
}

/// Copies `n` bytes from `src` to `dest`, handling overlap. Returns `dest`.
///
/// # Safety
///
/// `src` must be valid for `n` reads and `dest` for `n` writes (the C
/// `memmove` contract; overlap is permitted).
///
/// ```ignore
/// // memmove is a compiler intrinsic — runtime feature required; not called directly.
/// ```
#[unsafe(no_mangle)]
pub unsafe extern "C" fn memmove(dest: *mut u8, src: *const u8, n: usize) -> *mut u8 {
    if (dest as usize) < (src as usize) {
        // Forward copy: dest is below src, so reading ahead of the write
        // cursor never reads an already-overwritten byte.
        let mut i = 0;
        while i < n {
            // SAFETY: caller guarantees validity for `n` accesses; `i < n`
            // bounds both. Volatile defeats loop-idiom recognition.
            unsafe { dest.add(i).write_volatile(src.add(i).read_volatile()) };
            i += 1;
        }
    } else {
        // Backward copy: dest is at/above src, so copying from the end
        // never reads an already-overwritten byte.
        let mut i = n;
        while i > 0 {
            i -= 1;
            // SAFETY: caller guarantees validity for `n` accesses; `i < n`
            // bounds both. Volatile defeats loop-idiom recognition.
            unsafe { dest.add(i).write_volatile(src.add(i).read_volatile()) };
        }
    }
    dest
}

/// Lexicographically compares `n` bytes: `<0`, `0`, `>0` as `a` is below,
/// equal to, or above `b` at the first differing byte.
///
/// # Safety
///
/// `a` and `b` must each be valid for `n` reads.
///
/// ```ignore
/// // memcmp is a compiler intrinsic — runtime feature required; not called directly.
/// ```
#[unsafe(no_mangle)]
pub unsafe extern "C" fn memcmp(a: *const u8, b: *const u8, n: usize) -> i32 {
    let mut i = 0;
    while i < n {
        // SAFETY: caller guarantees both ranges are valid for `n` reads;
        // `i < n` bounds both. Volatile defeats idiom recognition.
        let (x, y) = unsafe { (a.add(i).read_volatile(), b.add(i).read_volatile()) };
        if x != y {
            return i32::from(x) - i32::from(y);
        }
        i += 1;
    }
    0
}

/// Equality-only comparison: `0` iff the `n`-byte ranges are equal (the
/// LLVM `bcmp` lowering of pure equality checks; the sign of a non-zero
/// result is unspecified).
///
/// # Safety
///
/// `a` and `b` must each be valid for `n` reads.
///
/// ```ignore
/// // bcmp is a compiler intrinsic — runtime feature required; not called directly.
/// ```
#[unsafe(no_mangle)]
pub unsafe extern "C" fn bcmp(a: *const u8, b: *const u8, n: usize) -> i32 {
    // SAFETY: forwarded verbatim; `memcmp`'s contract is identical.
    unsafe { memcmp(a, b, n) }
}
