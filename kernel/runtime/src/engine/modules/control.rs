use super::Runtime;
use alloc::{boxed::Box, rc::Rc, vec::Vec};
use core::cell::{Cell, RefCell};
use reovim_kernel_interface::{
    Error, Outcome, Pending, ResourceId, Value,
    declaration::Operation,
    driver::Lifecycle,
    module::control,
    provider::{ProviderInterface, ResourceOps, ResourceProvider},
};

mod request;
use request::Request;

pub(super) struct Control {
    identity: ResourceId,
    status: Rc<Cell<Lifecycle>>,
    jobs: RefCell<Vec<Rc<RefCell<Request>>>>,
    limit: usize,
    cleanup_error: Cell<Option<Error>>,
}
impl Control {
    pub fn cleanup_failed(&self, error: Error) {
        if error != Error::BUSY {
            self.cleanup_error.set(Some(error));
        }
    }
    fn status(&self) -> Outcome {
        if self.status.get() != Lifecycle::Closed
            && let Some(error) = self.cleanup_error.get()
        {
            return Outcome::Failed(error);
        }
        status(self.status.get())
    }
    pub const fn new(
        identity: ResourceId,
        status: Rc<Cell<Lifecycle>>,
        limit: usize,
    ) -> Self {
        Self {
            identity,
            status,
            jobs: RefCell::new(Vec::new()),
            limit,
            cleanup_error: Cell::new(None),
        }
    }
}

pub(super) fn provider(control: Rc<Control>) -> ResourceProvider {
    ResourceProvider::new(
        control,
        ResourceOps { open: None, namespace: None, io: None, stop: |_| Ok(()) },
        Box::new([ProviderInterface {
            prepare: None,
            declaration: control::declaration().unwrap(),
            begin: Some(begin),
            create: None,
        }]),
        Rc::new(()),
    )
    .ok()
    .unwrap()
}
fn begin(
    control: &mut Rc<Control>,
    _: usize,
    operation: Operation,
    _: &Value,
) -> Result<Pending, Error> {
    if operation == Operation::Ioctl(control::STATUS) {
        return Ok(request::pending(Request::completed(control.status())));
    }
    if operation != Operation::Ioctl(control::UNLOAD) {
        return Err(Error::UNSUPPORTED);
    }
    let mut jobs = control.jobs.borrow_mut();
    if jobs.len() == control.limit {
        return Err(Error::PRESSURE);
    }
    jobs.try_reserve(1).map_err(|_| Error::PRESSURE)?;
    let job = Rc::new(RefCell::new(Request::new()));
    jobs.push(job.clone());
    Ok(request::pending(job))
}
fn status(lifecycle: Lifecycle) -> Outcome {
    let value: u32 = match lifecycle {
        Lifecycle::Constructed => 0,
        Lifecycle::Starting => 1,
        Lifecycle::Ready => 2,
        Lifecycle::Closing => 3,
        Lifecycle::Closed => 4,
        Lifecycle::Failed(error) => return Outcome::Failed(error),
    };
    match Value::copy(&value.to_le_bytes(), &[]) {
        Ok(value) => Outcome::Ok(value),
        Err(error) => Outcome::Failed(error),
    }
}

pub(super) fn advance(runtime: &mut Runtime) -> bool {
    let mut progress = false;
    for index in 0..runtime.modules.len() {
        let control = runtime.modules[index].control.clone();
        progress |= advance_control(runtime, &control);
    }
    progress
}
fn advance_control(runtime: &mut Runtime, control: &Control) -> bool {
    let count = control.jobs.borrow().len();
    let mut progress = false;
    for index in 0..count {
        let job = control.jobs.borrow()[index].clone();
        progress |= advance_job(runtime, control.identity, &job);
    }
    control.jobs.borrow_mut().retain(|job| !job.borrow().done());
    progress
}
fn advance_job(
    runtime: &mut Runtime,
    identity: ResourceId,
    job: &Rc<RefCell<Request>>,
) -> bool {
    if job.borrow().done() {
        return false;
    }
    let state = if job.borrow().admitted {
        runtime.module_status(identity)
    } else {
        runtime.unload_module(identity)
    };
    let outcome = match state {
        Ok(Lifecycle::Closed) => Some(Outcome::Ok(Value::unit())),
        Ok(_) => None,
        Err(error) => Some(Outcome::Failed(error)),
    };
    if let Some(outcome) = outcome {
        request::complete(job, outcome);
        return true;
    }
    let mut job = job.borrow_mut();
    let admitted = !job.admitted;
    job.admitted = true;
    admitted
}
