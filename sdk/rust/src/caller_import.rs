use alloc::{rc::Rc, vec::Vec};
use core::{
    any::Any,
    cell::{Cell, RefCell},
    task::Context,
};
use reovim_kernel_interface::{
    Error, abi,
    call::Cancellation,
    caller::{Call, Caller, CallerOps, Status},
    declaration::ValueLimits,
};

use crate::raw;
mod scoped;
mod view;
pub use scoped::ImportedCaller;

struct State {
    raw: abi::Caller,
    ops: abi::CallerOps,
    owner: Rc<dyn Any>,
    limits: ValueLimits,
    capacity: usize,
    records: RefCell<Vec<Rc<RefCell<Record>>>>,
    frontier: Cell<u64>,
    closed: Cell<bool>,
    active: Cell<usize>,
}

struct Record {
    id: u64,
    view: Option<view::View>,
    busy: bool,
}

/// Import a native Caller, e.g. a loaded Rust module's granted imports.
/// Delivery views are observed and closed separately from original custody.
/// # Safety
/// Caller storage and callbacks must be valid and obey the caller-view ABI.
/// The owner must retain native context and code through all views/wakes.
/// Use the returned Caller on the native context's serialized executor thread;
/// Callbacks must not unwind. Nonnull pointers remain a native promise.
/// # Errors
/// Invalid table extents, required slots or bounds reject before calls.
pub unsafe fn import_caller(
    native: abi::Caller,
    owner: Rc<dyn Any>,
    limits: ValueLimits,
    capacity: usize,
) -> Result<Caller, Error> {
    // SAFETY: The supplied owner retains the native context and callbacks.
    let state = unsafe { state(native, owner, limits, capacity)? };
    Ok(caller(state))
}

unsafe fn state(
    native: abi::Caller,
    owner: Rc<dyn Any>,
    limits: ValueLimits,
    capacity: usize,
) -> Result<Rc<State>, Error> {
    // SAFETY: The caller retains valid native table storage; raw::table checks
    // its leading extent before copying any callback fields.
    let ops: abi::CallerOps = unsafe { raw::table(native.ops)? };
    validate(ops, capacity)?;
    let mut records = Vec::new();
    records.try_reserve_exact(capacity).map_err(|_| Error::PRESSURE)?;
    Ok(Rc::new(State {
        raw: native,
        ops,
        owner,
        limits,
        capacity,
        records: RefCell::new(records),
        frontier: Cell::new(0),
        closed: Cell::new(false),
        active: Cell::new(0),
    }))
}

fn caller(state: Rc<State>) -> Caller {
    Caller::new(
        state,
        CallerOps {
            reserve: |state| invoke(state, reserve),
            submit: |state, call| invoke(state, |state| submit(state, &call)),
            result: |state, id, cx| {
                invoke(state, |state| result(state, id, cx))
            },
            cancel: |state, id| invoke(state, |state| cancel(state, id)),
            reconcile: |state, id| invoke(state, |state| reconcile(state, id)),
            adopt: |state, id| invoke(state, |state| adopt(state, id)),
            release: |state, id| invoke(state, |state| release(state, id)),
        },
    )
}

struct Active<'a>(&'a Cell<usize>);
impl Drop for Active<'_> {
    fn drop(&mut self) {
        self.0.set(self.0.get() - 1);
    }
}

fn invoke<T>(
    state: &State,
    call: impl FnOnce(&State) -> Result<T, Error>,
) -> Result<T, Error> {
    if state.closed.get() {
        return Err(Error::CLOSED);
    }
    let active = state.active.get().checked_add(1).ok_or(Error::PRESSURE)?;
    state.active.set(active);
    let _active = Active(&state.active);
    call(state)
}

fn validate(ops: abi::CallerOps, capacity: usize) -> Result<(), Error> {
    if capacity == 0
        || ops.reserve.is_none()
        || ops.submit.is_none()
        || ops.result.is_none()
        || ops.cancel.is_none()
        || ops.reconcile.is_none()
        || ops.adopt.is_none()
        || ops.release.is_none()
    {
        return Err(Error::INVALID);
    }
    Ok(())
}

