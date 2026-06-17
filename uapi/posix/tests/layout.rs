//! Behavioral + layout tests for `uapi/posix`.
//!
//! These assert:
//! - the exact canonical Linux/x86-64 values of the `O_*` constants (the AB3
//!   data precondition the kabi/platform slot re-type relies on);
//! - byte-identity of each newtype with its underlying int via `size_of` /
//!   `align_of` (the runtime mirror of the compile-time `const` assertions in
//!   `lib.rs`). Together with `#[repr(transparent)]` on each newtype, this is
//!   the full byte-identity guarantee — no `transmute` is needed (the
//!   workspace forbids `unsafe`).
//!
//! The newtypes' byte-identity is the precondition for re-typing the effectful
//! `kabi/platform` slots to these types without an ABI change. If an assert
//! here fails, the layout has drifted — fix the type, not the test.

use reovim_uapi_posix::{
    Errno, Fd, Mode, O_CLOEXEC, O_CREAT, O_RDONLY, O_TRUNC, O_WRONLY, OpenFlags,
};

// ---- O_* canonical values ----------------------------------------------------

#[test]
fn open_flag_values_are_canonical() {
    assert_eq!(O_RDONLY.bits(), 0);
    assert_eq!(O_WRONLY.bits(), 0o1);
    assert_eq!(O_CREAT.bits(), 0o100);
    assert_eq!(O_TRUNC.bits(), 0o1000);
    assert_eq!(O_CLOEXEC.bits(), 0o2_000_000);
}

#[test]
fn open_flag_bitor_composes() {
    let combined = O_WRONLY | O_CREAT | O_CLOEXEC;
    assert_eq!(combined.bits(), 0o1 | 0o100 | 0o2_000_000);
}

// ---- byte-identity (size + align) --------------------------------------------

#[test]
fn newtypes_match_underlying_int_layout() {
    assert_eq!(size_of::<OpenFlags>(), size_of::<i32>());
    assert_eq!(align_of::<OpenFlags>(), align_of::<i32>());

    assert_eq!(size_of::<Mode>(), size_of::<u32>());
    assert_eq!(align_of::<Mode>(), align_of::<u32>());

    assert_eq!(size_of::<Fd>(), size_of::<i32>());
    assert_eq!(align_of::<Fd>(), align_of::<i32>());

    assert_eq!(size_of::<Errno>(), size_of::<i32>());
    assert_eq!(align_of::<Errno>(), align_of::<i32>());
}

// ---- constructor / accessor round-trips (safe) -------------------------------

#[test]
fn accessors_round_trip_their_constructors() {
    assert_eq!(OpenFlags(0o2_000_101).bits(), 0o2_000_101);
    assert_eq!(Mode(0o755).bits(), 0o755);
    assert_eq!(Fd(7).as_i32(), 7);
    assert_eq!(Errno(9).code(), 9); // EBADF
}
