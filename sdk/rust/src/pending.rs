use alloc::{boxed::Box, rc::Rc};
use core::{
    any::Any,
    ptr,
    task::{Context, Poll},
};
use reovim_kernel_interface::{
    Error, Outcome, Pending, abi,
    call::{Cancellation, Completion, PendingOps},
    declaration::ValueLimits,
};

use crate::{native::Instance, raw};

pub mod wake;

#[derive(Clone, Copy, Eq, PartialEq)]
enum Phase {
    Waiting,
    Ready,
    Unknown,
    Final,
    Fenced,
    FaultTaken,
    Finished,
}

struct Imported {
    pending: abi::Pending,
    ops: Option<abi::PendingOps>,
    instance: Option<Rc<Instance>>,
    request_owner: Option<Rc<dyn Any>>,
    quarantine: Option<Box<Taken>>,
    phase: Phase,
    limits: ValueLimits,
}

impl Drop for Imported {
    fn drop(&mut self) {
        if self.phase != Phase::Finished {
            // Runtime must keep unfinished work driven. If assembly abandons
            // it, freeing native code/context would create dangling callbacks.
            core::mem::forget(self.instance.take());
            core::mem::forget(self.request_owner.take());
            core::mem::forget(self.quarantine.take());
        }
    }
}

struct Taken {
    outcome: abi::Outcome,
    _instance: Rc<Instance>,
    _request_owner: Option<Rc<dyn Any>>,
}

impl Drop for Taken {
    fn drop(&mut self) {
        if let Some(release) = self.outcome.release {
            // SAFETY: Successful take transferred this unique owner and its
            // matching destructor. The instance/code lease outlives release.
            unsafe { release(self.outcome.owner) };
        }
    }
}

pub unsafe fn import(
    pending: abi::Pending,
    instance: Rc<Instance>,
    limits: ValueLimits,
    fault: bool,
    request_owner: Option<Rc<dyn Any>>,
) -> Pending {
    // SAFETY: A successful begin supplies the pending table under the retained
    // instance lease. table checks its extent/slots; faults invoke no table.
    let ops = if fault { None } else { unsafe { table(pending.ops).ok() } };
    let phase = if ops.is_some() { Phase::Waiting } else { Phase::Fenced };
    Pending::new(
        Imported {
            pending,
            ops,
            instance: Some(instance),
            request_owner,
            quarantine: None,
            phase,
            limits,
        },
        PendingOps { poll, take, cancel, reconcile, finish },
    )
}

unsafe fn table(
    pointer: *const abi::PendingOps,
) -> Result<abi::PendingOps, Error> {
    // SAFETY: Import's caller guarantees native header/storage validity;
    // raw::table checks the exact extent before copying the callback fields.
    let ops: abi::PendingOps = unsafe { raw::table(pointer)? };
    if ops.poll.is_none()
        || ops.take.is_none()
        || ops.cancel.is_none()
        || ops.finish.is_none()
    {
        return Err(Error::INVALID);
    }
    Ok(ops)
}

fn poll(
    state: &mut Imported,
    context: &mut Context<'_>,
) -> Result<Poll<()>, Error> {
    match state.phase {
        Phase::Fenced | Phase::Ready => return Ok(Poll::Ready(())),
        Phase::Waiting => {}
        _ => return Err(Error::CLOSED),
    }
    let wake = wake::Export::new(context.waker().clone());
    // SAFETY: Waiting retains an unfinished context and a checked poll slot.
    // Export owns wake through the callback; native retention uses its Arc ops.
    let status = unsafe {
        (state.ops.unwrap().poll.unwrap())(state.pending.context, wake.0)
    };
    match status {
        0 => Ok(Poll::Pending),
        1 => {
            state.phase = Phase::Ready;
            Ok(Poll::Ready(()))
        }
        _ => {
            state.phase = Phase::Fenced;
            Ok(Poll::Ready(()))
        }
    }
}

