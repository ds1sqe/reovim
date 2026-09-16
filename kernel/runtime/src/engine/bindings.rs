use super::{Binding, Grant, Runtime, access::Scope, uncertain};
use alloc::rc::Rc;
use core::cell::RefCell;
use reovim_kernel_interface::Error;

impl Runtime {
    /// Grants a newly admitted Resource to an existing composition binding.
    /// Example: expose a loaded package to a retained service Caller without
    /// replacing its access, request IDs or original result custody.
    /// An identical grant is idempotent; existing authority cannot be changed.
    /// Registered or live module import bindings cannot be extended.
    /// # Errors
    /// Rejects foreign/closed bindings, invalid grants, changed authority,
    /// frozen module imports and exhausted grant capacity without mutation.
    pub fn grant(
        &mut self,
        binding: &Binding,
        grant: Grant,
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
        if let Some((entry, prior)) = scope
            .grants
            .iter()
            .find(|(entry, _)| entry.identity == grant.resource)
        {
            return repeated(entry, *prior, grant);
        }
        if self
            .module_registrations
            .iter()
            .any(|r| r.uses_binding(&binding.scope))
            || super::modules::uses_binding(self, &binding.scope)
        {
            return Err(Error::BUSY);
        }
        if scope.grants.len() + scope.publications.len()
            >= self.limits.resources
        {
            return Err(Error::PRESSURE);
        }
        scope.grants.try_reserve(1).map_err(|_| Error::PRESSURE)?;
        super::access::capture_grant(
            &mut scope.grants,
            &self.registry.borrow().entries,
            grant,
        )
    }

    /// Retires one composition binding without stopping other callers.
    /// Live module/activation imports reject Busy before fencing. Otherwise
    /// new admission is fenced immediately. Existing receipts retain result,
    /// cancellation and recovery access until released and cleaned up.
    /// # Errors
    /// Foreign bindings are denied. Busy retains admitted work and grants;
    /// Incomplete retains unresolved effects. Continue driving, then retry.
    pub fn retire_binding(&mut self, binding: &Binding) -> Result<(), Error> {
        let owner = binding.scope.borrow().registry.upgrade();
        if !owner.is_some_and(|owner| Rc::ptr_eq(&owner, &self.registry)) {
            return Err(Error::DENIED);
        }
        if super::modules::uses_binding(self, &binding.scope)
            || self.probes.iter().any(|work| work.uses_binding(&binding.scope))
        {
            return Err(Error::BUSY);
        }
        retire(self, &binding.scope)
    }
}

fn repeated(
    entry: &super::Entry,
    prior: Grant,
    grant: Grant,
) -> Result<(), Error> {
    if entry.closed.get() {
        return Err(Error::CLOSED);
    }
    if prior != grant {
        return Err(Error::CONFLICT);
    }
    Ok(())
}

pub(super) fn retire(
    runtime: &mut Runtime,
    target: &Rc<RefCell<Scope>>,
) -> Result<(), Error> {
    target.borrow_mut().closed = true;
    if uncertain(target) {
        return Err(Error::INCOMPLETE);
    }
    if let Some(error) = imports_in_use(runtime, target) {
        return Err(error);
    }
    let mut scope = target.borrow_mut();
    if !scope.records.is_empty() || scope.reserved_handles != 0 {
        return Err(Error::BUSY);
    }
    scope.handles.clear();
    scope.grants.clear();
    scope.publications.clear();
    scope.aliases = None;
    drop(scope);
    runtime.scopes.retain(|scope| !Rc::ptr_eq(scope, target));
    Ok(())
}

fn imports_in_use(
    runtime: &Runtime,
    target: &Rc<RefCell<Scope>>,
) -> Option<Error> {
    if runtime.registry.borrow().opening.uses_imports(target) {
        return Some(Error::BUSY);
    }
    runtime.scopes.iter().find_map(|scope| {
        scope
            .borrow()
            .records
            .iter()
            .find_map(|record| record_use(&record.borrow(), target))
    })
}

fn record_use(
    record: &super::custody::Record,
    target: &Rc<RefCell<Scope>>,
) -> Option<Error> {
    if record.busy {
        return Some(Error::BUSY);
    }
    let task = record.task.as_ref()?;
    let error = if task.uncertain() { Error::INCOMPLETE } else { Error::BUSY };
    task.uses_imports(target).then_some(error)
}
