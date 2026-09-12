use alloc::{boxed::Box, rc::Rc, vec::Vec};
use core::{
    cell::RefCell,
    task::{Context, Poll},
};
use reovim_kernel_interface::access::Method;
use reovim_kernel_interface::buffer::{State, integer};
use reovim_kernel_interface::provider::Permissions;
use reovim_kernel_interface::{Error, Message, RequestError, ResourceId};

use crate::namespace::{Entry, Kind};
use crate::request::{Record, Work};
use crate::{Editing, Progress, Runtime, diagnostics::Stage};

#[cfg(test)]
mod tests;

enum Step {
    Resource(Result<Message, RequestError>),
    Pending(Work),
    Finished(Result<Message, Error>),
}

impl Runtime {
    pub(super) fn advance_one(
        &mut self,
        context: &Context<'_>,
    ) -> Result<Progress, Error> {
        if !self.shared.borrow().ready {
            return Err(Error::NOT_READY);
        }
        let Some(record) = self.next_request(context) else {
            return Ok(self.idle());
        };
        let work = record.borrow_mut().work.take().ok_or(Error::BUSY)?;
        self.shared.borrow_mut().active = Some(record.borrow().id);
        let step = self.execute(&record, work, context);
        self.shared.borrow_mut().active = None;
        self.settle(&record, step);
        Ok(Progress::More)
    }

    fn idle(&self) -> Progress {
        if self.shared.borrow().entries.is_empty() {
            Progress::Stopped
        } else {
            Progress::Idle
        }
    }

    fn next_request(
        &mut self,
        context: &Context<'_>,
    ) -> Option<Rc<RefCell<Record>>> {
        let shared = self.shared.borrow();
        let total = shared.requests.len();
        let found = (0..total)
            .map(|offset| (self.cursor + offset) % total)
            .find(|index| {
                let record = shared.requests[*index].borrow();
                record.outcome.is_none()
                    && (matches!(
                        record.target,
                        Kind::Catalog(_) | Kind::Module(_)
                    ) || record.schedule.runnable(context.waker()))
            })?;
        self.cursor = (found + 1) % total;
        Some(Rc::clone(&shared.requests[found]))
    }

    fn execute(
        &mut self,
        record: &Rc<RefCell<Record>>,
        work: Work,
        context: &Context<'_>,
    ) -> Step {
        let waker = record.borrow_mut().schedule.polling(context.waker());
        let mut context = Context::from_waker(&waker);
        match work {
            Work::Queued => {
                // Creating a future does not poll it; schedule its first poll.
                waker.wake_by_ref();
                self.begin(record)
            }
            Work::ModuleUnload => self.poll_module_unload(record),
            Work::Resource(mut future) => {
                match future.as_mut().poll(&mut context) {
                    Poll::Pending => Step::Pending(Work::Resource(future)),
                    Poll::Ready(result) => Step::Resource(result),
                }
            }
            Work::Model(mut future) => {
                let polled = future
                    .as_mut()
                    .poll(&mut context)
                    .map(|result| self.install(record, result));
                match polled {
                    Poll::Pending => Step::Pending(Work::Model(future)),
                    Poll::Ready(result) => Step::Finished(result),
                }
            }
        }
    }

