//! Same-build providers behind typed operation tables.
//!
//! Example: composition registers a `ResourceProvider`; callers bind one of
//! its canonical declarations. Independent native providers enter through ABI
//! adapters, not Rust layout. Runtime owns admission, execution and cleanup.

mod creation;
mod edit;
mod io;
mod namespace;
mod open;
mod packet;
mod registrar;
mod state;

pub use creation::{Creation, CreationOps, StageKeyed, StageResource};
pub use edit::{Edit, Propose};
pub use io::{IoInfo, IoOps, Read, ReadFn, Write, WriteFn};
pub use namespace::{
    CreateChildFn, LinkFn, ListFn, LookupFn, MaterializeFn, NamespaceInfo,
    NamespaceOps, RemoveFn, Rename, RenameFn,
};
pub use open::{Open, OpenFn};
pub use packet::{Oversize, Receive, ReceiveFn, ReceiveOps, SendFn, SendOps};
pub use registrar::{Registrar, RegistrarOps, StageExport};
pub use state::State;

use alloc::{boxed::Box, rc::Rc, vec::Vec};
use core::any::Any;

use crate::{
    Error, Pending, Value,
    declaration::{Declaration, Dispatch as Authority, Operation},
};

/// Ordinary entry; for example, a module-owned control operation.
pub type Begin<State> =
    fn(&mut State, usize, Operation, &Value) -> Result<Pending, Error>;

/// Factory entry supplied by a provider; Runtime supplies its staging owner.
pub type FactoryCreate<State> = fn(
    &mut State,
    usize,
    Operation,
    &Value,
    &Creation,
) -> Result<Pending, Error>;

/// Prepares an edit against selected state without installing it.
pub type Prepare<State> =
    fn(&mut State, usize, Operation, &Edit, &Value) -> Result<Pending, Error>;

/// One offered declaration and its callbacks, e.g. a module's control API.
/// Validation rejects a declared authority without its matching callback.
pub struct ProviderInterface<State> {
    /// Preparation callback for declared Edit operations. The request owner
    /// supplies selected state and exclusive candidate staging authority.
    pub prepare: Option<Prepare<State>>,
    /// Complete canonical definition; for example, a readonly device API.
    pub declaration: Declaration,
    /// Ordinary operations for this interface; absent for factory-only APIs.
    /// On error no effect or input retention occurred. Return Pending once
    /// effects become possible.
    pub begin: Option<Begin<State>>,
    /// Factory operations stage through the supplied request's creation owner.
    /// Example: create an immutable result without publishing it directly.
    pub create: Option<FactoryCreate<State>>,
}

/// Provider-wide optional I/O and cleanup, independent of declared APIs.
/// Example: a byte-only provider offers I/O and no custom interfaces.
pub struct ResourceOps<State> {
    /// Optional per-open implementation; e.g. a fresh sequential cursor.
    pub open: Option<OpenFn<State>>,
    /// Optional provider-owned names and lazy object materialization.
    pub namespace: Option<NamespaceOps<State>>,
    /// Optional independent byte and declared packet transfer directions.
    pub io: Option<IoOps<State>>,
    /// Checks/drains provider cleanup. Example: Busy retains its context/code.
    pub stop: fn(&mut State) -> Result<(), Error>,
}

