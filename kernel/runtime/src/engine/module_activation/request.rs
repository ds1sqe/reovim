use super::{ModuleProbe, Output};
use alloc::rc::Rc;
use core::{
    cell::RefCell,
    task::{Poll, Waker},
};
use reovim_kernel_interface::{
    Error, Outcome, Pending, ResourceId,
    call::{Cancellation, Completion, PendingOps},
};

pub(super) struct Request {
    pub package: ResourceId,
    pub probe: Option<ModuleProbe>,
    pub output: Option<Output>,
    pub cancelled: bool,
    outcome: Option<Outcome>,
    taken: bool,
    wake: Option<Waker>,
}
impl Request {
    pub const fn new(package: ResourceId, output: Output) -> Self {
        Self {
            package,
            probe: None,
            output: Some(output),
            cancelled: false,
            outcome: None,
            taken: false,
            wake: None,
        }
    }
    pub const fn done(&self) -> bool {
        self.outcome.is_some() || self.taken
    }
}
pub(super) fn complete(request: &Rc<RefCell<Request>>, outcome: Outcome) {
    let mut state = request.borrow_mut();
    state.outcome = Some(outcome);
    let wake = state.wake.take();
    drop(state);
    if let Some(wake) = wake {
        wake.wake();
    }
}
pub(super) fn pending(request: Rc<RefCell<Request>>) -> Pending {
    Pending::new(
        request,
        PendingOps {
            poll: |request, cx| {
                let mut state = request.borrow_mut();
                if state.outcome.is_some() {
                    return Ok(Poll::Ready(()));
                }
                if state.taken {
                    return Err(Error::CLOSED);
                }
                state.wake = Some(cx.waker().clone());
                Ok(Poll::Pending)
            },
            take: |request| {
                let mut state = request.borrow_mut();
                let outcome = state.outcome.take().ok_or(Error::NOT_READY)?;
                state.taken = true;
                Ok(Completion { outcome, activity: 0, owner: None })
            },
            cancel: |request| Ok(cancel(request)),
            reconcile: |_| Err(Error::UNSUPPORTED),
            finish: |request| {
                let mut state = request.borrow_mut();
                if !state.done() {
                    return Err(Error::BUSY);
                }
                state.output.take();
                state.wake = None;
                Ok(())
            },
        },
    )
}
fn cancel(request: &Rc<RefCell<Request>>) -> Cancellation {
    let mut state = request.borrow_mut();
    if state.done() {
        return Cancellation::TooLate;
    }
    if let Some(probe) = &state.probe {
        return probe.cancel();
    }
    state.cancelled = true;
    Cancellation::Accepted
}