fn reserve(state: &State) -> Result<u64, Error> {
    let record = slot(state, 0)?;
    let mut id = 0;
    // SAFETY: Import checked this callback and retained its context/code.
    // id is writable aligned storage, read as a reservation only after OK.
    let status =
        unsafe { state.ops.reserve.unwrap()(state.raw.context, &raw mut id) };
    if status != 0 || id <= state.frontier.get() {
        remove(state, &record);
        return Err(if status == 0 {
            Error::PROVIDER
        } else {
            Error::new(status)
        });
    }
    record.borrow_mut().id = id;
    state.frontier.set(id);
    Ok(id)
}

fn slot(state: &State, id: u64) -> Result<Rc<RefCell<Record>>, Error> {
    if state.closed.get() {
        return Err(Error::CLOSED);
    }
    let mut records = state.records.borrow_mut();
    if id != 0
        && let Some(record) =
            records.iter().find(|record| record.borrow().id == id)
    {
        return Ok(record.clone());
    }
    if records.len() == state.capacity {
        return Err(Error::PRESSURE);
    }
    let record = Rc::new(RefCell::new(Record { id, view: None, busy: false }));
    records.push(record.clone());
    Ok(record)
}

fn remove(state: &State, record: &Rc<RefCell<Record>>) {
    let mut records = state.records.borrow_mut();
    let index = records.iter().position(|other| Rc::ptr_eq(other, record));
    let removed = index.map(|index| records.remove(index));
    drop(records);
    drop(removed);
}

fn submit(state: &State, call: &Call) -> Result<(), Error> {
    call.validate()?;
    let record = slot(state, call.request)?;
    if record.borrow().busy {
        return Err(Error::BUSY);
    }
    // An old delivery view does not own the original request. Close it before
    // asking for another view, so a retry works with a one-view native budget.
    close_view(&record)?;
    let references = refs(call, state.limits)?;
    let input = encoded(call, &references);
    let mut output = view::empty();
    let mut error = abi::Error::default();
    record.borrow_mut().busy = true;
    // SAFETY: The checked callback borrows captured input/Ref storage until
    // return, and receives initialized writable pending/error destinations.
    let status = unsafe {
        state.ops.submit.unwrap()(
            state.raw.context,
            &raw const input,
            &raw mut output,
            &raw mut error,
        )
    };
    let has_view = !output.ops.is_null() || !output.context.is_null();
    if status != 0
        && !has_view
        && let Ok(error) = raw::error(error)
    {
        record.borrow_mut().busy = false;
        remove(state, &record);
        return Err(error);
    }
    // SAFETY: A successful or contradictory submit may own a native view.
    // View checks its table before callbacks and retains the code on faults.
    let view = unsafe {
        view::View::new(output, state.owner.clone(), state.limits, status != 0)
    };
    let mut record = record.borrow_mut();
    record.view = Some(view);
    record.busy = false;
    Ok(())
}

fn refs(call: &Call, limits: ValueLimits) -> Result<Vec<u64>, Error> {
    if call.input.data.len() > limits.bytes
        || call.input.references.len() > limits.references
    {
        return Err(Error::PRESSURE);
    }
    let mut refs = Vec::new();
    refs.try_reserve_exact(call.input.references.len())
        .map_err(|_| Error::PRESSURE)?;
    refs.extend(call.input.references.iter().map(|token| token.0));
    Ok(refs)
}

fn encoded(call: &Call, refs: &[u64]) -> abi::Call {
    let expected = call.expected.map_or_else(abi::Expected::default, |value| {
        abi::Expected {
            present: 1,
            subject: abi::Identity {
                epoch: value.subject.epoch,
                serial: value.subject.serial,
            },
            revision: value.revision,
        }
    });
    abi::Call {
        request: call.request,
        operation: call.operation as u32,
        subject: call.subject.0,
        command: call.command,
        expected,
        input: abi::Args {
            data: raw::bytes(&call.input.data),
            refs: abi::Refs { ptr: refs.as_ptr(), len: refs.len() },
        },
    }
}

fn result(
    state: &State,
    id: u64,
    cx: Option<&mut Context<'_>>,
) -> Result<Status, Error> {
    if id == 0 {
        return Err(Error::INVALID);
    }
    let record = slot(state, id)?;
    if record.borrow().busy {
        return Err(Error::BUSY);
    }
    record.borrow_mut().busy = true;
    let view = record.borrow_mut().view.take();
    let (view, result) = observe(state, id, view, cx);
    let remove_empty = view.is_none() && result.is_err();
    let mut borrowed = record.borrow_mut();
    borrowed.busy = false;
    borrowed.view = view;
    drop(borrowed);
    if remove_empty {
        remove(state, &record);
    }
    result
}

