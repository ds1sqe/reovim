use super::*;
use reovim_kernel_interface::{
    call::Cancellation,
    caller::{Caller, CallerOps, Status},
};
use reovim_sdk::ExportedCaller;
use std::sync::{Mutex, mpsc};

type Waiters = Arc<Mutex<Vec<Waker>>>;
fn waiting() -> (ExportedCaller, Waiters) {
    let waiters = Arc::new(Mutex::new(Vec::new()));
    let caller = Caller::new(
        waiters.clone(),
        CallerOps {
            reserve: |_| Ok(1),
            submit: |_, _| Ok(()),
            result: |state: &Waiters, _, cx| {
                if let Some(cx) = cx {
                    state.lock().unwrap().push(cx.waker().clone());
                }
                Ok(Status::Pending)
            },
            cancel: |_, _| Ok(Cancellation::TooLate),
            reconcile: |_, _| Err(Error::UNSUPPORTED),
            adopt: |_, _| Ok(()),
            release: |_, _| Ok(()),
        },
    );
    (ExportedCaller::new(caller, LIMITS, 8).unwrap(), waiters)
}
fn view(owner: &ExportedCaller) -> abi::Pending {
    let caller = owner.raw();
    let mut output = core::mem::MaybeUninit::uninit();
    let mut error = abi::Error::default();
    assert_eq!(
        unsafe {
            (*caller.ops).result.unwrap()(
                caller.context,
                1,
                output.as_mut_ptr(),
                &raw mut error,
            )
        },
        0
    );
    unsafe { output.assume_init() }
}

#[test]
fn native_view_finish_disconnects_foreign_wakes_from_retained_waiters() {
    let library = Library::compile();
    let signal: unsafe extern "C" fn() -> abi::Wake =
        unsafe { library.symbol(c"fixture_signal") };
    let (owner, waiters) = waiting();
    let view = view(&owner);
    let ops = unsafe { &*view.ops };
    assert_eq!(unsafe { ops.poll.unwrap()(view.context, signal()) }, 0);
    assert_eq!(library.count(c"fixture_wake_refs"), 1);
    assert_eq!(unsafe { ops.finish.unwrap()(view.context) }, 0);
    assert_eq!(library.count(c"fixture_wake_refs"), 0);
    let waker = waiters.lock().unwrap().pop().unwrap();
    waker.wake();
    assert_eq!(library.count(c"fixture_wakes"), 0);
}

struct Block {
    entered: mpsc::SyncSender<()>,
    resume: Mutex<mpsc::Receiver<()>>,
}
unsafe extern "C" fn retain(context: *mut c_void) {
    unsafe { Arc::increment_strong_count(context.cast::<Block>()) };
}
unsafe extern "C" fn release(context: *mut c_void) {
    unsafe { Arc::decrement_strong_count(context.cast::<Block>()) };
}
unsafe extern "C" fn wake(context: *mut c_void) {
    let state = unsafe { &*context.cast::<Block>() };
    state.entered.send(()).unwrap();
    state.resume.lock().unwrap().recv().unwrap();
}

#[test]
fn native_view_finish_waits_for_an_in_flight_foreign_wake() {
    let (entered_tx, entered_rx) = mpsc::sync_channel(1);
    let (resume_tx, resume_rx) = mpsc::sync_channel(1);
    let block =
        Arc::new(Block { entered: entered_tx, resume: Mutex::new(resume_rx) });
    let weak = Arc::downgrade(&block);
    let signal = abi::Wake {
        context: Arc::as_ptr(&block).cast_mut().cast(),
        retain: Some(retain),
        release: Some(release),
        wake: Some(wake),
    };
    let (owner, waiters) = waiting();
    let view = view(&owner);
    let ops = unsafe { &*view.ops };
    assert_eq!(unsafe { ops.poll.unwrap()(view.context, signal) }, 0);
    let relay = waiters.lock().unwrap()[0].clone();
    let thread = std::thread::spawn(move || relay.wake());
    entered_rx.recv_timeout(std::time::Duration::from_secs(5)).unwrap();
    let closing = unsafe { ops.finish.unwrap()(view.context) };
    resume_tx.send(()).unwrap();
    thread.join().unwrap();
    assert_eq!(closing, Error::BUSY.code.0);
    assert_eq!(unsafe { ops.finish.unwrap()(view.context) }, 0);
    drop(block);
    assert!(weak.upgrade().is_none());
    waiters.lock().unwrap().pop().unwrap().wake();
}

