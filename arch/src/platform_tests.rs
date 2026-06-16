//! Boot/install selftests for arch's platform-contract implementation
//! (SP02 Phase 3), compiled into the lib under `selftest` (#785 Phase 5).
//!
//! ## Boot ordering (why these read, never install)
//!
//! The arch-owned `rust_entry` (the real boot entry in `start.rs`) installs
//! `PLATFORM_VTABLE` write-once BEFORE it calls the bin's `arch_main` shim, and
//! the selftest runner runs as that shim. So by the time any test body runs the
//! handle is already installed: these tests read it through `kabi::handle` and
//! prove the end-to-end dispatch, and assert that a *second* install is
//! rejected (AB12) — they never perform the first install themselves.
//!
//! This makes the boot selftest a genuine order proof: `_start` → `rust_entry`
//! → allocator (heap-free) → static vtable built → install → `arch_main` →
//! these tests read the clock THROUGH the handle.

use reovim_kabi_platform::{InstallError, handle};

use crate::{arch_test, platform::install_platform, testrt};

arch_test!(platform_clock_reads_through_installed_handle, {
    // The boot path already installed the handle before this runner started.
    // Read the monotonic clock THROUGH the handle: handle → fn pointer → arch
    // `Instant::now`. The value must flow end-to-end, not via a bypass.
    // SAFETY: the handle is installed by `rust_entry`; `clock` has no precondition.
    let t0 = unsafe { (handle().clock)() };
    // A monotonic nanosecond reading after boot is strictly positive (the
    // process has been up for a nonzero time).
    testrt::check(t0 > 0, "clock through the handle returns a positive reading");

    // A second read is non-decreasing (the source is monotonic). This proves
    // the dispatch is repeatable, not a one-shot.
    // SAFETY: as above.
    let t1 = unsafe { (handle().clock)() };
    testrt::check(t1 >= t0, "the monotonic clock never steps backward through the handle");
});

arch_test!(platform_install_is_write_once, {
    // `rust_entry` performed the first (winning) install at boot. Any further
    // install must be rejected — write-once (AB12), mirroring the panic-handler
    // write-once seams. This is the end-to-end AB12 proof: the real boot
    // install is the first writer, and this is the second.
    testrt::check_eq(install_platform(), Err(InstallError::AlreadyInstalled));
});
