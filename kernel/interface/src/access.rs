use alloc::{boxed::Box, rc::Rc, vec::Vec};
use core::{
    fmt,
    future::{Future, Ready, ready},
    pin::Pin,
};

use crate::{Error, ResourceId, namespace::ResourceEntry};

/// Exact, component-owned encoding and reference-slot contract.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Contract {
    pub name: Box<str>,
    pub version: u32,
}

impl Contract {
    #[must_use]
    pub fn new(name: &str, version: u32) -> Self {
        Self { name: name.into(), version }
    }
}

/// One complete value. References are translated separately from encoded data.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Message {
    pub contract: Contract,
    pub data: Box<[u8]>,
    pub resources: Box<[ResourceId]>,
}

impl Message {
    #[must_use]
    pub fn new(contract: Contract, data: &[u8]) -> Self {
        Self { contract, data: data.into(), resources: Box::new([]) }
    }

    #[must_use]
    pub fn reference(contract: Contract, resource: ResourceId) -> Self {
        Self { contract, data: Box::new([]), resources: Box::new([resource]) }
    }
}

/// Unknown is not a failure of the effect or permission to replay it.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RequestError {
    Rejected(Error),
    Failed(Error),
    Unknown { cause: Error, recovery: Box<str> },
}

impl From<Error> for RequestError {
    fn from(error: Error) -> Self {
        Self::Failed(error)
    }
}

impl fmt::Display for RequestError {
    fn fmt(&self, output: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(output, "Resource request: {self:?}")
    }
}

impl core::error::Error for RequestError {}

/// An ordinary awaited result; polling observes work owned by Runtime.
pub type Reply = Pin<Box<dyn Future<Output = Result<Message, RequestError>>>>;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Method {
    Read,
    Write,
}

/// Common adapter boundary. Read-view factories only construct inert access;
/// request execution remains with Runtime.
pub trait Access {
    /// Acquires a producer-offered independent, data-only read view.
    /// # Errors
    /// Unsupported, denied and capacity failures preserve the original access.
    fn read_view(&self, _: ResourceId) -> Result<Connection, Error> {
        Err(Error::UNSUPPORTED)
    }
    /// # Errors
    /// Rejects unsupported retention or rights not already granted.
    fn capture(
        &self,
        _: ResourceId,
        _: crate::provider::Permissions,
    ) -> Result<Connection, Error> {
        Err(Error::UNSUPPORTED)
    }
    /// Withdraws a quiescent subtree owned by the executing module.
    /// # Errors
    /// Denied/Busy leave names and providers unchanged.
    fn withdraw(&self, _: ResourceId) -> Result<(), Error> {
        Err(Error::DENIED)
    }

    /// Publishes an initialized child within the executing module's namespace.
    /// # Errors
    /// Rejection returns the provider without publishing a name.
    fn publish(
        &self,
        _: ResourceId,
        _: &str,
        provider: crate::provider::ResourceProvider,
    ) -> Result<ResourceId, (Error, crate::provider::ResourceProvider)> {
        Err((Error::DENIED, provider))
    }

    /// # Errors
    /// Rejects a closed or replaced connection.
    fn root(&self) -> Result<ResourceId, Error>;
    /// # Errors
    /// Rejects invalid, unavailable or unauthorized paths.
    fn resolve_at(
        &self,
        parent: ResourceId,
        path: &str,
    ) -> Result<ResourceId, Error>;
    /// # Errors
    /// Rejects incomplete listings or unavailable access.
    fn children(&self, parent: ResourceId)
    -> Result<Vec<ResourceEntry>, Error>;
    fn request(
        &self,
        method: Method,
        subject: ResourceId,
        input: &Message,
    ) -> Reply;
    /// # Errors
    /// Busy access retains its outstanding work.
    fn close(&self, access: ResourceId) -> Result<(), Error>;
    /// # Errors
    /// Rejects lost scope or a closed Runtime.
    fn reconnect(&self) -> Result<Connection, Error>;
}

