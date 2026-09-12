use super::{Session, references};
use crate::{
    Runtime,
    namespace::{Entry, Kind, Ownership},
    request::Record,
};
use alloc::{format, rc::Rc, vec::Vec};
use core::cell::RefCell;
use reovim_kernel_interface::{
    Contract, Error, Message, ResourceId, provider::Permissions,
};

#[derive(Clone, Copy)]
pub struct Change {
    pub request: u64,
    pub before: u64,
    pub after: u64,
}

pub fn contract(value: &Contract) -> bool {
    value == &Contract::new("kernel.session.observe", 1)
}

impl Runtime {
    pub(super) fn observe_session(
        &self,
        record: &Rc<RefCell<Record>>,
        session: &Rc<RefCell<Session>>,
    ) -> Result<Message, Error> {
        if !record.borrow().input.data.is_empty() {
            return Err(Error::INVALID);
        }
        let (model, state, revision, receiver, feed) = {
            let session = session.borrow();
            (
                session.model.clone(),
                session.state.clone(),
                session.revision,
                session.receiver.clone(),
                session.feed.clone(),
            )
        };
        let limits = self.shared.borrow().limits;
        if limits.message_bytes < 25 {
            return Err(Error::PRESSURE);
        }
        let message = super::observe(&model, &state, limits)?;
        let value = references::capture(
            &self.shared.borrow(),
            receiver.as_ref(),
            &model.borrow(),
            message,
        )?;
        references::set_output(record, &value, limits.resources)?;
        let data = envelope(&value.message, revision, limits.message_bytes)?;
        let mut resources = Vec::new();
        resources
            .try_reserve_exact(3 + value.message.resources.len())
            .map_err(|_| Error::PRESSURE)?;
        resources.extend_from_slice(&[ResourceId(0); 3]);
        resources.extend_from_slice(&value.message.resources);
        let mut reply = Message {
            contract: record.borrow().input.contract.clone(),
            data: data.into(),
            resources: resources.into(),
        };
        let reader = feed.borrow_mut().prepare()?;
        let provider =
            crate::watch::mapped(reader.clone(), "kernel.session.next", encode);
        let record = record.borrow();
        if record.cancelled {
            return Err(Error::CANCELLED);
        }
        let mut shared = self.shared.borrow_mut();
        let mut scope = record.scope.borrow_mut();
        let parent =
            crate::binding::state_container(&shared, &scope, "snapshots", 2)?;
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
        let watch = shared.id()?;
        let snapshot = shared.id()?;
        let parent = parent.unwrap_or_else(|| {
            shared.insert_at(scope.root, "snapshots", Kind::Directory)
        });
        super::publish_snapshot(
            &mut shared,
            &mut scope,
            snapshot,
            ResourceId(record.subject),
            revision,
            value,
        );
        reply.resources[..3].copy_from_slice(&[
            ResourceId(record.subject),
            ResourceId(snapshot),
            ResourceId(watch),
        ]);
        shared.entries.push(Entry {
            key: watch,
            parent,
            name: format!("{watch}").into(),
            kind: Kind::Provider(Rc::new(RefCell::new(provider))),
            activation: None,
            receiver: None,
            group: None,
            ownership: Some(Ownership { client: scope.owner, closed: false }),
        });
        scope.grants.push((watch, Permissions::READ));
        feed.borrow_mut().subscribe(&reader);
        Ok(reply)
    }
}

fn envelope(
    value: &Message,
    revision: u64,
    limit: usize,
) -> Result<Vec<u8>, Error> {
    let name = value.contract.name.as_bytes();
    let length = u32::try_from(name.len()).map_err(|_| Error::PRESSURE)?;
    let size = 16_usize
        .checked_add(name.len())
        .and_then(|n| n.checked_add(value.data.len()))
        .filter(|n| *n <= limit)
        .ok_or(Error::PRESSURE)?;
    let mut data = Vec::new();
    data.try_reserve_exact(size).map_err(|_| Error::PRESSURE)?;
    data.extend_from_slice(&revision.to_le_bytes());
    data.extend_from_slice(&value.contract.version.to_le_bytes());
    data.extend_from_slice(&length.to_le_bytes());
    data.extend_from_slice(name);
    data.extend_from_slice(&value.data);
    Ok(data)
}
fn encode(contract: &Contract, change: Change) -> Message {
    let mut data = [0; 25];
    data[1..9].copy_from_slice(&change.request.to_le_bytes());
    data[9..17].copy_from_slice(&change.before.to_le_bytes());
    data[17..].copy_from_slice(&change.after.to_le_bytes());
    Message::new(contract.clone(), &data)
}
