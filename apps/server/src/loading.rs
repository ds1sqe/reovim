//! Hosted native loading, driven outside Runtime callbacks and registry guards.

use alloc::{boxed::Box, format, rc::Rc, string::String, vec::Vec};
use core::{
    cell::RefCell,
    future::poll_fn,
    task::{Context, Poll, Waker},
};
use reovim_kernel_interface::{
    Connection, Contract, Error, Message, RequestError, ResourceId,
    access::Reply,
    driver::{DriverOps, Lifecycle, Progress},
    module::Module,
    provider::{
        Contribution, Permissions, ResourceOps, ResourceProvider, Surface,
    },
};
use reovim_kernel_runtime::{Binding, Runtime};

struct Job {
    contract: Contract,
    library: String,
    export: String,
    import: Option<reovim_kernel_interface::ResourceId>,
    result: Rc<RefCell<Completion>>,
    ready: Option<(reovim_kernel_interface::ResourceId, Reply)>,
    rejected: Option<(Error, Module)>,
}

#[derive(Default)]
struct Completion {
    result: Option<Result<Message, Error>>,
    waker: Option<Waker>,
}

pub struct Loader {
    owner: Binding,
    root: ResourceId,
    grants: Vec<(ResourceId, Permissions)>,
    pending: Rc<RefCell<Option<Job>>>,
    next: u64,
}

impl Loader {
    pub fn new(
        owner: Binding,
        root: ResourceId,
        grants: Vec<(ResourceId, Permissions)>,
    ) -> Self {
        Self { owner, root, grants, pending: Rc::default(), next: 0 }
    }

    pub(crate) const fn binding(&self) -> &Binding {
        &self.owner
    }

    pub fn is_pending(&self) -> bool {
        self.pending.borrow().is_some()
    }

    pub fn provider(&self) -> ResourceProvider {
        ResourceProvider::new(
            Rc::clone(&self.pending),
            ResourceOps {
                unload: None,
                read: |_, _, _| {
                    Box::pin(core::future::ready(Err(Error::UNSUPPORTED)))
                },
                write: |pending, _, input| submit(pending, &input),
            },
            Box::new([
                Surface {
                    contract: Contract::new("host.module.load", 1),
                    methods: Permissions::WRITE,
                    input: Box::new([]),
                    output: Box::new([Permissions::READ]),
                },
                Surface {
                    contract: Contract::new("host.module.load-with", 1),
                    methods: Permissions::WRITE,
                    input: Box::new([Permissions::ALL]),
                    output: Box::new([Permissions::READ]),
                },
            ]),
            Rc::new(()),
        )
    }

    pub fn advance(
        &mut self,
        runtime: &mut Runtime,
        kernel: &Connection,
        wake: &mut Context<'_>,
    ) {
        let Some(mut job) = self.pending.borrow_mut().take() else {
            return;
        };
        let result = step(self, runtime, kernel, &mut job, wake);
        let Some(result) = result else {
            *self.pending.borrow_mut() = Some(job);
            return;
        };
        let mut completion = job.result.borrow_mut();
        completion.result = Some(result);
        let waker = completion.waker.take();
        drop(completion);
        if let Some(waker) = waker {
            waker.wake();
        }
    }

    fn start(
        &mut self,
        runtime: &mut Runtime,
        kernel: &Connection,
        job: &mut Job,
    ) -> Result<(reovim_kernel_interface::ResourceId, Reply), Error> {
        self.next = self.next.checked_add(1).ok_or(Error::PRESSURE)?;
        let grants = if job.import == Some(self.root) {
            self.grants.clone()
        } else {
            job.import.into_iter().map(|id| (id, Permissions::ALL)).collect()
        };
        let binding = runtime.bind_for(&self.owner, &grants)?;
        let imports = binding.connection();
        let export = self.owner.publication_path(&job.export)?;
        let contributions = load(&job.library, &export, Some(&imports))?;
        let module = module(contributions);
        if let Err(error) = validate_exports(&self.owner, &module) {
            job.rejected = Some((error, module));
            return Err(error);
        }
        let local_path = format!("instances/native-{}", self.next);
        let path = self.owner.publication_path(&local_path)?;
        let id = match runtime.activate_bound_module(&binding, &path, module) {
            Ok(id) => id,
            Err((error, module)) => {
                job.rejected = Some((error, module));
                return Err(error);
            }
        };
        let ready = match kernel.resolve(&local_path).into_inner() {
            Ok(local) => kernel.read(
                local,
                &Message::new(Contract::new("kernel.module.ready", 1), &[]),
            ),
            Err(error) => Box::pin(core::future::ready(Err(error))),
        };
        Ok((id, ready))
    }
}

