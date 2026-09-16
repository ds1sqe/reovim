use alloc::{boxed::Box, rc::Rc, vec::Vec};
use core::{cell::Cell, ffi::c_void, task::Context};
use reovim_kernel_interface::{
    Error, Outcome, abi,
    call::Cancellation,
    caller::{Caller, Status},
    declaration::ValueLimits,
};

use super::{State, checked_error, wake};

#[derive(Clone, Copy, Eq, PartialEq)]
enum Phase {
    Waiting,
    Ready,
    Unknown,
    Final,
}

pub(super) struct View {
    caller: Caller,
    id: u64,
    limits: ValueLimits,
    views: Rc<Cell<usize>>,
    outstanding: Rc<Cell<bool>>,
    observed: Option<Rc<Outcome>>,
    phase: Phase,
    signals: Vec<wake::Scoped>,
    signal_capacity: usize,
    closing: bool,
}

impl View {
    pub(super) fn new(state: &State, id: u64) -> Result<Box<Self>, Error> {
        if state.views.get() == state.capacity {
            return Err(Error::PRESSURE);
        }
        let view = Box::new(Self {
            caller: state.caller.clone(),
            id,
            limits: state.limits,
            views: state.views.clone(),
            outstanding: Rc::new(Cell::new(false)),
            observed: None,
            phase: Phase::Waiting,
            signals: Vec::new(),
            signal_capacity: state.capacity,
            closing: false,
        });
        state.views.set(state.views.get() + 1);
        Ok(view)
    }
}

impl Drop for View {
    fn drop(&mut self) {
        self.views.set(self.views.get() - 1);
    }
}

static OPS: abi::PendingOps = abi::PendingOps {
    bytes: size_of::<abi::PendingOps>(),
    poll: Some(poll),
    take: Some(take),
    cancel: Some(cancel),
    reconcile: Some(reconcile),
    finish: Some(finish),
};

pub(super) fn into_raw(view: Box<View>) -> abi::Pending {
    abi::Pending { context: Box::into_raw(view).cast(), ops: &raw const OPS }
}

unsafe fn view<'a>(context: *mut c_void) -> Result<&'a mut View, Error> {
    if context.is_null() {
        return Err(Error::INVALID);
    }
    // SAFETY: into_raw transfers this unique Box<View>; native callbacks are
    // serialized on the executor thread and finish has not consumed it.
    Ok(unsafe { &mut *context.cast::<View>() })
}

unsafe extern "C" fn poll(context: *mut c_void, signal: abi::Wake) -> u32 {
    // SAFETY: The native caller retains this view and serializes operations.
    let Ok(view) = (unsafe { view(context) }) else { return 1 };
    if view.closing || view.phase != Phase::Waiting {
        return 1;
    }
    // SAFETY: A nonempty native signal lends live, thread-safe callbacks.
    // observe checks its slots and retains it before registering wake interest.
    let status = unsafe { observe(view, signal) };
    let outcome = match status {
        Ok(Status::Pending | Status::Reserved) => return 0,
        Ok(Status::Outcome(outcome)) => outcome,
        Err(error) => Rc::new(Outcome::Unknown {
            error: checked_error(error),
            progress: None,
        }),
    };
    view.observed = Some(outcome);
    view.phase = Phase::Ready;
    1
}

unsafe fn observe(view: &mut View, signal: abi::Wake) -> Result<Status, Error> {
    if wake::absent(signal) {
        return view.caller.snapshot(view.id);
    }
    let index = view.signals.iter().position(|old| wake::same(old.raw, signal));
    let index = if let Some(index) = index {
        index
    } else {
        if view.signals.len() == view.signal_capacity {
            return Err(Error::PRESSURE);
        }
        view.signals.try_reserve(1).map_err(|_| Error::PRESSURE)?;
        // SAFETY: The caller lends signal; Scoped retains its foreign
        // reference until view finish disconnects all registered relays.
        view.signals.push(unsafe { wake::Scoped::new(signal)? });
        view.signals.len() - 1
    };
    let waker = view.signals[index].waker();
    view.caller.result(view.id, &mut Context::from_waker(&waker))
}

unsafe extern "C" fn take(
    context: *mut c_void,
    output: *mut abi::Outcome,
) -> u32 {
    if output.is_null() {
        return Error::INVALID.code.0;
    }
    // SAFETY: The native caller retains a serialized, unfinished view.
    let result = unsafe { view(context) }.and_then(transfer);
    match result {
        Ok(value) => {
            // SAFETY: The caller supplies writable Outcome storage; transfer
            // has reserved its separate owner before this successful write.
            unsafe { output.write(value) };
            0
        }
        Err(error) => checked_error(error).code.0,
    }
}

fn transfer(view: &mut View) -> Result<abi::Outcome, Error> {
    if view.closing {
        return Err(Error::CLOSED);
    }
    if view.phase != Phase::Ready {
        return Err(if view.phase == Phase::Waiting {
            Error::NOT_READY
        } else {
            Error::CLOSED
        });
    }
    let output = crate::outcome_export::export(
        view.observed.as_ref().unwrap().clone(),
        0,
        &mut None,
        view.outstanding.clone(),
        view.limits,
    )?;
    view.phase = if output.status == 2 { Phase::Unknown } else { Phase::Final };
    Ok(output)
}

unsafe extern "C" fn finish(context: *mut c_void) -> u32 {
    // SAFETY: A serialized finish may inspect this unconsumed view.
    let Ok(view) = (unsafe { view(context) }) else {
        return Error::INVALID.code.0;
    };
    if view.outstanding.get() {
        return Error::BUSY.code.0;
    }
    view.closing = true;
    for signal in &mut view.signals {
        if let Err(error) = signal.close() {
            return error.code.0;
        }
    }
    // SAFETY: No taken owner is outstanding. This consumes the unique box and
    // invalidates native context; original execution stays with Caller/Runtime.
    drop(unsafe { Box::from_raw(context.cast::<View>()) });
    0
}

unsafe extern "C" fn cancel(context: *mut c_void) -> u32 {
    // SAFETY: Native callers serialize this live view's control callbacks.
    match unsafe { view(context) }.and_then(|view| view.caller.cancel(view.id))
    {
        Ok(Cancellation::Accepted) => 0,
        Ok(Cancellation::TooLate) => 1,
        Err(error) => checked_error(error).code.0,
    }
}

unsafe extern "C" fn reconcile(context: *mut c_void) -> u32 {
    // SAFETY: Native callers serialize this live view's control callbacks.
    let result = unsafe { view(context) }.and_then(|view| {
        if view.closing || view.phase != Phase::Unknown {
            return Err(Error::CLOSED);
        }
        if view.outstanding.get() {
            return Err(Error::BUSY);
        }
        view.caller.reconcile(view.id)?;
        view.observed = None;
        view.phase = Phase::Waiting;
        Ok(())
    });
    result.map_or_else(|error| checked_error(error).code.0, |()| 0)
}
