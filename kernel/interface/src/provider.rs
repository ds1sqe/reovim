use alloc::{boxed::Box, rc::Rc};
use core::{future::Future, pin::Pin};

use crate::{Contract, Error, Message, ResourceId, access::Method};

/// Ready contributions transferred together by the activation owner.
/// Names are captured here; installed namespace entries remain private.
pub enum Contribution {
    Resource { path: Box<str>, provider: ResourceProvider },
    Model { path: Box<str>, provider: crate::buffer::ModelProvider },
}

impl Contribution {
    #[must_use]
    pub fn path(&self) -> &str {
        match self {
            Self::Resource { path, .. } | Self::Model { path, .. } => path,
        }
    }
}

/// Rights are checked for the subject and each declared reference slot.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Permissions {
    pub read: bool,
    pub write: bool,
}

impl Permissions {
    pub const READ: Self = Self { read: true, write: false };
    pub const WRITE: Self = Self { read: false, write: true };
    pub const ALL: Self = Self { read: true, write: true };

    #[must_use]
    pub const fn allows(self, method: Method) -> bool {
        match method {
            Method::Read => self.read,
            Method::Write => self.write,
        }
    }

    #[must_use]
    pub const fn contains(self, other: Self) -> bool {
        (!other.read || self.read) && (!other.write || self.write)
    }
}

/// This initial profile transfers borrowed subject references, never leases.
#[derive(Clone, Debug)]
pub struct Surface {
    pub contract: Contract,
    pub methods: Permissions,
    pub input: Box<[Permissions]>,
    pub output: Box<[Permissions]>,
}

pub type ProviderReply = Pin<Box<dyn Future<Output = Result<Message, Error>>>>;

/// Runtime alone invokes and polls the selected table outside Core authority.
pub type Unload<State> = fn(&mut State) -> Result<(), Error>;

pub struct ResourceOps<State> {
    /// Checks readiness outside registry guards; leaves the provider usable.
    /// Another contribution may reject the owner's grouped unload.
    pub unload: Option<Unload<State>>,
    pub read: fn(&mut State, ResourceId, Message) -> ProviderReply,
    pub write: fn(&mut State, ResourceId, Message) -> ProviderReply,
}

trait Dispatch {
    fn unload(&mut self) -> Result<(), Error>;
    fn call(
        &mut self,
        method: Method,
        subject: ResourceId,
        input: Message,
    ) -> ProviderReply;
}

struct Implementation<State> {
    state: State,
    operations: ResourceOps<State>,
}

impl<State> Dispatch for Implementation<State> {
    fn unload(&mut self) -> Result<(), Error> {
        self.operations.unload.map_or(Ok(()), |unload| unload(&mut self.state))
    }

    fn call(
        &mut self,
        method: Method,
        subject: ResourceId,
        input: Message,
    ) -> ProviderReply {
        match method {
            Method::Read => {
                (self.operations.read)(&mut self.state, subject, input)
            }
            Method::Write => {
                (self.operations.write)(&mut self.state, subject, input)
            }
        }
    }
}

/// Erased same-build implementation; Runtime's type never enumerates providers.
pub struct ResourceProvider {
    pub surfaces: Box<[Surface]>,
    implementation: Box<dyn Dispatch>,
    // Keep executable ownership after implementation/context destruction.
    _code: Rc<dyn core::any::Any>,
}

impl ResourceProvider {
    #[must_use]
    pub fn new<State: 'static>(
        state: State,
        operations: ResourceOps<State>,
        surfaces: Box<[Surface]>,
        code: Rc<dyn core::any::Any>,
    ) -> Self {
        Self {
            surfaces,
            implementation: Box::new(Implementation { state, operations }),
            _code: code,
        }
    }

    /// # Errors
    /// Busy leaves the provider usable; success permits context destruction.
    pub fn unload(&mut self) -> Result<(), Error> {
        self.implementation.unload()
    }

    pub fn call(
        &mut self,
        method: Method,
        subject: ResourceId,
        input: Message,
    ) -> ProviderReply {
        self.implementation.call(method, subject, input)
    }
}
