use alloc::{
    boxed::Box,
    rc::{Rc, Weak},
    vec::Vec,
};
use core::{
    cell::{Cell, RefCell},
    task::Context,
};
use reovim_kernel_interface::{
    Error, ResourceId,
    caller::{Caller, CallerOps},
    declaration::{Declaration, ValueLimits},
    driver::Progress,
    provider::ResourceProvider,
};

mod access;
mod activation;
mod bindings;
mod codec;
mod custody;
mod discovery;
mod execution;
mod factory;
mod info;
mod io;
mod loans;
mod module_activation;
mod module_registry;
mod modules;
mod probe;
pub use module_registry::{ModuleSelection, SelectedModule};
pub use modules::Activation;
pub use probe::ModuleProbe;
mod namespace;
mod publication;
pub use publication::PublicationGrant;
mod opening;
mod outputs;
mod packet;
mod registry;

use access::Scope;
use registry::Registry;

/// Finite admission reservations, e.g. Pressure leaves providers uncalled.
#[derive(Clone, Copy)]
pub struct Limits {
    /// Registered subjects, e.g. 256.
    pub resources: usize,
    /// Caller contexts, including module imports; e.g. 16.
    pub callers: usize,
    /// Receipts per caller, including released but unfinished work.
    pub requests: usize,
    /// Access tokens per caller, including provisional returned handles.
    pub handles: usize,
    /// Concurrent result wake registrations per receipt, e.g. 16 readers.
    pub waiters: usize,
    /// Canonical value decoding bounds, e.g. one MiB per input.
    pub values: ValueLimits,
}

impl Default for Limits {
    fn default() -> Self {
        Self {
            resources: 256,
            callers: 16,
            requests: 64,
            handles: 256,
            waiters: 16,
            values: ValueLimits {
                bytes: 1024 * 1024,
                elements: 65536,
                references: 256,
            },
        }
    }
}

struct Entry {
    closed: Cell<bool>,
    identity: ResourceId,
    path: Box<[u8]>,
    implementation: Rc<Implementation>,
    imports: Option<Weak<RefCell<Scope>>>,
    // Drop the provider before releasing the factory state it may borrow.
    _origin: Option<Rc<Self>>,
    namespace_owner: Option<Weak<Self>>,
    directory: bool,
    parent: Option<ResourceId>,
    publication:
        Option<(ResourceId, reovim_kernel_interface::module::ModuleScope)>,
}

// Operation tables and their context have a lifetime distinct from identity.
// An opened implementation is retained by accesses and admitted requests.
struct Implementation {
    interfaces: Box<[Declaration]>,
    io: Option<reovim_kernel_interface::provider::IoInfo>,
    namespace: Option<reovim_kernel_interface::provider::NamespaceInfo>,
    opens: bool,
    receive_type: Option<reovim_kernel_interface::declaration::ValueType>,
    send_type: Option<reovim_kernel_interface::declaration::ValueType>,
    provider: RefCell<ResourceProvider>,
    _origin: Option<Rc<Self>>,
}

impl Implementation {
    fn new(provider: ResourceProvider) -> Rc<Self> {
        Self::with_origin(provider, None)
    }

    fn with_origin(
        provider: ResourceProvider,
        origin: Option<Rc<Self>>,
    ) -> Rc<Self> {
        Rc::new(Self {
            interfaces: provider.interfaces().into(),
            io: provider.io(),
            namespace: provider.namespace(),
            opens: provider.has_open(),
            receive_type: provider.receive_type().cloned(),
            send_type: provider.send_type().cloned(),
            provider: RefCell::new(provider),
            _origin: origin,
        })
    }
}

impl Entry {
    fn is_closed(&self) -> bool {
        self.closed.get()
            || self
                .namespace_owner
                .as_ref()
                .and_then(Weak::upgrade)
                .is_some_and(|owner| owner.closed.get())
    }
}

/// One composition grant, e.g. READ with temporary input loans only.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Grant {
    /// Granted subject identity, e.g. the Resource selected during assembly.
    pub resource: ResourceId,
    /// Allowed operation mask, e.g. 1 permits READ after `SetAccess`.
    pub access: u32,
    /// Maximum initial rights returned by factories invoked through this grant.
    /// Example: READ permits immutable results without granting child WRITE.
    pub create_access: u32,
    /// Allows recipients to capture input loans derived from this grant.
    /// Example: false keeps borrowed access scoped to its original request.
    pub retain_loans: bool,
}

