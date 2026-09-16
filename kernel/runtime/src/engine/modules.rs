use alloc::{boxed::Box, rc::Rc, vec::Vec};
use core::{
    any::Any,
    cell::{Cell, RefCell},
    task::Context,
};
use reovim_kernel_interface::{
    Error, ResourceId,
    driver::{Lifecycle, Progress},
    module::{Module, ModuleContext, ModuleImplementation, ModuleScope},
};

use super::{Binding, Entry, Runtime, access::Scope, activation::Batch};

mod cleanup;
mod control;
mod imports;
pub(super) mod probe;
mod unload;

/// Composition's bounded activation selection; modules receive only Context.
/// Example: publish required `queue` and `control` exports below namespace.
#[derive(Clone, Copy)]
pub struct Activation<'a> {
    /// Existing namespace Resource; its identity pins the selected mount.
    pub namespace: ResourceId,
    /// Exact state scope. Global activation is currently implemented.
    pub scope: ModuleScope,
    /// Required relative export paths, validated before construction.
    pub required: &'a [&'a [u8]],
    /// Maximum staged exports, including optional contributions.
    pub exports: usize,
}

#[derive(Clone, Copy, Eq, PartialEq)]
enum Cleanup {
    Idle,
    Pending,
    Stopped,
}

pub(super) struct Instance {
    resource: Rc<Entry>,
    _namespace: Rc<Entry>,
    imports: Rc<RefCell<Scope>>,
    parent_imports: Option<Rc<RefCell<Scope>>>,
    batch: Batch,
    module: Option<Module>,
    status: Rc<Cell<Lifecycle>>,
    failure: Option<Error>,
    exports: Vec<ResourceId>,
    dependencies: Vec<ResourceId>,
    cleanup: Cleanup,
    closing: bool,
    control: Rc<control::Control>,
    probe_uses: usize,
    provisional: bool,
}

impl Instance {
    const fn stopped(&self) -> bool {
        matches!(self.cleanup, Cleanup::Stopped)
    }
}

impl Runtime {
    /// Constructs an inert module in the supplied composition context.
    /// Example: activate, then drive until `module_status` is Ready.
    /// The returned Resource identifies this attempt. Callbacks run outside
    /// registry guards.
    /// # Errors
    /// Invalid mounts/scopes/bindings, pressure or inert construction reject.
    /// Starting failures remain observable on the retained instance identity.
    pub fn activate<State: 'static, Imports>(
        &mut self,
        selection: Activation<'_>,
        implementation: ModuleImplementation<State, Imports>,
        imports: Imports,
        binding: &Binding,
        code: Rc<dyn Any>,
    ) -> Result<ResourceId, Error> {
        self.construct_module(
            selection,
            imports,
            binding,
            Vec::new(),
            |context| Module::construct(implementation, context, code),
        )
    }

    pub(super) fn construct_module<Imports>(
        &mut self,
        selection: Activation<'_>,
        imports: Imports,
        binding: &Binding,
        dependencies: Vec<ResourceId>,
        construct: impl FnOnce(ModuleContext<Imports>) -> Result<Module, Error>,
    ) -> Result<ResourceId, Error> {
        let namespace = namespace(self, &selection, binding)?;
        let batch = Batch::new(
            &self.registry,
            Some(Rc::downgrade(&binding.scope)),
            &namespace.path,
            selection.required,
            selection.exports,
            self.non_ascii_names,
        )?;
        batch.publication(namespace.identity, selection.scope);
        batch.inert();
        let identity = self.registry.borrow_mut().reserve()?;
        let context = ModuleContext {
            instance: identity,
            namespace: namespace.identity,
            scope: selection.scope,
            imports,
            exports: batch.registrar(),
        };
        let module = match construct(context) {
            Ok(module) => module,
            Err(error) => {
                self.registry.borrow_mut().release();
                return Err(error);
            }
        };
        let lifecycle = module.status();
        let failure =
            (lifecycle != Lifecycle::Constructed).then_some(Error::PROVIDER);
        let status =
            Rc::new(Cell::new(failure.map_or(lifecycle, Lifecycle::Failed)));
        let control = Rc::new(control::Control::new(
            identity,
            status.clone(),
            self.limits.requests,
        ));
        let resource = instance_resource(identity, control.clone());
        self.registry.borrow_mut().install(resource.clone());
        self.modules.push(Instance {
            resource,
            _namespace: namespace,
            imports: binding.scope.clone(),
            parent_imports: None,
            batch,
            module: Some(module),
            status,
            failure,
            exports: Vec::new(),
            dependencies,
            cleanup: Cleanup::Idle,
            closing: false,
            control,
            probe_uses: 0,
            provisional: false,
        });
        Ok(identity)
    }

    /// Reads cached lifecycle without invoking the module, e.g. Starting.
    /// # Errors
    /// An unknown or expired instance identity is unavailable.
    pub fn module_status(
        &self,
        identity: ResourceId,
    ) -> Result<Lifecycle, Error> {
        Ok(self.instance(identity)?.status.get())
    }

    /// Reads the published batch identities, e.g. for composition grants.
    /// Dynamic consumers still discover each API through ordinary Interfaces.
    /// # Errors
    /// Unpublished instances are `NotReady` or retain their startup failure.
    pub fn module_exports(
        &self,
        identity: ResourceId,
    ) -> Result<&[ResourceId], Error> {
        let instance = self.instance(identity)?;
        if let Some(error) = instance.failure {
            return Err(error);
        }
        if instance.closing {
            return Err(Error::CLOSED);
        }
        if instance.status.get() != Lifecycle::Ready {
            return Err(Error::NOT_READY);
        }
        Ok(&instance.exports)
    }

    pub(super) fn module_dependencies(
        &self,
        identity: ResourceId,
    ) -> Result<&[ResourceId], Error> {
        Ok(&self.instance(identity)?.dependencies)
    }

    pub(super) fn module_import_owner(
        &mut self,
        id: ResourceId,
        parent: Rc<RefCell<Scope>>,
    ) {
        self.modules
            .iter_mut()
            .find(|instance| instance.resource.identity == id)
            .unwrap()
            .parent_imports = Some(parent);
    }

    fn instance(&self, identity: ResourceId) -> Result<&Instance, Error> {
        self.modules
            .iter()
            .find(|i| i.resource.identity == identity)
            .ok_or(Error::UNAVAILABLE)
    }
}

