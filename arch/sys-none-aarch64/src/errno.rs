//! Errno newtype and the kernel's negative-return mapping.
//!
//! This is target-neutral floor vocabulary: every backend's wrappers return
//! `Result<_, Errno>`, and freestanding backends report the same constants
//! directly. The values are drawn from the Linux errno space so error
//! meaning is identical across targets.
//!
//! [`from_ret`] is the Linux-family translation branch: the Linux syscall
//! convention encodes errors as a small negative return value — a raw
//! result in `-4095..=-1` is `-errno`; anything else is a successful
//! result.

/// A Linux errno value (always positive).
///
/// ```rust
/// use reovim_arch_sys_none_aarch64::{Errno, EBADF, from_ret};
/// let e = from_ret(-9).unwrap_err();
/// assert_eq!(e, EBADF);
/// assert_eq!(e.code(), 9);
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Errno(pub i32);

impl Errno {
    /// The positive errno code.
    ///
    /// ```rust
    /// use reovim_arch_sys_none_aarch64::EINVAL;
    /// assert_eq!(EINVAL.code(), 22);
    /// ```
    #[must_use]
    pub const fn code(self) -> i32 {
        self.0
    }
}

/// Operation not permitted on a bad file descriptor.
///
/// See [`from_ret`] for usage.
pub const EBADF: Errno = Errno(9);
/// No such file or directory.
///
/// See [`from_ret`] for usage.
pub const ENOENT: Errno = Errno(2);
/// Invalid argument.
///
/// See [`from_ret`] for usage.
pub const EINVAL: Errno = Errno(22);
/// Resource temporarily unavailable. On Linux `EWOULDBLOCK == EAGAIN`.
///
/// See [`from_ret`] for usage.
pub const EAGAIN: Errno = Errno(11);
/// Alias for [`EAGAIN`]; Linux defines them to the same value.
///
/// See [`from_ret`] for usage.
pub const EWOULDBLOCK: Errno = EAGAIN;
/// Out of memory.
///
/// See [`from_ret`] for usage.
pub const ENOMEM: Errno = Errno(12);
/// Bad address.
///
/// See [`from_ret`] for usage.
pub const EFAULT: Errno = Errno(14);
/// Address already in use (e.g. `bind` on an existing socket path).
///
/// See [`from_ret`] for usage.
pub const EADDRINUSE: Errno = Errno(98);
/// Connection refused (e.g. `connect` when no listener is present).
///
/// See [`from_ret`] for usage.
pub const ECONNREFUSED: Errno = Errno(111);
/// Operation not supported on socket (e.g. `listen` on a datagram socket).
///
/// See [`from_ret`] for usage.
pub const EOPNOTSUPP: Errno = Errno(95);

/// `EPIPE` — write/send on a stream whose peer has closed
/// (`MSG_NOSIGNAL` turns the default `SIGPIPE` into this errno).
pub const EPIPE: Errno = Errno(32);
/// Inappropriate ioctl for device (e.g. `TCGETS` on a non-tty fd).
///
/// See [`from_ret`] for usage.
pub const ENOTTY: Errno = Errno(25);

/// Highest (most negative) raw value still interpreted as an error.
/// Linux reserves `-4095..=-1` for `-errno`.
const ERRNO_FLOOR: isize = -4095;

/// Maps a raw syscall return into `Result<usize, Errno>`.
///
/// A value in `-4095..=-1` is an error (`Err(Errno(-ret))`); any other
/// value (including large unsigned values that arrive as negative `isize`,
/// e.g. an `mmap` address near the top of the address space) is a success.
///
/// # Errors
///
/// Returns [`Errno`] when `ret` is in the reserved negative-errno range.
///
/// ```rust
/// use reovim_arch_sys_none_aarch64::{EBADF, EAGAIN, from_ret};
///
/// // Negative errno range → Err.
/// assert_eq!(from_ret(-9), Err(EBADF));
/// assert_eq!(from_ret(-11), Err(EAGAIN));
/// // Zero and positive → Ok.
/// assert_eq!(from_ret(0), Ok(0));
/// assert_eq!(from_ret(42), Ok(42));
/// // Large address-shaped value: -4096 is outside the errno range → Ok.
/// assert!(from_ret(-4096).is_ok());
/// ```
pub const fn from_ret(ret: isize) -> Result<usize, Errno> {
    // `(ERRNO_FLOOR..0).contains(&ret)` is not const-callable, so the range
    // is spelled out; the two comparisons are the success/error branches the
    // tests must cover both ways.
    if ret >= ERRNO_FLOOR && ret < 0 {
        // `-ret` is in `1..=4095`; the value provably fits in `i32`, but
        // clippy cannot prove the bound at this cast, so the truncation lint
        // is suppressed here with that justification.
        #[allow(clippy::cast_possible_truncation)]
        let code = (-ret) as i32;
        Err(Errno(code))
    } else {
        // A non-error return is reinterpreted as the kernel's `usize`
        // result (a byte count, fd, or address); the sign-preserving cast
        // is intentional for address-shaped returns.
        Ok(ret.cast_unsigned())
    }
}

// L12 layout (#785 Phase 5): tests live in the sibling file `errno_tests.rs`,
// declared here as a `#[path]` child. All items under test are public, so the
// test reaches them through `crate::` rather than `super::`. The file is only
// compiled when this module is (the lib.rs target gate), so the inner
// declaration needs only the `selftest` feature gate.
#[cfg(feature = "selftest")]
#[path = "errno_tests.rs"]
mod tests;
