//! Panic/log configuration bridge from up-face `uapi/panic` to down-face KABI.
//!
//! Product code names disposition, ring-tail, state-record, flush target, and
//! pre-exit policy through `uapi/panic`. This bridge owns the mapping to the
//! always-present `kabi/panic` write-once fault-floor atoms.

use {
    reovim_kabi_panic::SetError,
    reovim_uapi_panic::{
        Disposition, PanicConfigError, PanicControl, PanicFlushTarget, PreExitHookFn,
        RingTailProviderFn, StateRecordHookFn,
    },
};

/// Returns the up-face panic/log control table backed by this bridge.
///
/// Composition roots pass this value into product-facing runtime code that
/// should not import `system/lib/kernel` or `kabi/panic` directly.
///
/// ```rust,no_run
/// use reovim_system_kernel::panic::panic_control;
/// use reovim_uapi_panic::Disposition;
///
/// let panic = panic_control();
/// let _ = panic.set_disposition(Disposition::Recover);
/// ```
#[must_use]
pub const fn panic_control() -> PanicControl {
    PanicControl::new(
        set_disposition,
        set_ring_tail_provider,
        set_state_record_hook,
        set_flush_target,
        set_pre_exit_hook,
    )
}

/// Configures the process panic disposition.
///
/// # Errors
///
/// Returns [`PanicConfigError::AlreadyConfigured`] when a disposition was
/// already registered.
pub fn set_disposition(disposition: Disposition) -> Result<(), PanicConfigError> {
    map_set_result(reovim_kabi_panic::set_disposition(disposition))
}

/// Configures the panic ring-tail provider.
///
/// # Errors
///
/// Returns [`PanicConfigError::AlreadyConfigured`] when a provider was already
/// registered.
pub fn set_ring_tail_provider(provider: RingTailProviderFn) -> Result<(), PanicConfigError> {
    map_set_result(reovim_kabi_panic::set_ring_tail_provider(provider))
}

/// Configures the panic state-record hook.
///
/// # Errors
///
/// Returns [`PanicConfigError::AlreadyConfigured`] when a hook was already
/// registered.
pub fn set_state_record_hook(hook: StateRecordHookFn) -> Result<(), PanicConfigError> {
    map_set_result(reovim_kabi_panic::set_state_record_hook(hook))
}

/// Configures the panic flush target.
///
/// The current bridge interprets [`PanicFlushTarget`] as the log sink fd key.
/// Later system log/fs services can issue the same up-face target type without
/// requiring editor code to call `kabi/panic` directly.
///
/// # Errors
///
/// Returns [`PanicConfigError::AlreadyConfigured`] when a flush target was
/// already registered.
pub fn set_flush_target(target: PanicFlushTarget) -> Result<(), PanicConfigError> {
    map_set_result(reovim_kabi_panic::set_flush_fd(target.raw_fd()))
}

/// Configures the panic pre-exit hook.
///
/// # Errors
///
/// Returns [`PanicConfigError::AlreadyConfigured`] when a hook was already
/// registered.
pub fn set_pre_exit_hook(hook: PreExitHookFn) -> Result<(), PanicConfigError> {
    map_set_result(reovim_kabi_panic::set_pre_exit_hook(hook))
}

pub(crate) const fn map_set_result(result: Result<(), SetError>) -> Result<(), PanicConfigError> {
    match result {
        Ok(()) => Ok(()),
        Err(SetError::AlreadySet) => Err(PanicConfigError::AlreadyConfigured),
    }
}
