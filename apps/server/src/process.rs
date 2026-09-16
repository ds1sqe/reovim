use reovim_host_reference::{
    native,
    process::{Termination, identity},
};
use reovim_kernel_interface::{Error, ResourceId, driver::Progress};
use reovim_kernel_runtime::{
    Binding, Grant, Limits, PublicationGrant, Runtime,
};
use reovim_server::{
    boot::{Config, Modules, Startup},
    clients::Clients,
    loading::{Admit, Loader, Target},
};
use std::{
    ffi::OsStr,
    io::{self, Write},
    net::{SocketAddr, TcpListener},
    os::unix::ffi::OsStrExt,
    path::{Path, PathBuf},
    rc::Rc,
    sync::{Arc, atomic::Ordering},
    task::{Context, Wake, Waker},
    thread,
    time::Duration,
};

type Failure = Box<dyn std::error::Error>;

pub fn run() -> Result<(), Failure> {
    let mut arguments = std::env::args().skip(1);
    let address: SocketAddr =
        arguments.next().ok_or("missing listener address")?.parse()?;
    if !address.ip().is_loopback() {
        return Err("the control service requires loopback".into());
    }
    if arguments.next().as_deref() != Some("--trusted-module-root") {
        return Err("expected --trusted-module-root DIRECTORY".into());
    }
    let root = PathBuf::from(arguments.next().ok_or("missing module root")?)
        .canonicalize()?;
    if !root.is_dir() {
        return Err("module root must be a directory".into());
    }
    let config = Config::parse(arguments)?;
    let termination = Termination::install()?;
    let limits = Limits::default();
    let mut runtime = reovim_server::assemble(identity()?, limits)?;
    let recipient = Rc::new(runtime.bind(&[])?);
    let registry = namespace(&mut runtime, b"/catalog")?;
    let admit = policy(root);
    let modules = Modules::discover(config, &admit, limits.values, 64)?;
    configured(&mut runtime, &modules, registry, &recipient, &termination)?;
    let late_mount = namespace(&mut runtime, b"/loaded")?;
    runtime.grant(&recipient, grant(late_mount, 1))?;
    runtime.grant_publication(
        &recipient,
        PublicationGrant {
            namespace: late_mount,
            scope: reovim_kernel_interface::module::ModuleScope::Global,
            access: 7,
            create_access: 0,
            retain_loans: false,
        },
    )?;
    let mut loader = Loader::new(
        Target {
            registry,
            namespace: late_mount,
            imports: Rc::new(runtime.bind(&[])?),
            recipient: recipient.clone(),
            approved: Box::new([]),
        },
        admit,
        64,
        limits.values,
    )?;
    let load = runtime
        .register(b"/load", loader.provider()?, None)
        .map_err(|(error, _)| error)?;
    runtime.grant(&recipient, grant(load, 5))?;
    let credential = identity()?;
    let mut clients = Clients::new(1)?;
    clients.admit(credential, &recipient)?;
    let listener = TcpListener::bind(address)?;
    if !listener.local_addr()?.ip().is_loopback() {
        return Err("the control service requires loopback".into());
    }
    println!("ready {} {}", listener.local_addr()?, hex(credential));
    io::stdout().flush()?;
    reovim_server::service::serve_closing(
        listener,
        &mut runtime,
        &clients,
        std::slice::from_mut(&mut loader),
        termination.requested(),
    )?;
    Ok(())
}

fn configured(
    runtime: &mut Runtime,
    modules: &Modules,
    registry: ResourceId,
    recipient: &Rc<Binding>,
    termination: &Termination,
) -> Result<(), Failure> {
    let mut packages = Vec::new();
    for source in &modules.config().sources {
        let target = Target {
            registry,
            namespace: namespace(
                runtime,
                format!("/modules/{}", source.label).as_bytes(),
            )?,
            imports: Rc::new(runtime.bind(&[])?),
            recipient: recipient.clone(),
            approved: Box::new([]),
        };
        packages.push(modules.register(&source.label, runtime, &target)?);
    }
    for label in &modules.config().start {
        let index = modules
            .config()
            .sources
            .iter()
            .position(|source| &source.label == label)
            .ok_or(Error::INVALID)?;
        let mut startup = Startup::new(recipient.caller(), packages[index])?;
        let instance = activate(runtime, &mut startup, termination)?;
        let exports = runtime.module_exports(instance)?.to_vec();
        for resource in exports {
            runtime.grant(recipient, grant(resource, 7))?;
        }
    }
    Ok(())
}

