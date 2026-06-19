//! Kernel-ABI networking types — shared by the Linux-family `sys/` tier.
//!
//! These are the `#[repr(C)]` structs the socket syscalls read from and write
//! into, mirroring the Linux kernel UAPI headers exactly. Only the `AF_UNIX`
//! socket address shape is needed for this floor (the walking skeleton uses
//! only Unix-domain sockets; other address families are out of scope until a
//! second consumer demands them — rule of three).
//!
//! Source citations name the kernel UAPI header, not libc, per L9/DAG6.

// ---- address family constants -----------------------------------------------

/// `AF_UNIX` — Unix-domain socket address family.
///
/// Source: Linux `include/uapi/linux/socket.h` `AF_UNIX = 1`.
///
/// See [`SockaddrUn`] for usage.
pub const AF_UNIX: u16 = 1;

/// `SOCK_STREAM` — connection-oriented byte-stream socket type.
///
/// Source: Linux `include/uapi/linux/net.h` `SOCK_STREAM = 1`.
///
/// See [`crate::sys::socket`] for usage.
pub const SOCK_STREAM: usize = 1;

/// `SOCK_CLOEXEC` — set close-on-exec on the new socket atomically.
///
/// Source: Linux `include/uapi/linux/net.h` `SOCK_CLOEXEC = O_CLOEXEC`.
/// On Linux `O_CLOEXEC == 0o2_000_000` (octal) == `0x80000`.
///
/// See [`crate::sys::socket`] for usage.
pub const SOCK_CLOEXEC: usize = 0x0008_0000;

/// Maximum path length inside [`SockaddrUn::sun_path`], including the NUL
/// terminator.
///
/// Source: Linux `include/uapi/linux/un.h` `#define UNIX_PATH_MAX 108`.
pub const UNIX_PATH_MAX: usize = 108;

// ---- socket address structures -----------------------------------------------

/// `struct sockaddr_un` — Unix-domain socket address.
///
/// The kernel UAPI layout (Linux `include/uapi/linux/un.h`):
/// ```text
/// struct sockaddr_un {
///     __kernel_sa_family_t sun_family;  /* AF_UNIX, u16 */
///     char sun_path[UNIX_PATH_MAX];     /* socket path [108] */
/// };
/// ```
/// Total: 2 + 108 = 110 bytes. `addrlen` passed to `bind`/`connect` must be
/// `2 + strlen(sun_path) + 1` (the NUL terminator is included in the length
/// passed to the kernel — this is the Linux abstract-vs-pathname convention for
/// regular (pathname) sockets).
///
/// ```rust
/// use reovim_arch_sys_linux_aarch64::net::{SockaddrUn, AF_UNIX, UNIX_PATH_MAX};
///
/// let mut sa = SockaddrUn::zeroed();
/// sa.sun_family = AF_UNIX;
/// assert_eq!(sa.sun_path.len(), UNIX_PATH_MAX);
/// assert_eq!(sa.sun_family, AF_UNIX);
/// ```
#[repr(C)]
#[derive(Clone, Copy)]
pub struct SockaddrUn {
    /// Address family — must be `AF_UNIX` for Unix-domain sockets.
    pub sun_family: u16,
    /// NUL-terminated filesystem path (or abstract name with leading `\0`).
    pub sun_path: [u8; UNIX_PATH_MAX],
}

impl SockaddrUn {
    /// Returns a zeroed `SockaddrUn` (all fields zero).
    ///
    /// ```rust
    /// use reovim_arch_sys_linux_aarch64::net::SockaddrUn;
    ///
    /// let sa = SockaddrUn::zeroed();
    /// assert_eq!(sa.sun_family, 0);
    /// assert!(sa.sun_path.iter().all(|&b| b == 0));
    /// ```
    #[must_use]
    pub const fn zeroed() -> Self {
        Self {
            sun_family: 0,
            sun_path: [0u8; UNIX_PATH_MAX],
        }
    }

    /// Returns the `addrlen` value to pass to `bind` / `connect` for a
    /// pathname socket: `offsetof(sun_path) + strlen(sun_path) + 1`.
    ///
    /// The kernel counts the NUL terminator in the length for pathname sockets
    /// (unlike abstract sockets, where the leading `\0` replaces it). This
    /// method returns the correct length when `sun_path` contains a valid
    /// NUL-terminated C string.
    ///
    /// ```rust
    /// use reovim_arch_sys_linux_aarch64::net::{SockaddrUn, AF_UNIX};
    ///
    /// let mut sa = SockaddrUn::zeroed();
    /// sa.sun_family = AF_UNIX;
    /// sa.sun_path[..8].copy_from_slice(b"/tmp/ab\0");
    /// // offsetof(sun_family)=0, sizeof(sun_family)=2, "ab\0" is 3 bytes path.
    /// // addrlen = 2 + 7 + 1 = 10.
    /// assert_eq!(sa.addrlen(), 10);
    /// ```
    #[must_use]
    pub fn addrlen(&self) -> usize {
        // `sun_family` is 2 bytes; add the path bytes up to and including the NUL.
        let path_len = self
            .sun_path
            .iter()
            .position(|&b| b == 0)
            .map_or(UNIX_PATH_MAX, |n| n + 1);
        2 + path_len
    }
}

// `SockaddrUn` is `Copy`; all fields are plain integers/arrays; no heap.

// L12: tests live in the sibling file `net_tests.rs`, declared here as a
// `#[path]` child. The tests reach the public items through `crate::net::`. The
// file is only compiled when this module is (the lib.rs target gate), so the
// inner declaration needs only the `selftest` feature gate.
#[cfg(feature = "selftest")]
#[path = "net_tests.rs"]
mod tests;
