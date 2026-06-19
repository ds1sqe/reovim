//! Kernel-ABI terminal types — shared by the Linux-family `sys/` tier.
//!
//! The `termios` struct layout mirrors the Linux kernel UAPI exactly. The
//! `TCGETS`/`TCSETS` ioctl request codes are the values the kernel checks,
//! not the glibc-derived constants (which match on Linux but we cite the
//! kernel source here per L9/DAG6).
//!
//! Source: Linux `include/uapi/asm-generic/termbits.h`.

// ---- ioctl request codes for termios ----------------------------------------

/// `TCGETS` — get the current terminal parameters into a [`Termios`] struct.
///
/// Source: Linux `include/uapi/asm-generic/ioctls.h` `TCGETS = 0x5401`.
///
/// See [`crate::sys::ioctl`] for usage.
pub const TCGETS: usize = 0x5401;

/// `TCSETS` — set terminal parameters immediately from a [`Termios`] struct.
///
/// Source: Linux `include/uapi/asm-generic/ioctls.h` `TCSETS = 0x5402`.
///
/// See [`crate::sys::ioctl`] for usage.
pub const TCSETS: usize = 0x5402;

// ---- termios flag bits -------------------------------------------------------

/// `ICANON` — canonical (line-buffered) input mode bit in `c_lflag`.
///
/// Source: Linux `include/uapi/asm-generic/termbits.h` `ICANON = 0x00000002`.
///
/// See [`Termios`] for usage.
pub const ICANON: u32 = 0x0000_0002;

/// `ECHO` — echo input characters bit in `c_lflag`.
///
/// Source: Linux `include/uapi/asm-generic/termbits.h` `ECHO = 0x00000008`.
///
/// See [`Termios`] for usage.
pub const ECHO: u32 = 0x0000_0008;

/// `ECHOE` — echo erase character visually bit in `c_lflag`.
///
/// Source: Linux `include/uapi/asm-generic/termbits.h` `ECHOE = 0x00000010`.
///
/// See [`Termios`] for usage.
pub const ECHOE: u32 = 0x0000_0010;

/// `ECHOK` — echo kill character on a new line bit in `c_lflag`.
///
/// Source: Linux `include/uapi/asm-generic/termbits.h` `ECHOK = 0x00000020`.
///
/// See [`Termios`] for usage.
pub const ECHOK: u32 = 0x0000_0020;

/// `ISIG` — generate signals (`SIGINT`/`SIGQUIT`/`SIGSUSP`) bit in `c_lflag`.
///
/// Source: Linux `include/uapi/asm-generic/termbits.h` `ISIG = 0x00000001`.
///
/// See [`Termios`] for usage.
pub const ISIG: u32 = 0x0000_0001;

/// `OPOST` — enable output post-processing bit in `c_oflag`.
///
/// Source: Linux `include/uapi/asm-generic/termbits.h` `OPOST = 0x00000001`.
///
/// See [`Termios`] for usage.
pub const OPOST: u32 = 0x0000_0001;

/// `BRKINT` — signal interrupt on break bit in `c_iflag`.
///
/// Source: Linux `include/uapi/asm-generic/termbits.h` `BRKINT = 0x00000002`.
///
/// See [`Termios`] for usage.
pub const BRKINT: u32 = 0x0000_0002;

/// `ICRNL` — translate carriage return to newline on input bit in `c_iflag`.
///
/// Source: Linux `include/uapi/asm-generic/termbits.h` `ICRNL = 0x00000100`.
///
/// See [`Termios`] for usage.
pub const ICRNL: u32 = 0x0000_0100;

/// `IXON` — enable XON/XOFF flow control bit in `c_iflag`.
///
/// Source: Linux `include/uapi/asm-generic/termbits.h` `IXON = 0x00000400`.
///
/// See [`Termios`] for usage.
pub const IXON: u32 = 0x0000_0400;