fn take(state: &mut Imported) -> Result<Completion, Error> {
    if state.phase == Phase::Fenced {
        state.phase = Phase::FaultTaken;
        return Ok(fault());
    }
    if state.phase != Phase::Ready {
        return Err(if state.phase == Phase::Waiting {
            Error::NOT_READY
        } else {
            Error::CLOSED
        });
    }
    let mut output = abi::Outcome {
        status: u32::MAX,
        activity: 0,
        error: abi::Error::default(),
        value: raw::empty(),
        owner: ptr::null_mut(),
        release: None,
    };
    // SAFETY: Ready permits one take on this live, serialized context. The
    // checked callback receives initialized writable output and pinned code.
    let status = unsafe {
        (state.ops.unwrap().take.unwrap())(
            state.pending.context,
            &raw mut output,
        )
    };
    let owner = Box::new(Taken {
        outcome: output,
        _instance: state.instance.as_ref().unwrap().clone(),
        _request_owner: state.request_owner.clone(),
    });
    if status != 0 || owner.outcome.release.is_none() {
        state.phase = Phase::FaultTaken;
        state.quarantine = Some(owner);
        return Ok(fault());
    }
    state.phase = match owner.outcome.status {
        0 | 1 => Phase::Final,
        2 => Phase::Unknown,
        _ => Phase::FaultTaken,
    };
    // SAFETY: Taken owns the native result spans until its matching release;
    // decoding copies them while both result storage and code remain alive.
    let decoded = unsafe { decode(&owner.outcome, state.limits) };
    Ok(if let Ok((outcome, activity)) = decoded {
        Completion { outcome, activity, owner: Some(owner) }
    } else {
        state.phase = Phase::FaultTaken;
        state.quarantine = Some(owner);
        fault()
    })
}

fn fault() -> Completion {
    Completion {
        outcome: Outcome::Unknown { error: Error::PROVIDER, progress: None },
        activity: 0,
        owner: None,
    }
}

pub unsafe fn decode(
    output: &abi::Outcome,
    limits: ValueLimits,
) -> Result<(Outcome, u8), Error> {
    if output.activity & !7 != 0 {
        return Err(Error::PROVIDER);
    }
    let outcome = match output.status {
        0 if output.error == abi::Error::default() => {
            // SAFETY: decode's caller retains the taken result owner while
            // raw::value bounds-checks and copies its initialized ABI spans.
            Outcome::Ok(unsafe { raw::value(output.value, limits)? })
        }
        1 if output.value.data.len == 0 && output.value.refs.len == 0 => {
            Outcome::Failed(raw::error(output.error)?)
        }
        2 if output.value.refs.len == 0 => Outcome::Unknown {
            error: raw::error(output.error)?,
            progress: if output.value.data.len == 0 {
                None
            } else {
                // SAFETY: The same taken owner retains this progress span;
                // the branch above already excluded returned reference slots.
                Some(unsafe { raw::value(output.value, limits)? })
            },
        },
        _ => return Err(Error::PROVIDER),
    };
    Ok((outcome, u8::try_from(output.activity).map_err(|_| Error::PROVIDER)?))
}

fn cancel(state: &mut Imported) -> Result<Cancellation, Error> {
    if matches!(state.phase, Phase::Fenced | Phase::FaultTaken) {
        return Ok(Cancellation::TooLate);
    }
    if state.phase == Phase::Finished {
        return Err(Error::CLOSED);
    }
    // SAFETY: Fenced/finished owners were excluded. This serialized call uses
    // a checked cancellation slot with its original context and code retained.
    match unsafe { (state.ops.unwrap().cancel.unwrap())(state.pending.context) }
    {
        0 => Ok(Cancellation::Accepted),
        1 => Ok(Cancellation::TooLate),
        _ => {
            state.phase = Phase::Fenced;
            Err(Error::PROVIDER)
        }
    }
}

fn reconcile(state: &mut Imported) -> Result<(), Error> {
    if state.phase != Phase::Unknown {
        return Err(Error::CLOSED);
    }
    let reconcile = state.ops.unwrap().reconcile.ok_or(Error::UNSUPPORTED)?;
    // SAFETY: A taken UNKNOWN permits reconciliation on this original live
    // context; the optional callback exists and its instance remains retained.
    let status = unsafe { reconcile(state.pending.context) };
    if status != 0 {
        return Err(Error::new(status));
    }
    state.phase = Phase::Waiting;
    Ok(())
}

fn finish(state: &mut Imported) -> Result<(), Error> {
    if state.phase == Phase::Finished {
        return Ok(());
    }
    if matches!(state.phase, Phase::Fenced | Phase::FaultTaken) {
        return Err(Error::BUSY);
    }
    // SAFETY: Only a checked unfinished context reaches this callback. OK
    // immediately marks it Finished, preventing any later native invocation.
    let status =
        unsafe { (state.ops.unwrap().finish.unwrap())(state.pending.context) };
    if status != 0 {
        return Err(Error::new(status));
    }
    state.phase = Phase::Finished;
    state.instance.take();
    state.request_owner.take();
    Ok(())
}