/// Composition's caller binding, e.g. the import context of one provider.
/// Consumers receive `caller()`; only assembly associates providers with it.
pub struct Binding {
    scope: Rc<RefCell<Scope>>,
}

impl Binding {
    /// Returns the declared caller interface for this exact grant context.
    /// Example: pass this Caller to a module before registering its exports.
    #[must_use]
    pub fn caller(&self) -> Caller {
        Caller::new(
            self.scope.clone(),
            CallerOps {
                reserve: custody::reserve,
                submit: custody::submit,
                result: custody::result,
                cancel: custody::cancel,
                reconcile: custody::reconcile,
                adopt: custody::adopt,
                release: custody::release,
            },
        )
    }
}

/// Execution owner; ordinary consumers receive only Interface's Caller.
/// Example: register, bind, start, then repeatedly advance on wake.
pub struct Runtime {
    limits: Limits,
    registry: Rc<RefCell<Registry>>,
    scopes: Vec<Rc<RefCell<Scope>>>,
    modules: Vec<modules::Instance>,
    module_registrations: Vec<Rc<module_registry::Registration>>,
    probes: Vec<probe::Work>,
    module_activation: Rc<module_activation::Queue>,
    started: bool,
    closing: bool,
    non_ascii_names: bool,
}

impl Runtime {
    /// Reserve registry/caller storage under an instance epoch.
    /// Example: a restarted host supplies a fresh epoch, never old identities.
    /// # Errors
    /// Zero epoch/bounds reject; allocation failure returns Pressure.
    pub fn new(epoch: [u8; 16], limits: Limits) -> Result<Self, Error> {
        if epoch == [0; 16]
            || limits.resources == 0
            || limits.callers == 0
            || limits.requests == 0
            || limits.handles == 0
            || limits.waiters == 0
            || limits.values.bytes == 0
        {
            return Err(Error::INVALID);
        }
        let opened = limits
            .callers
            .checked_mul(limits.handles)
            .ok_or(Error::PRESSURE)?;
        let registry = Rc::new(RefCell::new(Registry::new(
            epoch,
            limits.resources,
            opened,
        )?));
        let module_activation =
            Rc::new(module_activation::Queue::new(&registry, limits)?);
        Ok(Self {
            limits,
            registry,
            module_activation,
            scopes: reserve(limits.callers)?,
            modules: reserve(limits.resources)?,
            module_registrations: reserve(limits.resources)?,
            probes: reserve(limits.requests)?,
            started: false,
            closing: false,
            non_ascii_names: false,
        })
    }

    /// Opt the root namespace into lossless non-ASCII names before wiring.
    /// Example: a native filesystem host opts in before registering paths;
    /// ordinary namespaces retain the default ASCII restriction.
    /// # Errors
    /// Closed rejects teardown; Busy rejects changes after registration,
    /// caller connection or startup. Contents are unrestricted in either mode.
    pub fn allow_non_ascii_names(&mut self) -> Result<(), Error> {
        if self.closing {
            return Err(Error::CLOSED);
        }
        if self.started
            || !self.registry.borrow().entries.is_empty()
            || !self.scopes.is_empty()
        {
            return Err(Error::BUSY);
        }
        self.non_ascii_names = true;
        Ok(())
    }

    /// Install one immutable provider identity, e.g. `/devices/counter`.
    /// Assembly supplies its actual import binding for reference-bearing calls;
    /// None admits only reference-free input. Providers use the binding's
    /// Caller.
    /// # Errors
    /// Foreign bindings and conflicting API definitions/names reject without
    /// taking ownership.
    pub fn register(
        &mut self,
        path: &[u8],
        provider: ResourceProvider,
        imports: Option<&Binding>,
    ) -> Result<ResourceId, (Error, ResourceProvider)> {
        self.register_entry(path, provider, imports, false)
    }

    fn register_entry(
        &self,
        path: &[u8],
        provider: ResourceProvider,
        imports: Option<&Binding>,
        directory: bool,
    ) -> Result<ResourceId, (Error, ResourceProvider)> {
        if imports.is_some_and(|binding| {
            !self.scopes.iter().any(|s| Rc::ptr_eq(s, &binding.scope))
        }) {
            return Err((Error::DENIED, provider));
        }
        if self.closing {
            return Err((Error::CLOSED, provider));
        }
        if let Err(error) = codec::path(path, self.non_ascii_names) {
            return Err((error, provider));
        }
        let batch = match activation::Batch::new(
            &self.registry,
            imports.map(|binding| Rc::downgrade(&binding.scope)),
            path,
            &[b""],
            1,
            self.non_ascii_names,
        ) {
            Ok(batch) => batch,
            Err(error) => return Err((error, provider)),
        };
        if directory {
            batch.directory();
        }
        batch.registrar().stage(b"", provider)?;
        match batch.publish() {
            Ok(identities) => Ok(identities[0]),
            Err(error) => Err((error, batch.take_single())),
        }
    }

