use super::{Binding, Entry, Runtime, access::Scope};
use alloc::{boxed::Box, rc::Rc, vec::Vec};
use core::cell::{Cell, RefCell};
use reovim_kernel_interface::{
    Error, ResourceId,
    module::{Declaration, Definition, ModuleScope},
};
mod imports;
mod resolve;

pub(super) struct Registration {
    registry: ResourceId,
    namespace: ResourceId,
    entry: Rc<Entry>,
    definition: Rc<Definition>,
    imports: Rc<RefCell<Scope>>,
    instances: RefCell<Vec<(ModuleScope, ResourceId)>>,
    authorized: Vec<usize>,
}

impl Registration {
    pub(super) fn uses_binding(&self, scope: &Rc<RefCell<Scope>>) -> bool {
        Rc::ptr_eq(&self.imports, scope)
    }

    pub(super) fn identity(&self) -> ResourceId {
        self.entry.identity
    }
    pub(super) fn select(
        &self,
        runtime: &Runtime,
        scope: ModuleScope,
    ) -> Result<ModuleSelection, Error> {
        runtime.select_module(
            self.registry,
            self.definition.declaration().name(),
            self.definition.declaration().revision(),
            scope,
        )
    }
}

/// One exact declaration/implementation and state scope retained by selection.
#[derive(Clone)]
pub struct SelectedModule {
    registration: Rc<Registration>,
    scope: ModuleScope,
    dependencies: Vec<(Rc<Registration>, ModuleScope)>,
}
impl SelectedModule {
    pub(super) fn uses_binding(&self, scope: &Rc<RefCell<Scope>>) -> bool {
        Rc::ptr_eq(&self.registration.imports, scope)
    }

    #[must_use]
    pub fn declaration(&self) -> &Declaration {
        self.registration.definition.declaration()
    }
    #[must_use]
    pub fn resource(&self) -> ResourceId {
        self.registration.entry.identity
    }
    #[must_use]
    pub const fn scope(&self) -> ModuleScope {
        self.scope
    }
}

/// Captured exact graph ordered before consumers; selection is passive.
/// Runtime drives activation through composition or declared Caller control.
pub struct ModuleSelection {
    order: Vec<SelectedModule>,
}
impl ModuleSelection {
    #[must_use]
    pub fn modules(&self) -> &[SelectedModule] {
        &self.order
    }
}

impl Runtime {
    /// Admits passive metadata paired with exact code and composition grants.
    /// Reuses an identical definition, mount, binding and approval set.
    /// Approved names grant the immutable import request's exact rights.
    /// # Errors
    /// Invalid roots/bindings, conflicts or pressure construct no module.
    pub fn register_module(
        &mut self,
        registry: ResourceId,
        namespace: ResourceId,
        definition: Rc<Definition>,
        binding: &Binding,
        authorized_imports: &[&str],
    ) -> Result<ResourceId, Error> {
        let authorized = imports::authorize(&definition, authorized_imports)?;
        self.module_root(registry)?;
        self.module_root(namespace)?;
        if !self.scopes.iter().any(|s| Rc::ptr_eq(s, &binding.scope)) {
            return Err(Error::DENIED);
        }
        if let Some(registered) = self.module_registrations.iter().find(|r| {
            r.registry == registry
                && conflicting(
                    r.definition.declaration(),
                    definition.declaration(),
                )
        }) {
            return duplicate(
                registered,
                namespace,
                &definition,
                binding,
                &authorized,
            );
        }
        let identity = self.registry.borrow_mut().reserve()?;
        let entry = entry(identity, self.module_activation.clone());
        self.registry.borrow_mut().install(entry.clone());
        self.module_registrations.push(Rc::new(Registration {
            registry,
            namespace,
            entry,
            definition,
            imports: binding.scope.clone(),
            instances: RefCell::new(Vec::new()),
            authorized,
        }));
        Ok(identity)
    }

    /// Selects an exact named revision and captures its complete scope graph.
    /// # Errors
    /// Missing/incompatible, cyclic, conflicting or oversized graphs reject
    /// without constructing providers. Optional absence does not hide denial.
    pub fn select_module(
        &self,
        registry: ResourceId,
        name: &str,
        revision: u32,
        scope: ModuleScope,
    ) -> Result<ModuleSelection, Error> {
        self.module_root(registry)?;
        resolve::select(
            &self.module_registrations,
            registry,
            name,
            revision,
            scope,
            self.limits.resources,
        )
    }

    /// Constructs a captured selection with its admitted code and grants.
    /// Dependencies must be Ready; repeated calls join the same attempt.
    /// # Errors
    /// Cross-Runtime selections, expired roots or constructor rejection fail.
    pub fn activate_selected(
        &mut self,
        selected: &SelectedModule,
    ) -> Result<ResourceId, Error> {
        let (id, _) = self.activate_selection(selected)?;
        super::modules::probe::commit(self, id);
        Ok(id)
    }

    pub(super) fn activate_for_probe(
        &mut self,
        selected: &SelectedModule,
    ) -> Result<ResourceId, Error> {
        let (id, fresh) = self.activate_selection(selected)?;
        super::modules::probe::claim(self, id, fresh);
        Ok(id)
    }

