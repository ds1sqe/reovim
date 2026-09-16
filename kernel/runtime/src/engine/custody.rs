use alloc::{rc::Rc, vec::Vec};
use core::{
    cell::RefCell,
    task::{Context, Waker},
};
use reovim_kernel_interface::{
    Error, Outcome,
    call::Cancellation,
    caller::{Call, Operation, Status},
};

use super::{
    access::{Opened, Scope},
    execution::Task,
};

#[derive(Clone, Copy, Eq, PartialEq)]
pub(super) enum Custody {
    Retained,
    Adopted,
    Released,
}

pub(super) struct Record {
    pub id: u64,
    pub call: Option<Rc<Call>>,
    pub task: Option<Task>,
    pub opened: Option<Rc<RefCell<Opened>>>,
    pub closing: bool,
    pub status: Status,
    pub custody: Custody,
    pub busy: bool,
    pub wakes: Vec<Waker>,
}

impl Record {
    const fn new(id: u64) -> Self {
        Self {
            id,
            call: None,
            task: None,
            opened: None,
            closing: false,
            status: Status::Reserved,
            custody: Custody::Retained,
            busy: false,
            wakes: Vec::new(),
        }
    }
}

pub(super) fn reserve(state: &Rc<RefCell<Scope>>) -> Result<u64, Error> {
    let mut scope = state.borrow_mut();
    if scope.closed {
        return Err(Error::CLOSED);
    }
    if scope.records.len() == scope.limits.requests
        || scope.next_request == u64::MAX
    {
        return Err(Error::PRESSURE);
    }
    let id = scope.next_request;
    scope.records.push(Rc::new(RefCell::new(Record::new(id))));
    scope.next_request += 1;
    Ok(id)
}

pub(super) fn record(
    scope: &Scope,
    id: u64,
) -> Result<Rc<RefCell<Record>>, Error> {
    scope
        .records
        .iter()
        .find(|record| {
            let record = record.borrow();
            record.id == id && record.custody != Custody::Released
        })
        .cloned()
        .ok_or(Error::CLOSED)
}

pub(super) fn submit(
    state: &Rc<RefCell<Scope>>,
    call: Call,
) -> Result<(), Error> {
    let mut scope = state.borrow_mut();
    let receipt = record(&scope, call.request)?;
    let mut receipt = receipt.borrow_mut();
    if let Some(previous) = &receipt.call {
        return if previous.as_ref() == &call {
            Ok(())
        } else {
            Err(Error::CONFLICT)
        };
    }
    admission(&scope, &call)?;
    let call = Rc::new(call);
    let mut outcome = Rc::new(Outcome::Failed(Error::NOT_READY));
    if matches!(call.operation, Operation::Exec | Operation::Ioctl) {
        receipt.task = Some(scope.declared(&call, state)?);
        receipt.opened = Some(scope.live(call.subject)?.opened.clone());
        receipt.status = Status::Pending;
    } else if matches!(
        call.operation,
        Operation::Read
            | Operation::ReadAt
            | Operation::Write
            | Operation::WriteAt
    ) {
        receipt.task = Some(super::io::admit(&scope, &call)?);
        receipt.opened = Some(scope.live(call.subject)?.opened.clone());
        receipt.status = Status::Pending;
    } else if call.operation == Operation::Receive {
        receipt.task = Some(super::packet::receive(&mut scope, state, &call)?);
        receipt.opened = Some(scope.live(call.subject)?.opened.clone());
        receipt.status = Status::Pending;
    } else if call.operation == Operation::Send {
        receipt.task = Some(super::packet::send(&mut scope, state, &call)?);
        receipt.opened = Some(scope.live(call.subject)?.opened.clone());
        receipt.status = Status::Pending;
    } else if call.operation == Operation::Create {
        receipt.task =
            Some(super::namespace::create::admit(&mut scope, state, &call)?);
        receipt.opened = Some(scope.live(call.subject)?.opened.clone());
        receipt.status = Status::Pending;
    } else if call.operation == Operation::Acquire {
        if let Some(task) =
            super::namespace::acquire::admit(&mut scope, state, &call)?
        {
            receipt.task = Some(task);
            receipt.status = Status::Pending;
        } else {
            *Rc::get_mut(&mut outcome).unwrap() = scope.common(&call)?;
            receipt.status = Status::Outcome(outcome);
        }
    } else if call.operation == Operation::Entries {
        if let Some(task) = super::namespace::listing::admit(&scope, &call)? {
            receipt.task = Some(task);
            receipt.opened = Some(scope.live(call.subject)?.opened.clone());
            receipt.status = Status::Pending;
        } else {
            *Rc::get_mut(&mut outcome).unwrap() = scope.common(&call)?;
            receipt.status = Status::Outcome(outcome);
        }
    } else if call.operation == Operation::Link {
        receipt.task = Some(super::namespace::link::admit(&scope, &call)?);
        receipt.opened = Some(scope.live(call.subject)?.opened.clone());
        receipt.status = Status::Pending;
    } else if matches!(call.operation, Operation::Delete | Operation::Rename) {
        receipt.task = Some(super::namespace::mutation::admit(&scope, &call)?);
        receipt.opened = Some(scope.live(call.subject)?.opened.clone());
        receipt.status = Status::Pending;
    } else if call.operation == Operation::Info {
        if let Some(task) = super::info::admit(&scope, &call)? {
            receipt.task = Some(task);
            receipt.opened = Some(scope.live(call.subject)?.opened.clone());
            receipt.status = Status::Pending;
        } else {
            *Rc::get_mut(&mut outcome).unwrap() = scope.common(&call)?;
            receipt.status = Status::Outcome(outcome);
        }
    } else if call.operation == Operation::Close {
        if !call.input.data.is_empty() || !call.input.references.is_empty() {
            return Err(Error::INVALID);
        }
        let opened = scope.opened(call.subject)?.opened.clone();
        opened.borrow_mut().closed = true;
        receipt.opened = Some(opened);
        receipt.closing = true;
        receipt.status = Status::Pending;
    } else {
        *Rc::get_mut(&mut outcome).unwrap() = scope.common(&call)?;
        receipt.status = Status::Outcome(outcome);
    }
    if let Some(task) = &mut receipt.task
        && call.operation != Operation::Acquire
    {
        task.select(scope.live(call.subject)?.implementation.clone());
    }
    receipt.call = Some(call);
    let wakes = core::mem::take(&mut receipt.wakes);
    drop(receipt);
    drop(scope);
    notify(wakes);
    Ok(())
}