fn activate(
    runtime: &mut Runtime,
    startup: &mut Startup,
    termination: &Termination,
) -> Result<ResourceId, Failure> {
    let waker = Waker::from(Arc::new(OwnerWake(thread::current())));
    let mut cx = Context::from_waker(&waker);
    loop {
        let progress = startup_step(startup, termination, &mut cx);
        let progress = match progress {
            Ok(progress) => progress,
            Err(error) => {
                abort(runtime, startup, &mut cx)?;
                return Err(error.into());
            }
        };
        let driven = runtime.advance(&mut cx)?;
        if let Some(instance) = startup.result()? {
            return Ok(instance);
        }
        if progress == Progress::Idle && driven == Progress::Idle {
            thread::park_timeout(Duration::from_millis(100));
        }
    }
}

fn startup_step(
    startup: &mut Startup,
    termination: &Termination,
    cx: &mut Context<'_>,
) -> Result<Progress, Error> {
    if termination.requested().load(Ordering::Relaxed) {
        return Err(Error::CANCELLED);
    }
    startup.advance(cx)
}

fn abort(
    runtime: &mut Runtime,
    startup: &Startup,
    cx: &mut Context<'_>,
) -> Result<(), Error> {
    use reovim_kernel_interface::{Outcome, caller::Status};
    let Some(request) = startup.request() else {
        return close(runtime, cx);
    };
    loop {
        match request.status(cx)? {
            Status::Outcome(outcome)
                if matches!(outcome.as_ref(), Outcome::Unknown { .. }) =>
            {
                let _ = request.reconcile();
            }
            Status::Reserved | Status::Outcome(_) => break,
            Status::Pending => {
                let _ = request.cancel();
            }
        }
        runtime.advance(cx)?;
        thread::park_timeout(Duration::from_millis(100));
    }
    request.release()?;
    close(runtime, cx)
}

fn close(runtime: &mut Runtime, cx: &mut Context<'_>) -> Result<(), Error> {
    loop {
        match runtime.close() {
            Ok(()) => return Ok(()),
            Err(Error::BUSY | Error::INCOMPLETE) => {}
            Err(error) => return Err(error),
        }
        runtime.advance(cx)?;
        thread::park_timeout(Duration::from_millis(100));
    }
}

fn namespace(runtime: &mut Runtime, path: &[u8]) -> Result<ResourceId, Error> {
    runtime.register_namespace(path)
}

const fn grant(resource: ResourceId, access: u32) -> Grant {
    Grant { resource, access, create_access: 0, retain_loans: false }
}

#[allow(unsafe_code)]
fn policy(root: PathBuf) -> Admit {
    Rc::new(move |bytes| {
        let path = Path::new(OsStr::from_bytes(bytes))
            .canonicalize()
            .map_err(|_| Error::UNAVAILABLE)?;
        if !path.starts_with(&root) {
            return Err(Error::DENIED);
        }
        // SAFETY: --trusted-module-root explicitly selects trusted native code
        // obeying the ABI. Directory restriction is selection policy, not an
        // in-process sandbox; the operator controls artifacts and replacement.
        unsafe { native::load(&path) }
    })
}

fn hex(bytes: [u8; 16]) -> String {
    const DIGITS: &[u8; 16] = b"0123456789abcdef";
    bytes
        .into_iter()
        .flat_map(|byte| {
            [
                char::from(DIGITS[usize::from(byte >> 4)]),
                char::from(DIGITS[usize::from(byte & 15)]),
            ]
        })
        .collect()
}

struct OwnerWake(thread::Thread);
impl Wake for OwnerWake {
    fn wake(self: Arc<Self>) {
        self.0.unpark();
    }
}
