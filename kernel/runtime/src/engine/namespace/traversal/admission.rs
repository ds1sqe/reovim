use super::{
    Entry, Error, Grant, Rc, RefCell, Registry, Scope, Step, Traversal, codec,
};

type Admitted = (Rc<RefCell<Traversal>>, Rc<Entry>, Grant);
pub(in crate::engine::namespace) fn admit(
    scope: &Scope,
    data: &[u8],
) -> Result<Option<Admitted>, Error> {
    let path = codec::bytes(data, 4096)?;
    codec::path(path, scope.non_ascii_names)?;
    let registry = scope.registry.upgrade().ok_or(Error::CLOSED)?;
    let registry = registry.borrow();
    if registry.entries.iter().any(|entry| entry.path.as_ref() == path)
        || scope.aliases.as_ref().is_some_and(|aliases| {
            aliases.iter().any(|(alias, _)| alias.as_ref() == path)
        })
    {
        return Ok(None);
    }
    let (owner, prefix) = root(scope, &registry, path)?;
    let Some(namespace) = owner
        .implementation
        .provider
        .try_borrow()
        .map_err(|_| Error::BUSY)?
        .namespace()
    else {
        return Ok(None);
    };
    if !namespace.lookup {
        return Err(Error::UNSUPPORTED);
    }
    let publication = match super::super::acquire::authority(scope, &owner) {
        Ok(grant) => Some(grant),
        Err(Error::DENIED) => None,
        Err(error) => return Err(error),
    };
    let grant = scope
        .grants
        .iter()
        .find(|(entry, _)| entry.identity == owner.identity)
        .map(|(_, grant)| *grant)
        .or(publication)
        .ok_or(Error::DENIED)?;
    let mut grants = super::super::super::reserve(scope.grants.len())?;
    grants.extend(scope.grants.iter().map(|(_, grant)| *grant));
    let names = &path[prefix..];
    let count = names.split(|byte| *byte == b'/').count();
    let owners = super::super::super::reserve(count)?;
    let work = Rc::new(RefCell::new(Traversal {
        registry: scope.registry.clone(),
        names: names.into(),
        offset: 0,
        current: owner.clone(),
        owners,
        step: Step::Lookup,
        native: None,
        work: None,
        observed: None,
        outcome: None,
        taken: false,
        fenced: false,
        quarantine: None,
        maximum: scope.limits.values.bytes,
        publication,
        grants,
        grant: None,
        ready: None,
    }));
    Ok(Some((work, owner, grant)))
}
fn root(
    scope: &Scope,
    registry: &Registry,
    path: &[u8],
) -> Result<(Rc<Entry>, usize), Error> {
    if let Some(aliases) = &scope.aliases
        && path.starts_with(b"/imports/")
    {
        let (alias, id) = aliases
            .iter()
            .filter(|(alias, _)| below(path, alias))
            .max_by_key(|(alias, _)| alias.len())
            .ok_or(Error::DENIED)?;
        let owner = registry
            .entries
            .iter()
            .find(|entry| entry.identity == *id)
            .cloned()
            .ok_or(Error::CLOSED)?;
        return Ok((owner, alias.len() + 1));
    }
    let owner = registry
        .entries
        .iter()
        .filter(|entry| below(path, &entry.path))
        .max_by_key(|entry| entry.path.len())
        .cloned()
        .ok_or(Error::DENIED)?;
    let prefix =
        if owner.path.as_ref() == b"/" { 1 } else { owner.path.len() + 1 };
    Ok((owner, prefix))
}
fn below(path: &[u8], prefix: &[u8]) -> bool {
    !prefix.is_empty()
        && path.starts_with(prefix)
        && (prefix == b"/" || path.get(prefix.len()) == Some(&b'/'))
}
