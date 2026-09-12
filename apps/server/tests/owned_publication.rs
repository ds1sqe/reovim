use reovim_host_reference::run;
use reovim_kernel_interface::{
    Connection, Contract, Error, Message, RequestError, ResourceId,
    driver::{DriverOps, Lifecycle, Progress},
    module::Module,
    provider::{
        Contribution, Permissions, ResourceOps, ResourceProvider, Surface,
    },
};
use reovim_kernel_runtime::{Binding, Limits, Runtime};
use std::{
    cell::{Cell, RefCell},
    future::{Future, poll_fn},
    rc::Rc,
    task::{Poll, Waker},
};

#[derive(Default)]
struct Reader {
    closed: bool,
    started: bool,
    wake: Option<Waker>,
    cleanup_ready: Option<Rc<Cell<bool>>>,
    cleaned: bool,
    error_resource: Option<ResourceId>,
}
fn subscription(reader: Rc<RefCell<Reader>>) -> ResourceProvider {
    let closing = reader.clone();
    ResourceProvider::new(
        reader,
        ResourceOps {
            unload: None,
            read: |reader, _, _| {
                let reader = reader.clone();
                Box::pin(poll_fn(move |context| poll_reader(&reader, context)))
            },
            write: |_, _, _| Box::pin(async { Err(Error::UNSUPPORTED) }),
        },
        Box::new([surface("next", &[])]),
        Rc::new(()),
    )
    .try_owned(move || {
        let (wake, ready) = {
            let mut reader = closing.borrow_mut();
            assert!(!reader.cleaned, "successful cleanup is invoked once");
            reader.closed = true;
            let ready =
                reader.cleanup_ready.as_ref().is_none_or(|ready| ready.get());
            reader.cleaned = ready;
            (reader.wake.take(), ready)
        };
        if let Some(wake) = wake {
            wake.wake();
        }
        if ready {
            Ok(())
        } else {
            Err(Error { id: closing.borrow().error_resource, ..Error::BUSY })
        }
    })
}
fn poll_reader(
    reader: &Rc<RefCell<Reader>>,
    context: &std::task::Context<'_>,
) -> Poll<Result<Message, Error>> {
    let mut reader = reader.borrow_mut();
    reader.started = true;
    if reader.closed {
        return Poll::Ready(Err(Error::CLOSED));
    }
    reader.wake = Some(context.waker().clone());
    Poll::Pending
}

fn surface(name: &str, output: &[Permissions]) -> Surface {
    Surface {
        contract: Contract::new(name, 1),
        methods: Permissions::READ,
        input: Box::new([]),
        output: output.into(),
    }
}
fn module(binding: &Binding, readers: Vec<Rc<RefCell<Reader>>>) -> Module {
    let provider = ResourceProvider::new(
        (reovim_sdk::local(binding.connection()), readers.into_iter()),
        ResourceOps {
            unload: None,
            read: |(kernel, readers), _, input| {
                let result = publish(kernel, readers.next().unwrap(), input);
                Box::pin(std::future::ready(result))
            },
            write: |_, _, _| {
                Box::pin(async {
                    Err(RequestError::Failed(Error::UNSUPPORTED))
                })
            },
        },
        Box::new([surface("observe", &[Permissions::READ])]),
        Rc::new(()),
    );
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
        vec![Contribution::Resource { path: "source".into(), provider }],
        Rc::new(()),
    )
}
fn publish(
    kernel: &Connection,
    reader: Rc<RefCell<Reader>>,
    input: Message,
) -> Result<Message, RequestError> {
    let parent = kernel.resolve("source").into_inner()?;
    reader.borrow_mut().error_resource = Some(parent);
    let name = std::str::from_utf8(&input.data).map_err(|_| Error::INVALID)?;
    let id = kernel
        .publish(parent, name, subscription(reader))
        .map_err(|(e, _)| e)?;
    Ok(Message::reference(input.contract, id))
}
fn finish<T>(runtime: &mut Runtime, future: impl Future<Output = T>) -> T {
    run(future, |context| runtime.advance(context).map(|_| ())).unwrap()
}
fn input(name: &str) -> Message {
    Message::new(Contract::new(name, 1), &[])
}
fn unload(
    runtime: &mut Runtime,
    client: &Connection,
    instance: ResourceId,
) -> Result<Message, RequestError> {
    finish(runtime, client.write(instance, &input("kernel.module.unload")))
}

