//! Linux process identity and termination notification for hosted assembly.

use core::sync::atomic::{AtomicBool, Ordering};
use std::{
    fs::File,
    io::{self, Read},
};

static CLAIMED: AtomicBool = AtomicBool::new(false);
static STOP: AtomicBool = AtomicBool::new(false);

unsafe extern "C" {
    fn signal(number: i32, handler: usize) -> usize;
}

extern "C" fn stop(_: i32) {
    // AtomicBool uses a lock-free byte operation on the supported Linux target.
    STOP.store(true, Ordering::Relaxed);
}

/// Owns process SIGINT/SIGTERM handlers, e.g. while the main service runs.
/// Only one owner may be installed; dropping it restores the prior handlers.
pub struct Termination {
    interrupt: usize,
    terminate: usize,
}

impl Termination {
    /// Installs notification only; the execution owner performs cleanup.
    /// # Errors
    /// Concurrent ownership or native handler installation failure rejects.
    pub fn install() -> io::Result<Self> {
        CLAIMED
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .map_err(|_| {
                io::Error::other("termination handlers already owned")
            })?;
        STOP.store(false, Ordering::Relaxed);
        // SAFETY: Linux signal uses pointer-sized handler values. This static
        // extern-C handler never unwinds and only stores a lock-free atomic.
        let interrupt = unsafe { signal(2, stop as *const () as usize) };
        if interrupt == usize::MAX {
            CLAIMED.store(false, Ordering::Release);
            return Err(io::Error::last_os_error());
        }
        // SAFETY: Same handler contract as above; SIGTERM is 15 on Linux.
        let terminate = unsafe { signal(15, stop as *const () as usize) };
        if terminate == usize::MAX {
            let error = io::Error::last_os_error();
            // SAFETY: Restore the exact prior SIGINT handler from signal.
            unsafe { signal(2, interrupt) };
            CLAIMED.store(false, Ordering::Release);
            return Err(error);
        }
        Ok(Self { interrupt, terminate })
    }

    /// Supplies the flag to the driver, e.g. `service::serve_closing`.
    #[must_use]
    pub const fn requested(&self) -> &AtomicBool {
        &STOP
    }
}

impl Drop for Termination {
    fn drop(&mut self) {
        // SAFETY: This exclusive owner restores the exact prior pointer-sized
        // native handler values; both installed handlers have static storage.
        unsafe {
            signal(2, self.interrupt);
            signal(15, self.terminate);
        }
        CLAIMED.store(false, Ordering::Release);
    }
}

/// Returns an OS-random nonzero identity, e.g. a Kernel epoch or credential.
/// # Errors
/// OS entropy read failure or an invalid all-zero identity rejects startup.
pub fn identity() -> io::Result<[u8; 16]> {
    let mut bytes = [0; 16];
    File::open("/dev/urandom")?.read_exact(&mut bytes)?;
    if bytes == [0; 16] {
        return Err(io::Error::other("zero random process identity"));
    }
    Ok(bytes)
}
