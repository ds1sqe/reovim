//! Behavioral tests for `uapi/panic`: disposition exit codes, the record
//! shape, and that the hook fn-pointer aliases accept matching fn items.

use core::sync::atomic::{AtomicBool, Ordering};

use reovim_uapi_panic::{
    Disposition, PanicRecord, PreExitHookFn, RingTailProviderFn, StateRecordHookFn,
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
