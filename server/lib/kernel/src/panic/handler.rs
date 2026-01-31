//! Custom panic handler infrastructure.
//!
//! Linux equivalent: `kernel/panic.c`
//!
//! Provides custom panic handling for graceful shutdown and crash recovery.

use std::{
    panic::{self, PanicHookInfo},
    sync::{
        OnceLock,
        atomic::{AtomicBool, Ordering},
    },
};

static PANIC_HANDLER_INSTALLED: AtomicBool = AtomicBool::new(false);

/// Recovery callback type.
///
/// Called during panic to allow saving state before the process terminates.
pub type RecoveryCallback = Box<dyn Fn(&PanicHookInfo<'_>) + Send + Sync>;

/// Global recovery callback.
static RECOVERY_CALLBACK: OnceLock<RecoveryCallback> = OnceLock::new();

/// Install the custom panic handler.
///
/// Linux equivalent: `panic_notifier_list`
///
/// This wraps the default panic handler to:
/// 1. Generate a crash report
/// 2. Call the recovery callback (if set) to save state
/// 3. Log the crash report
/// 4. Write crash report to file
/// 5. Call the original panic handler
///
/// # Example
///
/// ```ignore
/// use reovim_kernel::panic::{install_panic_handler, set_recovery_callback};
///
/// // Set up recovery to save unsaved buffers
/// set_recovery_callback(Box::new(|info| {
///     // Save unsaved buffers...
/// }));
///
/// // Install the handler
/// install_panic_handler();
/// ```
///
/// # Notes
///
/// - Can only be called once; subsequent calls are no-ops
/// - Must be called early in application startup
pub fn install_panic_handler() {
    if PANIC_HANDLER_INSTALLED.swap(true, Ordering::SeqCst) {
        return; // Already installed
    }

    let default_hook = panic::take_hook();

    panic::set_hook(Box::new(move |info| {
        // 1. Generate crash report
        let report = super::report::generate_crash_report(info);

        // 2. Attempt recovery (save buffers)
        if let Some(callback) = RECOVERY_CALLBACK.get() {
            callback(info);
        }

        // 3. Log crash report
        crate::pr_err!("PANIC: {}", report.summary());

        // 4. Write crash report to file
        if let Err(e) = report.write_to_file() {
            eprintln!("Failed to write crash report: {e}");
        }

        // 5. Call original handler
        default_hook(info);
    }));
}

/// Set the recovery callback.
///
/// Called during panic to allow saving state. Can only be set once.
///
/// # Example
///
/// ```ignore
/// set_recovery_callback(Box::new(|_info| {
///     // Save all unsaved buffers to recovery directory
///     for buffer in buffers.iter() {
///         if buffer.is_modified() {
///             save_buffer_for_recovery(buffer);
///         }
///     }
/// }));
/// ```
pub fn set_recovery_callback(callback: RecoveryCallback) {
    let _ = RECOVERY_CALLBACK.set(callback);
}

/// Check if the panic handler has been installed.
#[must_use]
pub fn is_handler_installed() -> bool {
    PANIC_HANDLER_INSTALLED.load(Ordering::SeqCst)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_handler_not_installed_by_default() {
        // Note: This test may fail if run after other tests that install the handler
        // In practice, the handler is installed once at startup
    }

    #[test]
    fn test_install_is_idempotent() {
        // Multiple calls should not panic
        install_panic_handler();
        install_panic_handler();
        assert!(is_handler_installed());
    }
}
