use crate::{
    caller_export::{checked_error, wake},
    outcome_export,
};
use alloc::{boxed::Box, rc::Rc};
use core::{
    any::Any,
    cell::Cell,
    ffi::c_void,
    task::{Context, Poll, Waker},
};
use reovim_kernel_interface::{
    Error, Outcome, Pending, abi, call::Cancellation, declaration::ValueLimits,
};

#[derive(Clone, Copy, Eq, PartialEq)]
enum Phase {
    Waiting,
    Ready,
    Unknown,
    Final,
    Fenced,
}

struct Buffered {
    outcome: Rc<Outcome>,
    activity: u8,
    cleanup: Option<Box<dyn Any>>,
}

struct State {
    pending: Pending,
    limits: ValueLimits,
    phase: Phase,
    buffered: Option<Buffered>,
    fault: Option<Error>,
    outstanding: Rc<Cell<bool>>,
    signal: Option<(abi::Wake, Waker)>,
    _owner: Option<Box<dyn Any>>,
}

/// Transfers Rust provider request custody into the native pending ABI.
///
/// For example, a Rust provider's native begin callback returns this record.
/// Native calls must serialize, retain adapter code, release each taken outcome
/// once, and finish until OK before discarding the context. Failures retain
/// the work and its owners.
#[must_use]
pub fn export_pending(pending: Pending, limits: ValueLimits) -> abi::Pending {
    export_owned(pending, limits, None)
}

pub fn export_owned(
    pending: Pending,
    limits: ValueLimits,
    owner: Option<Box<dyn Any>>,
) -> abi::Pending {
    let state = Box::new(State {
        pending,
        limits,
        phase: Phase::Waiting,
        buffered: None,
        fault: None,
        outstanding: Rc::new(Cell::new(false)),
        signal: None,
        _owner: owner,
    });
    abi::Pending { context: Box::into_raw(state).cast(), ops: &raw const OPS }
}

static OPS: abi::PendingOps = abi::PendingOps {
    bytes: size_of::<abi::PendingOps>(),
    poll: Some(poll),
    take: Some(take),
    cancel: Some(cancel),
    reconcile: Some(reconcile),
    finish: Some(finish),
};

unsafe fn state<'a>(context: *mut c_void) -> Result<&'a mut State, Error> {
    if context.is_null() {
        return Err(Error::INVALID);
    }
    // SAFETY: export_pending transferred this unique Box. Native callers
    // serialize callbacks and retain its context until finish returns OK.
    Ok(unsafe { &mut *context.cast::<State>() })
}

unsafe extern "C" fn poll(context: *mut c_void, signal: abi::Wake) -> u32 {
    // SAFETY: The caller lends one serialized live pending context.
    let Ok(state) = (unsafe { state(context) }) else { return 1 };
    let closing = state.phase == Phase::Fenced;
    if state.phase != Phase::Waiting && !closing {
        return 1;
    }
    // SAFETY: A supplied native signal has live thread-safe callbacks, retained
    // before storing a Waker. Empty signals select an unregistered observation.
    let result = unsafe { observe(state, signal) };
    if closing {
        return u32::from(!matches!(result, Ok(Poll::Pending)));
    }
    match result {
        Ok(Poll::Pending) => return 0,
        Ok(Poll::Ready(())) => {}
        Err(error) => state.fault = Some(checked_error(error)),
    }
    state.phase = Phase::Ready;
    1
}

unsafe fn observe(
    state: &mut State,
    signal: abi::Wake,
) -> Result<Poll<()>, Error> {
    if wake::absent(signal) {
        return state.pending.poll(&mut Context::from_waker(Waker::noop()));
    }
    if !state.signal.as_ref().is_some_and(|(old, _)| wake::same(*old, signal)) {
        // SAFETY: poll lends the signal; import owns a foreign reference before
        // this assignment. Its last Waker releases that reference.
        state.signal = Some((signal, unsafe { wake::import(signal)? }));
    }
    let (_, signal) = state.signal.as_ref().unwrap();
    state.pending.poll(&mut Context::from_waker(signal))
}

unsafe extern "C" fn take(
    context: *mut c_void,
    output: *mut abi::Outcome,
) -> u32 {
    if output.is_null() {
        return Error::INVALID.code.0;
    }
    // SAFETY: The caller supplies a serialized live pending context.
    let result = unsafe { state(context) }.and_then(transfer);
    match result {
        Ok(value) => {
            // SAFETY: The native caller supplies writable aligned output.
            // transfer retained spans and cleanup owners before writing.
            unsafe { output.write(value) };
            0
        }
        Err(error) => checked_error(error).code.0,
    }
}

fn transfer(state: &mut State) -> Result<abi::Outcome, Error> {
    if state.phase != Phase::Ready {
        return Err(if state.phase == Phase::Waiting {
            Error::NOT_READY
        } else {
            Error::CLOSED
        });
    }
    if state.buffered.is_none() {
        let completion = match state.fault.take() {
            Some(error) => reovim_kernel_interface::call::Completion {
                outcome: Outcome::Unknown { error, progress: None },
                activity: 0,
                owner: None,
            },
            None => state.pending.take()?,
        };
        state.buffered = Some(Buffered {
            outcome: Rc::new(completion.outcome),
            activity: completion.activity,
            cleanup: completion.owner,
        });
    }
    let buffered = state.buffered.as_mut().unwrap();
    let output = outcome_export::export(
        buffered.outcome.clone(),
        buffered.activity,
        &mut buffered.cleanup,
        state.outstanding.clone(),
        state.limits,
    )?;
    state.phase =
        if output.status == 2 { Phase::Unknown } else { Phase::Final };
    state.buffered = None;
    Ok(output)
}

unsafe extern "C" fn cancel(context: *mut c_void) -> u32 {
    // SAFETY: The caller serializes control on this unfinished context.
    let result = unsafe { state(context) }.and_then(|s| {
        if s.phase == Phase::Fenced {
            return Err(Error::CLOSED);
        }
        s.pending.cancel()
    });
    match result {
        Ok(Cancellation::Accepted) => 0,
        Ok(Cancellation::TooLate) => 1,
        Err(error) => checked_error(error).code.0,
    }
}

unsafe extern "C" fn reconcile(context: *mut c_void) -> u32 {
    // SAFETY: The caller serializes control on this unfinished context.
    let result = unsafe { state(context) }.and_then(|s| {
        if s.phase != Phase::Unknown {
            return Err(Error::CLOSED);
        }
        if s.outstanding.get() {
            return Err(Error::BUSY);
        }
        s.pending.reconcile()?;
        s.phase = Phase::Waiting;
        Ok(())
    });
    result.map_or_else(|error| checked_error(error).code.0, |()| 0)
}

unsafe extern "C" fn finish(context: *mut c_void) -> u32 {
    // SAFETY: The caller serializes cleanup and has not consumed this context.
    let result = unsafe { state(context) }.and_then(|s| {
        // UNKNOWN still needs reconciliation; cleanup cannot fence that path.
        if s.outstanding.get() || s.phase == Phase::Unknown {
            return Err(Error::BUSY);
        }
        s.phase = Phase::Fenced;
        s.buffered = None;
        s.pending.finish()
    });
    if let Err(error) = result {
        return checked_error(error).code.0;
    }
    // SAFETY: No taken result remains, provider finish succeeded, and this
    // consumes the unique exported Box exactly once. Native context expires.
    drop(unsafe { Box::from_raw(context.cast::<State>()) });
    0
}