fn observe(
    state: &State,
    id: u64,
    view: Option<view::View>,
    cx: Option<&mut Context<'_>>,
) -> (Option<view::View>, Result<Status, Error>) {
    let mut view = match view {
        Some(view) => view,
        None => match recover(state, id) {
            Ok(Some(view)) => view,
            Ok(None) => return (None, Ok(Status::Reserved)),
            Err(error) => return (None, Err(error)),
        },
    };
    let status = view.observe(cx);
    let retained = if view.finished() { None } else { Some(view) };
    (retained, status)
}

fn recover(state: &State, id: u64) -> Result<Option<view::View>, Error> {
    let mut output = view::empty();
    let mut error = abi::Error::default();
    // SAFETY: The retained native caller owns this checked result callback;
    // pending/error slots are initialized and writable for the call.
    let status = unsafe {
        state.ops.result.unwrap()(
            state.raw.context,
            id,
            &raw mut output,
            &raw mut error,
        )
    };
    if status != 0 && output.ops.is_null() && output.context.is_null() {
        let error = raw::error(error)?;
        return if status == Error::NOT_READY.code.0 && error == Error::NOT_READY
        {
            Ok(None)
        } else {
            Err(error)
        };
    }
    // SAFETY: Successful/contradictory native output is retained; View checks
    // extents before callbacks and fences malformed ownership.
    Ok(Some(unsafe {
        view::View::new(output, state.owner.clone(), state.limits, status != 0)
    }))
}

fn close_view(record: &Rc<RefCell<Record>>) -> Result<(), Error> {
    let mut view = record.borrow_mut().view.take();
    record.borrow_mut().busy = true;
    let result = view.as_mut().map_or(Ok(()), view::View::finish);
    let mut record = record.borrow_mut();
    record.busy = false;
    if result.is_err() {
        record.view = view;
    }
    result
}

fn cancel(state: &State, id: u64) -> Result<Cancellation, Error> {
    if id == 0 {
        return Err(Error::INVALID);
    }
    if slot(state, id)?.borrow().busy {
        return Err(Error::BUSY);
    }
    // SAFETY: Import checked the callback and retained its context/code.
    match unsafe { state.ops.cancel.unwrap()(state.raw.context, id) } {
        0 => Ok(Cancellation::Accepted),
        1 => Ok(Cancellation::TooLate),
        status => Err(Error::new(status)),
    }
}

fn reconcile(state: &State, id: u64) -> Result<(), Error> {
    if id == 0 {
        return Err(Error::INVALID);
    }
    let record = slot(state, id)?;
    if record.borrow().busy {
        return Err(Error::BUSY);
    }
    close_view(&record)?;
    // SAFETY: The retained callback refines original work; no submit occurs.
    status(unsafe { state.ops.reconcile.unwrap()(state.raw.context, id) })
}

fn adopt(state: &State, id: u64) -> Result<(), Error> {
    if id == 0 {
        return Err(Error::INVALID);
    }
    let record = slot(state, id)?;
    let record = record.borrow();
    if record.busy {
        return Err(Error::BUSY);
    }
    if record.view.as_ref().is_some_and(view::View::faulted) {
        return Err(Error::CONFLICT);
    }
    drop(record);
    // SAFETY: The native context owns adoption for this original request.
    status(unsafe { state.ops.adopt.unwrap()(state.raw.context, id) })
}

fn release(state: &State, id: u64) -> Result<(), Error> {
    if id == 0 {
        return Err(Error::INVALID);
    }
    let record = slot(state, id)?;
    if record.borrow().busy {
        return Err(Error::BUSY);
    }
    close_view(&record)?;
    // SAFETY: Original custody is explicitly released through the checked
    // caller callback, after the local delivery view has been closed.
    status(unsafe { state.ops.release.unwrap()(state.raw.context, id) })?;
    remove(state, &record);
    Ok(())
}

const fn status(status: u32) -> Result<(), Error> {
    if status == 0 { Ok(()) } else { Err(Error::new(status)) }
}
