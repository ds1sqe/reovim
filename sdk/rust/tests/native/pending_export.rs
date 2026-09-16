use super::*;
use reovim_kernel_interface::call::{Cancellation, Completion, PendingOps};
use std::cell::Cell;

struct Cleanup(Rc<Cell<u32>>);
impl Drop for Cleanup {
    fn drop(&mut self) {
        self.0.set(self.0.get() + 1);
    }
}

struct Work {
    generation: u64,
    taken: bool,
    closing: bool,
    ready: Rc<Cell<bool>>,
    cleanup_ready: Rc<Cell<bool>>,
    releases: Rc<Cell<u32>>,
    takes: Rc<Cell<u32>>,
    dropped: Rc<Cell<bool>>,
}

impl Drop for Work {
    fn drop(&mut self) {
        self.dropped.set(true);
    }
}

fn pending(work: Work) -> Pending {
    Pending::new(
        work,
        PendingOps {
            poll: |s, _| {
                if s.closing {
                    s.cleanup_ready.set(true);
                }
                Ok(if s.ready.get() { Poll::Ready(()) } else { Poll::Pending })
            },
            take: |s| {
                if s.taken {
                    return Err(Error::CLOSED);
                }
                s.taken = true;
                s.takes.set(s.takes.get() + 1);
                let value = Value::copy(&s.generation.to_le_bytes(), &[])?;
                Ok(Completion {
                    outcome: if s.generation == 7 {
                        Outcome::Unknown {
                            error: Error::UNAVAILABLE,
                            progress: Some(value),
                        }
                    } else {
                        Outcome::Ok(value)
                    },
                    activity: 1,
                    owner: Some(Box::new(Cleanup(s.releases.clone()))),
                })
            },
            cancel: |_| Ok(Cancellation::TooLate),
            reconcile: |s| {
                s.generation = 9;
                s.taken = false;
                Ok(())
            },
            finish: |s| {
                if s.cleanup_ready.get() {
                    return Ok(());
                }
                s.closing = true;
                Err(Error::BUSY)
            },
        },
    )
}

#[test]
fn rust_pending_exports_uncertainty_and_retains_cleanup_until_finish() {
    let library = Library::compile_source("pending.c");
    let observe: unsafe extern "C" fn(*mut abi::Pending, u32, u32) -> u64 =
        unsafe { library.symbol(c"fixture_observe") };
    let ready = Rc::new(Cell::new(false));
    let gate = Rc::new(Cell::new(false));
    let releases = Rc::new(Cell::new(0));
    let takes = Rc::new(Cell::new(0));
    let dropped = Rc::new(Cell::new(false));
    let mut raw = reovim_sdk::export_pending(
        pending(Work {
            generation: 7,
            taken: false,
            closing: false,
            ready: ready.clone(),
            cleanup_ready: gate.clone(),
            releases: releases.clone(),
            takes: takes.clone(),
            dropped: dropped.clone(),
        }),
        LIMITS,
    );
    let ops = unsafe { &*raw.ops };
    let empty = abi::Wake {
        context: core::ptr::null_mut(),
        wake: None,
        retain: None,
        release: None,
    };
    assert_eq!(unsafe { ops.poll.unwrap()(raw.context, empty) }, 0);
    assert_eq!(unsafe { ops.cancel.unwrap()(raw.context) }, 1);
    ready.set(true);
    assert_eq!(unsafe { observe(&raw mut raw, 2, 1) }, 7);
    assert_eq!(releases.get(), 1);
    assert_eq!(unsafe { ops.reconcile.unwrap()(raw.context) }, 0);
    assert_eq!(unsafe { observe(&raw mut raw, 0, 1) }, 9);
    assert_eq!(releases.get(), 2);
    assert_eq!(takes.get(), 2);
    assert_eq!(unsafe { ops.finish.unwrap()(raw.context) }, Error::BUSY.code.0);
    assert!(!dropped.get());
    assert_eq!(unsafe { ops.poll.unwrap()(raw.context, empty) }, 1);
    assert!(gate.get());
    assert_eq!(unsafe { ops.finish.unwrap()(raw.context) }, 0);
    assert!(dropped.get());
}

#[test]
fn failed_native_take_keeps_rust_completion_until_explicit_cleanup() {
    let releases = Rc::new(Cell::new(0));
    let takes = Rc::new(Cell::new(0));
    let dropped = Rc::new(Cell::new(false));
    let raw = reovim_sdk::export_pending(
        pending(Work {
            generation: 9,
            taken: false,
            closing: false,
            ready: Rc::new(Cell::new(true)),
            cleanup_ready: Rc::new(Cell::new(true)),
            releases: releases.clone(),
            takes: takes.clone(),
            dropped: dropped.clone(),
        }),
        ValueLimits { bytes: 4, ..LIMITS },
    );
    let ops = unsafe { &*raw.ops };
    let empty = abi::Wake {
        context: core::ptr::null_mut(),
        wake: None,
        retain: None,
        release: None,
    };
    assert_eq!(unsafe { ops.poll.unwrap()(raw.context, empty) }, 1);
    let mut output = core::mem::MaybeUninit::uninit();
    for _ in 0..2 {
        assert_eq!(
            unsafe { ops.take.unwrap()(raw.context, output.as_mut_ptr()) },
            Error::PRESSURE.code.0
        );
        assert_eq!(releases.get(), 0);
        assert_eq!(takes.get(), 1);
    }
    assert_eq!(unsafe { ops.finish.unwrap()(raw.context) }, 0);
    assert_eq!(releases.get(), 1);
    assert!(dropped.get());
}
