use alloc::{rc::Rc, vec::Vec};
use core::{
    cell::RefCell,
    task::{Context, Poll},
};
use reovim_kernel_interface::{
    Error, Outcome, Pending, Value,
    call::{Cancellation, Completion},
    caller::Status,
    declaration::{Method, ValueLimits},
};

use super::{
    Entry,
    access::{Opened, Scope},
    custody::{Custody, Record, notify},
    factory::Factory,
    loans::Loans,
    outputs::{Outputs, Schema},
};

#[derive(Clone, Copy, Eq, PartialEq)]
enum Phase {
    Begin,
    Cancelled,
    Waiting,
    Preparing,
    Unknown,
    Final,
    Fenced,
    Finished,
}

enum Invocation {
    Declared { interface: usize, method: Method },
    Io(super::io::Io),
    Send,
    Receive(super::packet::Received),
    Listing(super::namespace::listing::Listing),
    Metadata(super::info::Metadata),
    Mutation(super::namespace::mutation::Mutation),
    Link(super::namespace::link::Link),
    Acquisition(super::namespace::acquire::Acquire),
    Creation(super::namespace::create::Create),
}

pub(super) struct Task {
    implementation: Rc<super::Implementation>,
    entry: Rc<Entry>,
    invocation: Invocation,
    pending: Option<Pending>,
    phase: Phase,
    loans: Option<Loans>,
    output: Option<Outputs>,
    quarantine: Option<Completion>,
    factory: Option<Factory>,
    initializing: bool,
}

impl Task {
    pub fn select(&mut self, implementation: Rc<super::Implementation>) {
        self.implementation = implementation;
    }
    pub fn uses_imports(&self, scope: &Rc<RefCell<Scope>>) -> bool {
        self.entry.imports.as_ref().is_some_and(|imports| {
            alloc::rc::Weak::ptr_eq(imports, &Rc::downgrade(scope))
        })
    }

    pub const fn uncertain(&self) -> bool {
        matches!(self.phase, Phase::Unknown | Phase::Fenced)
    }

    pub fn new(
        entry: Rc<Entry>,
        interface: usize,
        method: Method,
        loans: Option<Loans>,
        output: Outputs,
        factory: Option<Factory>,
    ) -> Self {
        Self {
            implementation: entry.implementation.clone(),
            entry,
            invocation: Invocation::Declared { interface, method },
            pending: None,
            phase: Phase::Begin,
            loans,
            output: Some(output),
            initializing: false,
            quarantine: None,
            factory,
        }
    }

    pub fn io(entry: Rc<Entry>, io: super::io::Io) -> Self {
        Self {
            implementation: entry.implementation.clone(),
            entry,
            invocation: Invocation::Io(io),
            pending: None,
            phase: Phase::Begin,
            loans: None,
            output: None,
            initializing: false,
            quarantine: None,
            factory: None,
        }
    }

    pub fn receive(
        entry: Rc<Entry>,
        received: super::packet::Received,
        output: Outputs,
    ) -> Self {
        Self {
            implementation: entry.implementation.clone(),
            entry,
            invocation: Invocation::Receive(received),
            pending: None,
            phase: Phase::Begin,
            loans: None,
            output: Some(output),
            initializing: false,
            quarantine: None,
            factory: None,
        }
    }

    pub fn send(entry: Rc<Entry>, loans: Option<Loans>) -> Self {
        Self {
            implementation: entry.implementation.clone(),
            entry,
            invocation: Invocation::Send,
            pending: None,
            phase: Phase::Begin,
            loans,
            output: None,
            initializing: false,
            quarantine: None,
            factory: None,
        }
    }

    pub fn listing(
        entry: Rc<Entry>,
        listing: super::namespace::listing::Listing,
    ) -> Self {
        Self {
            implementation: entry.implementation.clone(),
            entry,
            invocation: Invocation::Listing(listing),
            pending: None,
            phase: Phase::Begin,
            loans: None,
            output: None,
            initializing: false,
            quarantine: None,
            factory: None,
        }
    }

    pub fn link(entry: Rc<Entry>, link: super::namespace::link::Link) -> Self {
        Self {
            implementation: entry.implementation.clone(),
            entry,
            invocation: Invocation::Link(link),
            pending: None,
            phase: Phase::Begin,
            loans: None,
            output: None,
            initializing: false,
            quarantine: None,
            factory: None,
        }
    }