fn module(contributions: Vec<Contribution>) -> Module {
    Module::new(
        Lifecycle::Constructed,
        DriverOps {
            status: |state| *state,
            advance: |state, _| {
                if *state == Lifecycle::Closed {
                    return Ok(Progress::Stopped);
                }
                *state = Lifecycle::Ready;
                Ok(Progress::Idle)
            },
            close: |state| {
                *state = Lifecycle::Closed;
                Ok(())
            },
        },
        contributions,
        Rc::new(()),
    )
}

fn step(
    loader: &mut Loader,
    runtime: &mut Runtime,
    kernel: &Connection,
    job: &mut Job,
    wake: &mut Context<'_>,
) -> Option<Result<Message, Error>> {
    if job.rejected.is_some() {
        return cleanup(job);
    }
    if let Some((id, ready)) = job.ready.as_mut() {
        return match ready.as_mut().poll(wake) {
            Poll::Pending => None,
            Poll::Ready(reply) => {
                Some(Ok(installed(&job.contract, *id, reply)))
            }
        };
    }
    match loader.start(runtime, kernel, job) {
        Ok(ready) => {
            job.ready = Some(ready);
            None
        }
        Err(_) if job.rejected.is_some() => None,
        Err(error) => Some(Err(error)),
    }
}

fn cleanup(job: &mut Job) -> Option<Result<Message, Error>> {
    let (error, module) = job.rejected.as_mut()?;
    for contribution in &mut module.contributions {
        let result = match contribution {
            Contribution::Resource { provider, .. } => provider.unload(),
            Contribution::Model { provider, .. } => provider.unload(),
        };
        if result.is_err() {
            return None;
        }
    }
    let error = *error;
    job.rejected = None;
    Some(Err(error))
}

fn submit(
    pending: &Rc<RefCell<Option<Job>>>,
    input: &Message,
) -> reovim_kernel_interface::provider::ProviderReply {
    let parsed = core::str::from_utf8(&input.data)
        .ok()
        .and_then(|value| value.split_once('\0'));
    let Some((library, export)) = parsed else {
        return Box::pin(core::future::ready(Err(Error::INVALID)));
    };
    if library.is_empty() || export.contains('\0') {
        return Box::pin(core::future::ready(Err(Error::INVALID)));
    }
    if pending.borrow().is_some() {
        return Box::pin(core::future::ready(Err(Error::BUSY)));
    }
    let result = Rc::new(RefCell::new(Completion::default()));
    *pending.borrow_mut() = Some(Job {
        contract: input.contract.clone(),
        library: library.into(),
        export: export.into(),
        import: input.resources.first().copied(),
        result: Rc::clone(&result),
        ready: None,
        rejected: None,
    });
    Box::pin(poll_fn(move |context| {
        let mut completion = result.borrow_mut();
        if let Some(result) = completion.result.take() {
            return Poll::Ready(result);
        }
        completion.waker = Some(context.waker().clone());
        Poll::Pending
    }))
}

fn installed(
    contract: &Contract,
    id: ResourceId,
    ready: Result<Message, RequestError>,
) -> Message {
    let data = match ready {
        Ok(_) => "ready".into(),
        Err(RequestError::Rejected(error) | RequestError::Failed(error)) => {
            format!("readiness-error\t{}", error.code.0)
        }
        Err(RequestError::Unknown { cause, recovery }) => {
            format!("readiness-unknown\t{}\t{recovery}", cause.code.0)
        }
    };
    let mut reply = Message::reference(contract.clone(), id);
    reply.data = data.into_bytes().into();
    reply
}

#[cfg(target_os = "linux")]
#[allow(unsafe_code)]
fn load(
    path: &str,
    export: &str,
    kernel: Option<&Connection>,
) -> Result<Vec<Contribution>, Error> {
    // This local control scope admits code obeying the current native ABI.
    // Loading is host execution, outside the Kernel's provider callbacks.
    unsafe {
        reovim_host_reference::native::load_contributions(
            std::path::Path::new(path),
            export,
            kernel,
        )
    }
}

#[cfg(not(target_os = "linux"))]
fn load(
    _: &str,
    _: &str,
    _: Option<&Connection>,
) -> Result<Vec<Contribution>, Error> {
    Err(Error::UNSUPPORTED)
}

#[cfg(all(test, target_os = "linux"))]
mod tests;

fn validate_exports(owner: &Binding, module: &Module) -> Result<(), Error> {
    let root = owner.publication_path("")?;
    for contribution in &module.contributions {
        let path = contribution.path();
        let relative = if root.is_empty() {
            path
        } else {
            path.strip_prefix(&*root)
                .and_then(|path| path.strip_prefix('/'))
                .ok_or(Error::DENIED)?
        };
        owner.publication_path(relative)?;
    }
    Ok(())
}
