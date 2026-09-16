use super::super::{
    Entry,
    access::{Opened, Scope},
    codec,
    execution::Task,
};
use alloc::{boxed::Box, rc::Rc};
use core::{
    cell::RefCell,
    task::{Context, Poll},
};
use reovim_kernel_interface::{
    Error, Outcome, Pending,
    call::{Cancellation, Completion, PendingOps},
    caller::{Call, Operation},
    provider::{Rename, ResourceProvider},
};

pub(in crate::engine) struct Mutation {
    implementation: Rc<crate::engine::Implementation>,
    owner: Rc<Entry>,
    name: Box<[u8]>,
    maximum: usize,
    destination: Option<Destination>,
}
pub(in crate::engine) fn admit(
    scope: &Scope,
    call: &Call,
) -> Result<Task, Error> {
    let handle = scope.live(call.subject)?;
    if handle.opened.borrow().held & 2 == 0 {
        return Err(Error::DENIED);
    }
    let support = handle
        .implementation
        .provider
        .try_borrow()
        .map_err(|_| Error::BUSY)?
        .namespace()
        .ok_or(Error::UNSUPPORTED)?;
    if !support.lookup
        || (call.operation == Operation::Delete && !support.remove)
        || (call.operation == Operation::Rename && !support.rename)
    {
        return Err(Error::UNSUPPORTED);
    }
    let (name, destination) = if call.operation == Operation::Rename {
        destination(scope, call, &handle.entry)?
    } else {
        (codec::bytes(codec::no_refs(&call.input)?, 4096)?, None)
    };
    codec::name(name, scope.non_ascii_names)?;
    Ok(Task::mutation(
        handle.entry.clone(),
        Mutation {
            owner: handle.entry.clone(),
            implementation: handle.implementation.clone(),
            name: name.into(),
            maximum: scope.limits.values.bytes,
            destination,
        },
    ))
}
impl Mutation {
    pub fn uses(&self, opened: &Rc<RefCell<Opened>>) -> bool {
        self.destination
            .as_ref()
            .is_some_and(|destination| Rc::ptr_eq(&destination.opened, opened))
    }

    pub fn begin(&self) -> Result<Pending, Error> {
        let native =
            self.implementation.provider.borrow_mut().lookup(&self.name)?;
        Ok(Pending::new(
            Work {
                owner: self.owner.clone(),
                implementation: self.implementation.clone(),
                name: self.name.clone(),
                maximum: self.maximum,
                destination: self.destination.clone(),
                native: Some(native),
                phase: Phase::Lookup,
                key: None,
                outcome: None,
                activity: 0,
                taken: false,
                quarantine: None,
                fenced: false,
            },
            PendingOps {
                poll: |state, cx| Ok(state.poll(cx)),
                take: Work::take,
                cancel: |_| Ok(Cancellation::TooLate),
                reconcile: Work::reconcile,
                finish: Work::finish,
            },
        ))
    }
}
#[derive(Clone, Copy, PartialEq, Eq)]
enum Phase {
    Lookup,
    Prepared,
    Apply,
}
struct Work {
    implementation: Rc<crate::engine::Implementation>,
    owner: Rc<Entry>,
    name: Box<[u8]>,
    maximum: usize,
    destination: Option<Destination>,
    native: Option<Pending>,
    phase: Phase,
    key: Option<Box<[u8]>>,
    outcome: Option<Outcome>,
    activity: u8,
    taken: bool,
    quarantine: Option<Completion>,
    fenced: bool,
}
impl Work {
    fn take(&mut self) -> Result<Completion, Error> {
        if self.taken {
            return Err(Error::CLOSED);
        }
        let outcome = self.outcome.clone().ok_or(Error::NOT_READY)?;
        self.taken = true;
        Ok(Completion { outcome, activity: self.activity, owner: None })
    }

