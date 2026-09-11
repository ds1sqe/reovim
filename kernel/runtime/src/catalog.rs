mod resolve;
mod selection;
mod withdraw;

use alloc::{boxed::Box, format, rc::Rc};
use core::{
    cell::{Cell, RefCell},
    future::{poll_fn, ready},
    task::Poll,
};
use reovim_kernel_interface::{
    Contract, Error, Message, ResourceId,
    access::Method,
    driver::Lifecycle,
    module::Factory,
    provider::{Permissions, ProviderReply},
};

use crate::{Runtime, namespace::Kind, request::Record};

pub struct Catalog {
    identity: Contract,
    factory: Option<Factory>,
    selected: Option<(ResourceId, Rc<Cell<Lifecycle>>)>,
    binding: Option<crate::Binding>,
}

impl Runtime {
    /// Registers an exact named factory without executing its constructor.
    /// # Errors
    /// Rejection returns the declaration and factory unchanged.
    pub fn register_factory(
        &mut self,
        path: &str,
        identity: Contract,
        factory: Factory,
    ) -> Result<ResourceId, (Error, Contract, Factory)> {
        self.install_factory(path, identity, factory, None)
    }

    fn install_factory(
        &self,
        path: &str,
        identity: Contract,
        factory: Factory,
        binding: Option<crate::Binding>,
    ) -> Result<ResourceId, (Error, Contract, Factory)> {
        let shared = self.shared.borrow();
        let validation = shared
            .check_registration(path)
            .and_then(|()| validate(&identity, shared.limits))
            .and_then(|()| validate_dependencies(&factory, shared.limits));
        drop(shared);
        if let Err(error) = validation {
            return Err((error, identity, factory));
        }
        let entry = Catalog {
            identity,
            factory: Some(factory),
            selected: None,
            binding,
        };
        let key = self
            .shared
            .borrow_mut()
            .insert(path, Kind::Catalog(Rc::new(RefCell::new(entry))));
        Ok(ResourceId(key))
    }

    /// Registers a catalog factory with its actual receiving namespace.
    /// # Errors
    /// Rejects foreign/closed bindings or invalid catalog declarations.
    pub fn register_bound_factory(
        &mut self,
        binding: &crate::Binding,
        path: &str,
        identity: Contract,
        factory: Factory,
    ) -> Result<ResourceId, (Error, Contract, Factory)> {
        if !Rc::ptr_eq(&self.shared, &binding.shared) {
            return Err((Error::INVALID, identity, factory));
        }
        if binding.scope.borrow().closed {
            return Err((Error::CLOSED, identity, factory));
        }
        self.install_factory(path, identity, factory, Some(binding.clone()))
    }

    pub(super) fn catalog_request(
        &mut self,
        catalog: &Rc<RefCell<Catalog>>,
        record: &Rc<RefCell<Record>>,
    ) -> ProviderReply {
        self.begin_catalog(catalog, record)
            .unwrap_or_else(|error| Box::pin(ready(Err(error))))
    }

    fn begin_catalog(
        &mut self,
        catalog: &Rc<RefCell<Catalog>>,
        record: &Rc<RefCell<Record>>,
    ) -> Result<ProviderReply, Error> {
        let (subject, method, input, scope) = {
            let record = record.borrow();
            (
                record.subject,
                record.method,
                record.input.clone(),
                Rc::clone(&record.scope),
            )
        };
        if !input.data.is_empty() {
            return Err(Error::INVALID);
        }
        if method == Method::Read {
            let catalog = catalog.borrow();
            let mut data = catalog.identity.version.to_le_bytes().to_vec();
            data.extend_from_slice(catalog.identity.name.as_bytes());
            return Ok(Box::pin(ready(Ok(Message::new(
                input.contract,
                &data,
            )))));
        }
        if input.contract.name.as_ref() == "kernel.module.withdraw" {
            self.withdraw_catalog(subject, catalog)?;
            return Ok(Box::pin(ready(Ok(Message::new(input.contract, &[])))));
        }
        self.shared.borrow().entry_index(subject)?;
        scope
            .borrow_mut()
            .grants
            .try_reserve(1)
            .map_err(|_| Error::PRESSURE)?;
        let graph =
            resolve::resolve(&self.shared.borrow(), &scope.borrow(), subject)?;
        let (instance, status) = self.select_graph(&graph)?;
        let mut scope = scope.borrow_mut();
        if let Some((_, rights)) =
            scope.grants.iter_mut().find(|(id, _)| *id == instance.0)
        {
            *rights |= Permissions::READ;
        } else {
            scope.grants.push((instance.0, Permissions::READ));
        }
        Ok(activation(instance, status, input.contract))
    }

    fn select(
        &mut self,
        catalog: &mut Catalog,
        key: u64,
    ) -> Result<(ResourceId, Rc<Cell<Lifecycle>>), Error> {
        if let Some(selected) = &catalog.selected {
            return Ok(selected.clone());
        }
        let path = match &catalog.binding {
            Some(binding) => {
                binding.publication_path(&format!("instances/{key}"))?
            }
            None => format!("modules/instances/{key}").into(),
        };
        let instance = self
            .adopt_factory(
                &path,
                catalog.factory.as_ref().ok_or(Error::UNAVAILABLE)?.clone(),
            )
            .map_err(|(error, _)| error)?;
        if let Some(binding) = &catalog.binding {
            self.attach_binding(binding);
        }
        let shared = self.shared.borrow();
        let Kind::Module(status) =
            &shared.entries[shared.entry_index(instance.0)?].kind
        else {
            return Err(Error::PROVIDER);
        };
        let selected = (instance, Rc::clone(status));
        catalog.selected = Some(selected.clone());
        Ok(selected)
    }
}

pub fn surface(
    method: Method,
    contract: &Contract,
) -> Result<Box<[Permissions]>, Error> {
    match (method, contract.name.as_ref(), contract.version) {
        (Method::Read, "kernel.module.info", 1)
        | (Method::Write, "kernel.module.withdraw", 1) => Ok(Box::new([])),
        (Method::Write, "kernel.module.activate", 1) => {
            Ok(Box::new([Permissions::READ]))
        }
        _ => Err(Error::UNSUPPORTED),
    }
}

fn validate_dependencies(
    factory: &Factory,
    limits: crate::Limits,
) -> Result<(), Error> {
    if factory.dependencies.len() > limits.resources {
        return Err(Error::PRESSURE);
    }
    for (index, identity) in factory.dependencies.iter().enumerate() {
        validate(identity, limits)?;
        if factory.dependencies[..index].contains(identity) {
            return Err(Error::INVALID);
        }
    }
    Ok(())
}

fn validate(identity: &Contract, limits: crate::Limits) -> Result<(), Error> {
    if identity.name.is_empty() {
        return Err(Error::INVALID);
    }
    if identity.name.len() > limits.path_bytes
        || identity.name.len().saturating_add(4) > limits.message_bytes
    {
        return Err(Error::PRESSURE);
    }
    Ok(())
}

fn activation(
    instance: ResourceId,
    status: Rc<Cell<Lifecycle>>,
    contract: Contract,
) -> ProviderReply {
    Box::pin(poll_fn(move |_| {
        let result = match status.get() {
            Lifecycle::Constructed | Lifecycle::Starting => {
                return Poll::Pending;
            }
            Lifecycle::Ready => {
                Ok(Message::reference(contract.clone(), instance))
            }
            Lifecycle::Failed(error) => Err(error),
            Lifecycle::Closing | Lifecycle::Closed => Err(Error::CLOSED),
        };
        Poll::Ready(result)
    }))
}
