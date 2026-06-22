//! Behavioral tests for `uapi/panic`: disposition exit codes, the record
//! shape, and that the hook fn-pointer aliases accept matching fn items.

use core::sync::atomic::{AtomicBool, Ordering};

use reovim_uapi_panic::{
    Disposition, PanicConfigError, PanicControl, PanicFlushTarget, PanicRecord, PreExitHookFn,
    RingTailProviderFn, StateRecordHookFn,
};

#[test]
fn disposition_exit_codes_are_normative() {
    assert_eq!(Disposition::Recover.exit_code(), 75);
    assert_eq!(Disposition::Halt.exit_code(), 70);
}

#[test]
fn disposition_variants_are_distinct() {
    assert_ne!(Disposition::Recover, Disposition::Halt);
}

#[test]
fn panic_record_carries_its_fields() {
    let halt = PanicRecord {
        disposition: Disposition::Halt,
        rollback_failed: false,
    };
    assert_eq!(halt.disposition, Disposition::Halt);
    assert!(!halt.rollback_failed);

    let recovered = PanicRecord {
        disposition: Disposition::Recover,
        rollback_failed: true,
    };
    assert_eq!(recovered.disposition, Disposition::Recover);
    assert!(recovered.rollback_failed);
}

#[test]
fn ring_tail_provider_fn_accepts_fn_item() {
    fn tail() -> &'static [u8] {
        b"tail"
    }
    let p: RingTailProviderFn = tail;
    assert_eq!(p(), b"tail");
}

#[test]
fn state_record_hook_fn_accepts_fn_item() {
    static CALLED: AtomicBool = AtomicBool::new(false);
    fn hook(r: PanicRecord) {
        CALLED.store(r.rollback_failed, Ordering::SeqCst);
    }
    let h: StateRecordHookFn = hook;
    h(PanicRecord {
        disposition: Disposition::Halt,
        rollback_failed: true,
    });
    assert!(CALLED.load(Ordering::SeqCst));
}

#[test]
fn pre_exit_hook_fn_accepts_fn_item() {
    static CALLED: AtomicBool = AtomicBool::new(false);
    fn pre_exit() {
        CALLED.store(true, Ordering::SeqCst);
    }
    let h: PreExitHookFn = pre_exit;
    h();
    assert!(CALLED.load(Ordering::SeqCst));
}

#[test]
fn panic_flush_target_wraps_current_bridge_key() {
    let target = PanicFlushTarget::from_raw_fd(11);
    assert_eq!(target.raw_fd(), 11);
}

#[test]
fn panic_config_error_names_write_once_refusal() {
    assert!(PanicConfigError::AlreadyConfigured.is_already_configured());
}

#[test]
fn panic_control_dispatches_function_pointers() {
    static PRE_EXIT_CALLED: AtomicBool = AtomicBool::new(false);

    fn tail() -> &'static [u8] {
        b"tail"
    }
    fn record(_: PanicRecord) {}
    fn pre_exit() {
        PRE_EXIT_CALLED.store(true, Ordering::SeqCst);
    }
    fn set_disposition(disposition: Disposition) -> Result<(), PanicConfigError> {
        assert_eq!(disposition, Disposition::Halt);
        Ok(())
    }
    fn set_tail(provider: RingTailProviderFn) -> Result<(), PanicConfigError> {
        assert_eq!(provider(), b"tail");
        Ok(())
    }
    fn set_record(hook: StateRecordHookFn) -> Result<(), PanicConfigError> {
        hook(PanicRecord {
            disposition: Disposition::Recover,
            rollback_failed: false,
        });
        Ok(())
    }
    fn set_flush(target: PanicFlushTarget) -> Result<(), PanicConfigError> {
        assert_eq!(target.raw_fd(), 4);
        Ok(())
    }
    fn set_pre_exit(hook: PreExitHookFn) -> Result<(), PanicConfigError> {
        hook();
        Ok(())
    }

    let panic = PanicControl::new(set_disposition, set_tail, set_record, set_flush, set_pre_exit);

    assert_eq!(panic.set_disposition(Disposition::Halt), Ok(()));
    assert_eq!(panic.set_ring_tail_provider(tail), Ok(()));
    assert_eq!(panic.set_state_record_hook(record), Ok(()));
    assert_eq!(panic.set_flush_target(PanicFlushTarget::from_raw_fd(4)), Ok(()));
    assert_eq!(panic.set_pre_exit_hook(pre_exit), Ok(()));
    assert!(PRE_EXIT_CALLED.load(Ordering::SeqCst));
}
