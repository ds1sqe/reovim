//! Raw-terminal mode RAII guard.
//!
//! [`RawMode`] saves the current [`Termios`] via `ioctl(TCGETS)`, sets
//! the terminal into raw (cbreak, no-echo) mode, and restores the original
//! settings on `Drop`.
//!
//! ## What "raw mode" means here
//!
//! The guard disables:
//! - `ICANON` (canonical / line-buffered input),
//! - `ECHO`, `ECHOE`, `ECHOK` (character echo and echo-erase),
//! - `ISIG` (signal-generating input characters like Ctrl-C),
//! - `OPOST` (output post-processing — prevents `\n` → `\r\n` translation),
//! - `BRKINT`, `ICRNL`, `IXON`, `INPCK`, `ISTRIP` (misc. cooked-mode
//!   translations).
//!
//! It sets:
//! - `c_cc[VMIN] = 1` — reads block until at least one byte is available.
//! - `c_cc[VTIME] = 0` — no read timeout.
//!
//! ## Non-tty fds
//!
//! If `fd` is not a terminal, `ioctl(TCGETS)` returns
//! [`crate::sys::ENOTTY`]. [`RawMode::enter`] propagates this error rather
//! than panicking, so the pipe-based E2E harness (DEV2) can assert the
//! `ENOTTY` arm without a real TTY.
//!
//! ## panic = "abort" and `Drop`
//!
//! Under `panic = "abort"` (the workspace profile) `Drop` does not run on a
//! panic, so a panicking TUI client leaves the terminal in raw mode. The
//! pre-exit callback seam (gap-7, #797 — `arch::panic::register_pre_exit_hook`)
//! addresses this; it lands before #797 Phase 4 of the walking skeleton. Phase 1
//! implements the correct RAII path and documents the gap.

use crate::sys::{
    Errno, ioctl,
    term::{
        BRKINT, ECHO, ECHOE, ECHOK, ICANON, ICRNL, INPCK, ISIG, ISTRIP, IXON, OPOST, TCGETS,
        TCSETS, VMIN, VTIME,
    },
};

/// A RAII guard that places a terminal file descriptor into raw (cbreak,
/// no-echo) mode and restores the original settings on `Drop`.
///
/// ## Typical usage
///
/// ```no_run
/// // Entering raw mode requires a real TTY (stdin fd 0) — no_run.
/// use reovim_arch::term::RawMode;
///
/// // Enter raw mode on stdin.
/// let _raw = RawMode::enter(0).expect("stdin is a tty");
/// // Read raw bytes, paint frames, etc.
/// // On drop, the original cooked mode is restored.
/// ```
pub struct RawMode {
    /// The fd whose settings were changed.
    fd: i32,
    /// The original termios to restore on `Drop`.
    saved: Termios,
}

impl RawMode {
    /// Enters raw mode on `fd`, returning a guard that restores the original
    /// settings on `Drop`.
    ///
    /// # Errors
    ///
    /// Returns [`Errno`] when:
    /// - `fd` is not a terminal ([`crate::sys::ENOTTY`]).
    /// - `fd` is not a valid file descriptor ([`crate::sys::EBADF`]).
    ///
    /// ```rust
    /// // ENOTTY arm: entering raw mode on /dev/null (not a tty).
    /// use reovim_arch::sys::{ENOTTY, AT_FDCWD, O_RDONLY, O_CLOEXEC, openat, close};
    /// use reovim_arch::term::RawMode;
    ///
    /// let fd = openat(AT_FDCWD, b"/dev/null\0", O_RDONLY | O_CLOEXEC, 0).unwrap();
    /// let result = RawMode::enter(fd as i32);
    /// close(fd as i32).unwrap();
    /// assert_eq!(result, Err(ENOTTY));
    /// ```
    pub fn enter(fd: i32) -> Result<Self, Errno> {
        // Read the current termios via TCGETS.
        let mut saved = Termios::zeroed();
        ioctl(fd, TCGETS, core::ptr::from_mut(&mut saved).addr())?;

        // Build the raw-mode termios from the saved baseline.
        let mut raw = saved;
        // Input flags: turn off cooked-mode translations.
        raw.c_iflag &= !(BRKINT | ICRNL | INPCK | ISTRIP | IXON);
        // Output flags: disable post-processing so \n isn't mapped to \r\n.
        raw.c_oflag &= !OPOST;
        // Local flags: disable canonical, echo, and signal-generating chars.
        raw.c_lflag &= !(ECHO | ECHOE | ECHOK | ICANON | ISIG);
        // Read control: block until at least 1 byte; no timeout.
        raw.c_cc[VMIN] = 1;
        raw.c_cc[VTIME] = 0;

        // Apply raw termios via TCSETS (immediate, no drain).
        ioctl(fd, TCSETS, core::ptr::from_ref(&raw).addr())?;

        Ok(Self { fd, saved })
    }

    /// Returns the saved (pre-raw) [`Termios`] snapshot for inspection.
    ///
    /// Useful in tests to compare the before/after state.
    ///
    /// ```rust
    /// // saved() is tested via the arch selftest ENOTTY fixture — no
    /// // standalone runnable example (requires a real tty).
    /// ```
    #[must_use]
    pub const fn saved(&self) -> &Termios {
        &self.saved
    }
}

impl PartialEq for RawMode {
    /// Two `RawMode` guards are equal when they protect the same `fd` and
    /// carry the same saved `Termios`.
    ///
    /// ```no_run
    /// // Equality on RawMode requires a real tty — no_run.
    /// ```
    fn eq(&self, other: &Self) -> bool {
        self.fd == other.fd && self.saved == other.saved
    }
}

impl Eq for RawMode {}

impl core::fmt::Debug for RawMode {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("RawMode")
            .field("fd", &self.fd)
            .finish_non_exhaustive()
    }
}

impl Drop for RawMode {
    /// Restores the terminal to the original cooked settings via `TCSETS`.
    ///
    /// Errors are silently ignored: the fd may already be closed, or the
    /// process may be shutting down. Under `panic = "abort"` this `Drop`
    /// does not run on a panic — see the module-level note.
    fn drop(&mut self) {
        // TCSETS to restore; ignore errors (best-effort restore on teardown).
        let _ = ioctl(self.fd, TCSETS, core::ptr::from_ref(&self.saved).addr());
    }
}

// Re-export `Termios` so callers can use `arch::term::Termios` directly.
// (`RawMode` is auto-`Send`: an fd scalar plus a plain `#[repr(C)]`
// integer struct — no manual impl needed.)
pub use crate::sys::term::Termios;

// Also re-export the errno variant callers need for the ENOTTY check.
pub use crate::sys::ENOTTY;

// L12 layout (#797 Phase 1): tests live in the sibling file `term_tests.rs`,
// declared in `arch/src/lib.rs` under `#[cfg(feature = "selftest")]`.
#[cfg(feature = "selftest")]
#[path = "term_tests.rs"]
mod tests;