    fn begin(&mut self, record: &Rc<RefCell<Record>>) -> Step {
        let (id, scope, subject, target, method, input, cancelled) = {
            let record = record.borrow();
            (
                record.id,
                record.scope.borrow().id,
                record.subject,
                record.target.clone(),
                record.method,
                record.input.clone(),
                record.cancelled,
            )
        };
        if cancelled {
            return Step::Finished(Err(Error::CANCELLED));
        }
        if let Err(error) =
            crate::modules::require_ready(record.borrow().activation.as_deref())
        {
            return Step::Finished(Err(error));
        }
        self.shared.borrow_mut().emit(
            id,
            scope,
            subject,
            Stage::Started,
            record.borrow().parent,
        );
        match target {
            Kind::Catalog(catalog) => Step::Pending(Work::provider(
                self.catalog_request(&catalog, record),
            )),
            Kind::Module(_) if method == Method::Write => {
                match self.begin_module_unload(ResourceId(subject), &input) {
                    Ok(()) => Step::Pending(Work::ModuleUnload),
                    Err(error) => Step::Finished(Err(error)),
                }
            }
            Kind::Module(status) => Step::Pending(Work::provider(
                crate::modules::read(status, input),
            )),
            Kind::Provider(provider) => {
                let received = crate::binding::receive(
                    &mut self.shared.borrow_mut(),
                    &record.borrow(),
                    input,
                );
                match received {
                    Ok((subject, input)) => Step::Pending(Work::Resource(
                        provider.borrow_mut().call(method, subject, input),
                    )),
                    Err(error) => Step::Finished(Err(error)),
                }
            }
            Kind::Model(model) => begin_model(&model, input),
            Kind::Session(_) | Kind::ClosedSession { .. }
                if method == Method::Write
                    && crate::sessions::closing(&input.contract) =>
            {
                Step::Finished(self.close_session(record))
            }
            Kind::Session(session) if method == Method::Write => {
                match Self::prepare_session(record, &session) {
                    Ok(future) => Step::Pending(Work::Model(future)),
                    Err(error) => Step::Finished(Err(error)),
                }
            }
            Kind::Session(session) => {
                Step::Finished(self.read_session(record, &session))
            }
            Kind::Snapshot(snapshot) => {
                Step::Finished(self.snapshot_request(record, &snapshot))
            }
            Kind::Buffer(_) | Kind::ClosedBuffer { .. }
                if method == Method::Write
                    && crate::closing::contract(&input.contract) =>
            {
                Step::Finished(self.close_buffer(record))
            }
            Kind::Buffer(buffer)
                if method == Method::Read
                    && crate::snapshots::contract(&input.contract) =>
            {
                Step::Finished(self.retain_state(record, &buffer))
            }
            Kind::Buffer(buffer) => begin_buffer(&buffer, method, input),
            Kind::History(buffer) => Step::Finished(history(&buffer, &input)),
            _ => Step::Finished(Err(Error::UNSUPPORTED)),
        }
    }

    fn poll_module_unload(&self, record: &Rc<RefCell<Record>>) -> Step {
        let record = record.borrow();
        match self
            .module_unload_result(ResourceId(record.subject), &record.input)
        {
            Ok(Some(reply)) => Step::Finished(Ok(reply)),
            Ok(None) => Step::Pending(Work::ModuleUnload),
            Err(error) => Step::Finished(Err(error)),
        }
    }

    fn install(
        &self,
        record: &Rc<RefCell<Record>>,
        result: Result<State, Error>,
    ) -> Result<Message, Error> {
        let mut candidate = result?;
        crate::modules::require_ready(record.borrow().activation.as_deref())?;
        if record.borrow().cancelled {
            return Err(Error::CANCELLED);
        }
        let target = record.borrow().target.clone();
        if let Kind::Session(session) = &target {
            return self.install_session(record, session, candidate);
        }
        let model = match &target {
            Kind::Model(model) => Rc::clone(model),
            Kind::Buffer(buffer) => Rc::clone(&buffer.borrow().model),
            _ => return Err(Error::PROVIDER),
        };
        candidate.retain(model.clone());
        if reovim_kernel_interface::session::is_create(
            &record.borrow().input.contract,
        ) {
            return self.create_session(record, model, candidate);
        }
        let value = model.borrow_mut().observe(&candidate)?;
        let limit = self.shared.borrow().limits.message_bytes;
        let mut reply =
            prepare_observation(value, &model.borrow().read, limit)?;
        if record.borrow().cancelled {
            return Err(Error::CANCELLED);
        }
        let buffer = match target {
            Kind::Model(model) => {
                self.create_buffer(record, model, candidate)?
            }
            Kind::Buffer(buffer) => {
                let expected = integer(&record.borrow().input.data[..8])?;
                let installed =
                    buffer.borrow_mut().state.install(expected, candidate);
                installed.map_err(|(error, _candidate)| error)?;
                buffer
            }
            _ => return Err(Error::PROVIDER),
        };
        self.accepted(record, &buffer);
        stamp_observation(&mut reply, &buffer.borrow());
        Ok(reply)
    }

