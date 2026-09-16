use super::{Registry, Stage};
use crate::engine::{Entry, Implementation, opening::Reservation, reserve};
use alloc::{rc::Rc, vec::Vec};
use core::{
    cell::RefCell,
    task::{Context, Poll},
};
use reovim_kernel_interface::{
    Error, Outcome, Pending, ResourceId, Value, call::Completion,
};

struct Slot {
    opening: Reservation,
    selected: Option<Rc<Implementation>>,
    identity: Option<ResourceId>,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Phase {
    Start,
    Waiting,
    Finishing,
    Retry,
    Unknown,
    Fenced,
}

pub(super) struct Batch {
    slots: Vec<Slot>,
    index: usize,
    phase: Phase,
    pending: Option<Pending>,
    quarantine: Option<Completion>,
}

impl Batch {
    pub fn reserve(
        registry: &mut Registry,
        count: usize,
    ) -> Result<Self, Error> {
        let mut slots = reserve(count)?;
        for opening in registry.opening.reserve_batch(count)? {
            slots.push(Slot { opening, selected: None, identity: None });
        }
        Ok(Self {
            slots,
            index: 0,
            phase: Phase::Start,
            pending: None,
            quarantine: None,
        })
    }

    pub fn unused(&self) {
        for slot in &self.slots {
            slot.opening.finish();
        }
    }

    pub fn selected(&self, identity: ResourceId) -> Option<Rc<Implementation>> {
        self.slots
            .iter()
            .find(|slot| slot.identity == Some(identity))?
            .selected
            .clone()
    }

    pub fn poll(
        &mut self,
        stage: &Rc<RefCell<Stage>>,
        cx: &mut Context<'_>,
    ) -> Option<Outcome> {
        let mut outcome = None;
        let mut progressed = true;
        while self.index < self.slots.len() && progressed && outcome.is_none() {
            let before = (self.index, self.phase);
            outcome = match self.phase {
                Phase::Start => self.begin(stage, cx),
                Phase::Waiting => self.wait(cx),
                Phase::Finishing => self.finish_one(cx),
                Phase::Retry | Phase::Unknown | Phase::Fenced => return None,
            };
            progressed = before != (self.index, self.phase);
        }
        if self.index == self.slots.len() {
            return Some(Outcome::Ok(Value::unit()));
        }
        outcome
    }

    fn begin(
        &mut self,
        stage: &Rc<RefCell<Stage>>,
        cx: &Context<'_>,
    ) -> Option<Outcome> {
        let subject = subject(&stage.borrow(), self.index);
        let Some(subject) = subject else {
            self.next(cx);
            return None;
        };
        self.slots[self.index].identity = Some(subject.identity);
        match self.slots[self.index].opening.begin(&subject) {
            Ok(Some(pending)) => {
                self.pending = Some(pending);
                self.phase = Phase::Waiting;
                cx.waker().wake_by_ref();
                None
            }
            Ok(None) => self.accept(cx),
            Err(error) => Some(self.failed(error)),
        }
    }

    fn wait(&mut self, cx: &mut Context<'_>) -> Option<Outcome> {
        let pending = self.pending.as_mut().unwrap();
        match pending.poll(cx) {
            Ok(Poll::Pending) => return None,
            Ok(Poll::Ready(())) => {}
            Err(_) => return Some(self.fault(None)),
        }
        let Ok(completion) = pending.take() else {
            return Some(self.fault(None));
        };
        if !valid(&completion) {
            return Some(self.fault(Some(completion)));
        }
        match completion.outcome {
            Outcome::Ok(_) => self.accept(cx),
            Outcome::Failed(error) => Some(self.failed(error)),
            outcome @ Outcome::Unknown { .. } => {
                self.phase = Phase::Unknown;
                Some(outcome)
            }
        }
    }

    fn accept(&mut self, cx: &Context<'_>) -> Option<Outcome> {
        let selected = self.slots[self.index].opening.selected();
        let Ok(selected) = selected else { return Some(self.fault(None)) };
        self.slots[self.index].selected = Some(selected);
        self.phase = Phase::Finishing;
        cx.waker().wake_by_ref();
        None
    }

    fn finish_one(&mut self, cx: &Context<'_>) -> Option<Outcome> {
        if self
            .pending
            .as_mut()
            .is_some_and(|pending| pending.finish().is_err())
        {
            cx.waker().wake_by_ref();
            return None;
        }
        self.pending = None;
        self.next(cx);
        None
    }

    fn next(&mut self, cx: &Context<'_>) {
        self.slots[self.index].opening.finish();
        self.index += 1;
        self.phase = Phase::Start;
        cx.waker().wake_by_ref();
    }

    fn failed(&mut self, error: Error) -> Outcome {
        self.slots[self.index].opening.fence();
        self.phase = Phase::Retry;
        Outcome::Unknown { error, progress: None }
    }

    fn fault(&mut self, completion: Option<Completion>) -> Outcome {
        self.quarantine = completion;
        self.slots[self.index].opening.fence();
        self.phase = Phase::Fenced;
        Outcome::Unknown { error: Error::PROVIDER, progress: None }
    }

    pub fn reconcile(&mut self) -> Result<(), Error> {
        match self.phase {
            Phase::Retry => {
                self.retry()?;
                self.phase = Phase::Start;
            }
            Phase::Unknown => {
                self.pending.as_mut().unwrap().reconcile()?;
                self.phase = Phase::Waiting;
            }
            Phase::Fenced => return Err(Error::PROVIDER),
            _ => return Err(Error::CLOSED),
        }
        Ok(())
    }

    fn retry(&mut self) -> Result<(), Error> {
        if let Some(pending) = &mut self.pending {
            pending.finish()?;
        }
        self.pending = None;
        self.slots[self.index].opening.reset()
    }

    pub fn finish(&mut self, installed: bool) -> Result<(), Error> {
        for slot in &mut self.slots {
            slot.selected = None;
            finish_slot(slot, installed)?;
        }
        Ok(())
    }
}

fn finish_slot(slot: &Slot, installed: bool) -> Result<(), Error> {
    if !installed {
        slot.opening.discard()?;
    }
    slot.opening.finish();
    Ok(())
}

fn subject(stage: &Stage, index: usize) -> Option<Rc<Entry>> {
    if index == 0 {
        return stage.primary.clone();
    }
    let auxiliary = &stage.auxiliary[index - 1];
    auxiliary.returned.then(|| auxiliary.handle.as_ref().unwrap().entry.clone())
}

fn valid(completion: &Completion) -> bool {
    if completion.activity != 0 {
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
