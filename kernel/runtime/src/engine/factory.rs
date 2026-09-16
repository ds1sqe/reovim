use alloc::{
    boxed::Box,
    rc::{Rc, Weak},
    vec::Vec,
};
use core::cell::RefCell;
use reovim_kernel_interface::{
    Error, ResourceId,
    declaration::{Method, ValueLimits},
    provider::{Creation, CreationOps, ResourceProvider},
    resource::AccessId,
};

use super::{
    Entry, Registry,
    access::{Handle, Opened, Scope},
    reserve,
};

mod auxiliary;
mod initialization;

struct Auxiliary {
    identity: ResourceId,
    token: AccessId,
    handle: Option<Handle>,
    returned: bool,
}

struct Stage {
    registry: Weak<RefCell<Registry>>,
    identity: ResourceId,
    imports: Option<Weak<RefCell<Scope>>>,
    api: Option<[u8; 16]>,
    closed: bool,
    primary: Option<Rc<Entry>>,
    auxiliary: Vec<Auxiliary>,
    ceiling: u32,
    retain_loans: bool,
    implementation: Rc<super::Implementation>,
    origin: Rc<Entry>,
}

pub(super) struct Factory {
    stage: Rc<RefCell<Stage>>,
    pub creation: Creation,
    pub rights: u32,
    pub retain_loans: bool,
    reserved: usize,
    credits: usize,
    initial: initialization::Batch,
    installed: bool,
}

impl Factory {
    pub fn reserve(
        scope: &mut Scope,
        current: &Rc<RefCell<Scope>>,
        entry: &Rc<Entry>,
        implementation: &Rc<super::Implementation>,
        opened: &Opened,
        method: &Method,
    ) -> Result<Option<Self>, Error> {
        let Some(created) = method.created() else { return Ok(None) };
        let mut api = None;
        let mut rights = 0;
        created.validate(
            &[0; 4],
            1,
            ValueLimits { bytes: 4, elements: 1, references: 1 },
            |_, expected, access| {
                api = expected;
                rights = access;
                Ok(())
            },
        )?;
        if rights & opened.create_access != rights {
            return Err(Error::DENIED);
        }
        let count =
            method.output().reference_bound(scope.limits.values.references - 1);
        let mut auxiliary = reserve(count)?;
        let registry = scope.registry.upgrade().ok_or(Error::CLOSED)?;
        let initial = initialization::Batch::reserve(
            &mut registry.borrow_mut(),
            count + 1,
        )?;
        let (identity, token) =
            match auxiliary::reserve(scope, current, entry, &registry, count) {
                Ok(reserved) => reserved,
                Err(error) => {
                    initial.unused();
                    return Err(error);
                }
            };
        for index in 0..count {
            auxiliary.push(Auxiliary {
                identity: ResourceId {
                    epoch: identity.epoch,
                    serial: identity.serial + index as u64 + 1,
                },
                token: AccessId(token + index as u64),
                handle: None,
                returned: false,
            });
        }
        let stage = Rc::new(RefCell::new(Stage {
            registry: Rc::downgrade(&registry),
            identity,
            imports: entry.imports.clone(),
            api,
            closed: false,
            primary: None,
            auxiliary,
            ceiling: opened.create_access,
            retain_loans: opened.ownership.retains_loans(),
            implementation: implementation.clone(),
            origin: entry.clone(),
        }));
        let creation = Creation::new(
            Rc::downgrade(&stage),
            CreationOps {
                keyed: None,
                resource: stage_resource,
                auxiliary: Some(auxiliary::stage),
            },
        );
        Ok(Some(Self {
            stage,
            creation,
            rights,
            retain_loans: opened.ownership.retains_loans(),
            reserved: count + 1,
            credits: count,
            initial,
            installed: false,
        }))
    }

    pub fn with_auxiliary(
        &self,
        token: AccessId,
        check: impl FnOnce(&Handle) -> Result<(), Error>,
    ) -> Option<Result<(), Error>> {
        let mut stage = self.stage.borrow_mut();
        let auxiliary =
            stage.auxiliary.iter_mut().find(|aux| aux.token == token)?;
        let result =
            auxiliary.handle.as_ref().ok_or(Error::DENIED).and_then(check);
        auxiliary.returned |= result.is_ok();
        Some(result)
    }

    pub fn prepare(&self) -> Result<Rc<Entry>, Error> {
        let stage = self.stage.borrow();
        let entry = stage.primary.as_ref().ok_or(Error::PROVIDER)?;
        let registry = stage.registry.upgrade().ok_or(Error::CLOSED)?;
        let registry = registry.borrow();
        registry.validate(&[], &entry.implementation.interfaces)?;
        auxiliary::validate_returned(&stage, &registry)?;
        Ok(entry.clone())
    }

