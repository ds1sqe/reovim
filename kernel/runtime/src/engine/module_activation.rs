use super::{
    Entry, Limits, ModuleProbe, Registry, Runtime, access::Scope, reserve,
};
use alloc::{boxed::Box, rc::Rc, vec::Vec};
use core::{cell::RefCell, task::Poll};
use reovim_kernel_interface::{
    Error, Outcome, Pending, ResourceId, Value,
    declaration::Operation,
    driver::Lifecycle,
    module::{ModuleScope, activation},
    provider::{ProviderInterface, ResourceOps, ResourceProvider},
};
mod output;
mod request;
use output::Output;
use request::Request;

pub(super) struct Queue {
    pub outputs: Rc<RefCell<Scope>>,
    jobs: RefCell<Vec<Rc<RefCell<Request>>>>,
    limit: usize,
}
impl Queue {
    pub fn new(
        registry: &Rc<RefCell<Registry>>,
        limits: Limits,
    ) -> Result<Self, Error> {
        // This private scope supplies owned result tokens. It has no Caller,
        // grants or executing receipts; its handle budget equals queued work.
        let output_limits =
            Limits { handles: limits.requests, requests: 1, ..limits };
        Ok(Self {
            outputs: Rc::new(RefCell::new(Scope::new(
                &[],
                registry,
                output_limits,
                true,
                false,
            )?)),
            jobs: RefCell::new(reserve(limits.requests)?),
            limit: limits.requests,
        })
    }
}
struct Provider {
    package: ResourceId,
    queue: Rc<Queue>,
}
pub(super) fn provider(
    package: ResourceId,
    queue: Rc<Queue>,
) -> ResourceProvider {
    ResourceProvider::new(
        Provider { package, queue },
        ResourceOps { open: None, namespace: None, io: None, stop: |_| Ok(()) },
        Box::new([ProviderInterface {
            prepare: None,
            declaration: activation::declaration().unwrap(),
            begin: Some(begin),
            create: None,
        }]),
        Rc::new(()),
    )
    .ok()
    .unwrap()
}
fn begin(
    provider: &mut Provider,
    _: usize,
    operation: Operation,
    _: &Value,
) -> Result<Pending, Error> {
    if operation != Operation::Exec {
        return Err(Error::UNSUPPORTED);
    }
    let mut jobs = provider.queue.jobs.borrow_mut();
    if jobs.len() == provider.queue.limit {
        return Err(Error::PRESSURE);
    }
    let output = Output::reserve(&provider.queue.outputs)?;
    let job = Rc::new(RefCell::new(Request::new(provider.package, output)));
    jobs.push(job.clone());
    Ok(request::pending(job))
}
pub(super) fn advance(runtime: &mut Runtime) -> bool {
    let queue = runtime.module_activation.clone();
    let count = queue.jobs.borrow().len();
    let mut progress = false;
    for index in 0..count {
        let job = queue.jobs.borrow()[index].clone();
        progress |= advance_job(runtime, &job);
    }
    queue.jobs.borrow_mut().retain(|job| !job.borrow().done());
    progress
}
fn advance_job(runtime: &mut Runtime, job: &Rc<RefCell<Request>>) -> bool {
    if job.borrow().done() {
        return false;
    }
    if job.borrow().probe.is_none() {
        return start(runtime, job);
    }
    let result = job.borrow().probe.as_ref().unwrap().status();
    let Poll::Ready(result) = result else { return false };
    let outcome = match result.and_then(|id| instance(runtime, id)) {
        Ok(entry) => Outcome::Ok(
            job.borrow_mut().output.as_mut().unwrap().publish(entry),
        ),
        Err(error) => Outcome::Failed(error),
    };
    request::complete(job, outcome);
    true
}
fn start(runtime: &mut Runtime, job: &Rc<RefCell<Request>>) -> bool {
    let state = job.borrow();
    let result = if state.cancelled {
        Err(Error::CANCELLED)
    } else {
        select(runtime, state.package)
            .and_then(|selected| runtime.probe_modules(selected))
    };
    drop(state);
    match result {
        Ok(probe) => job.borrow_mut().probe = Some(probe),
        Err(error) => request::complete(job, Outcome::Failed(error)),
    }
    true
}
fn select(
    runtime: &Runtime,
    id: ResourceId,
) -> Result<super::ModuleSelection, Error> {
    let registration = runtime
        .module_registrations
        .iter()
        .find(|registration| registration.identity() == id)
        .ok_or(Error::UNAVAILABLE)?;
    registration.select(runtime, ModuleScope::Global)
}
fn instance(runtime: &Runtime, id: ResourceId) -> Result<Rc<Entry>, Error> {
    if runtime.closing {
        return Err(Error::CLOSED);
    }
    match runtime.module_status(id)? {
        Lifecycle::Ready => runtime
            .registry
            .borrow()
            .entries
            .iter()
            .find(|entry| entry.identity == id)
            .cloned()
            .ok_or(Error::UNAVAILABLE),
        Lifecycle::Failed(error) => Err(error),
        _ => Err(Error::CLOSED),
    }
}
