use super::{Source, materialize};
use crate::engine::opening::Reservation;
use core::task::{Context, Poll};
use reovim_kernel_interface::{
    Error, Outcome, Pending, Value,
    call::{Cancellation, Completion, PendingOps},
};

pub(super) fn begin(source: Source, opening: Reservation) -> Pending {
    let mut driver = Driver {
        source,
        opening,
        native: None,
        phase: Phase::Source,
        outcome: None,
        taken: false,
        fenced: false,
        quarantine: None,
    };
    match &driver.source {
        Source::Ready(_) => driver.open(),
        Source::Identity(work) => {
            driver.native = Some(materialize::pending(work.clone()));
        }
        Source::Path(path) => {
            driver.native =
                Some(super::super::traversal::pending(path.clone()));
        }
    }
    Pending::new(
        driver,
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
    Source,
    Prepared,
    Open,
}

struct Driver {
    source: Source,
    opening: Reservation,
    native: Option<Pending>,
    phase: Phase,
    outcome: Option<Outcome>,
    taken: bool,
    fenced: bool,
    quarantine: Option<Completion>,
}

impl Driver {
    fn open(&mut self) {
        self.phase = Phase::Open;
        let result =
            self.source.ready().and_then(|entry| self.opening.begin(&entry));
        match result {
            Ok(Some(pending)) => self.native = Some(pending),
            Ok(None) => self.outcome = Some(Outcome::Ok(Value::unit())),
            Err(error) => self.outcome = Some(Outcome::Failed(error)),
        }
    }

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
        if matches!(completion.outcome, Outcome::Ok(_)) {
            return self.success(completion, cx);
        }
        self.outcome = Some(completion.outcome);
        Poll::Ready(())
    }

    fn success(
        &mut self,
        completion: Completion,
        cx: &Context<'_>,
    ) -> Poll<()> {
        if self.phase == Phase::Source {
            self.phase = Phase::Prepared;
            cx.waker().wake_by_ref();
            return Poll::Pending;
        }
        if self.opening.selected().is_err() {
            return self.fault(Some(completion));
        }
        self.outcome = Some(completion.outcome);
        Poll::Ready(())
    }

    fn prepared(&mut self, cx: &Context<'_>) -> Poll<()> {
        if self.native.as_mut().unwrap().finish().is_err() {
            cx.waker().wake_by_ref();
            return Poll::Pending;
        }
        self.native = None;
        self.open();
        cx.waker().wake_by_ref();
        Poll::Pending
    }

    fn fault(&mut self, completion: Option<Completion>) -> Poll<()> {
        self.quarantine = completion;
        self.fenced = true;
        self.opening.fence();
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
        Ok(Completion { outcome, activity: 0, owner: None })
    }

    fn cancel(&mut self) -> Result<Cancellation, Error> {
        if self.phase == Phase::Prepared {
            self.outcome = Some(Outcome::Failed(Error::CANCELLED));
            return Ok(Cancellation::Accepted);
        }
        self.native.as_mut().map_or(Ok(Cancellation::TooLate), Pending::cancel)
    }

    fn reconcile(&mut self) -> Result<(), Error> {
        if self.fenced {
            return Err(Error::PROVIDER);
        }
        self.native.as_mut().ok_or(Error::CLOSED)?.reconcile()?;
        self.outcome = None;
        self.taken = false;
        Ok(())
    }

    fn finish(&mut self) -> Result<(), Error> {
        if self.fenced
            || self.outcome.as_ref().is_none_or(|outcome| {
                matches!(outcome, Outcome::Unknown { .. })
            })
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
    let activity = u8::from(phase == Phase::Source);
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