    pub fn mutation(
        entry: Rc<Entry>,
        mutation: super::namespace::mutation::Mutation,
    ) -> Self {
        Self {
            implementation: entry.implementation.clone(),
            entry,
            invocation: Invocation::Mutation(mutation),
            pending: None,
            phase: Phase::Begin,
            loans: None,
            output: None,
            initializing: false,
            quarantine: None,
            factory: None,
        }
    }

    pub fn metadata(entry: Rc<Entry>, metadata: super::info::Metadata) -> Self {
        Self {
            implementation: entry.implementation.clone(),
            entry,
            invocation: Invocation::Metadata(metadata),
            pending: None,
            phase: Phase::Begin,
            loans: None,
            output: None,
            initializing: false,
            quarantine: None,
            factory: None,
        }
    }

    pub fn creation(
        entry: Rc<Entry>,
        create: super::namespace::create::Create,
    ) -> Self {
        Self {
            implementation: entry.implementation.clone(),
            entry,
            invocation: Invocation::Creation(create),
            pending: None,
            phase: Phase::Begin,
            loans: None,
            output: None,
            initializing: false,
            quarantine: None,
            factory: None,
        }
    }

    pub fn acquisition(
        entry: Rc<Entry>,
        acquire: super::namespace::acquire::Acquire,
    ) -> Self {
        Self {
            implementation: entry.implementation.clone(),
            entry,
            invocation: Invocation::Acquisition(acquire),
            pending: None,
            phase: Phase::Begin,
            loans: None,
            output: None,
            initializing: false,
            quarantine: None,
            factory: None,
        }
    }

    pub const fn waiting(&self) -> bool {
        matches!(self.phase, Phase::Begin | Phase::Waiting | Phase::Preparing)
    }

    pub fn cancel(&mut self) -> Result<Cancellation, Error> {
        if self.initializing {
            return Ok(Cancellation::TooLate);
        }
        if matches!(self.phase, Phase::Begin | Phase::Cancelled) {
            self.phase = Phase::Cancelled;
            return Ok(Cancellation::Accepted);
        }
        self.pending.as_mut().ok_or(Error::CLOSED)?.cancel()
    }

    pub fn reconcile(&mut self) -> Result<(), Error> {
        if self.phase != Phase::Unknown {
            return Err(Error::CLOSED);
        }
        if self.initializing {
            self.factory.as_mut().unwrap().reconcile()?;
            self.phase = Phase::Preparing;
            return Ok(());
        }
        self.pending.as_mut().ok_or(Error::CLOSED)?.reconcile()?;
        self.phase = Phase::Waiting;
        Ok(())
    }

    fn advance(
        &mut self,
        input: &Value,
        limits: ValueLimits,
        context: &mut Context<'_>,
    ) -> Option<Outcome> {
        match self.phase {
            Phase::Begin => self.begin(input),
            Phase::Cancelled => {
                self.phase = Phase::Final;
                let _ = self.finish();
                Some(Outcome::Failed(Error::CANCELLED))
            }
            Phase::Waiting => self.poll(limits, context),
            Phase::Preparing => self.initialize(limits, context),
            Phase::Final => self.finish(),
            _ => None,
        }
    }

    fn begin(&mut self, input: &Value) -> Option<Outcome> {
        if let Some(value) = self.invocation.immediate() {
            self.phase = Phase::Finished;
            return Some(Outcome::Ok(value));
        }
        let input = self.loans.as_ref().map_or(input, |loans| &loans.input);
        let result = self.invocation.begin(
            &self.implementation,
            input,
            self.factory.as_ref(),
        );
        match result {
            Ok(pending) => {
                self.pending = Some(pending);
                self.phase = Phase::Waiting;
                None
            }
            Err(error) => {
                self.phase = Phase::Final;
                let _ = self.finish();
                Some(Outcome::Failed(error))
            }
        }
    }

