//! Host package admission outside Runtime callbacks and registry guards.

use alloc::{boxed::Box, rc::Rc, vec::Vec};
use core::{cell::RefCell, task::Context};
use reovim_kernel_interface::{
    Error, Outcome, ResourceId, declaration::ValueLimits, driver::Progress,
    provider::ResourceProvider,
};
use reovim_kernel_runtime::{Binding, Grant, Runtime};
use reovim_sdk::NativeModule;

mod queue;

/// Host policy admits a selected artifact, including its native ABI trust.
///
/// Example: a Linux adapter checks an allowed path before `native::load`.
/// This callback runs only from `Loader::advance`, never a provider callback.
pub type Admit = Rc<dyn Fn(&[u8]) -> Result<NativeModule, Error>>;

/// Explicit publication, import and recipient authority for one loader.
/// Example: one client can admit packages into its configured namespace.
pub struct Target {
    pub registry: ResourceId,
    pub namespace: ResourceId,
    pub imports: Rc<Binding>,
    pub recipient: Rc<Binding>,
    /// Names approved against each package's immutable dependency requests.
    pub approved: Box<[Box<str>]>,
}

struct Admitted {
    path: Box<[u8]>,
    package: ResourceId,
}

/// Composition drives this owner alongside Runtime. Its provider only queues.
/// Dropping it fences admission and completes queued requests as Closed.
pub struct Loader {
    target: Target,
    admit: Admit,
    queue: Rc<RefCell<queue::Queue>>,
    admitted: Vec<Admitted>,
    capacity: usize,
    values: ValueLimits,
}

impl Loader {
    /// Constructs a bounded loader without executing artifact code.
    /// `capacity` bounds both queued requests and retained package selections.
    /// # Errors
    /// Invalid budgets, approval names or allocation pressure reject.
    pub fn new(
        target: Target,
        admit: Admit,
        capacity: usize,
        values: ValueLimits,
    ) -> Result<Self, Error> {
        if capacity == 0 || capacity > 256 || target.approved.len() > 256 {
            return Err(Error::INVALID);
        }
        validate_approvals(&target.approved)?;
        let mut admitted = Vec::new();
        admitted.try_reserve_exact(capacity).map_err(|_| Error::PRESSURE)?;
        Ok(Self {
            target,
            admit,
            queue: Rc::new(RefCell::new(queue::Queue::new(capacity)?)),
            admitted,
            capacity,
            values,
        })
    }

    /// Declares Exec(Bytes path <=4096) -> `ResourceId`, requiring EXEC.
    /// Success grants READ|EXEC on that passive package to the configured
    /// recipient. It does not activate the package or grant its exports.
    /// # Errors
    /// Declaration/provider allocation failure rejects without host execution.
    pub fn provider(&self) -> Result<ResourceProvider, Error> {
        queue::provider(self.queue.clone())
    }

    /// Fences admission and settles queued work, e.g. on process termination.
    /// Already admitted package and request custody stays with Runtime.
    pub fn close(&self) {
        queue::close(&self.queue);
    }

    /// Executes at most one queued admission outside Runtime provider calls.
    /// Repeating a path reuses its retained package, not changed file contents.
    /// A failed recipient grant retains the admitted package as error context;
    /// a new request retries that grant without loading the artifact again.
    pub fn advance(
        &mut self,
        runtime: &mut Runtime,
        cx: &Context<'_>,
    ) -> Progress {
        let Some(request) = queue::next(&self.queue, cx) else {
            return Progress::Idle;
        };
        let Some(mut work) = queue::start(&request) else {
            return Progress::More;
        };
        let result = self.load(runtime, work.path);
        let outcome = match result {
            Ok(package) => {
                work.output.data[..16].copy_from_slice(&package.epoch);
                work.output.data[16..]
                    .copy_from_slice(&package.serial.to_le_bytes());
                Outcome::Ok(work.output)
            }
            Err(error) => Outcome::Failed(error),
        };
        queue::complete(&request, outcome);
        Progress::More
    }

    fn load(
        &mut self,
        runtime: &mut Runtime,
        path: Box<[u8]>,
    ) -> Result<ResourceId, Error> {
        let package =
            match self.admitted.iter().find(|entry| entry.path == path) {
                Some(entry) => entry.package,
                None => self.admit(runtime, path)?,
            };
        runtime
            .grant(
                &self.target.recipient,
                Grant {
                    resource: package,
                    access: 5,
                    create_access: 0,
                    retain_loans: false,
                },
            )
            .map_err(|error| Error::with_id(error.code.0, package))?;
        Ok(package)
    }

    fn admit(
        &mut self,
        runtime: &mut Runtime,
        path: Box<[u8]>,
    ) -> Result<ResourceId, Error> {
        if self.admitted.len() == self.capacity {
            return Err(Error::PRESSURE);
        }
        let native = (self.admit)(&path)?;
        let definition =
            Rc::new(native.definition(self.values, self.capacity)?);
        let package = register(runtime, &self.target, definition)?;
        self.admitted.push(Admitted { path, package });
        Ok(package)
    }
}

/// Register a captured definition using the same authority for boot and load.
pub(crate) fn register(
    runtime: &mut Runtime,
    target: &Target,
    definition: Rc<reovim_kernel_interface::module::Definition>,
) -> Result<ResourceId, Error> {
    let mut approved = Vec::new();
    approved
        .try_reserve_exact(target.approved.len())
        .map_err(|_| Error::PRESSURE)?;
    approved.extend(target.approved.iter().map(AsRef::as_ref));
    runtime.register_module(
        target.registry,
        target.namespace,
        definition,
        &target.imports,
        &approved,
    )
}

impl Drop for Loader {
    fn drop(&mut self) {
        queue::close(&self.queue);
    }
}

fn validate_approvals(names: &[Box<str>]) -> Result<(), Error> {
    for (index, name) in names.iter().enumerate() {
        if name.is_empty()
            || name.len() > 255
            || !name.is_ascii()
            || name.contains(['\0', '/'])
            || matches!(name.as_ref(), "." | "..")
        {
            return Err(Error::INVALID);
        }
        if names[..index].contains(name) {
            return Err(Error::CONFLICT);
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests;
