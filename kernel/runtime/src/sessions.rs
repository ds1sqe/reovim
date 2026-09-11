mod access;
mod close;
mod update;
pub use access::{closing, read_contract, surface};
pub use update::capture_snapshot;

use crate::{
    Runtime,
    namespace::{Entry, Kind},
    request::Record,
};
use alloc::{boxed::Box, format, rc::Rc};
use core::cell::RefCell;
use reovim_kernel_interface::{
    Error, Message, ResourceId,
    buffer::{ModelProvider, State},
    provider::Permissions,
};

pub struct Session {
    model: Rc<RefCell<ModelProvider>>,
    state: State,
    revision: u64,
    pub(crate) owner: u64,
}

pub struct Snapshot {
    session: ResourceId,
    revision: u64,
    pub(crate) owner: u64,
    value: Message,
}

impl Runtime {
    pub(super) fn create_session(
        &self,
        record: &Rc<RefCell<Record>>,
        model: Rc<RefCell<ModelProvider>>,
        state: State,
    ) -> Result<Message, Error> {
        let limits = self.shared.borrow().limits;
        let value = observe(&model, &state, limits)?;
        let record = record.borrow();
        if record.cancelled {
            return Err(Error::CANCELLED);
        }
        let mut shared = self.shared.borrow_mut();
        let mut scope = record.scope.borrow_mut();
        let parent =
            crate::binding::state_container(&shared, &scope, "sessions", 2)?;
        scope.grants.try_reserve(2).map_err(|_| Error::PRESSURE)?;
        let session = Rc::new(RefCell::new(Session {
            model,
            state,
            revision: 1,
            owner: scope.owner,
        }));
        let parent = parent.unwrap_or_else(|| {
            shared.insert_at(scope.root, "sessions", Kind::Directory)
        });
        let id = shared.insert_at(
            parent,
            &format!("{}", record.id),
            Kind::Session(session),
        );
        let index = shared.entry_index(id).expect("new Session");
        shared.entries[index].activation.clone_from(&record.activation);
        scope.grants.push((id, Permissions::ALL));
        Ok(snapshot(&mut shared, &mut scope, ResourceId(id), 1, value))
    }

    pub(super) fn read_session(
        &self,
        record: &Rc<RefCell<Record>>,
        session: &Rc<RefCell<Session>>,
    ) -> Result<Message, Error> {
        if !record.borrow().input.data.is_empty() {
            return Err(Error::INVALID);
        }
        let (model, state, revision) = {
            let session = session.borrow();
            (Rc::clone(&session.model), session.state.clone(), session.revision)
        };
        let limits = self.shared.borrow().limits;
        let value = observe(&model, &state, limits)?;
        let record = record.borrow();
        if record.cancelled {
            return Err(Error::CANCELLED);
        }
        let mut shared = self.shared.borrow_mut();
        reserve_snapshot(&shared)?;
        let mut scope = record.scope.borrow_mut();
        scope.grants.try_reserve(1).map_err(|_| Error::PRESSURE)?;
        Ok(snapshot(
            &mut shared,
            &mut scope,
            ResourceId(record.subject),
            revision,
            value,
        ))
    }
}

fn observe(
    model: &Rc<RefCell<ModelProvider>>,
    state: &State,
    limits: crate::Limits,
) -> Result<Message, Error> {
    if state.cost > limits.state_bytes {
        return Err(Error::PRESSURE);
    }
    let value = model.borrow_mut().observe(state)?;
    if value.contract != model.borrow().read
        || !value.resources.is_empty()
        || value.data.len() > limits.message_bytes
    {
        return Err(Error::PROVIDER);
    }
    Ok(value)
}

fn reserve_snapshot(shared: &crate::Shared) -> Result<(), Error> {
    if shared.entries.len() == shared.limits.resources
        || !shared.ids.can_take(1)
    {
        return Err(Error::PRESSURE);
    }
    Ok(())
}

fn snapshot(
    shared: &mut crate::Shared,
    scope: &mut crate::namespace::Scope,
    session: ResourceId,
    revision: u64,
    value: Message,
) -> Message {
    let key = shared.ids.take().expect("snapshot identity reserved");
    let mut reply = value.clone();
    let snapshot = Snapshot { session, revision, owner: scope.owner, value };
    shared.entries.push(Entry {
        receiver: None,
        group: None,
        ownership: None,
        key,
        parent: session.0,
        name: format!("{key}").into(),
        kind: Kind::Snapshot(Rc::new(snapshot)),
        activation: None,
    });
    scope.grants.push((key, Permissions::ALL));
    reply.resources = Box::new([session, ResourceId(key)]);
    reply
}
