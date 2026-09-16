use super::{
    super::{Entry, Grant, Registry, access::Scope, codec},
    materialize::{self, Work},
};
use alloc::{
    boxed::Box,
    rc::{Rc, Weak},
    vec::Vec,
};
use core::{
    cell::RefCell,
    task::{Context, Poll},
};
use reovim_kernel_interface::{
    Error, Outcome, Pending, Value,
    call::{Cancellation, Completion, PendingOps},
};

mod admission;
pub(super) use admission::admit;

#[derive(Clone, Copy)]
enum Step {
    Lookup,
    Materialize,
}

pub(super) struct Traversal {
    registry: Weak<RefCell<Registry>>,
    names: Box<[u8]>,
    offset: usize,
    current: Rc<Entry>,
    owners: Vec<Rc<Entry>>,
    step: Step,
    native: Option<Pending>,
    work: Option<Rc<RefCell<Work>>>,
    observed: Option<Outcome>,
    outcome: Option<Outcome>,
    taken: bool,
    fenced: bool,
    quarantine: Option<Completion>,
    maximum: usize,
    publication: Option<Grant>,
    grants: Vec<Grant>,
    pub grant: Option<Grant>,
    pub ready: Option<Rc<Entry>>,
}

pub(super) fn pending(path: Rc<RefCell<Traversal>>) -> Pending {
    path.borrow_mut().begin();
    Pending::new(
        path,
        PendingOps {
            poll: |path, cx| Ok(path.borrow_mut().poll(cx)),
            take: |path| {
                let mut path = path.borrow_mut();
                if path.taken {
                    return Err(Error::CLOSED);
                }
                let outcome = path.outcome.clone().ok_or(Error::NOT_READY)?;
                path.taken = true;
                Ok(Completion { outcome, activity: 1, owner: None })
            },
            cancel: |_| Ok(Cancellation::TooLate),
            reconcile: |path| path.borrow_mut().reconcile(),
            finish: |path| path.borrow_mut().finish(),
        },
    )
}
impl Traversal {
    fn begin(&mut self) {
        let result = self.lookup();
        match result {
            Ok(pending) => self.native = Some(pending),
            Err(error) => self.outcome = Some(Outcome::Failed(error)),
        }
    }
    fn lookup(&self) -> Result<Pending, Error> {
        if self.current.is_closed() {
            return Err(Error::CLOSED);
        }
        let rest = &self.names[self.offset..];
        let end =
            rest.iter().position(|byte| *byte == b'/').unwrap_or(rest.len());
        self.current.implementation.provider.borrow_mut().lookup(&rest[..end])
    }
    fn poll(&mut self, cx: &mut Context<'_>) -> Poll<()> {
        if self.outcome.is_some() {
            return Poll::Ready(());
        }
        if self.observed.is_some() {
            self.advance(cx);
            return self.status();
        }
        if self.native.is_none() {
            self.begin();
            cx.waker().wake_by_ref();
            return self.status();
        }
        let native = self.native.as_mut().unwrap();
        match native.poll(cx) {
            Ok(Poll::Pending) => return Poll::Pending,
            Ok(Poll::Ready(())) => {}
            Err(_) => return self.fault(None),
        }
        let Ok(completion) = native.take() else {
            return self.fault(None);
        };
        if !valid(&completion, self.maximum, self.step) {
            return self.fault(Some(completion));
        }
        if matches!(completion.outcome, Outcome::Unknown { .. }) {
            self.outcome = Some(completion.outcome);
            return Poll::Ready(());
        }
        self.observed = Some(completion.outcome);
        // Cleanup and the next callback get their own driving turn.
        cx.waker().wake_by_ref();
        Poll::Pending
    }
    const fn status(&self) -> Poll<()> {
        if self.outcome.is_some() { Poll::Ready(()) } else { Poll::Pending }
    }
    fn advance(&mut self, cx: &Context<'_>) {
        if self.native.as_mut().unwrap().finish().is_err() {
            cx.waker().wake_by_ref();
            return;
        }
        self.native = None;
        let outcome = self.observed.take().unwrap();
        let Outcome::Ok(value) = outcome else {
            self.outcome = Some(outcome);
            return;
        };
        let result = match self.step {
            Step::Lookup => self.resolve(&value),
            Step::Materialize => {
                let ready = self.work.as_ref().unwrap().borrow().ready.clone();
                ready
                    .ok_or(Error::PROVIDER)
                    .and_then(|entry| self.reached(entry))
            }
        };
        if let Err(error) = result {
            self.outcome = Some(Outcome::Failed(error));
        }
        cx.waker().wake_by_ref();
    }
    fn resolve(&mut self, value: &Value) -> Result<(), Error> {
        let key = codec::bytes(&value.data, self.maximum)
            .map_err(|_| Error::PROVIDER)?;
        let registry = self.registry.upgrade().ok_or(Error::CLOSED)?;
        let mut registry = registry.borrow_mut();
        let ids = super::intern::page(
            &mut registry,
            &self.current,
            &[Box::from(key)],
        )?;
        let key = registry
            .keys
            .iter()
            .find(|key| key.identity == ids[0])
            .cloned()
            .ok_or(Error::PROVIDER)?;
        drop(registry);
        let ready = key.ready.borrow().upgrade();
        if let Some(entry) = ready {
            return self.reached(entry);
        }
        if self.publication.is_none() {
            return Err(Error::DENIED);
        }
        let owner = key.owner.upgrade().ok_or(Error::CLOSED)?;
        let work = Work::join(self.registry.clone(), &key, owner);
        self.native = Some(materialize::pending(work.clone()));
        self.work = Some(work);
        self.step = Step::Materialize;
        Ok(())
    }
    fn reached(&mut self, entry: Rc<Entry>) -> Result<(), Error> {
        if entry.is_closed() {
            return Err(Error::CLOSED);
        }
        let grant = self
            .grants
            .iter()
            .find(|grant| grant.resource == entry.identity)
            .copied()
            .or(self.publication)
            .ok_or(Error::DENIED)?;
        let rest = &self.names[self.offset..];
        self.offset += rest
            .iter()
            .position(|byte| *byte == b'/')
            .map_or(rest.len(), |slash| slash + 1);
        self.current = entry.clone();
        self.owners.push(entry.clone());
        self.work = None;
        self.step = Step::Lookup;
        if self.offset == self.names.len() {
            self.grant = Some(grant);
            self.ready = Some(entry);
            self.outcome = Some(Outcome::Ok(Value::unit()));
        }
        Ok(())
    }
    fn fault(&mut self, completion: Option<Completion>) -> Poll<()> {
        self.quarantine = completion;
        self.fenced = true;
        self.outcome =
            Some(Outcome::Unknown { error: Error::PROVIDER, progress: None });
        Poll::Ready(())
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
        if let Some(native) = &mut self.native {
            native.finish()?;
        }
        self.native = None;
        self.work = None;
        self.owners.clear();
        Ok(())
    }
}
fn valid(completion: &Completion, maximum: usize, step: Step) -> bool {
    let allowed = match step {
        Step::Lookup => 0,
        Step::Materialize => 1,
    };
    if completion.activity & !allowed != 0 {
        return false;
    }
    match &completion.outcome {
        Outcome::Ok(value) => {
            value.references.is_empty()
                && value.data.len() <= maximum
                && match step {
                    Step::Lookup => codec::bytes(&value.data, maximum).is_ok(),
                    Step::Materialize => value.data.is_empty(),
                }
        }
        Outcome::Unknown { error, progress } => {
            progress.is_none()
                && error.code.0 != 0
                && error.id.is_none_or(|id| id.serial != 0)
        }
        Outcome::Failed(error) => {
            error.code.0 != 0 && error.id.is_none_or(|id| id.serial != 0)
        }
    }
}
