//! Explicit module publication authority, independent of namespace ancestry.

use super::{Binding, Entry, Grant, Runtime};
use alloc::rc::Rc;
use reovim_kernel_interface::{Error, ResourceId, module::ModuleScope};

/// Authority over contributions published by an exact mount and instance scope.
///
/// Example: grant Global publications at a loader's configured mount before
/// any artifact arrives. This neither grants the namespace nor follows links.
/// The grant retains the scope owner, not each future member; acquiring a
/// member retains that subject. Exact subject grants take precedence.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PublicationGrant {
    /// Exact activation mount identity, e.g. the loader Target's namespace.
    pub namespace: ResourceId,
    /// Instance state owner, e.g. Global; Session execution remains separate.
    pub scope: ModuleScope,
    /// Operation ceiling for acquired members, e.g. READ|EXEC = 5.
    pub access: u32,
    /// Explicit factory result ceiling, e.g. zero forbids delegated creation.
    pub create_access: u32,
    /// Whether acquired members permit retaining input loans.
    pub retain_loans: bool,
}

impl PublicationGrant {
    pub(super) fn member(self, entry: &Entry) -> Option<Grant> {
        (entry.publication == Some((self.namespace, self.scope))).then_some(
            Grant {
                resource: entry.identity,
                access: self.access,
                create_access: self.create_access,
                retain_loans: self.retain_loans,
            },
        )
    }
}

impl Runtime {
    /// Grants current and future members of one explicit publication scope.
    /// Example: authorize late module exports without extending each receipt's
    /// Caller or granting subjects merely because their paths share a prefix.
    /// Existing opened/captured access keeps its originally captured ceiling.
    /// # Errors
    /// Rejects invalid owners, changed grants, frozen imports or pressure.
    pub fn grant_publication(
        &mut self,
        binding: &Binding,
        grant: PublicationGrant,
    ) -> Result<(), Error> {
        let owner = binding.scope.borrow().registry.upgrade();
        if !owner.is_some_and(|owner| Rc::ptr_eq(&owner, &self.registry)) {
            return Err(Error::DENIED);
        }
        let mut scope = binding.scope.borrow_mut();
        if self.closing || scope.closed {
            return Err(Error::CLOSED);
        }
        if (grant.access | grant.create_access) & !7 != 0 {
            return Err(Error::INVALID);
        }
        if let Some((owner, prior)) =
            scope.publications.iter().find(|(_, old)| {
                old.namespace == grant.namespace && old.scope == grant.scope
            })
        {
            return repeated(owner, *prior, grant);
        }
        if self
            .module_registrations
            .iter()
            .any(|r| r.uses_binding(&binding.scope))
            || super::modules::uses_binding(self, &binding.scope)
        {
            return Err(Error::BUSY);
        }
        if scope.publications.len() + scope.grants.len()
            >= self.limits.resources
        {
            return Err(Error::PRESSURE);
        }
        let registry = self.registry.borrow();
        let namespace = registry
            .entries
            .iter()
            .find(|entry| entry.identity == grant.namespace)
            .ok_or(Error::UNAVAILABLE)?;
        if namespace.closed.get() {
            return Err(Error::CLOSED);
        }
        scope.publications.try_reserve(1).map_err(|_| Error::PRESSURE)?;
        scope.publications.push((namespace.clone(), grant));
        Ok(())
    }
}

fn repeated(
    owner: &Entry,
    prior: PublicationGrant,
    grant: PublicationGrant,
) -> Result<(), Error> {
    if owner.closed.get() {
        return Err(Error::CLOSED);
    }
    if prior != grant {
        return Err(Error::CONFLICT);
    }
    Ok(())
}
