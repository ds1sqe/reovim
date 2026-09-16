use super::staging::Stage;
use crate::engine::opening::Reservation;
use alloc::rc::Rc;
use core::{
    cell::RefCell,
    task::{Context, Poll},
};
use reovim_kernel_interface::{
    Error, Outcome, Pending, Value,
    call::{Cancellation, Completion, PendingOps},
};

pub(super) fn begin(
    native: Pending,
    stage: Rc<RefCell<Stage>>,
    opening: Reservation,
) -> Pending {
    Pending::new(
        Driver {
            native: Some(native),
            stage,
            opening,
            phase: Phase::Create,
            outcome: None,
            activity: 0,
            taken: false,
            quarantine: None,
        },
        PendingOps {
            poll: |state, cx| Ok(state.poll(cx)),
            take: Driver::take,
            cancel: Driver::cancel,
            reconcile: Driver::reconcile,
            finish: Driver::finish,
        },
    )
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Phase {
    Create,
    Prepared,
    Open,
    Retry,
    Fenced,
}

struct Driver {
    native: Option<Pending>,
    stage: Rc<RefCell<Stage>>,
    opening: Reservation,
    phase: Phase,
    outcome: Option<Outcome>,
    activity: u8,
    taken: bool,
    quarantine: Option<Completion>,
}

impl Driver {
    fn poll(&mut self, cx: &mut Context<'_>) -> Poll<()> {
        if self.outcome.is_some() {
            return Poll::Ready(());
        }
        if self.phase == Phase::Prepared {
            return self.prepared(cx);
        }
        let native = self.native.as_mut().unwrap();
        match native.poll(cx) {
            Ok(Poll::Pending) => return Poll::Pending,
            Ok(Poll::Ready(())) => {}
            Err(_) => return self.fault(None),
        }
        let Ok(completion) = native.take() else { return self.fault(None) };
        if !valid(&completion, self.phase) {
            return self.fault(Some(completion));
        }
        if self.phase == Phase::Create {
            self.activity = completion.activity;
        }
        match &completion.outcome {
            Outcome::Ok(_) => self.success(completion, cx),
            Outcome::Failed(error) if self.phase == Phase::Open => {
                self.failed_open(*error)
            }
            _ => {
                self.outcome = Some(completion.outcome);
                Poll::Ready(())
            }
        }
    }

    fn success(
        &mut self,
        completion: Completion,
        cx: &Context<'_>,
    ) -> Poll<()> {
        if self.phase == Phase::Create {
            self.phase = Phase::Prepared;
            cx.waker().wake_by_ref();
            return Poll::Pending;
        }
        if self.opening.selected().is_err() {
            return self.fault(Some(completion));
        }
        self.outcome = Some(Outcome::Ok(Value::unit()));
        Poll::Ready(())
    }

    fn prepared(&mut self, cx: &Context<'_>) -> Poll<()> {
        if self.native.as_mut().is_some_and(|native| native.finish().is_err()) {
            cx.waker().wake_by_ref();
            return Poll::Pending;
        }
        self.native = None;
        let subject = self.stage.borrow().subject();
        let Ok(subject) = subject else { return self.fault(None) };
        self.phase = Phase::Open;
        match self.opening.begin(&subject) {
            Ok(Some(native)) => {
                self.native = Some(native);
                cx.waker().wake_by_ref();
                Poll::Pending
            }
            Ok(None) => {
                self.outcome = Some(Outcome::Ok(Value::unit()));
                Poll::Ready(())
            }
            Err(error) => self.failed_open(error),
        }
    }

    fn failed_open(&mut self, error: Error) -> Poll<()> {
        // Creation already succeeded. Retain its key/provider and recover
        // initialization; reporting a clean failure would hide the new name.
        self.opening.fence();
        self.phase = Phase::Retry;
        self.outcome = Some(Outcome::Unknown { error, progress: None });
        Poll::Ready(())
    }

    fn fault(&mut self, completion: Option<Completion>) -> Poll<()> {
        self.quarantine = completion;
        self.opening.fence();
        self.phase = Phase::Fenced;
        self.outcome =
            Some(Outcome::Unknown { error: Error::PROVIDER, progress: None });
        Poll::Ready(())
    }

    fn take(&mut self) -> Result<Completion, Error> {
        if self.taken {
            return Err(Error::CLOSED);
        }
        let outcome = self.outcome.clone().ok_or(Error::NOT_READY)?;
        self.taken = true;
        Ok(Completion { outcome, activity: self.activity, owner: None })
    }

    fn cancel(&mut self) -> Result<Cancellation, Error> {
        if self.phase != Phase::Create {
            return Ok(Cancellation::TooLate);
        }
        self.native.as_mut().unwrap().cancel()
    }

    fn reconcile(&mut self) -> Result<(), Error> {
        if self.phase == Phase::Fenced {
            return Err(Error::PROVIDER);
        }
        if self.phase == Phase::Retry {
            self.retry()?;
        } else {
            self.native.as_mut().ok_or(Error::CLOSED)?.reconcile()?;
        }
        self.outcome = None;
        self.taken = false;
        Ok(())
    }

    fn retry(&mut self) -> Result<(), Error> {
        if let Some(native) = &mut self.native {
            native.finish()?;
        }
        self.native = None;
        self.opening.reset()?;
        self.phase = Phase::Prepared;
        Ok(())
    }

    fn finish(&mut self) -> Result<(), Error> {
        if self
            .outcome
            .as_ref()
            .is_none_or(|outcome| matches!(outcome, Outcome::Unknown { .. }))
        {
            return Err(Error::BUSY);
        }
        self.opening.fence();
        if let Some(native) = &mut self.native {
            native.finish()?;
        }
        self.native = None;
        Ok(())
    }
}

fn valid(completion: &Completion, phase: Phase) -> bool {
    let activity = if phase == Phase::Create { 2 } else { 0 };
    if completion.activity & !activity != 0 {
        return false;
    }
    match &completion.outcome {
        Outcome::Ok(value) => {
            value.data.is_empty() && value.references.is_empty()
        }
        Outcome::Failed(error) => valid_error(*error),
        Outcome::Unknown { error, progress } => {
            valid_error(*error) && progress.is_none()
        }
    }
}

fn valid_error(error: Error) -> bool {
    error.code.0 != 0 && error.id.is_none_or(|id| id.serial != 0)
}