    fn create_buffer(
        &self,
        record: &Rc<RefCell<Record>>,
        model: Rc<RefCell<reovim_kernel_interface::buffer::ModelProvider>>,
        initial: State,
    ) -> Result<Rc<RefCell<Editing>>, Error> {
        let limits = self.shared.borrow().limits;
        let parent = self.reserve_buffer(record)?;
        let state = crate::history::History::new(
            initial,
            limits.history_nodes,
            limits.state_bytes,
        )
        .map_err(|(error, _initial)| error)?;
        let mut shared = self.shared.borrow_mut();
        let root = record.borrow().scope.borrow().root;
        let resource = shared.id()?;
        let history = shared.id()?;
        let owner = record.borrow().scope.borrow().owner;
        let buffer = Rc::new(RefCell::new(Editing {
            state,
            model,
            resource,
            history,
            owner,
        }));
        let name = alloc::format!("{resource}").into();
        let parent = parent.unwrap_or_else(|| {
            shared.insert_at(root, "buffers", Kind::Directory)
        });
        shared.entries.push(Entry {
            key: resource,
            parent,
            name,
            kind: Kind::Buffer(Rc::clone(&buffer)),
            activation: record.borrow().activation.clone(),
            receiver: None,
            group: None,
            ownership: None,
        });
        shared.entries.push(Entry {
            key: history,
            parent: resource,
            name: "history".into(),
            kind: Kind::History(Rc::clone(&buffer)),
            activation: record.borrow().activation.clone(),
            receiver: None,
            group: None,
            ownership: None,
        });
        record
            .borrow()
            .scope
            .borrow_mut()
            .grants
            .push((resource, Permissions::ALL));
        Ok(buffer)
    }

    fn reserve_buffer(
        &self,
        record: &Rc<RefCell<Record>>,
    ) -> Result<Option<u64>, Error> {
        let shared = self.shared.borrow();
        let record = record.borrow();
        let mut scope = record.scope.borrow_mut();
        let parent =
            crate::binding::state_container(&shared, &scope, "buffers", 2)?;
        scope.grants.try_reserve(1).map_err(|_| Error::PRESSURE)?;
        Ok(parent)
    }

    fn accepted(
        &self,
        record: &Rc<RefCell<Record>>,
        buffer: &Rc<RefCell<Editing>>,
    ) {
        let record = record.borrow();
        let buffer = buffer.borrow();
        self.shared.borrow_mut().emit(
            record.id,
            record.scope.borrow().id,
            buffer.resource,
            Stage::Accepted {
                revision: buffer.state.revision(),
                node: buffer.state.node() as u64,
            },
            record.parent,
        );
    }

    fn settle(&self, record: &Rc<RefCell<Record>>, step: Step) {
        let result = match step {
            Step::Finished(result) => result.map_err(RequestError::Failed),
            Step::Resource(result) => result,
            Step::Pending(work) => {
                record.borrow_mut().work = Some(work);
                return;
            }
        };
        let mut record = record.borrow_mut();
        let result = self.validate_outcome(&mut record, result);
        let stage = match &result {
            Ok(_) => Stage::Finished,
            Err(RequestError::Unknown { cause, .. }) => {
                Stage::Unknown(cause.code)
            }
            Err(
                RequestError::Rejected(error) | RequestError::Failed(error),
            ) => Stage::Failed(error.code),
        };
        self.shared.borrow_mut().emit(
            record.id,
            record.scope.borrow().id,
            record.subject,
            stage,
            record.parent,
        );
        record.outcome = Some(result);
        let wakers = core::mem::take(&mut record.wakers);
        drop(record);
        for (_, waker) in wakers {
            waker.wake();
        }
    }