    pub fn commit(&mut self) {
        let mut stage = self.stage.borrow_mut();
        stage.closed = true;
        let registry = stage.registry.upgrade().unwrap();
        let mut registry = registry.borrow_mut();
        registry.install(stage.primary.take().unwrap());
        self.reserved -= 1;
        self.reserved -= auxiliary::install(&mut stage, &mut registry);
        self.installed = true;
    }

    pub fn finish(&mut self) -> Result<(), Error> {
        self.stage.borrow_mut().closed = true;
        self.initial.finish(self.installed)?;
        let primary = finish_primary(&self.stage);
        let auxiliary = auxiliary::finish(&self.stage);
        primary?;
        auxiliary?;
        self.release_registry();
        let imports =
            self.stage.borrow().imports.as_ref().and_then(Weak::upgrade);
        if let Some(imports) = imports {
            imports.borrow_mut().reserved_handles -= self.credits;
        }
        self.credits = 0;
        Ok(())
    }

    pub fn abort(
        factory: &mut Option<Self>,
        scope: &mut Scope,
        current: &Rc<RefCell<Scope>>,
    ) {
        let Some(factory) = factory else { return };
        // Admission has not entered provider code; no providers are staged.
        factory.stage.borrow_mut().closed = true;
        factory.initial.unused();
        factory.release_registry();
        let imports =
            factory.stage.borrow().imports.as_ref().and_then(Weak::upgrade);
        auxiliary::release_credits(
            scope,
            current,
            imports.as_ref(),
            factory.credits,
        );
        factory.credits = 0;
    }

    fn release_registry(&mut self) {
        let registry = self.stage.borrow().registry.upgrade().unwrap();
        registry.borrow_mut().release_batch(self.reserved);
        self.reserved = 0;
    }

    pub fn freeze(&self) {
        self.stage.borrow_mut().closed = true;
    }

    pub fn initialize(
        &mut self,
        cx: &mut core::task::Context<'_>,
    ) -> Option<reovim_kernel_interface::Outcome> {
        self.initial.poll(&self.stage, cx)
    }

    pub fn reconcile(&mut self) -> Result<(), Error> {
        self.initial.reconcile()
    }

    pub fn selected(
        &self,
        identity: ResourceId,
    ) -> Option<Rc<super::Implementation>> {
        self.initial.selected(identity)
    }
}

fn stage_resource(
    owner: &Weak<RefCell<Stage>>,
    provider: ResourceProvider,
) -> Result<(), (Error, ResourceProvider)> {
    let Some(owner) = owner.upgrade() else {
        return Err((Error::CLOSED, provider));
    };
    let mut stage = owner.borrow_mut();
    if let Err(error) = validate_primary(&stage, &provider) {
        return Err((error, provider));
    }
    stage.primary = Some(entry(&stage, stage.identity, provider));
    Ok(())
}

fn entry(
    stage: &Stage,
    identity: ResourceId,
    provider: ResourceProvider,
) -> Rc<Entry> {
    Rc::new(Entry {
        closed: core::cell::Cell::new(false),
        identity,
        path: Box::new([]),
        implementation: crate::engine::Implementation::with_origin(
            provider,
            Some(stage.implementation.clone()),
        ),
        imports: stage.imports.clone(),
        _origin: Some(stage.origin.clone()),
        namespace_owner: None,
        directory: false,
        parent: None,
        publication: None,
    })
}

fn validate_primary(
    stage: &Stage,
    provider: &ResourceProvider,
) -> Result<(), Error> {
    if stage.closed {
        return Err(Error::CLOSED);
    }
    if stage.primary.is_some() {
        return Err(Error::CONFLICT);
    }
    if stage.api.is_some_and(|api| {
        !provider.interfaces().iter().any(|offered| offered.api() == &api)
    }) {
        return Err(Error::UNSUPPORTED);
    }
    auxiliary::compatible(stage, provider.interfaces())
}

fn finish_primary(stage: &Rc<RefCell<Stage>>) -> Result<(), Error> {
    let entry = stage.borrow().primary.clone();
    if let Some(entry) = entry {
        entry.implementation.provider.borrow_mut().stop()?;
        stage.borrow_mut().primary.take();
        // Destruction runs after the staging guard has been released.
        drop(entry);
    }
    Ok(())
}

#[cfg(test)]
mod tests;