fn namespace(
    runtime: &Runtime,
    selection: &Activation<'_>,
    binding: &Binding,
) -> Result<Rc<Entry>, Error> {
    if runtime.closing {
        return Err(Error::CLOSED);
    }
    if selection.scope != ModuleScope::Global {
        return Err(Error::UNSUPPORTED);
    }
    if !runtime.scopes.iter().any(|s| Rc::ptr_eq(s, &binding.scope)) {
        return Err(Error::DENIED);
    }
    if runtime.modules.len() == runtime.limits.resources {
        return Err(Error::PRESSURE);
    }
    runtime
        .registry
        .borrow()
        .entries
        .iter()
        .find(|entry| {
            entry.identity == selection.namespace
                && !entry.path.is_empty()
                && !entry.closed.get()
        })
        .cloned()
        .ok_or(Error::UNAVAILABLE)
}

fn instance_resource(
    identity: ResourceId,
    control: Rc<control::Control>,
) -> Rc<Entry> {
    let provider = control::provider(control);
    Rc::new(Entry {
        closed: core::cell::Cell::new(false),
        identity,
        path: Box::new([]),
        implementation: crate::engine::Implementation::new(provider),
        imports: None,
        _origin: None,
        namespace_owner: None,
        directory: false,
        parent: None,
        publication: None,
    })
}

pub(super) fn advance(runtime: &mut Runtime, cx: &mut Context<'_>) -> bool {
    let mut progress = control::advance(runtime);
    for index in 0..runtime.modules.len() {
        let failure = dependency_failure(runtime, &runtime.modules[index]);
        let instance = &mut runtime.modules[index];
        if let Some(error) = failure {
            progress |= instance.failure.is_none();
            fail(instance, error);
        }
        let closing = runtime.closing || instance.closing;
        let inert = closing
            && instance.module.as_ref().is_some_and(|module| {
                module.status() == Lifecycle::Constructed
            });
        if !(instance.stopped() || inert) {
            progress |= drive(instance, !closing, cx);
        }
        if instance.failure.is_some() {
            fence_exports(instance, &runtime.registry.borrow().entries);
        }
    }
    // Dependencies were constructed first. Drain consumers before releasing
    // state they may still need during cleanup, including across Busy turns.
    for index in (0..runtime.modules.len()).rev() {
        let id = runtime.modules[index].resource.identity;
        let in_use = runtime.modules.iter().any(|consumer| {
            !consumer.stopped() && consumer.dependencies.contains(&id)
        });
        let instance = &mut runtime.modules[index];
        if !in_use
            && (runtime.closing
                || instance.closing
                || instance.failure.is_some())
        {
            progress |= cleanup::advance(instance, &runtime.registry, cx);
        }
        progress |= imports::retire(runtime, index);
    }
    progress
}