    fn validate_outcome(
        &self,
        record: &mut Record,
        outcome: Result<Message, RequestError>,
    ) -> Result<Message, RequestError> {
        match outcome {
            Ok(message) => self
                .validate_result(record, Ok(message))
                .map_err(RequestError::Failed),
            Err(
                RequestError::Rejected(error) | RequestError::Failed(error),
            ) => {
                Err(RequestError::Failed(crate::binding::error(record, error)))
            }
            Err(RequestError::Unknown { cause, recovery }) => {
                record.recovery_work = crate::binding::capture_recovery(
                    &mut self.shared.borrow_mut(),
                    record,
                    &recovery,
                );
                Err(RequestError::Unknown {
                    cause: crate::binding::error(record, cause),
                    recovery,
                })
            }
        }
    }

    fn validate_result(
        &self,
        record: &mut Record,
        result: Result<Message, Error>,
    ) -> Result<Message, Error> {
        let mut result =
            result.map_err(|error| crate::binding::error(record, error))?;
        let shared = self.shared.borrow();
        if result.resources.len() > shared.limits.resources {
            return Err(Error::PROVIDER);
        }
        if crate::binding::forwarding(record) {
            record.output =
                crate::binding::output_rights(&shared, record, &result)?;
        }
        if result.data.len() > shared.limits.message_bytes
            || result.resources.len() != record.output.len()
        {
            return Err(Error::PROVIDER);
        }
        let valid_contract = match &record.target {
            Kind::Model(model) => result.contract == model.borrow().read,
            Kind::Session(_)
                if record.method == Method::Write
                    && crate::sessions::closing(&record.input.contract) =>
            {
                result.contract == record.input.contract
            }
            Kind::Session(session) => {
                result.contract == crate::sessions::read_contract(session)
            }
            Kind::Buffer(_)
                if record.method == Method::Read
                    && crate::snapshots::contract(&record.input.contract) =>
            {
                result.contract == record.input.contract
            }
            Kind::Buffer(_)
                if record.method == Method::Write
                    && crate::closing::contract(&record.input.contract) =>
            {
                result.contract == record.input.contract
            }
            Kind::Buffer(buffer) => {
                result.contract == buffer.borrow().model.borrow().read
            }
            _ => {
                result.contract == record.input.contract
                    || crate::binding::forwarding(record)
            }
        };
        if !valid_contract {
            return Err(Error::PROVIDER);
        }
        crate::binding::returned(&shared, record, &mut result)?;
        let scope = record.scope.borrow();
        let valid =
            result.resources.iter().zip(&record.output).all(|(id, rights)| {
                scope
                    .permissions(&shared, id.0)
                    .is_some_and(|allowed| allowed.contains(*rights))
            });
        if !valid {
            return Err(Error::PROVIDER);
        }
        Ok(result)
    }
}

fn begin_buffer(
    buffer: &Rc<RefCell<Editing>>,
    method: Method,
    mut input: Message,
) -> Step {
    if method == Method::Read {
        return Step::Finished(if input.data.is_empty() {
            observe(buffer)
        } else {
            Err(Error::INVALID)
        });
    }
    let Some(expected) =
        input.data.get(..8).and_then(|data| integer(data).ok())
    else {
        return Step::Finished(Err(Error::INVALID));
    };
    let (state, model, revision) = {
        let buffer = buffer.borrow();
        (
            buffer.state.state(),
            Rc::clone(&buffer.model),
            buffer.state.revision(),
        )
    };
    if expected != revision {
        return Step::Finished(Err(Error::STALE));
    }
    input.data = input.data[8..].into();
    input.resources = Box::new([]);
    Step::Pending(Work::Model(model.borrow_mut().prepare(&state, input)))
}

