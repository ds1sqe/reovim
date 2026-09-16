use alloc::rc::Rc;
use core::{
    cell::RefCell,
    task::{Poll, Waker},
};
use reovim_kernel_interface::{
    Error, Outcome, Pending,
    call::{Cancellation, Completion, PendingOps},
};

pub(super) struct Request {
    pub admitted: bool,
    outcome: Option<Outcome>,
    taken: bool,
    wake: Option<Waker>,
}
impl Request {
    pub const fn new() -> Self {
        Self { admitted: false, outcome: None, taken: false, wake: None }
    }
    pub fn completed(outcome: Outcome) -> Rc<RefCell<Self>> {
        Rc::new(RefCell::new(Self { outcome: Some(outcome), ..Self::new() }))
    }
    pub const fn done(&self) -> bool {
        self.outcome.is_some() || self.taken
    }
}
pub(super) fn complete(request: &Rc<RefCell<Request>>, outcome: Outcome) {
    let mut request = request.borrow_mut();
    request.outcome = Some(outcome);
    let wake = request.wake.take();
    drop(request);
    if let Some(wake) = wake {
        wake.wake();
    }
}
pub(super) fn pending(request: Rc<RefCell<Request>>) -> Pending {
    Pending::new(
        request,
        PendingOps {
            poll: |request, cx| {
                let mut request = request.borrow_mut();
                if request.outcome.is_some() {
                    return Ok(Poll::Ready(()));
                }
                if request.taken {
                    return Err(Error::CLOSED);
                }
                request.wake = Some(cx.waker().clone());
                Ok(Poll::Pending)
            },
            take: |request| {
                let mut request = request.borrow_mut();
                let outcome = request.outcome.take().ok_or(Error::NOT_READY)?;
                request.taken = true;
                Ok(Completion { outcome, activity: 0, owner: None })
            },
            cancel: |request| {
                let request_state = request.borrow();
                if request_state.admitted || request_state.done() {
                    return Ok(Cancellation::TooLate);
                }
                drop(request_state);
                complete(request, Outcome::Failed(Error::CANCELLED));
                Ok(Cancellation::Accepted)
            },
            reconcile: |_| Err(Error::UNSUPPORTED),
            finish: |request| {
                if !request.borrow().done() {
                    return Err(Error::BUSY);
                }
                request.borrow_mut().wake = None;
                Ok(())
            },
        },
    )
}