/// Granted access to one Runtime; new module surfaces add no fields here.
#[derive(Clone)]
pub struct Connection(Rc<dyn Access>);

impl Connection {
    /// Owns an independent read view whose root is the view subject.
    /// Clones share its lifetime; dropping the last owner releases the view.
    /// Acquisition performs no I/O. Requests still execute through Runtime.
    /// # Errors
    /// The producer must offer a self-contained data-only reader. Existing READ
    /// rights are required; original access close never closes the new view.
    pub fn read_view(&self, subject: ResourceId) -> Result<Self, Error> {
        self.0.read_view(subject)
    }
    /// Retains selected existing rights in this namespace through the result.
    ///
    /// Clones retain the same capture. Drop releases it without closing the
    /// shared subject. Existing namespace grants remain independent.
    /// # Errors
    /// Rejects missing rights, closed access or retention pressure.
    pub fn capture(
        &self,
        subject: ResourceId,
        rights: crate::provider::Permissions,
    ) -> Result<Self, Error> {
        self.0.capture(subject, rights)
    }
    /// Withdraws an idle provider subtree without unloading its owning module.
    /// # Errors
    /// Rejects foreign ownership and captured/in-flight use.
    pub fn withdraw(&self, resource: ResourceId) -> Result<(), Error> {
        self.0.withdraw(resource)
    }

    /// Publishes an initialized child through module publication access.
    /// # Errors
    /// Rejection returns the uninstalled provider and its state.
    pub fn publish(
        &self,
        parent: ResourceId,
        name: &str,
        provider: crate::provider::ResourceProvider,
    ) -> Result<ResourceId, (Error, crate::provider::ResourceProvider)> {
        self.0.publish(parent, name, provider)
    }

    #[must_use]
    pub fn new(access: Rc<dyn Access>) -> Self {
        Self(access)
    }

    /// # Errors
    /// Rejects a closed or replaced connection.
    pub fn root(&self) -> Ready<Result<ResourceId, RequestError>> {
        ready(self.0.root().map_err(RequestError::Rejected))
    }

    /// # Errors
    /// Rejects invalid, unavailable or unauthorized paths.
    pub fn resolve(
        &self,
        path: &str,
    ) -> Ready<Result<ResourceId, RequestError>> {
        ready(
            self.0
                .root()
                .and_then(|root| self.0.resolve_at(root, path))
                .map_err(RequestError::Rejected),
        )
    }

    /// # Errors
    /// Rejects invalid, unavailable or unauthorized paths.
    pub fn resolve_at(
        &self,
        parent: ResourceId,
        path: &str,
    ) -> Ready<Result<ResourceId, RequestError>> {
        ready(self.0.resolve_at(parent, path).map_err(RequestError::Rejected))
    }

    /// # Errors
    /// Returns a complete listing or an explicit access/capacity error.
    pub fn children(
        &self,
        parent: ResourceId,
    ) -> Ready<Result<Vec<ResourceEntry>, RequestError>> {
        ready(self.0.children(parent).map_err(RequestError::Rejected))
    }

    #[must_use = "Await the result or retain its recovery access"]
    pub fn read(&self, subject: ResourceId, input: &Message) -> Reply {
        self.0.request(Method::Read, subject, input)
    }

    #[must_use = "Await the result or retain its recovery access"]
    pub fn write(&self, subject: ResourceId, input: &Message) -> Reply {
        self.0.request(Method::Write, subject, input)
    }

    /// # Errors
    /// Busy access retains its outstanding work; shared data is not closed.
    pub fn close(&self, access: ResourceId) -> Ready<Result<(), RequestError>> {
        ready(self.0.close(access).map_err(RequestError::Rejected))
    }

    /// Reauthorizes the same scope with a fresh namespace and fences old IDs.
    /// # Errors
    /// Rejects lost scope or a closed Runtime.
    pub fn reconnect(&self) -> Result<Self, Error> {
        self.0.reconnect()
    }
}
