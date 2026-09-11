mod ownership;
mod unload;
pub use unload::{during_close, surface};

use alloc::{rc::Rc, vec::Vec};
use core::{cell::Cell, task::Context};
use reovim_kernel_interface::{
    Error, ResourceId,
    driver::{Lifecycle, Progress},
    module::{Factory, Module},
    provider::Contribution,
};

use crate::{Runtime, namespace::Kind};

pub struct Instance {
    pub(super) binding: Option<crate::Binding>,
    id: ResourceId,
    status: Rc<Cell<Lifecycle>>,
    module: Option<Module>,
    factory: Option<Factory>,
    registered: crate::namespace::Group,
    dependencies: Vec<(ResourceId, Rc<Cell<Lifecycle>>)>,
    // Explicit selection or dependency of a successful selection.
    retained: bool,
    closing: bool,
    failure: Option<Error>,
    cleanup_error: Option<Error>,
}

impl Runtime {
    /// Adopts a preconstructed inert instance; advancement activates it.
    /// # Errors
    /// Rejection returns all state/code/contributions without executing code.
    pub fn activate_module(
        &mut self,
        path: &str,
        module: Module,
    ) -> Result<ResourceId, (Error, Module)> {
        let validation =
            self.shared.borrow().check_registration(path).and_then(|()| {
                self.modules.try_reserve(1).map_err(|_| Error::PRESSURE)
            });
        if let Err(error) = validation {
            return Err((error, module));
        }
        let status = Rc::new(Cell::new(Lifecycle::Constructed));
        let id = ResourceId(
            self.shared
                .borrow_mut()
                .insert(path, Kind::Module(Rc::clone(&status))),
        );
        self.modules.push(Instance {
            binding: None,
            id,
            status,
            module: Some(module),
            factory: None,
            registered: Rc::default(),
            dependencies: Vec::new(),
            retained: true,
            closing: false,
            failure: None,
            cleanup_error: None,
        });
        Ok(id)
    }

    /// Captures a dependency-free factory; advancement constructs it.
    /// Factories with requirements must use catalog activation.
    /// # Errors
    /// Rejection returns unchanged factory/import/code ownership.
    pub fn activate_factory(
        &mut self,
        path: &str,
        factory: Factory,
    ) -> Result<ResourceId, (Error, Factory)> {
        if !factory.dependencies.is_empty() {
            return Err((Error::INVALID, factory));
        }
        self.adopt_factory(path, factory)
    }

    pub(super) fn adopt_factory(
        &mut self,
        path: &str,
        factory: Factory,
    ) -> Result<ResourceId, (Error, Factory)> {
        let validation =
            self.shared.borrow().check_registration(path).and_then(|()| {
                self.modules.try_reserve(1).map_err(|_| Error::PRESSURE)
            });
        if let Err(error) = validation {
            return Err((error, factory));
        }
        let status = Rc::new(Cell::new(Lifecycle::Constructed));
        let id = ResourceId(
            self.shared
                .borrow_mut()
                .insert(path, Kind::Module(Rc::clone(&status))),
        );
        self.modules.push(Instance {
            binding: None,
            id,
            status,
            module: None,
            factory: Some(factory),
            registered: Rc::default(),
            dependencies: Vec::new(),
            retained: true,
            closing: false,
            failure: None,
            cleanup_error: None,
        });
        Ok(id)
    }

    /// Requests instance teardown; persistent contribution users reject first.
    /// # Errors
    /// Busy preserves live contributions, or retains accepted driver cleanup.
    pub fn close_module(&mut self, id: ResourceId) -> Result<(), Error> {
        if self.module_has_dependants(id) {
            return Err(Error::BUSY);
        }
        let index = self
            .modules
            .iter()
            .position(|m| m.id == id)
            .ok_or(Error::UNAVAILABLE)?;
        let mut instance = self.modules.remove(index);
        let result = instance.close(self);
        self.modules.insert(index, instance);
        result
    }

    pub(super) fn set_dependencies(
        &mut self,
        id: ResourceId,
        dependencies: Vec<(ResourceId, Rc<Cell<Lifecycle>>)>,
    ) {
        let instance = self
            .modules
            .iter_mut()
            .find(|m| m.id == id)
            .expect("selected instance");
        instance.dependencies = dependencies;
        instance.retained = false;
    }

    pub(super) fn discard_unstarted(&mut self, id: ResourceId) {
        let index = self
            .modules
            .iter()
            .position(|m| m.id == id)
            .expect("unobserved selection");
        let instance = self.modules.remove(index);
        assert!(instance.module.is_none() && instance.factory.is_some());
        let mut shared = self.shared.borrow_mut();
        let index = shared.entry_index(id.0).expect("selected resource");
        let entry = shared.entries.remove(index);
        drop(shared);
        drop(entry);
        drop(instance);
    }

    pub(super) fn module_contribution(&self, id: ResourceId) -> bool {
        self.modules.iter().any(|m| m.registered.borrow().contains(&id))
    }