    fn poll(
        &mut self,
        limits: ValueLimits,
        cx: &mut Context<'_>,
    ) -> Option<Outcome> {
        match self.pending.as_mut().unwrap().poll(cx) {
            Ok(Poll::Pending) => return None,
            Ok(Poll::Ready(())) => {}
            Err(_) => return Some(self.fence()),
        }
        let Ok(mut completion) = self.pending.as_mut().unwrap().take() else {
            return Some(self.fence());
        };
        if self.factory.is_some()
            && matches!(completion.outcome, Outcome::Ok(_))
        {
            return self.poll_factory(completion, limits, cx);
        }
        match self.invocation.validate(
            &mut completion,
            limits,
            self.output.as_mut(),
        ) {
            Err(_) => {
                // Invalid evidence cannot authorize another provider call.
                self.quarantine = Some(completion);
                return Some(self.fence());
            }
            Ok(Some(error)) => {
                // Preparation completed, but installation rejected before its
                // commit cut. Staged providers drain with the original task.
                completion.outcome = Outcome::Failed(error);
                self.release_output();
            }
            Ok(None) => {}
        }
        self.phase = match completion.outcome {
            Outcome::Unknown { .. } => Phase::Unknown,
            _ => Phase::Final,
        };
        Some(completion.outcome)
    }

    fn poll_factory(
        &mut self,
        completion: Completion,
        limits: ValueLimits,
        cx: &mut Context<'_>,
    ) -> Option<Outcome> {
        let outcome = self.prepare_factory(completion, limits);
        if self.phase == Phase::Preparing {
            return self.initialize(limits, cx);
        }
        outcome
    }

    fn prepare_factory(
        &mut self,
        completion: Completion,
        limits: ValueLimits,
    ) -> Option<Outcome> {
        let Invocation::Declared { method, .. } = &self.invocation else {
            unreachable!()
        };
        let Outcome::Ok(value) = &completion.outcome else { unreachable!() };
        let result = if completion.activity & !method.activity() == 0 {
            self.output.as_mut().unwrap().prepare_factory(
                value,
                method,
                limits,
                self.factory.as_ref().unwrap(),
            )
        } else {
            Err(Error::PROVIDER)
        };
        match result {
            Err(_) => {
                self.quarantine = Some(completion);
                Some(self.fence())
            }
            Ok(Some(error)) => {
                self.release_output();
                self.phase = Phase::Final;
                Some(Outcome::Failed(error))
            }
            Ok(None) => {
                self.factory.as_mut().unwrap().freeze();
                self.initializing = true;
                self.phase = Phase::Preparing;
                None
            }
        }
    }

    fn initialize(
        &mut self,
        limits: ValueLimits,
        cx: &mut Context<'_>,
    ) -> Option<Outcome> {
        // The factory's completion owner has left scope before finish; its
        // returned access is retained privately by Outputs throughout Open.
        if self.pending.as_mut().is_some_and(|p| p.finish().is_err()) {
            cx.waker().wake_by_ref();
            return None;
        }
        self.pending = None;
        let factory = self.factory.as_mut().unwrap();
        let outcome = factory.initialize(cx)?;
        if matches!(outcome, Outcome::Unknown { .. }) {
            self.phase = Phase::Unknown;
            return Some(outcome);
        }
        let Invocation::Declared { method, .. } = &self.invocation else {
            unreachable!()
        };
        let result = self
            .output
            .as_mut()
            .unwrap()
            .install_factory(factory, method, limits);
        self.phase = Phase::Final;
        Some(match result {
            Ok(value) => Outcome::Ok(value),
            Err(error) => {
                self.release_output();
                Outcome::Failed(error)
            }
        })
    }

    fn release_output(&mut self) {
        if let Some(output) = &mut self.output {
            output.release();
        }
    }

    fn finish(&mut self) -> Option<Outcome> {
        if let Invocation::Creation(create) = &mut self.invocation
            && create.finish().is_err()
        {
            return None;
        }

        if self
            .factory
            .as_mut()
            .is_some_and(|factory| factory.finish().is_err())
        {
            return None;
        }
        if self
            .pending
            .as_mut()
            .is_some_and(|pending| pending.finish().is_err())
        {
            return None;
        }
        self.phase = Phase::Finished;
        None
    }

    const fn fence(&mut self) -> Outcome {
        self.phase = Phase::Fenced;
        let progress = match &mut self.invocation {
            Invocation::Io(io) => io.fault_progress(),
            Invocation::Declared { .. }
            | Invocation::Send
            | Invocation::Receive(_)
            | Invocation::Link(_)
            | Invocation::Mutation(_)
            | Invocation::Metadata(_)
            | Invocation::Listing(_)
            | Invocation::Acquisition(_)
            | Invocation::Creation(_) => None,
        };
        Outcome::Unknown { error: Error::PROVIDER, progress }
    }