trait Dispatch {
    fn prepare(
        &mut self,
        interface: usize,
        operation: Operation,
        edit: &Edit,
        input: &Value,
    ) -> Result<Pending, Error>;
    fn has_open(&self) -> bool;
    fn open(&mut self, opened: &Open) -> Result<Pending, Error>;
    fn create_child(
        &mut self,
        name: &[u8],
        creation: &Creation,
    ) -> Result<Pending, Error>;
    fn namespace(&self) -> Option<NamespaceInfo>;
    fn check_active(&self) -> Result<(), Error>;
    fn begin(
        &mut self,
        interface: usize,
        operation: Operation,
        input: &Value,
    ) -> Result<Pending, Error>;
    fn create(
        &mut self,
        interface: usize,
        operation: Operation,
        input: &Value,
        creation: &Creation,
    ) -> Result<Pending, Error>;
    fn receive_type(&self) -> Option<&crate::declaration::ValueType>;
    fn send_type(&self) -> Option<&crate::declaration::ValueType>;
    fn receive(&mut self, request: Receive) -> Result<Pending, Error>;
    fn send(&mut self, packet: &Value) -> Result<Pending, Error>;
    fn read(&mut self, request: Read) -> Result<Pending, Error>;
    fn write(&mut self, request: Write<'_>) -> Result<Pending, Error>;
    fn remove(&mut self, name: &[u8], key: &[u8]) -> Result<Pending, Error>;
    fn link(&mut self, name: &[u8], key: &[u8]) -> Result<Pending, Error>;
    fn rename(&mut self, request: Rename<'_>) -> Result<Pending, Error>;
    fn namespace_flags(&mut self) -> Result<Pending, Error>;
    fn lookup(&mut self, name: &[u8]) -> Result<Pending, Error>;
    fn list(&mut self, cursor: u64, maximum: u64) -> Result<Pending, Error>;
    fn materialize(
        &mut self,
        key: &[u8],
        creation: &Creation,
    ) -> Result<Pending, Error>;
    fn stop(&mut self) -> Result<(), Error>;
}

struct InterfaceOps<State> {
    prepare: Option<Prepare<State>>,
    begin: Option<Begin<State>>,
    create: Option<FactoryCreate<State>>,
}

struct Implementation<State> {
    state: State,
    ops: ResourceOps<State>,
    interfaces: Vec<InterfaceOps<State>>,
    lifecycle: Lifecycle,
}

impl<State> Dispatch for Implementation<State> {
    fn prepare(
        &mut self,
        interface: usize,
        operation: Operation,
        edit: &Edit,
        input: &Value,
    ) -> Result<Pending, Error> {
        let prepare =
            self.interfaces[interface].prepare.ok_or(Error::UNSUPPORTED)?;
        prepare(&mut self.state, interface, operation, edit, input)
    }

    fn has_open(&self) -> bool {
        self.ops.open.is_some()
    }
    fn open(&mut self, opened: &Open) -> Result<Pending, Error> {
        let open = self.ops.open.ok_or(Error::UNSUPPORTED)?;
        open(&mut self.state, opened)
    }

    fn create_child(
        &mut self,
        name: &[u8],
        creation: &Creation,
    ) -> Result<Pending, Error> {
        let callback = self
            .ops
            .namespace
            .as_ref()
            .and_then(|ops| ops.create)
            .ok_or(Error::UNSUPPORTED)?;
        callback(&mut self.state, name, creation)
    }

    fn namespace(&self) -> Option<NamespaceInfo> {
        self.ops.namespace.as_ref().map(NamespaceOps::info)
    }
    fn check_active(&self) -> Result<(), Error> {
        if self.lifecycle != Lifecycle::Active {
            return Err(Error::CLOSED);
        }
        Ok(())
    }

    fn begin(
        &mut self,
        interface: usize,
        operation: Operation,
        input: &Value,
    ) -> Result<Pending, Error> {
        let begin =
            self.interfaces[interface].begin.ok_or(Error::UNSUPPORTED)?;
        begin(&mut self.state, interface, operation, input)
    }

    fn create(
        &mut self,
        interface: usize,
        operation: Operation,
        input: &Value,
        creation: &Creation,
    ) -> Result<Pending, Error> {
        let create =
            self.interfaces[interface].create.ok_or(Error::UNSUPPORTED)?;
        create(&mut self.state, interface, operation, input, creation)
    }

    fn receive_type(&self) -> Option<&crate::declaration::ValueType> {
        Some(&self.ops.io.as_ref()?.receive.as_ref()?.packet)
    }
    fn send_type(&self) -> Option<&crate::declaration::ValueType> {
        Some(&self.ops.io.as_ref()?.send.as_ref()?.packet)
    }
    fn receive(&mut self, request: Receive) -> Result<Pending, Error> {
        let callback = self
            .ops
            .io
            .as_ref()
            .and_then(|io| io.receive.as_ref())
            .ok_or(Error::UNSUPPORTED)?;
        (callback.begin)(&mut self.state, request)
    }
    fn send(&mut self, packet: &Value) -> Result<Pending, Error> {
        let callback = self
            .ops
            .io
            .as_ref()
            .and_then(|io| io.send.as_ref())
            .ok_or(Error::UNSUPPORTED)?;
        (callback.begin)(&mut self.state, packet)
    }

    fn read(&mut self, request: Read) -> Result<Pending, Error> {
        let read = self
            .ops
            .io
            .as_ref()
            .and_then(|io| io.read)
            .ok_or(Error::UNSUPPORTED)?;
        read(&mut self.state, request)
    }

    fn write(&mut self, request: Write<'_>) -> Result<Pending, Error> {
        let write = self
            .ops
            .io
            .as_ref()
            .and_then(|io| io.write)
            .ok_or(Error::UNSUPPORTED)?;
        write(&mut self.state, request)
    }

    fn rename(&mut self, request: Rename<'_>) -> Result<Pending, Error> {
        let callback = self
            .ops
            .namespace
            .as_ref()
            .and_then(|ops| ops.rename)
            .ok_or(Error::UNSUPPORTED)?;
        callback(&mut self.state, request)
    }
    fn link(&mut self, name: &[u8], key: &[u8]) -> Result<Pending, Error> {
        let callback = self
            .ops
            .namespace
            .as_ref()
            .and_then(|ops| ops.link)
            .ok_or(Error::UNSUPPORTED)?;
        callback(&mut self.state, name, key)
    }
    fn remove(&mut self, name: &[u8], key: &[u8]) -> Result<Pending, Error> {
        let callback = self
            .ops
            .namespace
            .as_ref()
            .and_then(|ops| ops.remove)
            .ok_or(Error::UNSUPPORTED)?;
        callback(&mut self.state, name, key)
    }
    fn namespace_flags(&mut self) -> Result<Pending, Error> {
        let callback =
            self.ops.namespace.as_ref().ok_or(Error::UNSUPPORTED)?.flags;
        callback(&mut self.state)
    }
    fn lookup(&mut self, name: &[u8]) -> Result<Pending, Error> {
        let callback = self
            .ops
            .namespace
            .as_ref()
            .and_then(|ops| ops.lookup)
            .ok_or(Error::UNSUPPORTED)?;
        callback(&mut self.state, name)
    }
    fn list(&mut self, cursor: u64, maximum: u64) -> Result<Pending, Error> {
        let callback = self
            .ops
            .namespace
            .as_ref()
            .and_then(|ops| ops.list)
            .ok_or(Error::UNSUPPORTED)?;
        callback(&mut self.state, cursor, maximum)
    }
    fn materialize(
        &mut self,
        key: &[u8],
        creation: &Creation,
    ) -> Result<Pending, Error> {
        let callback = self
            .ops
            .namespace
            .as_ref()
            .and_then(|ops| ops.materialize)
            .ok_or(Error::UNSUPPORTED)?;
        callback(&mut self.state, key, creation)
    }

    fn stop(&mut self) -> Result<(), Error> {
        if self.lifecycle == Lifecycle::Stopped {
            return Ok(());
        }
        self.lifecycle = Lifecycle::Stopping;
        (self.ops.stop)(&mut self.state)?;
        self.lifecycle = Lifecycle::Stopped;
        Ok(())
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Lifecycle {
    Active,
    Stopping,
    Stopped,
}

/// Owned provider. Example: multiple compatible providers share API
/// declarations but retain distinct contexts; replacing names never retargets.
pub struct ResourceProvider {
    interfaces: Box<[Declaration]>,
    io: Option<IoInfo>,
    implementation: Box<dyn Dispatch>,
    code: Rc<dyn Any>,
}

impl ResourceProvider {
    /// Pairs dispatch callbacks with owned declarations and code lifetime.
    /// Example: a native adapter supplies its retained instance as code owner.
    /// # Errors
    /// Duplicate APIs and declared dispatch without its matching callback
    /// reject. Rejection returns private state unchanged.
    pub fn new<State: 'static>(
        state: State,
        ops: ResourceOps<State>,
        interfaces: Box<[ProviderInterface<State>]>,
        code: Rc<dyn Any>,
    ) -> Result<Self, (Error, State)> {
        if let Err(error) = supported(&interfaces) {
            return Err((error, state));
        }
        if let Some(io) = &ops.io
            && let Err(error) = io.validate()
        {
            return Err((error, state));
        }
        let mut declarations = Vec::new();
        let mut callbacks = Vec::new();
        if declarations.try_reserve_exact(interfaces.len()).is_err()
            || callbacks.try_reserve_exact(interfaces.len()).is_err()
        {
            return Err((Error::PRESSURE, state));
        }
        for interface in interfaces {
            declarations.push(interface.declaration);
            callbacks.push(InterfaceOps {
                prepare: interface.prepare,
                begin: interface.begin,
                create: interface.create,
            });
        }
        let io = ops.io.as_ref().map(|io| io.info);
        Ok(Self {
            interfaces: declarations.into_boxed_slice(),
            io,
            implementation: Box::new(Implementation {
                state,
                ops,
                interfaces: callbacks,
                lifecycle: Lifecycle::Active,
            }),
            code,
        })
    }

    /// Reports optional per-open initialization without invoking provider code.
    #[must_use]
    pub fn has_open(&self) -> bool {
        self.implementation.has_open()
    }

    /// Initializes one opened implementation, e.g. a private stream cursor.
    /// Subject identity is unchanged; Runtime owns validation and publication.
    /// # Errors
    /// Closed or Unsupported starts no work. Accepted work keeps its original
    /// pending cleanup and code ownership, including uncertainty.
    pub fn open(&mut self, opened: &Open) -> Result<Pending, Error> {
        self.implementation.check_active()?;
        let mut pending = self.implementation.open(opened)?;
        pending.keep_code(self.code.clone());
        Ok(pending)
    }

    /// Returns offered canonical definitions, e.g. for dynamic discovery.
    #[must_use]
    pub fn interfaces(&self) -> &[Declaration] {
        &self.interfaces
    }

    /// Copies supported byte directions and limits, e.g. positional READ only.
    #[must_use]
    pub const fn io(&self) -> Option<IoInfo> {
        self.io
    }

    /// Copies optional namespace support without invoking provider code.
    #[must_use]
    pub fn namespace(&self) -> Option<NamespaceInfo> {
        self.implementation.namespace()
    }

    /// Invokes the default child factory, e.g. creating an empty data subject.
    /// The factory stages its new object key and provider before publication.
    /// # Errors
    /// Closed, Unsupported and malformed names invoke no callback. Namespace
    /// rejection and original uncertain effects keep their ordinary custody.
    pub fn create_child(
        &mut self,
        name: &[u8],
        creation: &Creation,
    ) -> Result<Pending, Error> {
        self.implementation.check_active()?;
        namespace::name(name)?;
        let mut pending = self.implementation.create_child(name, creation)?;
        pending.keep_code(self.code.clone());
        Ok(pending)
    }

    /// Moves a captured name/key atomically, e.g. into another directory.
    ///
    /// Runtime qualifies and retains the destination; the provider revalidates
    /// source identity and destination absence before its atomic change.
    /// # Errors
    /// Closed, Unsupported and malformed names invoke no callback. Accepted
    /// work keeps the original recovery obligation; it never copies/deletes.
    pub fn rename(&mut self, request: Rename<'_>) -> Result<Pending, Error> {
        self.implementation.check_active()?;
        namespace::name(request.name)?;
        namespace::name(request.new_name)?;
        let mut pending = self.implementation.rename(request)?;
        pending.keep_code(self.code.clone());
        Ok(pending)
    }

    /// Adds an exclusive name for a retained source key, e.g. a device alias.
    ///
    /// Runtime qualifies the key to this mounted namespace and retains source
    /// access through finish. Linking grants no access and copies no subject.
    /// # Errors
    /// Closed, Unsupported and invalid names invoke no callback. The provider
    /// rejects a stale key or occupied destination; admitted work uses custody.
    pub fn link(&mut self, name: &[u8], key: &[u8]) -> Result<Pending, Error> {
        self.implementation.check_active()?;
        namespace::name(name)?;
        let mut pending = self.implementation.link(name, key)?;
        pending.keep_code(self.code.clone());
        Ok(pending)
    }

    /// Removes a name selecting the captured key, e.g. an alias to a device.
    /// Existing subject access remains live; the provider revalidates the key.
    /// # Errors
    /// Closed, Unsupported and invalid names invoke no callback. Accepted work
    /// retains its original outcome/recovery even if names later change.
    pub fn remove(
        &mut self,
        name: &[u8],
        key: &[u8],
    ) -> Result<Pending, Error> {
        self.implementation.check_active()?;
        namespace::name(name)?;
        let mut pending = self.implementation.remove(name, key)?;
        pending.keep_code(self.code.clone());
        Ok(pending)
    }

    /// Observes namespace flags, e.g. CONTAINER|EMPTY as canonical U32 (3).
    /// Metadata reports no activity; accepted work uses ordinary custody.
    /// # Errors
    /// Closed and non-namespace providers invoke no callback.
    pub fn namespace_flags(&mut self) -> Result<Pending, Error> {
        self.implementation.check_active()?;
        let mut pending = self.implementation.namespace_flags()?;
        pending.keep_code(self.code.clone());
        Ok(pending)
    }

    /// Resolves one child name to an opaque provider key, not a `ResourceId`.
    /// # Errors
    /// Closed, Unsupported or malformed names invoke no callback.
    pub fn lookup(&mut self, name: &[u8]) -> Result<Pending, Error> {
        self.implementation.check_active()?;
        namespace::name(name)?;
        let mut pending = self.implementation.lookup(name)?;
        pending.keep_code(self.code.clone());
        Ok(pending)
    }

    /// Begins one provider name/key page bounded by maximum output bytes.
    /// # Errors
    /// Closed and Unsupported invoke no callback; provider failures preserve
    /// ordinary request custody once admitted.
    pub fn list(
        &mut self,
        cursor: u64,
        maximum: u64,
    ) -> Result<Pending, Error> {
        self.implementation.check_active()?;
        let mut pending = self.implementation.list(cursor, maximum)?;
        pending.keep_code(self.code.clone());
        Ok(pending)
    }

    /// Stages an existing object's implementation under its captured key.
    /// Runtime supplies installation custody; this grants no operation rights.
    /// # Errors
    /// Closed and Unsupported invoke no callback. Replaced keys reject rather
    /// than staging a different object; admitted work retains ordinary custody.
    pub fn materialize(
        &mut self,
        key: &[u8],
        creation: &Creation,
    ) -> Result<Pending, Error> {
        self.implementation.check_active()?;
        let mut pending = self.implementation.materialize(key, creation)?;
        pending.keep_code(self.code.clone());
        Ok(pending)
    }

    /// Begins an admitted byte read through the opened implementation.
    /// # Errors
    /// Closed providers, unsupported modes, invalid alignment or pressure
    /// invoke no callback.
    pub fn read(&mut self, request: Read) -> Result<Pending, Error> {
        self.implementation.check_active()?;
        self.io.ok_or(Error::UNSUPPORTED)?.read(request)?;
        let mut pending = self.implementation.read(request)?;
        pending.keep_code(self.code.clone());
        Ok(pending)
    }

    /// Begins a byte write; pending work owns any retained input bytes.
    /// # Errors
    /// Closed providers, unsupported modes, invalid alignment or pressure
    /// invoke no callback.
    pub fn write(&mut self, request: Write<'_>) -> Result<Pending, Error> {
        self.implementation.check_active()?;
        self.io.ok_or(Error::UNSUPPORTED)?.write(&request)?;
        let mut pending = self.implementation.write(request)?;
        pending.keep_code(self.code.clone());
        Ok(pending)
    }

    /// Begins an admitted operation outside registry/Core guards.
    /// Example: Runtime already validated the selected method and input refs.
    /// # Errors
    /// Closed providers and unsupported selectors invoke nothing. Begin failure
    /// has no effect or retained input. Pending work keeps the provider code.
    pub fn begin(
        &mut self,
        interface: usize,
        operation: Operation,
        input: &Value,
    ) -> Result<Pending, Error> {
        self.implementation.check_active()?;
        self.select(interface, operation, Authority::Call)?;
        let mut pending =
            self.implementation.begin(interface, operation, input)?;
        pending.keep_code(self.code.clone());
        Ok(pending)
    }

    /// Begins factory preparation with the request's staging capability.
    /// Runtime supplies this only after reserving installation capacity.
    /// # Errors
    /// Closed providers and ordinary or editing selectors invoke nothing.
    /// Begin failure has no effects or retained inputs; preparation returns
    /// Pending once admitted.
    pub fn create(
        &mut self,
        interface: usize,
        operation: Operation,
        input: &Value,
        creation: &Creation,
    ) -> Result<Pending, Error> {
        self.implementation.check_active()?;
        self.select(interface, operation, Authority::Factory)?;
        let mut pending = self
            .implementation
            .create(interface, operation, input, creation)?;
        pending.keep_code(self.code.clone());
        Ok(pending)
    }

    /// Prepares declared editing work against the request's selected state.
    /// Runtime supplies this capability after authorization and reservation;
    /// a successful pending result alone never installs a candidate.
    /// # Errors
    /// Closed providers and ordinary/factory selectors invoke nothing. Begin
    /// failure retains no request input; accepted work keeps state and code.
    pub fn prepare(
        &mut self,
        interface: usize,
        operation: Operation,
        edit: &Edit,
        input: &Value,
    ) -> Result<Pending, Error> {
        self.implementation.check_active()?;
        self.select(interface, operation, Authority::Edit)?;
        let mut pending =
            self.implementation.prepare(interface, operation, edit, input)?;
        pending.keep_code(Rc::new((edit.clone(), self.code.clone())));
        Ok(pending)
    }

    fn select(
        &self,
        interface: usize,
        operation: Operation,
        authority: Authority,
    ) -> Result<(), Error> {
        let declaration =
            self.interfaces.get(interface).ok_or(Error::UNSUPPORTED)?;
        declaration
            .methods()
            .iter()
            .find(|method| {
                method.operation() == operation
                    && method.dispatch() == authority
            })
            .map(|_| ())
            .ok_or(Error::UNSUPPORTED)
    }

    /// Fences new work and joins provider cleanup; admitted requests may drain.
    /// Successful stop is idempotent.
    /// Example: Busy keeps this provider owned by Runtime for further progress.
    /// # Errors
    /// Failed stop leaves context and code retained, never safe to unload.
    pub fn stop(&mut self) -> Result<(), Error> {
        self.implementation.stop()
    }
}

fn supported<State>(
    interfaces: &[ProviderInterface<State>],
) -> Result<(), Error> {
    for (index, interface) in interfaces.iter().enumerate() {
        let declaration = &interface.declaration;
        if interfaces[..index]
            .iter()
            .any(|i| i.declaration.api() == declaration.api())
        {
            return Err(Error::INVALID);
        }
        if declaration.methods().iter().any(|method| {
            (method.dispatch() == Authority::Call && interface.begin.is_none())
                || (method.dispatch() == Authority::Edit
                    && interface.prepare.is_none())
                || (method.dispatch() == Authority::Factory
                    && interface.create.is_none())
        }) {
            return Err(Error::UNSUPPORTED);
        }
    }
    Ok(())
}