    pub(crate) fn close_client_modules(
        &mut self,
        owner: u64,
    ) -> Result<(), Error> {
        let ids: Vec<_> = self
            .modules
            .iter()
            .filter(|m| {
                m.binding
                    .as_ref()
                    .is_some_and(|b| b.scope.borrow().owner == owner)
            })
            .map(|m| m.id)
            .collect();
        let mut busy = false;
        for id in &ids {
            match self.close_module(*id) {
                Ok(()) => (),
                Err(Error::BUSY) => busy = true,
                Err(error) => return Err(error),
            }
        }
        if busy {
            return Err(Error::BUSY);
        }
        self.modules.retain(|m| !ids.contains(&m.id));
        Ok(())
    }

    pub(super) fn close_modules(&mut self) -> Result<bool, Error> {
        let ids: Vec<_> = self.modules.iter().map(|m| m.id).collect();
        for id in ids {
            match self.close_module(id) {
                Ok(()) => (),
                Err(error) if error == Error::BUSY => (),
                Err(error) => return Err(error),
            }
        }
        Ok(self
            .modules
            .iter()
            .all(|m| m.module.is_none() && m.factory.is_none()))
    }

    pub(super) fn advance_modules(
        &mut self,
        wake: &mut Context<'_>,
    ) -> Progress {
        let mut more = false;
        // One bounded turn per instance; Idle requires the complete sweep.
        for index in 0..self.modules.len() {
            let mut instance = self.modules.remove(index);
            let next = instance.advance(self, wake);
            self.modules.insert(index, instance);
            more |= next == Progress::More;
        }
        more |= self.settle_module_ownership();
        if more { Progress::More } else { Progress::Idle }
    }
}

impl Instance {
    fn dependencies_ready(&self) -> Result<bool, Error> {
        if self.closing || (self.factory.is_none() && self.module.is_none()) {
            return Ok(true);
        }
        let mut ready = true;
        for (_, status) in &self.dependencies {
            ready &= dependency_ready(status.get())?;
        }
        Ok(ready)
    }

    fn construct(&mut self, factory: &Factory) {
        let module = match factory.construct(self.id) {
            Ok(module) => module,
            Err(error) => {
                self.fail(error);
                return;
            }
        };
        let inert = module.status() == Lifecycle::Constructed;
        self.module = Some(module);
        if !inert {
            self.fail(Error::PROVIDER);
        }
    }
    fn fail(&mut self, error: Error) {
        let original = *self.failure.get_or_insert(error);
        self.status.set(Lifecycle::Failed(original));
    }

    fn retry_close(&mut self, runtime: &mut Runtime) -> Progress {
        match self.close(runtime) {
            Ok(()) => Progress::More,
            Err(error) if error == Error::BUSY => Progress::More,
            Err(error) => {
                self.fail(error);
                Progress::Idle
            }
        }
    }

    fn advance(
        &mut self,
        runtime: &mut Runtime,
        wake: &mut Context<'_>,
    ) -> Progress {
        match self.dependencies_ready() {
            Ok(true) => (),
            Ok(false) => return Progress::Idle,
            Err(error) => {
                self.fail(error);
                return self.retry_close(runtime);
            }
        }
        if let Some(factory) = self.factory.take() {
            self.construct(&factory);
            return Progress::More;
        }
        if self.module.is_none() {
            return Progress::Idle;
        }
        if matches!(self.status.get(), Lifecycle::Failed(_)) && !self.closing {
            return self.retry_close(runtime);
        }
        let Some(module) = self.module.as_mut() else {
            return Progress::Idle;
        };
        let progress = match module.advance(wake) {
            Ok(progress) => progress,
            Err(error) => {
                self.driver_failed(error);
                return Progress::More;
            }
        };
        if self.closing {
            return self.finish_close(progress);
        }
        if progress == Progress::Stopped {
            self.fail(Error::PROVIDER);
            return Progress::More;
        }
        match module.status() {
            Lifecycle::Ready if self.registered.borrow().is_empty() => {
                self.publish(runtime);
            }
            Lifecycle::Failed(error) => {
                self.fail(error);
            }
            Lifecycle::Constructed | Lifecycle::Starting
                if self.registered.borrow().is_empty() =>
            {
                self.status.set(Lifecycle::Starting);
            }
            Lifecycle::Ready => (),
            _ => self.fail(Error::PROVIDER),
        }
        progress
    }

    fn publish(&mut self, runtime: &mut Runtime) {
        let module = self.module.as_mut().expect("live instance");
        let values = core::mem::take(&mut module.contributions);
        let result = if let Some(binding) = &self.binding {
            runtime.register_bound(binding, values)
        } else {
            runtime.register_contributions(values)
        };
        match result {
            Ok(ids) => {
                attach(
                    &mut runtime.shared.borrow_mut(),
                    &ids,
                    &self.status,
                    &self.registered,
                );
                *self.registered.borrow_mut() = ids;
                self.status.set(Lifecycle::Ready);
            }
            Err((error, values)) => {
                module.contributions = values;
                self.fail(error);
            }
        }
    }

