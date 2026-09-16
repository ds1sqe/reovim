use super::{Completion, Context, Error, Pending, Poll, Rc, RefCell, Work};
use reovim_kernel_interface::call::{Cancellation, PendingOps};

struct Delivery {
    work: Rc<RefCell<Work>>,
    seen: Option<u64>,
}
pub(in crate::engine::namespace) fn pending(
    work: Rc<RefCell<Work>>,
) -> Pending {
    // Run begin in Task's admission-driving phase, which reports More. A
    // module driver gets another turn even if its newly queued work is Pending.
    let mut active = work.borrow_mut();
    if active.native.is_none() && active.outcome.is_none() {
        active.begin();
    }
    drop(active);
    Pending::new(
        Delivery { work, seen: None },
        PendingOps {
            poll,
            take,
            // Task cancels before begin; shared admitted work must drain.
            cancel: |_| Ok(Cancellation::TooLate),
            reconcile,
            finish,
        },
    )
}
fn poll(state: &mut Delivery, cx: &mut Context<'_>) -> Result<Poll<()>, Error> {
    Ok(state.work.try_borrow_mut().map_err(|_| Error::BUSY)?.poll(cx))
}
fn take(state: &mut Delivery) -> Result<Completion, Error> {
    let work = state.work.borrow();
    if state.seen == Some(work.generation) {
        return Err(Error::CLOSED);
    }
    let outcome = work.outcome.clone().ok_or(Error::NOT_READY)?;
    state.seen = Some(work.generation);
    Ok(Completion { outcome, activity: 1, owner: None })
}
fn reconcile(state: &mut Delivery) -> Result<(), Error> {
    let mut work = state.work.try_borrow_mut().map_err(|_| Error::BUSY)?;
    if state.seen != Some(work.generation) || work.outcome.is_none() {
        return Ok(());
    }
    if work.stage.borrow().closed {
        return Err(Error::PROVIDER);
    }
    work.native.as_mut().ok_or(Error::CLOSED)?.reconcile()?;
    work.outcome = None;
    Ok(())
}
fn finish(state: &mut Delivery) -> Result<(), Error> {
    state.work.try_borrow_mut().map_err(|_| Error::BUSY)?.finish()
}