#[test]
fn read_only_caller_owns_independent_native_subscriptions_until_close() {
    let mut runtime = Runtime::new(192, Limits::default()).unwrap();
    runtime.start().unwrap();
    let root = runtime.resource("").unwrap();
    let assembly = runtime.bind(&[(root, Permissions::ALL)]).unwrap();
    let provider =
        runtime.bind_for(&assembly, &[(root, Permissions::ALL)]).unwrap();
    let first = Rc::new(RefCell::new(Reader::default()));
    let second = Rc::new(RefCell::new(Reader::default()));
    runtime
        .activate_bound_module(
            &provider,
            "instance",
            module(&provider, vec![first.clone(), second.clone()]),
        )
        .map_err(|(e, _)| e)
        .unwrap();
    let admin = assembly.connection();
    let instance = admin.resolve("instance").into_inner().unwrap();
    finish(&mut runtime, admin.read(instance, &input("kernel.module.ready")))
        .unwrap();
    // Distinct lifetime owner from the provider, and no source write grant.
    let client = reovim_sdk::local(
        runtime.connect(&[(root, Permissions::READ)]).unwrap(),
    );
    let source = client.resolve("source").into_inner().unwrap();
    let create = |name: &[u8]| {
        client.read(source, &Message::new(Contract::new("observe", 1), name))
    };
    let one = finish(&mut runtime, create(b"one")).unwrap().resources[0];
    let two = finish(&mut runtime, create(b"two")).unwrap().resources[0];
    assert_eq!(
        unload(&mut runtime, &admin, instance),
        Err(RequestError::Failed(Error::BUSY))
    );
    let foreign = admin.resolve("source/one").into_inner().unwrap();
    assert_eq!(
        admin.close(foreign).into_inner(),
        Err(RequestError::Rejected(Error::DENIED))
    );
    let pending = client.read(one, &input("next"));
    finish(
        &mut runtime,
        poll_fn(|_| {
            if first.borrow().started { Poll::Ready(()) } else { Poll::Pending }
        }),
    );
    client.close(one).into_inner().unwrap();
    client.close(one).into_inner().unwrap();
    assert!(!second.borrow().closed);
    assert_eq!(
        finish(&mut runtime, client.read(one, &input("next"))),
        Err(RequestError::Rejected(Error::CLOSED))
    );
    assert_eq!(
        finish(&mut runtime, pending),
        Err(RequestError::Failed(Error::CLOSED))
    );
    assert_eq!(
        unload(&mut runtime, &admin, instance),
        Err(RequestError::Failed(Error::BUSY))
    );
    client.close(two).into_inner().unwrap();
    client.close(two).into_inner().unwrap();
    unload(&mut runtime, &admin, instance).unwrap();
    runtime.close().unwrap();
}