    fn poll(&mut self, cx: &mut Context<'_>) -> Poll<()> {
        if self.outcome.is_some() {
            return Poll::Ready(());
        }
        if self.phase == Phase::Prepared {
            return self.apply(cx);
        }
        let native = self.native.as_mut().unwrap();
        match native.poll(cx) {
            Ok(Poll::Pending) => return Poll::Pending,
            Ok(Poll::Ready(())) => {}
            Err(_) => return self.fault(None),
        }
        let Ok(completion) = native.take() else { return self.fault(None) };
        if !valid(&completion, self.phase, self.maximum) {
            return self.fault(Some(completion));
        }
        self.activity = completion.activity;
        if let Outcome::Ok(value) = &completion.outcome
            && self.phase == Phase::Lookup
        {
            self.key =
                Some(codec::bytes(&value.data, self.maximum).unwrap().into());
            self.phase = Phase::Prepared;
            cx.waker().wake_by_ref();
            return Poll::Pending;
        }
        self.outcome = Some(completion.outcome);
        Poll::Ready(())
    }
    fn apply(&mut self, cx: &Context<'_>) -> Poll<()> {
        if self.native.as_mut().unwrap().finish().is_err() {
            cx.waker().wake_by_ref();
            return Poll::Pending;
        }
        self.native = None;
        if self.owner.is_closed() {
            self.outcome = Some(Outcome::Failed(Error::CLOSED));
            return Poll::Ready(());
        }
        let result = match &self.destination {
            Some(destination) => destination.begin(
                &mut self.implementation.provider.borrow_mut(),
                &self.name,
                self.key.as_ref().unwrap(),
            ),
            None => self
                .implementation
                .provider
                .borrow_mut()
                .remove(&self.name, self.key.as_ref().unwrap()),
        };
        match result {
            Ok(native) => self.native = Some(native),
            Err(error) => {
                self.outcome = Some(Outcome::Failed(error));
                return Poll::Ready(());
            }
        }
        self.phase = Phase::Apply;
        cx.waker().wake_by_ref();
        Poll::Pending
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
        Ok(())
    }
}
fn valid(completion: &Completion, phase: Phase, maximum: usize) -> bool {
    let activity = if phase == Phase::Apply { 2 } else { 0 };
    if completion.activity & !activity != 0 {
        return false;
    }
    match &completion.outcome {
        Outcome::Ok(value) => {
            value.references.is_empty()
                && match phase {
                    Phase::Lookup => codec::bytes(&value.data, maximum).is_ok(),
                    Phase::Apply => value.data.is_empty(),
                    Phase::Prepared => false,
                }
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

#[derive(Clone)]
struct Destination {
    entry: Rc<Entry>,
    opened: Rc<RefCell<Opened>>,
    key: Option<Box<[u8]>>,
    name: Box<[u8]>,
}
impl Destination {
    fn begin(
        &self,
        provider: &mut ResourceProvider,
        name: &[u8],
        key: &[u8],
    ) -> Result<Pending, Error> {
        if self.entry.is_closed() {
            return Err(Error::CLOSED);
        }
        provider.rename(Rename {
            name,
            key,
            destination: self.key.as_deref(),
            new_name: &self.name,
        })
    }
}
fn destination<'a>(
    scope: &Scope,
    call: &'a Call,
    parent: &Rc<Entry>,
) -> Result<(&'a [u8], Option<Destination>), Error> {
    let (length, data) =
        call.input.data.split_at_checked(4).ok_or(Error::INVALID)?;
    let (name, data) = data
        .split_at_checked(codec::u32_value(length)? as usize)
        .ok_or(Error::INVALID)?;
    if name.len() > 4096 {
        return Err(Error::INVALID);
    }
    let (slot, data) = data.split_at_checked(4).ok_or(Error::INVALID)?;
    if codec::u32_value(slot)? != 0 || call.input.references.len() != 1 {
        return Err(Error::INVALID);
    }
    let new_name = codec::bytes(data, 4096)?;
    codec::name(new_name, scope.non_ascii_names)?;
    let target = scope.live(call.input.references[0])?;
    if target.opened.borrow().held & 2 == 0 {
        return Err(Error::DENIED);
    }
    if target
        .implementation
        .provider
        .try_borrow()
        .map_err(|_| Error::BUSY)?
        .namespace()
        .is_none()
    {
        return Err(Error::UNSUPPORTED);
    }
    let registry = scope.registry.upgrade().ok_or(Error::CLOSED)?;
    let key =
        super::intern::selected(&registry.borrow(), parent, &target.entry)?;
    Ok((
        name,
        Some(Destination {
            entry: target.entry.clone(),
            opened: target.opened.clone(),
            key,
            name: new_name.into(),
        }),
    ))
}
