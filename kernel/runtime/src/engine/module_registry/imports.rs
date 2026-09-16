use super::{Registration, Runtime, SelectedModule, existing};
use crate::engine::{Binding, Entry, Grant, access::Scope, reserve};
use alloc::{boxed::Box, rc::Rc, vec::Vec};
use core::cell::RefCell;
use reovim_kernel_interface::{
    Error, ResourceId,
    module::{Definition, Dependency, DependencyScope, Import, ModuleScope},
};

pub(super) fn authorize(
    definition: &Definition,
    names: &[&str],
) -> Result<Vec<usize>, Error> {
    if names.len() > 256 {
        return Err(Error::PRESSURE);
    }
    let mut approved = reserve(names.len())?;
    for name in names {
        let index = definition
            .declaration()
            .dependencies()
            .iter()
            .flat_map(|dependency| &dependency.imports)
            .position(|import| import.name() == *name)
            .ok_or(Error::INVALID)?;
        if approved.contains(&index) {
            return Err(Error::CONFLICT);
        }
        approved.push(index);
    }
    approved.sort_unstable();
    Ok(approved)
}

pub(super) fn validate(selected: &SelectedModule) -> Result<(), Error> {
    let mut index = 0;
    for dependency in selected.declaration().dependencies() {
        let end = index + dependency.imports.len();
        if selected_dependency(selected, dependency).is_some()
            && (index..end)
                .any(|index| !selected.registration.authorized.contains(&index))
        {
            return Err(Error::DENIED);
        }
        index = end;
    }
    Ok(())
}

pub(super) fn prepare(
    runtime: &mut Runtime,
    selected: &SelectedModule,
) -> Result<Binding, Error> {
    let count = selected
        .declaration()
        .dependencies()
        .iter()
        .map(|d| d.imports.len())
        .sum::<usize>();
    if runtime.scopes.len() == runtime.limits.callers {
        return Err(Error::PRESSURE);
    }
    let parent = selected.registration.imports.borrow();
    let mut grants = reserve(parent.grants.len() + count)?;
    grants.extend(parent.grants.iter().map(|(_, grant)| *grant));
    let mut publications = reserve(parent.publications.len())?;
    publications.extend(parent.publications.iter().cloned());
    drop(parent);
    let mut aliases = reserve(count)?;
    for dependency in selected.declaration().dependencies() {
        let Some((registration, scope)) =
            selected_dependency(selected, dependency)
        else {
            continue;
        };
        let id = existing(registration, *scope).ok_or(Error::NOT_READY)?;
        for import in &dependency.imports {
            let entry = export(runtime, registration, id, import)?;
            merge(&mut grants, &entry, import);
            aliases.push((alias(import.name())?, entry.identity));
        }
    }
    if grants.len() + publications.len() > runtime.limits.resources {
        return Err(Error::PRESSURE);
    }
    let mut scope = Scope::new(
        &grants,
        &runtime.registry,
        runtime.limits,
        false,
        runtime.non_ascii_names,
    )?;
    scope.aliases = Some(aliases);
    scope.publications = publications;
    let scope = Rc::new(RefCell::new(scope));
    runtime.scopes.push(scope.clone());
    Ok(Binding { scope })
}
fn selected_dependency<'a>(
    selected: &'a SelectedModule,
    dependency: &Dependency,
) -> Option<&'a (Rc<Registration>, ModuleScope)> {
    let scope = match dependency.scope {
        DependencyScope::Same => selected.scope,
        DependencyScope::Global => ModuleScope::Global,
    };
    selected.dependencies.iter().find(|(registration, actual)| {
        registration.definition.declaration().id() == dependency.module
            && *actual == scope
    })
}
fn export(
    runtime: &Runtime,
    registration: &Registration,
    id: ResourceId,
    import: &Import,
) -> Result<Rc<Entry>, Error> {
    let registry = runtime.registry.borrow();
    let namespace = registry
        .entries
        .iter()
        .find(|entry| entry.identity == registration.namespace)
        .ok_or(Error::UNAVAILABLE)?;
    let exports = runtime.module_exports(id)?;
    let entry = registry
        .entries
        .iter()
        .find(|entry| {
            exports.contains(&entry.identity)
                && relative(&entry.path, &namespace.path)
                    == Some(import.export())
        })
        .ok_or(Error::UNAVAILABLE)?;
    if entry.closed.get() {
        return Err(Error::CLOSED);
    }
    if !entry
        .implementation
        .interfaces
        .iter()
        .any(|api| api == import.declaration())
    {
        return Err(Error::UNSUPPORTED);
    }
    Ok(entry.clone())
}
fn relative<'a>(path: &'a [u8], namespace: &[u8]) -> Option<&'a [u8]> {
    let rest = path.strip_prefix(namespace)?;
    if rest.is_empty() {
        return Some(rest);
    }
    if namespace == b"/" {
        return Some(rest);
    }
    rest.strip_prefix(b"/")
}
fn alias(name: &str) -> Result<Box<[u8]>, Error> {
    let mut path = reserve(9 + name.len())?;
    path.extend_from_slice(b"/imports/");
    path.extend_from_slice(name.as_bytes());
    Ok(path.into_boxed_slice())
}
fn merge(grants: &mut Vec<Grant>, entry: &Entry, import: &Import) {
    if let Some(grant) =
        grants.iter_mut().find(|grant| grant.resource == entry.identity)
    {
        grant.access |= import.access();
        grant.create_access |= import.create_access();
        grant.retain_loans |= import.retain_loans();
        return;
    }
    grants.push(Grant {
        resource: entry.identity,
        access: import.access(),
        create_access: import.create_access(),
        retain_loans: import.retain_loans(),
    });
}
pub(super) fn discard(runtime: &mut Runtime, binding: &Binding) {
    let mut scope = binding.scope.borrow_mut();
    // The derived Caller was not ready during inert construction: no request
    // could execute. Fence retained clones and discard only unadmitted custody.
    scope.closed = true;
    scope.records.clear();
    scope.grants.clear();
    scope.publications.clear();
    scope.aliases = None;
    drop(scope);
    runtime.scopes.retain(|scope| !Rc::ptr_eq(scope, &binding.scope));
}