    fn uses(&self, opened: &Rc<RefCell<Opened>>) -> bool {
        self.loans.as_ref().is_some_and(|loans| loans.uses(opened))
            || matches!(&self.invocation, Invocation::Link(link)
                if link.uses(opened))
            || matches!(&self.invocation, Invocation::Mutation(mutation)
                if mutation.uses(opened))
    }

    fn end_loans(&mut self) {
        if self.phase != Phase::Finished {
            return;
        }
        if let Invocation::Acquisition(acquire) = &mut self.invocation {
            acquire.release();
        }
        if let Invocation::Creation(create) = &mut self.invocation {
            create.release();
        }
        if let Some(loans) = self.loans.take() {
            loans.revoke();
        }
        if let Some(output) = &mut self.output {
            output.release();
        }
    }
}

impl Invocation {
    const fn immediate(&mut self) -> Option<Value> {
        match self {
            Self::Io(io) => io.immediate.take(),
            Self::Declared { .. }
            | Self::Send
            | Self::Receive(_)
            | Self::Listing(_)
            | Self::Link(_)
            | Self::Mutation(_)
            | Self::Metadata(_)
            | Self::Acquisition(_)
            | Self::Creation(_) => None,
        }
    }

    fn begin(
        &self,
        implementation: &super::Implementation,
        input: &Value,
        factory: Option<&Factory>,
    ) -> Result<Pending, Error> {
        if let Self::Acquisition(acquire) = self {
            return Ok(acquire.begin());
        }
        if let Self::Mutation(mutation) = self {
            return mutation.begin();
        }
        let mut provider = implementation.provider.borrow_mut();
        match self {
            Self::Link(link) => link.begin(&mut provider),
            Self::Creation(create) => create.begin(&mut provider),
            Self::Io(io) => io.begin(&mut provider, input),
            Self::Send => provider.send(input),
            Self::Receive(received) => received.begin(&mut provider),
            Self::Listing(listing) => listing.begin(&mut provider),
            Self::Metadata(_) => super::info::Metadata::begin(&mut provider),
            Self::Acquisition(_) | Self::Mutation(_) => unreachable!(),
            Self::Declared { interface, method } => match factory {
                Some(factory) => provider.create(
                    *interface,
                    method.operation(),
                    input,
                    &factory.creation,
                ),
                None => provider.begin(*interface, method.operation(), input),
            },
        }
    }

    fn validate(
        &mut self,
        completion: &mut Completion,
        limits: ValueLimits,
        output: Option<&mut Outputs>,
    ) -> Result<Option<Error>, Error> {
        check_error(&completion.outcome)?;
        match self {
            Self::Io(io) => io.validate(completion, limits).map(|()| None),
            Self::Send => super::packet::validate_send(completion, limits),
            Self::Receive(received) => {
                received.validate(completion, limits, output.unwrap())
            }
            Self::Listing(listing) => listing.validate(completion, limits),
            Self::Metadata(metadata) => metadata.validate(completion),
            Self::Mutation(_) => Ok(None),
            Self::Link(_) => super::namespace::link::Link::validate(completion),
            Self::Acquisition(acquire) => acquire.validate(completion),
            Self::Creation(create) => create.validate(completion),
            Self::Declared { method, .. } => {
                validate(completion, method, limits, output.unwrap())
            }
        }
    }
}

fn validate(
    completion: &mut Completion,
    method: &Method,
    limits: ValueLimits,
    output: &mut Outputs,
) -> Result<Option<Error>, Error> {
    check_error(&completion.outcome)?;
    if completion.activity & !method.activity() != 0 {
        return Err(Error::PROVIDER);
    }
    match &mut completion.outcome {
        Outcome::Ok(value) => {
            output.transfer(value, Schema::Value(method.output()), limits)
        }
        Outcome::Unknown { progress: Some(value), .. } => method
            .progress()
            .validate(&value.data, value.references.len(), limits, |_, _, _| {
                Err(Error::INVALID)
            })
            .map(|()| None),
        Outcome::Failed(_) => {
            output.release();
            Ok(None)
        }
        Outcome::Unknown { .. } => Ok(None),
    }
}

fn check_error(outcome: &Outcome) -> Result<(), Error> {
    let error = match outcome {
        Outcome::Failed(error) | Outcome::Unknown { error, .. } => error,
        Outcome::Ok(_) => return Ok(()),
    };
    if error.code.0 == 0 || error.id.is_some_and(|id| id.serial == 0) {
        return Err(Error::PROVIDER);
    }
    Ok(())
}