    fn close(&mut self, runtime: &mut Runtime) -> Result<(), Error> {
        if self.factory.take().is_some() {
            self.status
                .set(self.failure.map_or(Lifecycle::Closed, Lifecycle::Failed));
        }
        let Some(module) = self.module.as_mut() else {
            return Ok(());
        };
        if self.closing {
            return Err(Error::BUSY);
        }
        if !self.registered.borrow().is_empty() {
            runtime.unregister_contributions(&self.registered.borrow())?;
            self.registered.borrow_mut().clear();
        }
        for value in &mut module.contributions {
            match value {
                Contribution::Resource { provider, .. } => provider.unload()?,
                Contribution::Model { provider, .. } => provider.unload()?,
            }
        }
        module.contributions.clear();
        self.closing = true;
        self.status
            .set(self.failure.map_or(Lifecycle::Closing, Lifecycle::Failed));
        let result = module.close();
        if let Err(error) = result {
            self.close_failed(error)?;
        }
        Err(Error::BUSY)
    }

    fn driver_failed(&mut self, error: Error) {
        if self.closing {
            self.cleanup_error.get_or_insert(error);
        }
        self.fail(error);
    }

    fn close_failed(&mut self, error: Error) -> Result<(), Error> {
        if error == Error::BUSY {
            return Ok(());
        }
        self.driver_failed(error);
        Err(error)
    }

    fn finish_close(&mut self, progress: Progress) -> Progress {
        if progress == Progress::Stopped {
            self.module = None;
            self.binding = None;
            self.status
                .set(self.failure.map_or(Lifecycle::Closed, Lifecycle::Failed));
            Progress::More
        } else {
            progress
        }
    }
}

pub fn describe(
    status: Lifecycle,
    input: &reovim_kernel_interface::Message,
) -> Result<reovim_kernel_interface::Message, Error> {
    if !input.data.is_empty() {
        return Err(Error::INVALID);
    }
    let value: &[u8] = match status {
        Lifecycle::Constructed => b"constructed",
        Lifecycle::Starting => b"starting",
        Lifecycle::Ready => b"ready",
        Lifecycle::Closing => b"closing",
        Lifecycle::Closed => b"closed",
        Lifecycle::Failed(_) => b"failed",
    };
    let mut data = value.to_vec();
    if let Lifecycle::Failed(error) = status {
        data.extend_from_slice(&error.code.0.to_le_bytes());
    }
    Ok(reovim_kernel_interface::Message::new(input.contract.clone(), &data))
}

pub fn read(
    status: Rc<Cell<Lifecycle>>,
    input: reovim_kernel_interface::Message,
) -> reovim_kernel_interface::provider::ProviderReply {
    use core::{
        future::{poll_fn, ready},
        task::Poll,
    };
    if input.contract.name.as_ref() == "kernel.module.status" {
        return alloc::boxed::Box::pin(ready(describe(status.get(), &input)));
    }
    alloc::boxed::Box::pin(poll_fn(move |_| {
        if !input.data.is_empty() {
            return Poll::Ready(Err(Error::INVALID));
        }
        let result = match status.get() {
            Lifecycle::Constructed | Lifecycle::Starting => {
                return Poll::Pending;
            }
            Lifecycle::Ready => Ok(reovim_kernel_interface::Message::new(
                input.contract.clone(),
                b"ready",
            )),
            Lifecycle::Failed(error) => Err(error),
            Lifecycle::Closing | Lifecycle::Closed => Err(Error::CLOSED),
        };
        Poll::Ready(result)
    }))
}

pub fn require_ready(status: Option<&Cell<Lifecycle>>) -> Result<(), Error> {
    match status.map(Cell::get) {
        None | Some(Lifecycle::Ready) => Ok(()),
        Some(Lifecycle::Failed(error)) => Err(Error::new(error.code.0)),
        Some(Lifecycle::Closing | Lifecycle::Closed) => Err(Error::CLOSED),
        Some(Lifecycle::Constructed | Lifecycle::Starting) => {
            Err(Error::NOT_READY)
        }
    }
}

fn attach(
    shared: &mut crate::Shared,
    ids: &[ResourceId],
    status: &Rc<Cell<Lifecycle>>,
    group: &crate::namespace::Group,
) {
    for id in ids {
        let index =
            shared.entry_index(id.0).expect("newly published contribution");
        shared.entries[index].activation = Some(Rc::clone(status));
        shared.entries[index].group = Some(group.clone());
    }
}

const fn dependency_ready(status: Lifecycle) -> Result<bool, Error> {
    match status {
        Lifecycle::Ready => Ok(true),
        Lifecycle::Constructed | Lifecycle::Starting => Ok(false),
        Lifecycle::Failed(error) => Err(error),
        Lifecycle::Closing | Lifecycle::Closed => Err(Error::CLOSED),
    }
}