    fn activate_selection(
        &mut self,
        selected: &SelectedModule,
    ) -> Result<(ResourceId, bool), Error> {
        self.validate_selected(selected)?;
        let registration = &selected.registration;
        if let Some(id) = reusable(self, selected)? {
            return Ok((id, false));
        }
        let dependencies = ready(self, &selected.dependencies)?;
        registration
            .instances
            .borrow_mut()
            .try_reserve(1)
            .map_err(|_| Error::PRESSURE)?;
        let mut required = Vec::new();
        required
            .try_reserve_exact(registration.definition.required().len())
            .map_err(|_| Error::PRESSURE)?;
        required.extend(
            registration.definition.required().iter().map(AsRef::as_ref),
        );
        let binding = imports::prepare(self, selected)?;
        let result = self.construct_module(
            super::modules::Activation {
                namespace: registration.namespace,
                scope: selected.scope,
                required: &required,
                exports: registration.definition.exports(),
            },
            binding.caller(),
            &binding,
            dependencies,
            |context| registration.definition.construct(context),
        );
        let identity =
            result.inspect_err(|_| imports::discard(self, &binding))?;
        binding.scope.borrow_mut().ready = self.started;
        self.module_import_owner(identity, registration.imports.clone());
        registration.instances.borrow_mut().push((selected.scope, identity));
        Ok((identity, true))
    }

    pub(super) fn validate_probe(
        &self,
        selection: &ModuleSelection,
    ) -> Result<(), Error> {
        for selected in selection.modules() {
            self.validate_selected(selected)?;
        }
        Ok(())
    }

    fn validate_selected(
        &self,
        selected: &SelectedModule,
    ) -> Result<(), Error> {
        let registration = &selected.registration;
        if !self
            .module_registrations
            .iter()
            .any(|r| Rc::ptr_eq(r, registration))
        {
            return Err(Error::DENIED);
        }
        self.module_root(registration.registry)?;
        self.module_root(registration.namespace)?;
        if registration.imports.borrow().closed {
            return Err(Error::CLOSED);
        }
        imports::validate(selected)?;
        compatible_instance(self, selected)
    }

    fn module_root(&self, id: ResourceId) -> Result<(), Error> {
        if self.closing {
            return Err(Error::CLOSED);
        }
        if self.registry.borrow().entries.iter().any(|entry| {
            entry.identity == id
                && !entry.closed.get()
                && !entry.path.is_empty()
        }) {
            Ok(())
        } else {
            Err(Error::UNAVAILABLE)
        }
    }
}

fn conflicting(first: &Declaration, second: &Declaration) -> bool {
    first.revision() == second.revision()
        && (first.id() == second.id() || first.name() == second.name())
}
fn entry(
    identity: ResourceId,
    queue: Rc<super::module_activation::Queue>,
) -> Rc<Entry> {
    let imports = Some(Rc::downgrade(&queue.outputs));
    let provider = super::module_activation::provider(identity, queue);
    Rc::new(Entry {
        closed: Cell::new(false),
        identity,
        path: Box::new([]),
        implementation: crate::engine::Implementation::new(provider),
        imports,
        _origin: None,
        namespace_owner: None,
        directory: false,
        parent: None,
        publication: None,
    })
}

fn duplicate(
    registered: &Registration,
    namespace: ResourceId,
    definition: &Rc<Definition>,
    binding: &Binding,
    authorized: &[usize],
) -> Result<ResourceId, Error> {
    if Rc::ptr_eq(&registered.definition, definition)
        && registered.namespace == namespace
        && Rc::ptr_eq(&registered.imports, &binding.scope)
        && registered.authorized == authorized
    {
        Ok(registered.entry.identity)
    } else {
        Err(Error::CONFLICT)
    }
}

fn existing(
    registration: &Registration,
    scope: ModuleScope,
) -> Option<ResourceId> {
    registration
        .instances
        .borrow()
        .iter()
        .find(|(existing, _)| *existing == scope)
        .map(|(_, identity)| *identity)
}
fn ready(
    runtime: &Runtime,
    dependencies: &[(Rc<Registration>, ModuleScope)],
) -> Result<Vec<ResourceId>, Error> {
    let mut identities = Vec::new();
    identities
        .try_reserve_exact(dependencies.len())
        .map_err(|_| Error::PRESSURE)?;
    for (registration, scope) in dependencies {
        let id = existing(registration, *scope).ok_or(Error::NOT_READY)?;
        match runtime.module_status(id)? {
            reovim_kernel_interface::driver::Lifecycle::Ready => {
                identities.push(id);
            }
            reovim_kernel_interface::driver::Lifecycle::Failed(error) => {
                return Err(error);
            }
            _ => return Err(Error::NOT_READY),
        }
    }
    Ok(identities)
}

fn compatible_instance(
    runtime: &Runtime,
    selected: &SelectedModule,
) -> Result<(), Error> {
    use reovim_kernel_interface::driver::Lifecycle;
    let Some(id) = existing(&selected.registration, selected.scope) else {
        return Ok(());
    };
    if matches!(
        runtime.module_status(id)?,
        Lifecycle::Closed | Lifecycle::Failed(_)
    ) {
        return Ok(());
    }
    let actual = runtime.module_dependencies(id)?;
    if actual.len() != selected.dependencies.len() {
        return Err(Error::CONFLICT);
    }
    if selected.dependencies.iter().zip(actual).any(
        |((registration, scope), id)| {
            existing(registration, *scope) != Some(*id)
        },
    ) {
        return Err(Error::CONFLICT);
    }
    Ok(())
}

fn reusable(
    runtime: &Runtime,
    selected: &SelectedModule,
) -> Result<Option<ResourceId>, Error> {
    use reovim_kernel_interface::driver::Lifecycle;
    let registration = &selected.registration;
    let Some(id) = existing(registration, selected.scope) else {
        return Ok(None);
    };
    match runtime.module_status(id)? {
        Lifecycle::Closing => Err(Error::CLOSED),
        Lifecycle::Closed => {
            registration.instances.borrow_mut().retain(|(_, old)| *old != id);
            Ok(None)
        }
        _ => {
            compatible_instance(runtime, selected)?;
            Ok(Some(id))
        }
    }
}
