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
    value == &Contract::new("kernel.buffer.retain", 1) || observing(value)
}

pub fn observing(value: &Contract) -> bool {
    value == &Contract::new("kernel.buffer.observe", 1)
}

#[derive(Clone)]
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
        let observing = observing(&record.input.contract);
        if observing
            && self.shared.borrow().limits.message_bytes
                < crate::watch::EVENT_BYTES
        {
            return Err(Error::PRESSURE);
        }
        let buffer = buffer.borrow();
        let expected = if observing && record.input.data.is_empty() {
            buffer.state.revision()
        } else if observing {
            return Err(Error::INVALID);
        } else {
            integer(&record.input.data)?
        };
        if expected != buffer.state.revision() {
            return Err(Error::STALE);
        }
        let feed = buffer.feed.clone();
        let reader =
            if observing { Some(feed.borrow_mut().prepare()?) } else { None };
        let watch = reader
            .as_ref()
            .map(|reader| crate::watch::provider(reader.clone()));
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
        let provider = state_reader(context, surface).owned(|| {});
        let mut reply = Message::new(
            record.input.contract.clone(),
            &[
                expected.to_le_bytes(),
                (buffer.state.node() as u64).to_le_bytes(),
            ]
            .concat(),
        );
        reply.resources = if observing {
            Box::new([
                ResourceId(buffer.resource),
                ResourceId(buffer.history),
                ResourceId(0),
                ResourceId(0),
            ])
        } else {
            Box::new([ResourceId(buffer.resource), ResourceId(0)])
        };
        drop(buffer);
        let mut shared = self.shared.borrow_mut();
        let mut scope = record.scope.borrow_mut();
        let count = 1 + usize::from(observing);
        let parent = crate::binding::state_container(
            &shared,
            &scope,
            "snapshots",
            count,
        )?;
        scope.grants.try_reserve(count).map_err(|_| Error::PRESSURE)?;
        let id = shared.id()?;
        let watch_id = if observing { shared.id()? } else { 0 };
        let parent = parent.unwrap_or_else(|| {
            shared.insert_at(scope.root, "snapshots", Kind::Directory)
        });
        publish(&mut shared, &mut scope, parent, id, provider);
        if let Some(watch) = watch {
            publish(&mut shared, &mut scope, parent, watch_id, watch);
        }
        reply.resources[count] = ResourceId(id);
        if let Some(reader) = reader {
            feed.borrow_mut().subscribe(&reader);
            reply.resources[3] = ResourceId(watch_id);
        }
        Ok(reply)
    }
}

fn state_reader(context: ReadState, surface: Surface) -> ResourceProvider {
    let view = context.clone();
    let declaration = surface.clone();
    ResourceProvider::new(
        context,
        ResourceOps { unload: None, read, write: unsupported },
        Box::new([surface]),
        Rc::new(()),
    )
    .with_read_view(move || Ok(state_reader(view.clone(), declaration.clone())))
}

pub fn selected(
    model: Rc<RefCell<ModelProvider>>,
    state: State,
) -> ResourceProvider {
    let surface = Surface {
        contract: model.borrow().read.clone(),
        methods: Permissions::READ,
        input: Box::new([]),
        output: Box::new([]),
    };
    state_reader(ReadState { state, model }, surface)
}

fn publish(
    shared: &mut crate::Shared,
    scope: &mut crate::namespace::Scope,
    parent: u64,
    id: u64,
    provider: ResourceProvider,
) {
    shared.entries.push(Entry {
        key: id,
        parent,
        name: alloc::format!("{id}").into(),
        kind: Kind::Provider(Rc::new(RefCell::new(provider))),
        activation: None,
        receiver: None,
        group: None,
        ownership: Some(Ownership { client: scope.owner, closed: false }),
    });
    scope.grants.push((id, Permissions::READ));
}

fn read(
    context: &mut ReadState,
    _: ResourceId,
    input: Message,
) -> ProviderReply {
    context.model.borrow_mut().inspect(&context.state, input)
}

fn unsupported(_: &mut ReadState, _: ResourceId, _: Message) -> ProviderReply {
    Box::pin(ready(Err(Error::UNSUPPORTED)))
}