fn observe(buffer: &Rc<RefCell<Editing>>) -> Result<Message, Error> {
    let (state, model, revision, node, resource, history) = {
        let buffer = buffer.borrow();
        (
            buffer.state.state(),
            Rc::clone(&buffer.model),
            buffer.state.revision(),
            buffer.state.node(),
            buffer.resource,
            buffer.history,
        )
    };
    let mut message = model.borrow_mut().observe(&state)?;
    let mut data = Vec::with_capacity(16 + message.data.len());
    data.extend_from_slice(&revision.to_le_bytes());
    data.extend_from_slice(&(node as u64).to_le_bytes());
    data.extend_from_slice(&message.data);
    message.data = data.into();
    message.resources = Box::new([ResourceId(resource), ResourceId(history)]);
    Ok(message)
}

fn history(
    buffer: &Rc<RefCell<Editing>>,
    input: &Message,
) -> Result<Message, Error> {
    if input.contract.name.as_ref() == "kernel.history.children" {
        if !input.data.is_empty() {
            return Err(Error::INVALID);
        }
        let children: Vec<u8> = buffer
            .borrow()
            .state
            .children()
            .into_iter()
            .flat_map(|node| (node as u64).to_le_bytes())
            .collect();
        return Ok(Message::new(input.contract.clone(), &children));
    }
    let expected = integer(input.data.get(..8).ok_or(Error::INVALID)?)?;
    let child = match input.data.len() {
        8 => None,
        16 => Some(
            usize::try_from(integer(&input.data[8..])?)
                .map_err(|_| Error::INVALID)?,
        ),
        _ => return Err(Error::INVALID),
    };
    let (revision, node) = {
        let mut buffer = buffer.borrow_mut();
        buffer.state.restore(
            expected,
            input.contract.name.as_ref() == "kernel.history.undo",
            child,
        )?;
        (buffer.state.revision(), buffer.state.node())
    };
    let mut data = Vec::new();
    data.extend_from_slice(&revision.to_le_bytes());
    data.extend_from_slice(&(node as u64).to_le_bytes());
    Ok(Message::new(input.contract.clone(), &data))
}

// Prepare the complete reply before transferring candidate state.
fn prepare_observation(
    mut value: Message,
    contract: &reovim_kernel_interface::Contract,
    limit: usize,
) -> Result<Message, Error> {
    let size = value.data.len().checked_add(16).ok_or(Error::PRESSURE)?;
    if value.contract != *contract
        || !value.resources.is_empty()
        || size > limit
    {
        return Err(Error::PROVIDER);
    }
    let mut data = Vec::new();
    data.try_reserve_exact(size).map_err(|_| Error::PRESSURE)?;
    data.resize(16, 0);
    data.extend_from_slice(&value.data);
    value.data = data.into();
    value.resources = Box::new([ResourceId(0), ResourceId(0)]);
    Ok(value)
}

fn stamp_observation(message: &mut Message, buffer: &Editing) {
    message.data[..8].copy_from_slice(&buffer.state.revision().to_le_bytes());
    message.data[8..16]
        .copy_from_slice(&(buffer.state.node() as u64).to_le_bytes());
    message.resources[0] = ResourceId(buffer.resource);
    message.resources[1] = ResourceId(buffer.history);
}

fn begin_model(
    model: &Rc<RefCell<reovim_kernel_interface::buffer::ModelProvider>>,
    input: Message,
) -> Step {
    let input = if reovim_kernel_interface::session::is_create(&input.contract)
    {
        match reovim_kernel_interface::session::decode_initial(&input) {
            Ok(value) if value.contract == model.borrow().create => value,
            Ok(_) => return Step::Finished(Err(Error::UNSUPPORTED)),
            Err(error) => return Step::Finished(Err(error)),
        }
    } else {
        input
    };
    Step::Pending(Work::Model(model.borrow_mut().initialize(input)))
}
