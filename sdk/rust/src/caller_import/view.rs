use alloc::rc::Rc;
use core::{
    any::Any,
    ptr,
    task::{Context, Waker},
};
use reovim_kernel_interface::{
    Error, Outcome, abi, caller::Status, declaration::ValueLimits,
};

use crate::{pending, raw};

pub(super) struct View {
    raw: abi::Pending,
    ops: Option<abi::PendingOps>,
    owner: Option<Rc<dyn Any>>,
    limits: ValueLimits,
    closed: bool,
    cached: Option<Rc<Outcome>>,
    failure: Option<Error>,
    signal: Option<(Waker, pending::wake::Export)>,
    quarantine: Option<abi::Outcome>,
}

impl View {
    pub(super) unsafe fn new(
        raw: abi::Pending,
        owner: Rc<dyn Any>,
        limits: ValueLimits,
        fault: bool,
    ) -> Self {
        // SAFETY: The native caller retains valid table storage under owner.
        // Header and callback checks precede invocation; faults use no table.
        let ops = if fault { None } else { unsafe { table(raw.ops).ok() } };
        Self {
            raw,
            ops,
            owner: Some(owner),
            limits,
            closed: false,
            cached: None,
            failure: None,
            signal: None,
            quarantine: None,
        }
    }

    pub(super) const fn finished(&self) -> bool {
        self.closed
    }

    pub(super) const fn faulted(&self) -> bool {
        self.ops.is_none()
    }

    pub(super) fn observe(
        &mut self,
        context: Option<&mut Context<'_>>,
    ) -> Result<Status, Error> {
        let Some(ops) = self.ops else {
            return Ok(fault());
        };
        if let Some(outcome) = self.cached.clone() {
            let _ = self.finish();
            return Ok(Status::Outcome(outcome));
        }
        if let Some(error) = self.failure {
            let _ = self.finish();
            return Err(error);
        }
        let wake = signal(&mut self.signal, context);
        // SAFETY: A checked live caller-view table is retained by owner. Poll
        // is serialized and only observes; optional wake has retained storage.
        match unsafe { ops.poll.unwrap()(self.raw.context, wake) } {
            0 => Ok(Status::Pending),
            1 => self.take(),
            _ => {
                self.ops = None;
                Ok(fault())
            }
        }
    }

    fn take(&mut self) -> Result<Status, Error> {
        let mut output = empty_outcome();
        // SAFETY: Ready permits one transfer into this initialized, writable
        // slot. The checked callback and original context remain owned.
        let status = unsafe {
            self.ops.unwrap().take.unwrap()(self.raw.context, &raw mut output)
        };
        if status != 0 {
            return self.failed_take(status, output);
        }
        if output.release.is_none() {
            self.quarantine = Some(output);
            self.ops = None;
            return Ok(fault());
        }
        let owner =
            Taken { output, _code: self.owner.as_ref().unwrap().clone() };
        // SAFETY: A successful take transferred an owner with a checked release
        // callback. It retains immutable spans through the bounded value copy.
        let decoded = unsafe { pending::decode(&owner.output, self.limits) };
        drop(owner);
        let (outcome, _) = match decoded {
            Ok(value) => value,
            Err(error) => {
                self.failure = Some(error);
                let _ = self.finish();
                return Err(error);
            }
        };
        let outcome = Rc::new(outcome);
        self.cached = Some(outcome.clone());
        let _ = self.finish();
        Ok(Status::Outcome(outcome))
    }

    fn failed_take(
        &mut self,
        status: u32,
        output: abi::Outcome,
    ) -> Result<Status, Error> {
        if changed(&output) {
            self.quarantine = Some(output);
            self.ops = None;
            return Ok(fault());
        }
        Err(Error::new(status))
    }

    pub(super) fn finish(&mut self) -> Result<(), Error> {
        if self.closed {
            return Ok(());
        }
        let ops = self.ops.ok_or(Error::INCOMPLETE)?;
        // SAFETY: Caller-view finish closes only delivery ownership. Every
        // transferred output owner was released before reaching this call.
        let status = unsafe { ops.finish.unwrap()(self.raw.context) };
        if status != 0 {
            return Err(Error::new(status));
        }
        self.closed = true;
        self.signal = None;
        Ok(())
    }
}

impl Drop for View {
    fn drop(&mut self) {
        if self.finish().is_err() {
            // Faulted native view ownership cannot authorize unloading its
            // context or code. Abandoned import retains the supplied owner.
            core::mem::forget(self.owner.take());
        }
    }
}

struct Taken {
    output: abi::Outcome,
    _code: Rc<dyn Any>,
}

impl Drop for Taken {
    fn drop(&mut self) {
        // SAFETY: Successful take supplied this unique release/context pair;
        // _code keeps native code and allocator alive until it returns.
        unsafe { self.output.release.unwrap()(self.output.owner) };
    }
}

unsafe fn table(
    pointer: *const abi::PendingOps,
) -> Result<abi::PendingOps, Error> {
    // SAFETY: Import's caller retains the aligned immutable native header and
    // full table when its advertised extent matches. raw checks that extent.
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

fn signal(
    cached: &mut Option<(Waker, pending::wake::Export)>,
    context: Option<&mut Context<'_>>,
) -> abi::Wake {
    let Some(context) = context else {
        return abi::Wake {
            context: ptr::null_mut(),
            wake: None,
            retain: None,
            release: None,
        };
    };
    if !cached.as_ref().is_some_and(|(old, _)| old.will_wake(context.waker())) {
        let waker = context.waker().clone();
        *cached = Some((waker.clone(), pending::wake::Export::new(waker)));
    }
    cached.as_ref().unwrap().1.0
}

fn fault() -> Status {
    Status::Outcome(Rc::new(Outcome::Unknown {
        error: Error::PROVIDER,
        progress: None,
    }))
}

pub(super) const fn empty() -> abi::Pending {
    abi::Pending { context: ptr::null_mut(), ops: ptr::null() }
}

fn empty_outcome() -> abi::Outcome {
    abi::Outcome {
        status: u32::MAX,
        activity: 0,
        error: abi::Error::default(),
        value: raw::empty(),
        owner: ptr::null_mut(),
        release: None,
    }
}

fn changed(output: &abi::Outcome) -> bool {
    output.status != u32::MAX
        || output.activity != 0
        || output.error != abi::Error::default()
        || output.release.is_some()
        || !output.owner.is_null()
        || !output.value.data.ptr.is_null()
        || output.value.data.len != 0
        || !output.value.refs.ptr.is_null()
        || output.value.refs.len != 0
}
