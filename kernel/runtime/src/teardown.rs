//! Assembly closes private namespaces after their recoverable work is adopted.

use crate::{Binding, Runtime, Shared, namespace::Kind};
use alloc::{rc::Rc, vec::Vec};
use reovim_kernel_interface::{Error, ResourceId};

impl Runtime {
    /// Retires one private client namespace and its bound modules.
    /// Caller-owned snapshots are released; live Buffer/Session state must be
    /// closed explicitly under its own policy before this operation can finish.
    /// Shared alias targets and other clients' snapshots remain alive.
    ///
    /// # Errors
    /// Busy retains recovery access and unfinished cleanup for a later retry.
    /// Rejects global, foreign, imported or multiply owned namespace bindings.
    pub fn retire_client(&mut self, client: &Binding) -> Result<(), Error> {
        let (owner, root) = validate(self, client)?;
        self.close_owned_access(client)?;
        quiescent(&self.shared.borrow(), owner, root)?;
        release_participations(self, owner);
        release_snapshots(self, owner);
        if self
            .shared
            .borrow()
            .entries
            .iter()
            .any(|entry| live(&entry.kind, owner))
        {
            return Err(Error::BUSY);
        }
        self.close_client_modules(owner)?;
        self.retire_catalogs(root)?;
        self.retire_providers(root)?;
        self.retire_names(owner, root)
    }

    fn retire_catalogs(&self, root: u64) -> Result<(), Error> {
        let shared = self.shared.borrow();
        let catalogs: Vec<_> = shared
            .entries
            .iter()
            .filter(|entry| shared.descends(entry.key, root))
            .filter_map(|entry| match &entry.kind {
                Kind::Catalog(catalog) => Some((entry.key, catalog.clone())),
                _ => None,
            })
            .collect();
        drop(shared);
        for (key, catalog) in catalogs {
            self.withdraw_catalog(key, &catalog)?;
        }
        Ok(())
    }

    fn retire_providers(&mut self, root: u64) -> Result<(), Error> {
        let shared = self.shared.borrow();
        let ids: Vec<_> = shared
            .entries
            .iter()
            .filter(|e| {
                shared.descends(e.key, root)
                    && matches!(e.kind, Kind::Provider(_) | Kind::Model(_))
            })
            .map(|e| ResourceId(e.key))
            .collect();
        drop(shared);
        for id in ids {
            self.unregister(id)?;
        }
        Ok(())
    }

    fn retire_names(&self, owner: u64, root: u64) -> Result<(), Error> {
        let mut shared = self.shared.borrow_mut();
        let ids: Vec<_> = shared
            .entries
            .iter()
            .filter(|entry| shared.descends(entry.key, root))
            .map(|entry| entry.key)
            .collect();
        if shared.entries.iter().any(|e| {
            ids.contains(&e.key)
                && (!settled(&e.kind) || self.module_live(ResourceId(e.key)))
        }) {
            return Err(Error::BUSY);
        }
        shared.entries.retain(|e| !ids.contains(&e.key));
        for scope in shared
            .scopes
            .iter()
            .filter_map(alloc::rc::Weak::upgrade)
            .filter(|scope| scope.borrow().owner == owner)
        {
            scope.borrow_mut().closed = true;
        }
        Ok(())
    }
}
fn validate(runtime: &Runtime, client: &Binding) -> Result<(u64, u64), Error> {
    if !Rc::ptr_eq(&runtime.shared, &client.shared) {
        return Err(Error::INVALID);
    }
    let scope = client.scope.borrow();
    if scope.closed {
        return Err(Error::CLOSED);
    }
    if scope.root == 1 || scope.id != scope.owner {
        return Err(Error::DENIED);
    }
    let shared = runtime.shared.borrow();
    let conflict =
        shared.scopes.iter().filter_map(alloc::rc::Weak::upgrade).any(
            |other| {
                other.borrow().root == scope.root
                    && other.borrow().owner != scope.owner
                    && !other.borrow().closed
            },
        );
    if conflict {
        return Err(Error::BUSY);
    }
    Ok((scope.owner, scope.root))
}
fn quiescent(shared: &Shared, owner: u64, root: u64) -> Result<(), Error> {
    let keys: Vec<_> = shared
        .entries
        .iter()
        .filter(|entry| {
            shared.descends(entry.key, root) || snapshot(&entry.kind, owner)
        })
        .map(|entry| ResourceId(entry.key))
        .collect();
    if shared.requests.iter().any(|record| {
        let record = record.borrow();
        let custodian = record
            .save
            .as_ref()
            .and_then(|save| save.borrow().custodian.clone());
        custodian.as_ref().unwrap_or(&record.scope).borrow().owner == owner
            || keys.contains(&ResourceId(record.subject))
            || keys.iter().any(|id| record.references(*id))
    }) {
        return Err(Error::BUSY);
    }
    Ok(())
}
fn snapshot(kind: &Kind, owner: u64) -> bool {
    matches!(kind, Kind::Snapshot(snapshot) if snapshot.owner == owner)
}
fn live(kind: &Kind, owner: u64) -> bool {
    match kind {
        Kind::Buffer(buffer) => buffer.borrow().owner == owner,
        Kind::Session(session) => session.borrow().owner == owner,
        _ => false,
    }
}
fn settled(kind: &Kind) -> bool {
    if let Kind::Participation(participant) = kind {
        return participant.borrow().settled();
    }
    matches!(
        kind,
        Kind::Directory
            | Kind::Module(_)
            | Kind::ClosedBuffer { .. }
            | Kind::ClosedSession { .. }
    )
}
fn release_participations(runtime: &Runtime, owner: u64) {
    let participants: Vec<_> = runtime
        .shared
        .borrow()
        .entries
        .iter()
        .filter_map(|entry| match &entry.kind {
            Kind::Participation(p) if p.borrow().owner == owner => {
                Some(p.clone())
            }
            _ => None,
        })
        .collect();
    for participant in participants {
        let retired = participant.borrow_mut().detach();
        drop(retired);
    }
}
fn release_snapshots(runtime: &Runtime, owner: u64) {
    loop {
        let mut shared = runtime.shared.borrow_mut();
        let Some(index) = shared
            .entries
            .iter()
            .position(|entry| snapshot(&entry.kind, owner))
        else {
            break;
        };
        let retired = shared.entries.remove(index);
        drop(shared);
        drop(retired);
    }
}
