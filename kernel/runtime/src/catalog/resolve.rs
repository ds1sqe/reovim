use alloc::{rc::Rc, vec::Vec};
use core::cell::RefCell;
use reovim_kernel_interface::{Contract, Error, access::Method};

use super::Catalog;
use crate::{
    Shared,
    namespace::{Kind, Scope},
};

pub type Selection = (u64, Rc<RefCell<Catalog>>);

struct Graph {
    visiting: Vec<(u64, usize)>,
    ordered: Vec<Selection>,
    edges: usize,
}

pub fn resolve(
    shared: &Shared,
    scope: &Scope,
    root: u64,
) -> Result<Vec<Selection>, Error> {
    let mut graph = Graph {
        visiting: crate::reserve(shared.limits.resources)?,
        ordered: crate::reserve(shared.limits.resources)?,
        edges: 0,
    };
    enter(shared, scope, root, &mut graph)?;
    while let Some(&(key, index)) = graph.visiting.last() {
        step(shared, scope, key, index, &mut graph)?;
    }
    Ok(graph.ordered)
}

fn step(
    shared: &Shared,
    scope: &Scope,
    key: u64,
    index: usize,
    graph: &mut Graph,
) -> Result<(), Error> {
    let Kind::Catalog(catalog) = &shared.entries[shared.entry_index(key)?].kind
    else {
        return Err(Error::INVALID);
    };
    let entry = catalog.borrow();
    let factory = entry.factory.as_ref().ok_or(Error::UNAVAILABLE)?;
    // A retained instance already owns exact dependency selections. New
    // declarations cannot invalidate that established graph.
    let required = entry
        .selected
        .is_none()
        .then(|| factory.dependencies.get(index))
        .flatten();
    let Some(identity) = required else {
        graph.visiting.pop();
        graph.ordered.push((key, Rc::clone(catalog)));
        return Ok(());
    };
    graph.edges = graph.edges.checked_add(1).ok_or(Error::PRESSURE)?;
    if graph.edges > shared.limits.resources {
        return Err(Error::PRESSURE);
    }
    graph.visiting.last_mut().expect("current declaration").1 += 1;
    let selected = dependency(shared, scope, identity)?;
    enter(shared, scope, selected, graph)
}

fn enter(
    shared: &Shared,
    scope: &Scope,
    key: u64,
    graph: &mut Graph,
) -> Result<(), Error> {
    if graph.visiting.iter().any(|(id, _)| *id == key) {
        return Err(Error::INVALID);
    }
    if graph.ordered.iter().any(|(id, _)| *id == key) {
        return Ok(());
    }
    if !scope.permissions(shared, key).is_some_and(|p| p.allows(Method::Write))
    {
        return Err(Error::DENIED);
    }
    graph.visiting.push((key, 0));
    Ok(())
}

fn dependency(
    shared: &Shared,
    scope: &Scope,
    identity: &Contract,
) -> Result<u64, Error> {
    let mut found = None;
    let mut error = Error::UNAVAILABLE;
    for entry in &shared.entries {
        let Kind::Catalog(catalog) = &entry.kind else {
            continue;
        };
        let declaration = &catalog.borrow().identity;
        if declaration.name != identity.name {
            continue;
        }
        if declaration != identity {
            error = mismatch(error);
            continue;
        }
        error = Error::DENIED;
        if !scope
            .permissions(shared, entry.key)
            .is_some_and(|p| p.allows(Method::Write))
        {
            continue;
        }
        if found.replace(entry.key).is_some() {
            return Err(Error::INVALID);
        }
    }
    found.ok_or(error)
}

fn mismatch(error: Error) -> Error {
    if error == Error::UNAVAILABLE { Error::UNSUPPORTED } else { error }
}