#[test]
fn assembly_closes_only_the_selected_clients_owned_access() {
    let mut runtime = Runtime::new(193, Limits::default()).unwrap();
    runtime.start().unwrap();
    let root = runtime.resource("").unwrap();
    let assembly = runtime.bind(&[(root, Permissions::ALL)]).unwrap();
    let provider =
        runtime.bind_for(&assembly, &[(root, Permissions::ALL)]).unwrap();
    let first = Rc::new(RefCell::new(Reader::default()));
    let second = Rc::new(RefCell::new(Reader::default()));
    let third = Rc::new(RefCell::new(Reader::default()));
    runtime
        .activate_bound_module(
            &provider,
            "instance",
            module(&provider, vec![first.clone(), second.clone(), third]),
        )
        .map_err(|(e, _)| e)
        .unwrap();
    let admin = assembly.connection();
    let instance = admin.resolve("instance").into_inner().unwrap();
    finish(&mut runtime, admin.read(instance, &input("kernel.module.ready")))
        .unwrap();
    let first_owner = runtime.bind(&[(root, Permissions::READ)]).unwrap();
    let second_owner = runtime.bind(&[(root, Permissions::READ)]).unwrap();
    let first_client = reovim_sdk::local(first_owner.connection());
    let second_client = reovim_sdk::local(second_owner.connection());
    let one = observe(&mut runtime, &first_client, b"one");
    let two = observe(&mut runtime, &second_client, b"two");
    let pending = first_client.read(one, &input("next"));
    finish(
        &mut runtime,
        poll_fn(|_| {
            if first.borrow().started { Poll::Ready(()) } else { Poll::Pending }
        }),
    );
    runtime.close_owned_access(&first_owner).unwrap();
    runtime.close_owned_access(&first_owner).unwrap();
    assert!(first.borrow().closed);
    assert!(!second.borrow().closed);
    assert_eq!(
        finish(&mut runtime, pending),
        Err(RequestError::Failed(Error::CLOSED))
    );
    // The common provider and the peer's connection still execute new work.
    let three = observe(&mut runtime, &second_client, b"three");
    assert_ne!(two, three);
    assert_eq!(
        unload(&mut runtime, &admin, instance),
        Err(RequestError::Failed(Error::BUSY))
    );
    runtime.close_owned_access(&second_owner).unwrap();
    assert!(second.borrow().closed);
    unload(&mut runtime, &admin, instance).unwrap();
    runtime.close().unwrap();
}

fn observe(
    runtime: &mut Runtime,
    client: &Connection,
    name: &[u8],
) -> ResourceId {
    let source = client.resolve("source").into_inner().unwrap();
    finish(
        runtime,
        client.read(source, &Message::new(Contract::new("observe", 1), name)),
    )
    .unwrap()
    .resources[0]
}

#[test]
fn failed_native_cleanup_stays_fenced_owned_and_retryable() {
    let mut runtime = Runtime::new(204, Limits::default()).unwrap();
    runtime.start().unwrap();
    let root = runtime.resource("").unwrap();
    let owner = runtime.bind(&[(root, Permissions::ALL)]).unwrap();
    let provider =
        runtime.bind_for(&owner, &[(root, Permissions::ALL)]).unwrap();
    let ready = Rc::new(Cell::new(false));
    let reader = Rc::new(RefCell::new(Reader {
        cleanup_ready: Some(ready.clone()),
        ..Reader::default()
    }));
    runtime
        .activate_bound_module(
            &provider,
            "instance",
            module(&provider, vec![reader.clone()]),
        )
        .map_err(|(e, _)| e)
        .unwrap();
    let client = reovim_sdk::local(owner.connection());
    let instance = client.resolve("instance").into_inner().unwrap();
    finish(&mut runtime, client.read(instance, &input("kernel.module.ready")))
        .unwrap();
    let access = observe(&mut runtime, &client, b"blocked");
    let source = client.resolve("source").into_inner().unwrap();
    assert_eq!(
        client.close(access).into_inner(),
        Err(RequestError::Rejected(Error { id: Some(source), ..Error::BUSY }))
    );
    assert!(reader.borrow().closed);
    assert!(!reader.borrow().cleaned);
    assert_eq!(
        finish(&mut runtime, client.read(access, &input("next"))),
        Err(RequestError::Rejected(Error::CLOSED))
    );
    assert_eq!(
        unload(&mut runtime, &client, instance),
        Err(RequestError::Failed(Error::BUSY))
    );
    assert_eq!(
        runtime.close_owned_access(&owner),
        Err(Error {
            id: Some(runtime.resource("source").unwrap()),
            ..Error::BUSY
        })
    );
    assert!(!reader.borrow().cleaned);
    ready.set(true);
    client.close(access).into_inner().unwrap();
    client.close(access).into_inner().unwrap();
    assert!(reader.borrow().cleaned);
    runtime.close_owned_access(&owner).unwrap();
    unload(&mut runtime, &client, instance).unwrap();
    runtime.close().unwrap();
}
