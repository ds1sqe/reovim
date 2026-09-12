//! Buffer metadata and same-build immutable model preparation.

use alloc::{boxed::Box, rc::Rc, vec::Vec};
use core::{any::Any, future::Future, pin::Pin};

use crate::{Contract, Error, Message, ResourceId};

mod descriptor;
mod snapshot;
pub use descriptor::{BufferDescriptor, describe, descriptor};
pub use snapshot::{RetainedState, retain, retained};
mod watch;
pub use watch::{
    Change, Transition, Watching, change, next, observe, watching,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct BufferState {
    pub buffer: ResourceId,
    pub revision: u64,
}

/// A copied semantic observation; metadata owns no Buffer or History lease.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Observation {
    pub state: BufferState,
    pub history: ResourceId,
    pub node: u64,
    pub value: Message,
}

/// Immutable provider data. Runtime pins the selected implementation in it.
#[derive(Clone)]
pub struct State {
    value: Rc<dyn Any>,
    owner: Option<Rc<dyn Any>>,
    pub cost: u64,
}

impl State {
    #[must_use]
    pub fn new<T: 'static>(value: T, cost: u64) -> Self {
        Self { value: Rc::new(value), owner: None, cost }
    }

    #[must_use]
    pub fn get<T: 'static>(&self) -> Option<&T> {
        self.value.downcast_ref()
    }

    /// Called by the preparation owner before installing state.
    pub fn retain(&mut self, owner: Rc<dyn Any>) {
        self.owner = Some(owner);
    }
}

pub type StateReply = Pin<Box<dyn Future<Output = Result<State, Error>>>>;

/// Providers prepare before installation; candidates cannot mutate live state.
pub struct ModelOps<Context> {
    /// Runs only after retained states/work drain, outside registry guards.
    pub unload: Option<crate::provider::Unload<Context>>,
    pub initialize: fn(&mut Context, Message) -> StateReply,
    pub prepare: fn(&mut Context, &State, Message) -> StateReply,
    /// Empty input requests the finite descriptor used in acceptance replies.
    /// Other payloads describe model-owned reads of the same immutable state.
    /// Must not wait on external work; deferred reads use inspect.
    pub observe: fn(&mut Context, &State, Message) -> Result<Message, Error>,
    /// Optional deferred inspection. Owns state until its read settles.
    /// None uses the finite observation callback for retained reads too.
    pub inspect: Option<
        fn(&mut Context, State, Message) -> crate::provider::ProviderReply,
    >,
}

trait Dispatch {
    fn inspect(
        &mut self,
        state: State,
        input: Message,
    ) -> crate::provider::ProviderReply;
    fn unload(&mut self) -> Result<(), Error>;
    fn initialize(&mut self, input: Message) -> StateReply;
    fn prepare(&mut self, state: &State, input: Message) -> StateReply;
    fn observe(
        &mut self,
        state: &State,
        input: Message,
    ) -> Result<Message, Error>;
}

struct Implementation<Context> {
    context: Context,
    operations: ModelOps<Context>,
}

impl<Context> Dispatch for Implementation<Context> {
    fn inspect(
        &mut self,
        state: State,
        input: Message,
    ) -> crate::provider::ProviderReply {
        if let Some(inspect) = self.operations.inspect {
            return inspect(&mut self.context, state, input);
        }
        Box::pin(core::future::ready(self.observe(&state, input)))
    }
    fn unload(&mut self) -> Result<(), Error> {
        self.operations
            .unload
            .map_or(Ok(()), |unload| unload(&mut self.context))
    }

    fn initialize(&mut self, input: Message) -> StateReply {
        (self.operations.initialize)(&mut self.context, input)
    }
    fn prepare(&mut self, state: &State, input: Message) -> StateReply {
        (self.operations.prepare)(&mut self.context, state, input)
    }

    fn observe(
        &mut self,
        state: &State,
        input: Message,
    ) -> Result<Message, Error> {
        (self.operations.observe)(&mut self.context, state, input)
    }
}

