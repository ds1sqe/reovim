use alloc::{boxed::Box, rc::Rc};
use core::{future::Future, pin::Pin};

use crate::{
    Contract, Error, Message, ResourceId,
    access::{Method, Reply, RequestError},
};

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

reovim_bitflags::rv_bitflags! {
    /// Rights checked for the subject and each declared reference slot.
    pub struct Permissions: u32 {
        const READ = 1;
        const WRITE = 2;
        const ALL = Self::READ.bits() | Self::WRITE.bits();
    }
}

impl Permissions {
    #[must_use]
    pub const fn allows(self, method: Method) -> bool {
        match method {
            Method::Read => self.intersects(Self::READ),
            Method::Write => self.intersects(Self::WRITE),
        }
    }
}

/// Slots declare operation rights; publication determines access ownership.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Surface {
    pub contract: Contract,
    pub methods: Permissions,
    pub input: Box<[Permissions]>,
    pub output: Box<[Permissions]>,
}

/// Ordinary implementations use `Error`. Adapters may preserve `RequestError`
/// when delivery itself is uncertain; external effect evidence is a Message.
pub type ProviderReply<Failure = Error> =
    Pin<Box<dyn Future<Output = Result<Message, Failure>>>>;

/// Runtime alone invokes and polls the selected table outside Core authority.
pub type Unload<State> = fn(&mut State) -> Result<(), Error>;

pub struct ResourceOps<State, Failure = Error> {
    /// Checks readiness outside registry guards; leaves the provider usable.
    /// Another contribution may reject the owner's grouped unload.
    pub unload: Option<Unload<State>>,
    pub read: fn(&mut State, ResourceId, Message) -> ProviderReply<Failure>,
    pub write: fn(&mut State, ResourceId, Message) -> ProviderReply<Failure>,
}

trait Dispatch {
    fn unload(&mut self) -> Result<(), Error>;
    fn call(
        &mut self,
        method: Method,
        subject: ResourceId,
        input: Message,
    ) -> Reply;
}

struct Implementation<State, Failure> {
    state: State,
    operations: ResourceOps<State, Failure>,
}

impl<State, Failure: Into<RequestError> + 'static> Dispatch
    for Implementation<State, Failure>
{
    fn unload(&mut self) -> Result<(), Error> {
        self.operations.unload.map_or(Ok(()), |unload| unload(&mut self.state))
    }

    fn call(
        &mut self,
        method: Method,
        subject: ResourceId,
        input: Message,
    ) -> Reply {
        let reply = match method {
            Method::Read => {
                (self.operations.read)(&mut self.state, subject, input)
            }
            Method::Write => {
                (self.operations.write)(&mut self.state, subject, input)
            }
        };
        Box::pin(async move { reply.await.map_err(Into::into) })
    }
}

/// Erased same-build implementation; Runtime's type never enumerates providers.
pub struct ResourceProvider {
    pub surfaces: Box<[Surface]>,
    /// A bound relay forwards complete Messages and each reference's granted
    /// permissions. Its declared surfaces have empty fixed slot lists.
    pub forwarding: bool,
    implementation: Box<dyn Dispatch>,
    close: Option<Box<dyn FnMut() -> Result<(), Error>>>,
    // Keep executable ownership after implementation/context destruction.
    _code: Rc<dyn core::any::Any>,
}

impl ResourceProvider {
    #[must_use]
    pub fn new<State: 'static, Failure: Into<RequestError> + 'static>(
        state: State,
        operations: ResourceOps<State, Failure>,
        surfaces: Box<[Surface]>,
        code: Rc<dyn core::any::Any>,
    ) -> Self {
        Self {
            surfaces,
            forwarding: false,
            close: None,
            implementation: Box::new(Implementation { state, operations }),
            _code: code,
        }
    }

    /// A published access belongs to the requesting client until close.
    /// The callback fences delivery and wakes pending calls; it must not fail.
    /// Runtime invokes it once, outside registry guards, before reclamation.
    #[must_use]
    pub fn owned(self, mut close: impl FnMut() + 'static) -> Self {
        self.try_owned(move || {
            close();
            Ok(())
        })
    }

    /// Fences access, then attempts cleanup outside registry guards.
    ///
    /// Failure retains context/code and this callback for a close retry.
    /// The callback must tolerate its own partial cleanup from earlier calls.
    #[must_use]
    pub fn try_owned(
        mut self,
        close: impl FnMut() -> Result<(), Error> + 'static,
    ) -> Self {
        self.close = Some(Box::new(close));
        self
    }

    #[must_use]
    pub const fn is_owned(&self) -> bool {
        self.close.is_some()
    }

    /// # Errors
    /// Failed cleanup leaves the callback owned and retryable.
    pub fn close(&mut self) -> Result<(), Error> {
        if let Some(close) = self.close.as_mut() {
            close()?;
        }
        self.close = None;
        Ok(())
    }

    /// Uses the receiving Connection's grants for forwarded reference slots.
    /// The returned Message preserves its original contract and schema.
    #[must_use]
    pub const fn forwarding(mut self) -> Self {
        self.forwarding = true;
        self
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
    ) -> Reply {
        self.implementation.call(method, subject, input)
    }
}