impl Wake for Block {
    fn wake(self: Arc<Self>) {
        self.entered.send(()).unwrap();
        self.resume.lock().unwrap().recv().unwrap();
    }
}

#[test]
fn imported_caller_close_fences_clones_and_retries_busy_delivery_cleanup() {
    let (entered_tx, entered_rx) = mpsc::sync_channel(1);
    let (resume_tx, resume_rx) = mpsc::sync_channel(1);
    let block =
        Arc::new(Block { entered: entered_tx, resume: Mutex::new(resume_rx) });
    let weak = Arc::downgrade(&block);
    let waker = Waker::from(block.clone());
    let (owner, waiters) = waiting();
    let mut imported = unsafe {
        reovim_sdk::ImportedCaller::new(owner.raw(), Rc::new(()), LIMITS, 8)
    }
    .unwrap();
    let caller = imported.caller();
    assert_eq!(
        caller.result(1, &mut Context::from_waker(&waker)),
        Ok(Status::Pending)
    );
    drop(waker);
    drop(block);
    let relay = waiters.lock().unwrap()[0].clone();
    let thread = std::thread::spawn(move || relay.wake());
    entered_rx.recv_timeout(std::time::Duration::from_secs(5)).unwrap();
    let closing = imported.close();
    resume_tx.send(()).unwrap();
    thread.join().unwrap();
    assert_eq!(closing, Err(Error::BUSY));
    assert_eq!(caller.reserve(), Err(Error::CLOSED));
    assert_eq!(caller.snapshot(1), Err(Error::CLOSED));
    assert_eq!(caller.release(1), Err(Error::CLOSED));
    assert_eq!(caller.cancel(1), Err(Error::CLOSED));
    assert_eq!(caller.adopt(1), Err(Error::CLOSED));
    assert_eq!(caller.reconcile(1), Err(Error::CLOSED));
    imported.close().unwrap();
    assert!(weak.upgrade().is_none());
    drop(owner);
    assert_eq!(imported.caller().reserve(), Err(Error::CLOSED));
    imported.close().unwrap();
    drop(imported);
    assert_eq!(caller.snapshot(1), Err(Error::CLOSED));
    waiters.lock().unwrap().pop().unwrap().wake();
}

#[test]
fn imported_caller_close_cannot_expire_an_active_native_reservation() {
    use std::cell::RefCell;
    let scope = Rc::new(RefCell::new(None::<reovim_sdk::ImportedCaller>));
    let origin = Caller::new(
        scope.clone(),
        CallerOps {
            reserve: |scope| {
                let mut guard = scope.borrow_mut();
                assert_eq!(guard.as_mut().unwrap().close(), Err(Error::BUSY));
                Ok(1)
            },
            submit: |_, _| Err(Error::UNSUPPORTED),
            result: |_, _, _| Err(Error::UNSUPPORTED),
            cancel: |_, _| Err(Error::UNSUPPORTED),
            reconcile: |_, _| Err(Error::UNSUPPORTED),
            adopt: |_, _| Err(Error::UNSUPPORTED),
            release: |_, _| Err(Error::UNSUPPORTED),
        },
    );
    let owner = ExportedCaller::new(origin, LIMITS, 8).unwrap();
    let imported = unsafe {
        reovim_sdk::ImportedCaller::new(owner.raw(), Rc::new(()), LIMITS, 8)
    }
    .unwrap();
    let caller = imported.caller();
    *scope.borrow_mut() = Some(imported);
    assert_eq!(caller.reserve(), Ok(1));
    assert_eq!(caller.reserve(), Err(Error::CLOSED));
    let mut imported = scope.borrow_mut().take().unwrap();
    imported.close().unwrap();
    drop(owner);
    assert_eq!(caller.reserve(), Err(Error::CLOSED));
}