fn admission(scope: &Scope, call: &Call) -> Result<(), Error> {
    if scope.closed {
        return Err(Error::CLOSED);
    }
    if !scope.ready {
        return Err(Error::NOT_READY);
    }
    call.validate()?;
    if call.input.data.len() > scope.limits.values.bytes
        || call.input.references.len() > scope.limits.values.references
    {
        return Err(Error::PRESSURE);
    }
    // Only staged state operations accept expected state; ordinary callbacks
    // cannot silently discard a requested revision precondition.
    if call.expected.is_some() {
        return Err(Error::UNSUPPORTED);
    }
    Ok(())
}

pub(super) fn result(
    state: &Rc<RefCell<Scope>>,
    id: u64,
    context: Option<&mut Context<'_>>,
) -> Result<Status, Error> {
    let wake = context.map(|context| context.waker().clone());
    let scope = state.borrow();
    let limit = scope.limits.waiters;
    let receipt = record(&scope, id)?;
    drop(scope);
    let mut receipt = receipt.borrow_mut();
    let status = receipt.status.clone();
    let Some(wake) = wake else { return Ok(status) };
    if !matches!(status, Status::Outcome(_))
        && !receipt.wakes.iter().any(|old| old.will_wake(&wake))
    {
        if receipt.wakes.len() == limit {
            return Err(Error::PRESSURE);
        }
        receipt.wakes.try_reserve(1).map_err(|_| Error::PRESSURE)?;
        receipt.wakes.push(wake);
    }
    drop(receipt);
    Ok(status)
}

pub(super) fn adopt(state: &Rc<RefCell<Scope>>, id: u64) -> Result<(), Error> {
    let mut scope = state.borrow_mut();
    let receipt = record(&scope, id)?;
    let mut receipt = receipt.borrow_mut();
    if receipt.custody == Custody::Adopted {
        return Ok(());
    }
    let Status::Outcome(outcome) = &receipt.status else {
        return Err(Error::NOT_READY);
    };
    if !matches!(outcome.as_ref(), Outcome::Ok(_)) {
        return Err(Error::CONFLICT);
    }
    for handle in &mut scope.handles {
        if handle.provisional == Some(id) {
            handle.provisional = None;
        }
    }
    receipt.custody = Custody::Adopted;
    Ok(())
}

pub(super) fn release(
    state: &Rc<RefCell<Scope>>,
    id: u64,
) -> Result<(), Error> {
    let mut scope = state.borrow_mut();
    let receipt = record(&scope, id)?;
    receipt.borrow_mut().custody = Custody::Released;
    // Registry ownership keeps providers alive while provisional aliases drop.
    // Handles have no callbacks; provider cleanup remains Runtime-owned.
    scope.handles.retain(|handle| handle.provisional != Some(id));
    drop(scope);
    Ok(())
}

pub(super) fn cancel(
    state: &Rc<RefCell<Scope>>,
    id: u64,
) -> Result<Cancellation, Error> {
    control(state, id, Task::cancel)
}

pub(super) fn reconcile(
    state: &Rc<RefCell<Scope>>,
    id: u64,
) -> Result<(), Error> {
    control(state, id, Task::reconcile)
}

fn control<T>(
    state: &Rc<RefCell<Scope>>,
    id: u64,
    action: fn(&mut Task) -> Result<T, Error>,
) -> Result<T, Error> {
    let receipt = record(&state.borrow(), id)?;
    let mut record = receipt.borrow_mut();
    if record.busy {
        return Err(Error::BUSY);
    }
    let mut task = record.task.take().ok_or(Error::CLOSED)?;
    record.busy = true;
    drop(record);
    let result = action(&mut task);
    let mut record = receipt.borrow_mut();
    if task.waiting() {
        record.status = Status::Pending;
    }
    record.task = Some(task);
    record.busy = false;
    let wakes = core::mem::take(&mut record.wakes);
    drop(record);
    notify(wakes);
    result
}

pub(super) fn notify(wakes: Vec<Waker>) {
    for wake in wakes {
        wake.wake();
    }
}
