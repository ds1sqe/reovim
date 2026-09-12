//! Retained Model reads use the ordinary owned Resource lifetime.

use alloc::{boxed::Box, rc::Rc};
use core::{cell::RefCell, future::ready};
use reovim_kernel_interface::{
    Contract, Error, Message, ResourceId,
    buffer::{ModelProvider, State, integer},
    provider::{
        Permissions, ProviderReply, ResourceOps, ResourceProvider, Surface,
    },
};

use crate::{
    Editing, Runtime,
    namespace::{Entry, Kind, Ownership},
    request::Record,
};

pub fn contract(value: &Contract) -> bool {
    value == &Contract::new("kernel.buffer.retain", 1)
}

struct ReadState {
    state: State,
    model: Rc<RefCell<ModelProvider>>,
}

impl Runtime {
    pub(super) fn retain_state(
        &self,
        record: &Rc<RefCell<Record>>,
        buffer: &Rc<RefCell<Editing>>,
    ) -> Result<Message, Error> {
        let record = record.borrow();
        let expected = integer(&record.input.data)?;
        let buffer = buffer.borrow();
        if expected != buffer.state.revision() {
            return Err(Error::STALE);
        }
        let context = ReadState {
            state: buffer.state.state(),
            model: buffer.model.clone(),
        };
        let surface = Surface {
            contract: buffer.model.borrow().read.clone(),
            methods: Permissions::READ,
            input: Box::new([]),
            output: Box::new([]),
        };
        let provider = ResourceProvider::new(
            context,
            ResourceOps { unload: None, read, write: unsupported },
            Box::new([surface]),
            Rc::new(()),
        )
        .owned(|| {});
        let mut reply = Message::new(
            record.input.contract.clone(),
            &[
                expected.to_le_bytes(),
                (buffer.state.node() as u64).to_le_bytes(),
            ]
            .concat(),
        );
        reply.resources =
            Box::new([ResourceId(buffer.resource), ResourceId(0)]);
        drop(buffer);
        let mut shared = self.shared.borrow_mut();
        let mut scope = record.scope.borrow_mut();
        let parent =
            crate::binding::state_container(&shared, &scope, "snapshots", 1)?;
        scope.grants.try_reserve(1).map_err(|_| Error::PRESSURE)?;
        let id = shared.id()?;
        let name = alloc::format!("{id}").into();
        let provider = Rc::new(RefCell::new(provider));
        let parent = parent.unwrap_or_else(|| {
            shared.insert_at(scope.root, "snapshots", Kind::Directory)
        });
        shared.entries.push(Entry {
            key: id,
            parent,
            name,
            kind: Kind::Provider(provider),
            activation: None,
            receiver: None,
            group: None,
            ownership: Some(Ownership { client: scope.owner, closed: false }),
        });
        scope.grants.push((id, Permissions::READ));
        reply.resources[1] = ResourceId(id);
        Ok(reply)
    }
}

fn read(
    context: &mut ReadState,
    _: ResourceId,
    input: Message,
) -> ProviderReply {
    Box::pin(ready(context.model.borrow_mut().inspect(&context.state, input)))
}

fn unsupported(_: &mut ReadState, _: ResourceId, _: Message) -> ProviderReply {
    Box::pin(ready(Err(Error::UNSUPPORTED)))
}