    /// Grant exact registered subjects to a fresh caller.
    /// Example: EXEC with zero `create_access` permits ordinary execution after
    /// `SetAccess`, without authorizing nonzero factory result rights.
    /// # Errors
    /// Unknown identities, invalid masks or exhausted caller capacity reject.
    pub fn bind(&mut self, grants: &[Grant]) -> Result<Binding, Error> {
        if self.closing {
            return Err(Error::CLOSED);
        }
        if self.scopes.len() == self.limits.callers {
            return Err(Error::PRESSURE);
        }
        let scope = Rc::new(RefCell::new(Scope::new(
            grants,
            &self.registry,
            self.limits,
            self.started,
            self.non_ascii_names,
        )?));
        self.scopes.push(scope.clone());
        Ok(Binding { scope })
    }

    /// Enable admission, e.g. after composition has registered providers.
    /// # Errors
    /// Closing Runtime cannot restart.
    pub fn start(&mut self) -> Result<(), Error> {
        if self.closing {
            return Err(Error::CLOSED);
        }
        self.started = true;
        for scope in &self.scopes {
            scope.borrow_mut().ready = true;
        }
        Ok(())
    }

    /// Drive bounded work independently of delivery observers.
    /// Example: dropping a caller result view never stops this execution.
    /// # Errors
    /// `NotReady` means startup has not enabled execution.
    pub fn advance(
        &mut self,
        context: &mut Context<'_>,
    ) -> Result<Progress, Error> {
        if !self.started {
            return Err(Error::NOT_READY);
        }
        let mut progress = module_activation::advance(self);
        progress |= probe::advance(self);
        progress |= modules::advance(self, context);
        for scope in &self.scopes {
            progress |= execution::advance(scope, context);
        }
        progress |= registry::reclaim(&self.registry)?;
        Ok(if progress { Progress::More } else { Progress::Idle })
    }

    /// Fence admission and join provider cleanup after receipts are released.
    /// Example: Busy requires more driving; Incomplete retains uncertain work.
    /// # Errors
    /// Unreleased receipts or pending cleanup retain all required owners.
    pub fn close(&mut self) -> Result<(), Error> {
        self.closing = true;
        self.registry.borrow_mut().closed = true;
        for scope in &self.scopes {
            scope.borrow_mut().closed = true;
        }
        if self.scopes.iter().any(uncertain) {
            return Err(Error::INCOMPLETE);
        }
        if self.scopes.iter().any(|scope| !scope.borrow().records.is_empty()) {
            return Err(Error::BUSY);
        }
        release_external_scopes(self);
        opening::reclaim(&self.registry)?;
        modules::close(self)?;
        opening::reclaim(&self.registry)?;
        if !self.registry.borrow().opening.is_empty() {
            return Err(Error::BUSY);
        }
        registry::close(&self.registry)?;
        probe::closed(self);
        self.module_registrations.clear();
        self.scopes.clear();
        Ok(())
    }
}

impl Drop for Runtime {
    fn drop(&mut self) {
        if self.close().is_err() {
            // An abandoned driver cannot prove native execution has stopped.
            // Keep the complete custody graph, not dangling callback addresses.
            core::mem::forget(core::mem::take(&mut self.scopes));
            core::mem::forget(core::mem::take(&mut self.modules));
            core::mem::forget(core::mem::take(&mut self.probes));
            core::mem::forget(self.module_activation.clone());
            core::mem::forget(self.registry.clone());
        }
    }
}

fn release_external_scopes(runtime: &Runtime) {
    for scope in &runtime.scopes {
        if !modules::owns_imports(runtime, scope) {
            let mut scope = scope.borrow_mut();
            scope.handles.clear();
            scope.grants.clear();
            scope.publications.clear();
        }
    }
}

fn uncertain(scope: &Rc<RefCell<Scope>>) -> bool {
    scope.borrow().records.iter().any(|record| {
        record.borrow().task.as_ref().is_some_and(execution::Task::uncertain)
    })
}

fn reserve<T>(capacity: usize) -> Result<Vec<T>, Error> {
    let mut values = Vec::new();
    values.try_reserve_exact(capacity).map_err(|_| Error::PRESSURE)?;
    Ok(values)
}