fn dependency_failure(runtime: &Runtime, instance: &Instance) -> Option<Error> {
    instance.dependencies.iter().find_map(|id| {
        match runtime.module_status(*id) {
            Ok(Lifecycle::Ready) => None,
            Ok(Lifecycle::Failed(error)) | Err(error) => Some(error),
            _ => Some(Error::NOT_READY),
        }
    })
}

fn drive(
    instance: &mut Instance,
    observe_status: bool,
    cx: &mut Context<'_>,
) -> bool {
    let Some(module) = instance.module.as_mut() else { return false };
    if observe_status && instance.failure.is_none() {
        instance.batch.starting();
    }
    let previous = instance.status.get();
    let progress = module.advance(cx);
    let lifecycle = module.status();
    if let Err(error) = progress {
        if instance.closing {
            instance.control.cleanup_failed(error);
        }
        fail(instance, error);
    } else if observe_status && instance.failure.is_none() {
        observe(instance, lifecycle);
    }
    previous != instance.status.get() || progress == Ok(Progress::More)
}

fn observe(instance: &mut Instance, lifecycle: Lifecycle) {
    match lifecycle {
        Lifecycle::Ready if instance.status.get() != Lifecycle::Ready => {
            match instance.batch.publish() {
                Ok(exports) => {
                    instance.exports = exports;
                    instance.status.set(Lifecycle::Ready);
                }
                Err(error) => fail(instance, error),
            }
        }
        Lifecycle::Failed(error) => fail(instance, error),
        Lifecycle::Closing | Lifecycle::Closed => {
            fail(instance, Error::PROVIDER);
        }
        _ => instance.status.set(lifecycle),
    }
}

fn fail(instance: &mut Instance, error: Error) {
    let error = *instance.failure.get_or_insert(error);
    if !instance.closing {
        instance.status.set(Lifecycle::Failed(error));
    }
    instance.batch.fence();
}

fn fence_exports(instance: &Instance, entries: &[Rc<Entry>]) {
    for entry in entries {
        if instance.exports.contains(&entry.identity) {
            entry.closed.set(true);
        }
    }
}

pub(super) fn close(runtime: &mut Runtime) -> Result<(), Error> {
    for index in (0..runtime.modules.len()).rev() {
        close_instance(&mut runtime.modules[index], &runtime.registry)?;
        imports::retire(runtime, index);
        if !runtime.modules[index].stopped() {
            return Err(Error::BUSY);
        }
    }
    runtime.modules.clear();
    Ok(())
}
fn close_instance(
    instance: &mut Instance,
    registry: &Rc<RefCell<super::Registry>>,
) -> Result<(), Error> {
    cleanup::exports(registry, &instance.exports)?;
    instance.batch.close()?;
    if let Some(module) = &mut instance.module {
        module.close()?;
    }
    instance.module = None;
    if instance.parent_imports.is_none() {
        instance.cleanup = Cleanup::Stopped;
    }
    Ok(())
}

pub(super) fn owns_imports(
    runtime: &Runtime,
    scope: &Rc<RefCell<Scope>>,
) -> bool {
    runtime.modules.iter().any(|instance| {
        instance.parent_imports.is_some()
            && Rc::ptr_eq(&instance.imports, scope)
    })
}

pub(super) fn uses_binding(
    runtime: &Runtime,
    scope: &Rc<RefCell<Scope>>,
) -> bool {
    runtime.modules.iter().any(|instance| {
        !instance.stopped()
            && (Rc::ptr_eq(&instance.imports, scope)
                || instance
                    .parent_imports
                    .as_ref()
                    .is_some_and(|parent| Rc::ptr_eq(parent, scope)))
    })
}