#[cfg(test)]
#[path = "execution_tests.rs"]
mod tests;

pub(super) fn advance(
    scope: &Rc<RefCell<Scope>>,
    cx: &mut Context<'_>,
) -> bool {
    let count = scope.borrow().records.len();
    let mut progress = false;
    for index in 0..count {
        progress |= advance_one(scope, index, cx);
    }
    while reclaim(scope) {}
    progress
}

fn advance_one(
    scope: &Rc<RefCell<Scope>>,
    index: usize,
    cx: &mut Context<'_>,
) -> bool {
    let state = scope.borrow();
    let Some(receipt) = state.records.get(index).cloned() else {
        return false;
    };
    let limits = state.limits.values;
    drop(state);
    let mut record = receipt.borrow_mut();
    if record.busy {
        return false;
    }
    if record.closing {
        drop(record);
        return close_access(scope, &receipt);
    }
    let Some(mut task) = record.task.take() else {
        return false;
    };
    let call = record.call.as_ref().unwrap().clone();
    record.busy = true;
    drop(record);
    // Reserve the generation's shared owner before entering provider code.
    let mut generation = Rc::new(Outcome::Failed(Error::NOT_READY));
    let previous = task.phase;
    let outcome = task.advance(&call.input, limits, cx);
    task.end_loans();
    let changed = outcome.is_some();
    let progress = previous != task.phase
        || outcome.is_some()
        || task.phase == Phase::Final;
    let mut record = receipt.borrow_mut();
    if let Some(outcome) = outcome {
        *Rc::get_mut(&mut generation).unwrap() = outcome;
        record.status = Status::Outcome(generation);
    }
    if task.phase != Phase::Finished {
        record.task = Some(task);
    }
    record.busy = false;
    let wakes =
        if changed { core::mem::take(&mut record.wakes) } else { Vec::new() };
    drop(record);
    notify(wakes);
    progress
}

fn reclaim(scope: &Rc<RefCell<Scope>>) -> bool {
    let mut state = scope.borrow_mut();
    let retired = state.records.iter().position(|receipt| {
        let record = receipt.borrow();
        record.custody == Custody::Released
            && !record.busy
            && !record.closing
            && record.task.is_none()
    });
    let record = retired.map(|index| state.records.remove(index));
    let reclaimed = record.is_some();
    drop(state);
    drop(record);
    reclaimed
}

fn close_access(
    scope: &Rc<RefCell<Scope>>,
    receipt: &Rc<RefCell<Record>>,
) -> bool {
    let opened = receipt.borrow().opened.as_ref().unwrap().clone();
    let state = scope.borrow();
    let busy =
        state.records.iter().filter(|other| !Rc::ptr_eq(other, receipt)).any(
            |other| {
                let other = other.borrow();
                (other.busy || other.task.is_some())
                    && (other.task.as_ref().is_some_and(|t| t.uses(&opened))
                        || other
                            .opened
                            .as_ref()
                            .is_some_and(|access| Rc::ptr_eq(access, &opened)))
            },
        );
    drop(state);
    if busy {
        return false;
    }
    if finish_opened(scope, &opened).is_err() {
        return false;
    }
    let outcome = Rc::new(Outcome::Ok(Value::unit()));
    scope
        .borrow_mut()
        .handles
        .retain(|handle| !Rc::ptr_eq(&handle.opened, &opened));
    let mut record = receipt.borrow_mut();
    record.status = Status::Outcome(outcome);
    record.closing = false;
    let wakes = core::mem::take(&mut record.wakes);
    drop(record);
    notify(wakes);
    true
}

fn finish_opened(
    scope: &Rc<RefCell<Scope>>,
    opened: &Rc<RefCell<Opened>>,
) -> Result<(), Error> {
    let state = scope.borrow();
    let mut aliases = state
        .handles
        .iter()
        .filter(|handle| Rc::ptr_eq(&handle.opened, opened));
    let Some(first) = aliases.next() else { return Ok(()) };
    let implementation = first.implementation.clone();
    let count = 1 + aliases.count();
    let registry = state.registry.upgrade().ok_or(Error::CLOSED)?;
    drop(state);
    super::opening::close(&registry, &implementation, count)
}
