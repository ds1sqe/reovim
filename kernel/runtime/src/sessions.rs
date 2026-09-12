mod access;
mod close;
mod participation;
mod references;
pub use participation::Participation;
mod update;
pub mod watch;
pub use access::{closing, control, read_contract, surface};
pub use update::capture_snapshot;

use crate::{
    Runtime,
    namespace::{Entry, Kind},
    request::Record,
};
use alloc::{format, rc::Rc};
use core::cell::RefCell;
use reovim_kernel_interface::{
    Error, Message, ResourceId,
    buffer::{ModelProvider, State},
    provider::Permissions,
};

pub struct Session {
    pub(crate) model: Rc<RefCell<ModelProvider>>,
    receiver: Option<Rc<RefCell<crate::namespace::Scope>>>,
    state: State,
    revision: u64,
    feed: Rc<RefCell<crate::watch::Feed<watch::Change>>>,
    pub(crate) owner: u64,
}

pub struct Snapshot {
    session: ResourceId,
    revision: u64,
    pub(crate) owner: u64,
    value: references::Value,
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
        let receiver = record.borrow().receiver.clone();
        let value = references::capture(
            &self.shared.borrow(),
            receiver.as_ref(),
            &model.borrow(),
            value,
        )?;
        references::set_output(record, &value, limits.resources)?;
        let record = record.borrow();
        if record.cancelled {
            return Err(Error::CANCELLED);
        }
        let mut shared = self.shared.borrow_mut();
        let mut scope = record.scope.borrow_mut();
        let parent =
            crate::binding::state_container(&shared, &scope, "sessions", 2)?;
        scope
            .grants
            .try_reserve(2 + value.rights.len())
            .map_err(|_| Error::PRESSURE)?;
        crate::binding::grant_outputs(
            &shared,
            &mut scope,
            &value.rights,
            &value.message,
        )?;
        let session = Rc::new(RefCell::new(Session {
            model,
            receiver,
            state,
            revision: 1,
            feed: Rc::default(),
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
        shared.entries[index].receiver.clone_from(&record.receiver);
        scope.grants.push((id, Permissions::ALL));
        Ok(snapshot(&mut shared, &mut scope, ResourceId(id), 1, value))
    }

    pub(super) fn read_session(
        &self,
        record: &Rc<RefCell<Record>>,
        session: &Rc<RefCell<Session>>,
    ) -> Result<Message, Error> {
        if watch::contract(&record.borrow().input.contract) {
            return self.observe_session(record, session);
        }
        if participation::attaching(&record.borrow().input.contract) {
            return self.attach_session(record, session);
        }
        let subject = ResourceId(record.borrow().subject);
        self.read_session_at(record, session, subject)
    }

    fn read_session_at(
        &self,
        record: &Rc<RefCell<Record>>,
        session: &Rc<RefCell<Session>>,
        subject: ResourceId,
    ) -> Result<Message, Error> {
        if !record.borrow().input.data.is_empty() {
            return Err(Error::INVALID);
        }
        let (model, state, revision, receiver) = {
            let session = session.borrow();
            (
                Rc::clone(&session.model),
                session.state.clone(),
                session.revision,
                session.receiver.clone(),
            )
        };
        let limits = self.shared.borrow().limits;
        let value = observe(&model, &state, limits)?;
        let value = references::capture(
            &self.shared.borrow(),
            receiver.as_ref(),
            &model.borrow(),
            value,
        )?;
        references::set_output(record, &value, limits.resources)?;
        let record = record.borrow();
        if record.cancelled {
            return Err(Error::CANCELLED);
        }
        let mut shared = self.shared.borrow_mut();
        reserve_snapshot(&shared)?;
        let mut scope = record.scope.borrow_mut();
        scope
            .grants
            .try_reserve(1 + value.rights.len())
            .map_err(|_| Error::PRESSURE)?;
        crate::binding::grant_outputs(
            &shared,
            &mut scope,
            &value.rights,
            &value.message,
        )?;
        Ok(snapshot(&mut shared, &mut scope, subject, revision, value))
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
    value: references::Value,
) -> Message {
    let key = shared.ids.take().expect("snapshot identity reserved");
    let mut reply = value.message.clone();
    reply.resources = [session, ResourceId(key)]
        .into_iter()
        .chain(reply.resources.iter().copied())
        .collect();
    publish_snapshot(shared, scope, key, session, revision, value);
    reply
}

fn publish_snapshot(
    shared: &mut crate::Shared,
    scope: &mut crate::namespace::Scope,
    key: u64,
    session: ResourceId,
    revision: u64,
    value: references::Value,
) {
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
}
