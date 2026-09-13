use super::*;
use core::future::Future;
use reovim_kernel_interface::provider::{
    Contribution, ResourceOps, ResourceProvider, Surface,
};
use reovim_kernel_interface::{
    driver::{DriverOps, Lifecycle, Progress},
    module::Module,
};
use reovim_kernel_runtime::{Limits, Runtime};

fn reader() -> ResourceProvider {
    counted_reader(Rc::new(Cell::new(0)))
}

fn counted_reader(reads: Rc<Cell<u32>>) -> ResourceProvider {
    ResourceProvider::new(
        reads,
        ResourceOps {
            unload: None,
            read: |reads, _, input| {
                reads.set(reads.get() + 1);
                Box::pin(
                    async move { Ok(Message::new(input.contract, b"data")) },
                )
            },
            write: |_, _, _| Box::pin(async { Err(Error::UNSUPPORTED) }),
        },
        Box::new([Surface {
            contract: reovim_kernel_interface::Contract::new("bytes", 1),
            methods: Permissions::READ,
            input: Box::new([]),
            output: Box::new([]),
        }]),
        Rc::new(()),
    )
}

fn setup(provider: ResourceProvider) -> (Runtime, Connection, ResourceId) {
    let mut runtime = Runtime::new(244, Limits::default()).unwrap();
    runtime.start().unwrap();
    let root = runtime.resource("").unwrap();
    let binding = runtime.bind(&[(root, Permissions::ALL)]).unwrap();
    let kernel = reovim_sdk::local(binding.connection());
    let publisher = factory(kernel.clone(), provider);
    let module = Module::new(
        Lifecycle::Ready,
        DriverOps {
            status: |state| *state,
            advance: |state, _| Ok(progress(*state)),
            close: |state| {
                *state = Lifecycle::Closed;
                Ok(())
            },
        },
        alloc::vec![Contribution::Resource {
            path: "factory".into(),
            provider: publisher,
        }],
        Rc::new(()),
    );
    runtime
        .activate_bound_module(&binding, "module", module)
        .map_err(|(error, _)| error)
        .unwrap();
    let control = kernel.resolve("module").into_inner().unwrap();
    finish(
        &mut runtime,
        kernel.read(
            control,
            &Message::new(
                reovim_kernel_interface::Contract::new(
                    "kernel.module.ready",
                    1,
                ),
                &[],
            ),
        ),
    )
    .unwrap();
    let subject = kernel.resolve("factory").into_inner().unwrap();
    let reply = finish(
        &mut runtime,
        kernel.read(
            subject,
            &Message::new(
                reovim_kernel_interface::Contract::new("capture", 1),
                &[],
            ),
        ),
    )
    .unwrap();
    let snapshot = reply.resources[0];
    (runtime, kernel, snapshot)
}

const fn progress(state: Lifecycle) -> Progress {
    if matches!(state, Lifecycle::Closed) {
        Progress::Stopped
    } else {
        Progress::Idle
    }
}

fn factory(kernel: Connection, provider: ResourceProvider) -> ResourceProvider {
    ResourceProvider::new(
        (kernel, Some(provider)),
        ResourceOps {
            unload: None,
            read: |(kernel, provider), subject, input| {
                let result = kernel
                    .publish(subject, "snapshot", provider.take().unwrap())
                    .map(|id| Message::reference(input.contract, id))
                    .map_err(|(error, _)| error);
                Box::pin(core::future::ready(result))
            },
            write: |_, _, _| Box::pin(async { Err(Error::UNSUPPORTED) }),
        },
        Box::new([Surface {
            contract: reovim_kernel_interface::Contract::new("capture", 1),
            methods: Permissions::READ,
            input: Box::new([]),
            output: Box::new([Permissions::READ]),
        }]),
        Rc::new(()),
    )
}

fn finish<T>(runtime: &mut Runtime, future: impl Future<Output = T>) -> T {
    reovim_host_reference::run_with(future, runtime, &Runtime::DRIVER).unwrap()
}

#[test]
fn failed_close_retains_cleanup_until_retry_succeeds() {
    let ready = Rc::new(Cell::new(false));
    let closed = Rc::new(Cell::new(false));
    let can_close = ready.clone();
    let did_close = closed.clone();
    let provider = reader().try_owned(move || {
        if !can_close.get() {
            return Err(Error::BUSY);
        }
        did_close.set(true);
        Ok(())
    });
    let (mut runtime, kernel, snapshot) = setup(provider);
    let command = Command::new(kernel);
    let active = command.reserve().unwrap();
    active.0.snapshot.set(Some(snapshot));
    drop(active);
    assert_eq!(command.unload(), Err(Error::BUSY));
    assert!(!closed.get());
    assert!(
        command
            .pending
            .borrow()
            .iter()
            .any(|job| { job.snapshot.get() == Some(snapshot) })
    );
    ready.set(true);
    command.unload().unwrap();
    assert!(closed.get());
    command.unload().unwrap();
    stop(&mut runtime, &command.kernel);
    runtime.close().unwrap();
}

#[test]
fn nested_lost_reply_is_adopted_without_repeating_the_read() {
    let reads = Rc::new(Cell::new(0));
    let (mut runtime, kernel, snapshot) = setup(counted_reader(reads.clone()));
    let pending = kernel.read(
        snapshot,
        &Message::new(reovim_kernel_interface::Contract::new("bytes", 1), &[]),
    );
    let next = kernel.reconnect().unwrap();
    let lost = finish(&mut runtime, pending);
    let Err(RequestError::Unknown { recovery, .. }) = &lost else {
        panic!("lost old-connection reply");
    };
    let recovery = recovery.clone();
    let command = Command::new(next);
    let active = command.reserve().unwrap();
    let result = finish(&mut runtime, command.settle(&active.0, lost)).unwrap();
    assert_eq!(result.data.as_ref(), b"data");
    assert_eq!(reads.get(), 1, "recovery must not execute the read again");
    drop(active);
    command.unload().unwrap();
    assert!(command.kernel.resolve(&recovery).into_inner().is_err());
    stop(&mut runtime, &command.kernel);
    runtime.close().unwrap();
}

fn stop(runtime: &mut Runtime, kernel: &Connection) {
    let control = kernel.resolve("module").into_inner().unwrap();
    finish(runtime, reovim_kernel_interface::module::unload(kernel, control))
        .unwrap();
}
