use alloc::{boxed::Box, rc::Rc, vec::Vec};
use core::{
    cell::RefCell,
    task::{Context, Poll},
};
use reovim_kernel_interface::access::Method;
use reovim_kernel_interface::buffer::{State, integer};
use reovim_kernel_interface::provider::Permissions;
use reovim_kernel_interface::{Error, Message, ResourceId};

use crate::namespace::{Entry, Kind};
use crate::request::{Record, Work};
use crate::{Editing, Progress, Runtime, diagnostics::Stage};

enum Step {
    Pending(Work),
    Finished(Result<Message, Error>),
}

impl Runtime {
    pub(super) fn advance_one(
        &mut self,
        context: &mut Context<'_>,
    ) -> Result<Progress, Error> {
        if !self.shared.borrow().ready {
            return Err(Error::NOT_READY);
        }
        let Some(record) = self.next_request() else {
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

    fn next_request(&mut self) -> Option<Rc<RefCell<Record>>> {
        let shared = self.shared.borrow();
        let total = shared.requests.len();
        let found = (0..total)
            .map(|offset| (self.cursor + offset) % total)
            .find(|index| shared.requests[*index].borrow().outcome.is_none())?;
        self.cursor = (found + 1) % total;
        Some(Rc::clone(&shared.requests[found]))
    }

    fn execute(
        &self,
        record: &Rc<RefCell<Record>>,
        work: Work,
        context: &mut Context<'_>,
    ) -> Step {
        match work {
            Work::Queued => self.begin(record),
            Work::Resource(mut future) => match future.as_mut().poll(context) {
                Poll::Pending => Step::Pending(Work::Resource(future)),
                Poll::Ready(result) => Step::Finished(result),
            },
            Work::Model(mut future) => match future.as_mut().poll(context) {
                Poll::Pending => Step::Pending(Work::Model(future)),
                Poll::Ready(result) => {
                    Step::Finished(self.install(record, result))
                }
            },
        }
    }

    fn begin(&self, record: &Rc<RefCell<Record>>) -> Step {
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
        self.shared.borrow_mut().emit(
            id,
            scope,
            subject,
            Stage::Started,
            record.borrow().parent,
        );
        match target {
            Kind::Provider(provider) => Step::Pending(Work::Resource(
                provider.borrow_mut().call(method, ResourceId(subject), input),
            )),
            Kind::Model(model) => {
                Step::Pending(Work::Model(model.borrow_mut().initialize(input)))
            }
            Kind::Buffer(_) | Kind::ClosedBuffer { .. }
                if crate::closing::contract(&input.contract) =>
            {
                Step::Finished(self.close_buffer(record))
            }
            Kind::Buffer(buffer) => begin_buffer(&buffer, method, input),
            Kind::History(buffer) => Step::Finished(history(&buffer, &input)),
            _ => Step::Finished(Err(Error::UNSUPPORTED)),
        }
    }

    fn install(
        &self,
        record: &Rc<RefCell<Record>>,
        result: Result<State, Error>,
    ) -> Result<Message, Error> {
        let mut candidate = result?;
        if record.borrow().cancelled {
            return Err(Error::CANCELLED);
        }
        let target = record.borrow().target.clone();
        let model = match &target {
            Kind::Model(model) => Rc::clone(model),
            Kind::Buffer(buffer) => Rc::clone(&buffer.borrow().model),
            _ => return Err(Error::PROVIDER),
        };
        candidate.retain(model.clone());
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
                    buffer.borrow_mut().core.install(expected, candidate);
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
        self.reserve_buffer(record)?;
        let make = self.shared.borrow().core;
        let core = make(initial, limits.history_nodes, limits.state_bytes)
            .map_err(|(error, _initial)| error)?;
        let mut shared = self.shared.borrow_mut();
        let resource = shared.id()?;
        let history = shared.id()?;
        let owner = record.borrow().scope.borrow().id;
        let buffer = Rc::new(RefCell::new(Editing {
            core,
            model,
            resource,
            history,
            owner,
        }));
        shared.entries.push(Entry {
            key: resource,
            parent: 3,
            name: alloc::format!("{resource}").into(),
            kind: Kind::Buffer(Rc::clone(&buffer)),
        });
        shared.entries.push(Entry {
            key: history,
            parent: resource,
            name: "history".into(),
            kind: Kind::History(Rc::clone(&buffer)),
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
    ) -> Result<(), Error> {
        let shared = self.shared.borrow();
        if shared.entries.len().saturating_add(2) > shared.limits.resources
            || !shared.ids.can_take(2)
        {
            return Err(Error::PRESSURE);
        }
        record
            .borrow()
            .scope
            .borrow_mut()
            .grants
            .try_reserve(1)
            .map_err(|_| Error::PRESSURE)
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
                revision: buffer.core.revision(),
                node: buffer.core.node() as u64,
            },
            record.parent,
        );
    }

    fn settle(&self, record: &Rc<RefCell<Record>>, step: Step) {
        let result = match step {
            Step::Finished(result) => result,
            Step::Pending(work) => {
                record.borrow_mut().work = Some(work);
                return;
            }
        };
        let result = self.validate_result(record, result);
        let mut record = record.borrow_mut();
        let stage = result.as_ref().map_or_else(
            |error| Stage::Failed(error.code),
            |_| Stage::Finished,
        );
        self.shared.borrow_mut().emit(
            record.id,
            record.scope.borrow().id,
            record.subject,
            stage,
            record.parent,
        );
        record.outcome = Some(result);
        let waker = record.waker.take();
        drop(record);
        if let Some(waker) = waker {
            waker.wake();
        }
    }

    fn validate_result(
        &self,
        record: &Rc<RefCell<Record>>,
        result: Result<Message, Error>,
    ) -> Result<Message, Error> {
        let result = result?;
        let shared = self.shared.borrow();
        let record = record.borrow();
        if result.data.len() > shared.limits.message_bytes
            || result.resources.len() != record.output.len()
        {
            return Err(Error::PROVIDER);
        }
        let valid_contract = match &record.target {
            Kind::Model(model) => result.contract == model.borrow().read,
            Kind::Buffer(_)
                if crate::closing::contract(&record.input.contract) =>
            {
                result.contract == record.input.contract
            }
            Kind::Buffer(buffer) => {
                result.contract == buffer.borrow().model.borrow().read
            }
            _ => result.contract == record.input.contract,
        };
        if !valid_contract {
            return Err(Error::PROVIDER);
        }
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
        (buffer.core.state(), Rc::clone(&buffer.model), buffer.core.revision())
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
            buffer.core.state(),
            Rc::clone(&buffer.model),
            buffer.core.revision(),
            buffer.core.node(),
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
            .core
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
        buffer.core.restore(
            expected,
            input.contract.name.as_ref() == "kernel.history.undo",
            child,
        )?;
        (buffer.core.revision(), buffer.core.node())
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
    message.data[..8].copy_from_slice(&buffer.core.revision().to_le_bytes());
    message.data[8..16]
        .copy_from_slice(&(buffer.core.node() as u64).to_le_bytes());
    message.resources[0] = ResourceId(buffer.resource);
    message.resources[1] = ResourceId(buffer.history);
}
