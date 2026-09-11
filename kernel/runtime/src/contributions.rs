use alloc::{rc::Rc, vec::Vec};
use core::cell::RefCell;
use reovim_kernel_interface::{Error, ResourceId, provider::Contribution};

use crate::{Runtime, Shared, namespace::Kind, registration};

impl Runtime {
    /// Removes one owner's contribution set only when every member can unload.
    /// Readiness runs outside registry guards with every member fenced.
    /// # Errors
    /// Busy or callback failure restores every registration unchanged.
    pub fn unregister_contributions(
        &mut self,
        registrations: &[ResourceId],
    ) -> Result<(), Error> {
        let mut retired = Vec::new();
        retired
            .try_reserve_exact(registrations.len())
            .map_err(|_| Error::PRESSURE)?;
        let mut shared = self.shared.borrow_mut();
        removable(&shared, registrations)?;
        for id in registrations {
            let index = shared.entry_index(id.0)?;
            let kind = core::mem::replace(
                &mut shared.entries[index].kind,
                Kind::Unloading,
            );
            retired.push((*id, kind));
        }
        drop(shared);
        let readiness =
            retired.iter().try_for_each(|(_, kind)| crate::unload(kind));
        let mut shared = self.shared.borrow_mut();
        if let Err(error) = readiness {
            restore(&mut shared, retired)?;
            return Err(error);
        }
        shared
            .entries
            .retain(|entry| !registrations.iter().any(|id| id.0 == entry.key));
        drop(shared);
        drop(retired);
        Ok(())
    }

    /// Publishes ready contributions together, preserving input order.
    /// Validation executes no provider code. Rejection returns every owner.
    /// # Errors
    /// Rejects invalid metadata, colliding paths or insufficient capacity.
    pub fn register_contributions(
        &mut self,
        contributions: Vec<Contribution>,
    ) -> Result<Vec<ResourceId>, (Error, Vec<Contribution>)> {
        let mut result = Vec::new();
        let validation = validate(&self.shared.borrow(), &contributions)
            .and_then(|()| {
                result
                    .try_reserve_exact(contributions.len())
                    .map_err(|_| Error::PRESSURE)
            });
        if let Err(error) = validation {
            return Err((error, contributions));
        }
        let mut shared = self.shared.borrow_mut();
        for contribution in contributions {
            result.push(publish(&mut shared, contribution));
        }
        Ok(result)
    }
}

fn restore(
    shared: &mut Shared,
    retired: Vec<(ResourceId, Kind)>,
) -> Result<(), Error> {
    for (id, kind) in retired {
        let index = shared.entry_index(id.0)?;
        shared.entries[index].kind = kind;
    }
    Ok(())
}

fn removable(shared: &Shared, ids: &[ResourceId]) -> Result<(), Error> {
    if ids.is_empty() {
        return Err(Error::INVALID);
    }
    for (index, id) in ids.iter().enumerate() {
        if ids[..index].contains(id) {
            return Err(Error::INVALID);
        }
        let entry = &shared.entries[shared.entry_index(id.0)?];
        if !entry.kind.removable() {
            return Err(Error::BUSY);
        }
    }
    Ok(())
}

fn publish(shared: &mut Shared, contribution: Contribution) -> ResourceId {
    let (path, kind) = match contribution {
        Contribution::Resource { path, provider } => {
            (path, Kind::Provider(Rc::new(RefCell::new(provider))))
        }
        Contribution::Model { path, provider } => {
            (path, Kind::Model(Rc::new(RefCell::new(provider))))
        }
    };
    ResourceId(shared.insert(&path, kind))
}

fn validate(shared: &Shared, values: &[Contribution]) -> Result<(), Error> {
    if shared.closing {
        return Err(Error::CLOSED);
    }
    if values.is_empty() {
        return Err(Error::INVALID);
    }
    if values.len() > shared.limits.resources {
        return Err(Error::PRESSURE);
    }
    let mut missing = Vec::new();
    for value in values {
        let path = value.path();
        crate::namespace::validate(path, shared.limits.path_bytes)?;
        if path.is_empty() || shared.path(1, path).is_ok() {
            return Err(Error::INVALID);
        }
        metadata(shared, value)?;
        prefixes(shared, path, values, &mut missing)?;
    }
    if !shared.ids.can_take(missing.len()) {
        return Err(Error::PRESSURE);
    }
    Ok(())
}

fn metadata(shared: &Shared, value: &Contribution) -> Result<(), Error> {
    match value {
        Contribution::Resource { provider, .. } => {
            registration::resource(provider, shared.limits)
        }
        Contribution::Model { provider, .. } => {
            registration::model(provider, shared.limits)
        }
    }
}

fn prefixes<'a>(
    shared: &Shared,
    path: &'a str,
    values: &[Contribution],
    missing: &mut Vec<&'a str>,
) -> Result<(), Error> {
    let ends = path
        .match_indices('/')
        .map(|(index, _)| index)
        .chain(core::iter::once(path.len()));
    for end in ends {
        let prefix = &path[..end];
        if existing_directory(shared, prefix)? {
            continue;
        }
        if end != path.len() && values.iter().any(|v| v.path() == prefix) {
            return Err(Error::INVALID);
        }
        let duplicate = missing.contains(&prefix);
        if duplicate && end == path.len() {
            return Err(Error::INVALID);
        }
        if duplicate {
            continue;
        }
        if shared.entries.len() + missing.len() == shared.limits.resources {
            return Err(Error::PRESSURE);
        }
        missing.try_reserve(1).map_err(|_| Error::PRESSURE)?;
        missing.push(prefix);
    }
    Ok(())
}

fn existing_directory(shared: &Shared, path: &str) -> Result<bool, Error> {
    let Ok(key) = shared.path(1, path) else {
        return Ok(false);
    };
    let entry = &shared.entries[shared.entry_index(key)?];
    if !matches!(entry.kind, Kind::Directory) {
        return Err(Error::INVALID);
    }
    Ok(true)
}