/// Runtime can accept a new model without changing its compiled type.
pub struct ModelProvider {
    pub create: Contract,
    /// Borrowed Resource slots for initialization, in declaration order.
    pub initial_input: Box<[crate::provider::Permissions]>,
    pub read: Contract,
    pub edit: Contract,
    implementation: Box<dyn Dispatch>,
    _code: Rc<dyn Any>,
}

impl ModelProvider {
    #[must_use]
    pub fn new<Context: 'static>(
        context: Context,
        operations: ModelOps<Context>,
        create: Contract,
        read: Contract,
        edit: Contract,
        code: Rc<dyn Any>,
    ) -> Self {
        Self {
            create,
            initial_input: Box::new([]),
            read,
            edit,
            implementation: Box::new(Implementation { context, operations }),
            _code: code,
        }
    }

    /// # Errors
    /// Busy preserves an active model; success permits context destruction.
    pub fn unload(&mut self) -> Result<(), Error> {
        self.implementation.unload()
    }

    pub fn initialize(&mut self, input: Message) -> StateReply {
        self.implementation.initialize(input)
    }

    pub fn prepare(&mut self, state: &State, input: Message) -> StateReply {
        self.implementation.prepare(state, input)
    }

    /// # Errors
    /// Reports unsupported state or failed semantic interpretation.
    pub fn observe(&mut self, state: &State) -> Result<Message, Error> {
        self.implementation.observe(state, Message::new(self.read.clone(), &[]))
    }

    /// Reads exact immutable state using a model-defined payload.
    /// The future owns the required state; pending residency does not block
    /// Runtime. Its result reports foreign contracts or unsupported reads.
    pub fn inspect(
        &mut self,
        state: &State,
        input: Message,
    ) -> crate::provider::ProviderReply {
        if input.contract != self.read || !input.resources.is_empty() {
            return Box::pin(core::future::ready(Err(Error::INVALID)));
        }
        self.implementation.inspect(state.clone(), input)
    }
}

/// Adds exact currentness to a component-owned semantic edit.
#[must_use]
pub fn edit(expected: BufferState, mut input: Message) -> Message {
    let mut data = Vec::with_capacity(8 + input.data.len());
    data.extend_from_slice(&expected.revision.to_le_bytes());
    data.extend_from_slice(&input.data);
    input.data = data.into();
    input.resources = Box::new([expected.buffer]);
    input
}

/// Decodes the common observation prefix and its borrowed references.
/// # Errors
/// Rejects malformed observations without guessing another schema.
pub fn observation(mut reply: Message) -> Result<Observation, Error> {
    if reply.data.len() < 16 || reply.resources.len() != 2 {
        return Err(Error::INVALID);
    }
    let revision = integer(&reply.data[..8])?;
    let node = integer(&reply.data[8..16])?;
    let buffer = reply.resources[0];
    let history = reply.resources[1];
    reply.data = reply.data[16..].into();
    reply.resources = Box::new([]);
    Ok(Observation {
        state: BufferState { buffer, revision },
        history,
        node,
        value: reply,
    })
}

/// # Errors
/// Requires exactly one little-endian u64 value.
pub fn integer(data: &[u8]) -> Result<u64, Error> {
    data.try_into().map(u64::from_le_bytes).map_err(|_| Error::INVALID)
}

/// Shared close is separate from releasing one caller's access.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ClosePolicy {
    RejectModified,
    DiscardLocal,
}

/// Requests shared close with exact currentness and explicit loss policy.
#[must_use = "Await close completion or retain its recovery access"]
pub fn close(
    connection: &crate::Connection,
    expected: BufferState,
    policy: ClosePolicy,
) -> crate::access::Reply {
    let policy = match policy {
        ClosePolicy::RejectModified => 0,
        ClosePolicy::DiscardLocal => 1,
    };
    let request = edit(
        expected,
        Message::new(Contract::new("kernel.buffer.close", 1), &[policy]),
    );
    connection.write(expected.buffer, &request)
}
