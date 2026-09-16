use super::{
    Declaration, Error, ModuleScope, ModuleSelection, Rc, Registration,
    ResourceId, SelectedModule, Vec,
};
use reovim_kernel_interface::module::{Dependency, DependencyScope};

struct Frame {
    module: SelectedModule,
    next: usize,
}

pub(super) fn select(
    entries: &[Rc<Registration>],
    registry: ResourceId,
    name: &str,
    revision: u32,
    scope: ModuleScope,
    capacity: usize,
) -> Result<ModuleSelection, Error> {
    if name.is_empty() || !name.is_ascii() || name.len() > 255 || revision == 0
    {
        return Err(Error::INVALID);
    }
    valid_scope(scope)?;
    let named = entries.iter().find(|entry| {
        entry.registry == registry
            && entry.definition.declaration().name() == name
            && entry.definition.declaration().revision() == revision
    });
    let root = named
        .cloned()
        .ok_or_else(|| missing(entries, registry, |d| d.name() == name))?;
    supports(root.definition.declaration(), scope)?;
    let mut stack = Vec::new();
    let mut order = Vec::new();
    stack.try_reserve_exact(capacity).map_err(|_| Error::PRESSURE)?;
    order.try_reserve_exact(capacity).map_err(|_| Error::PRESSURE)?;
    stack.push(Frame { module: selected(root, scope), next: 0 });
    while !stack.is_empty() {
        step(entries, registry, &mut stack, &mut order, capacity)?;
    }
    Ok(ModuleSelection { order })
}

fn step(
    entries: &[Rc<Registration>],
    registry: ResourceId,
    stack: &mut Vec<Frame>,
    order: &mut Vec<SelectedModule>,
    capacity: usize,
) -> Result<(), Error> {
    let frame = stack.last_mut().unwrap();
    let dependencies = frame.module.declaration().dependencies();
    let Some(dependency) = dependencies.get(frame.next) else {
        order.push(stack.pop().unwrap().module);
        return Ok(());
    };
    frame.next += 1;
    let scope = match dependency.scope {
        DependencyScope::Same => frame.module.scope,
        DependencyScope::Global => ModuleScope::Global,
    };
    let Some(registration) = dependency_entry(entries, registry, dependency)?
    else {
        return Ok(());
    };
    supports(registration.definition.declaration(), scope)?;
    frame.module.dependencies.try_reserve(1).map_err(|_| Error::PRESSURE)?;
    frame.module.dependencies.push((registration.clone(), scope));
    let selected = selected(registration, scope);
    if stack.iter().any(|old| conflicts(&old.module, &selected))
        || order.iter().any(|old| conflicts(old, &selected))
    {
        return Err(Error::CONFLICT);
    }
    if stack.iter().any(|old| same(&old.module, &selected)) {
        return Err(Error::INVALID);
    }
    if order.iter().any(|old| same(old, &selected)) {
        return Ok(());
    }
    if stack.len() + order.len() == capacity {
        return Err(Error::PRESSURE);
    }
    stack.push(Frame { module: selected, next: 0 });
    Ok(())
}

fn dependency_entry(
    entries: &[Rc<Registration>],
    registry: ResourceId,
    dependency: &Dependency,
) -> Result<Option<Rc<Registration>>, Error> {
    let found = entries.iter().find(|entry| {
        entry.registry == registry
            && entry.definition.declaration().id() == dependency.module
            && entry.definition.declaration().revision() == dependency.revision
    });
    if let Some(entry) = found {
        return Ok(Some(entry.clone()));
    }
    let error = missing(entries, registry, |d| d.id() == dependency.module);
    if dependency.optional && error == Error::UNAVAILABLE {
        Ok(None)
    } else {
        Err(error)
    }
}

fn missing(
    entries: &[Rc<Registration>],
    registry: ResourceId,
    matches: impl Fn(&Declaration) -> bool,
) -> Error {
    if entries.iter().any(|entry| {
        entry.registry == registry && matches(entry.definition.declaration())
    }) {
        Error::UNSUPPORTED
    } else {
        Error::UNAVAILABLE
    }
}
fn same(first: &SelectedModule, second: &SelectedModule) -> bool {
    first.resource() == second.resource() && first.scope == second.scope
}
fn valid_scope(scope: ModuleScope) -> Result<(), Error> {
    if let ModuleScope::Session(id) = scope
        && (id.serial == 0 || id.epoch == [0; 16])
    {
        return Err(Error::INVALID);
    }
    Ok(())
}
const fn supports(
    declaration: &Declaration,
    scope: ModuleScope,
) -> Result<(), Error> {
    let bit = match scope {
        ModuleScope::Global => 1,
        ModuleScope::Session(_) => 2,
    };
    if declaration.scopes() & bit == 0 {
        Err(Error::UNSUPPORTED)
    } else {
        Ok(())
    }
}

fn conflicts(first: &SelectedModule, second: &SelectedModule) -> bool {
    first.scope == second.scope
        && first.declaration().id() == second.declaration().id()
        && first.declaration().revision() != second.declaration().revision()
}

const fn selected(
    registration: Rc<Registration>,
    scope: ModuleScope,
) -> SelectedModule {
    SelectedModule { registration, scope, dependencies: Vec::new() }
}