/// `INPCK` — enable input parity checking bit in `c_iflag`.
///
/// Source: Linux `include/uapi/asm-generic/termbits.h` `INPCK = 0x00000010`.
///
/// See [`Termios`] for usage.
pub const INPCK: u32 = 0x0000_0010;

/// `ISTRIP` — strip 8th bit of input characters bit in `c_iflag`.
///
/// Source: Linux `include/uapi/asm-generic/termbits.h` `ISTRIP = 0x00000020`.
///
/// See [`Termios`] for usage.
pub const ISTRIP: u32 = 0x0000_0020;

/// Number of control characters in the `c_cc` array.
///
/// Source: Linux `include/uapi/asm-generic/termbits.h` `NCCS = 19`.
pub const NCCS: usize = 19;

/// `VMIN` — index of the minimum number of characters for non-canonical reads
/// in `c_cc`.
///
/// Source: Linux `include/uapi/asm-generic/termbits.h` `VMIN = 6`.
///
/// See [`Termios`] for usage.
pub const VMIN: usize = 6;

/// `VTIME` — index of the timeout for non-canonical reads in `c_cc`.
///
/// Source: Linux `include/uapi/asm-generic/termbits.h` `VTIME = 5`.
///
/// See [`Termios`] for usage.
pub const VTIME: usize = 5;

// ---- termios struct ----------------------------------------------------------

/// `struct termios` — terminal line-discipline parameters.
///
/// The Linux kernel UAPI layout
/// (`include/uapi/asm-generic/termbits.h`):
/// ```text
/// struct termios {
///     tcflag_t c_iflag;     /* input mode flags */
///     tcflag_t c_oflag;     /* output mode flags */
///     tcflag_t c_cflag;     /* control mode flags */
///     tcflag_t c_lflag;     /* local mode flags */
///     cc_t     c_line;      /* line discipline */
///     cc_t     c_cc[NCCS];  /* control characters */
/// };
/// ```
/// (`tcflag_t` = `u32`, `cc_t` = `u8`). Total: 4*4 + 1 + 19 = 36 bytes.
///
/// ```rust
/// use reovim_arch_sys_linux_x86_64::term::{Termios, NCCS};
///
/// let t = Termios::zeroed();
/// assert_eq!(t.c_cc.len(), NCCS);
/// assert_eq!(t.c_iflag, 0);
/// assert_eq!(t.c_lflag, 0);
/// ```
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Termios {
    /// Input mode flags.
    pub c_iflag: u32,
    /// Output mode flags.
    pub c_oflag: u32,
    /// Control mode flags.
    pub c_cflag: u32,
    /// Local mode flags.
    pub c_lflag: u32,
    /// Line discipline character (typically `N_TTY`).
    pub c_line: u8,
    /// Special control characters (indexed by `VMIN`, `VTIME`, etc.).
    pub c_cc: [u8; NCCS],
}

impl Termios {
    /// Returns a zeroed `Termios` (all fields zero).
    ///
    /// ```rust
    /// use reovim_arch_sys_linux_x86_64::term::Termios;
    ///
    /// let t = Termios::zeroed();
    /// assert_eq!(t.c_iflag, 0);
    /// assert_eq!(t.c_lflag, 0);
    /// assert!(t.c_cc.iter().all(|&b| b == 0));
    /// ```
    #[must_use]
    pub const fn zeroed() -> Self {
        Self {
            c_iflag: 0,
            c_oflag: 0,
            c_cflag: 0,
            c_lflag: 0,
            c_line: 0,
            c_cc: [0u8; NCCS],
        }
    }
}

// L12: tests live in the sibling file `term_tests.rs`, declared here as a
// `#[path]` child. The tests reach the public items through `crate::term::`.
// The file is only compiled when this module is (the lib.rs target gate), so
// the inner declaration needs only the `selftest` feature gate.
#[cfg(feature = "selftest")]
#[path = "term_tests.rs"]
mod tests;
