use reovim_host_reference::run_with;
use reovim_kernel_interface::{
    Connection, Contract, Error, Message, RequestError, ResourceId,
    provider::{Permissions, ResourceOps, ResourceProvider, Surface},
};
use reovim_kernel_runtime::{Limits, Runtime};
use std::{
    cell::{Cell, RefCell},
    future::{Future, poll_fn},
    rc::Rc,
    task::{Context, Poll, Waker},
};
#[path = "source_base/model.rs"]
mod model;

struct Version {
    base: &'static [u8],
    active: bool,
}
struct Source {
    current: RefCell<Rc<Version>>,
    prepared: RefCell<Option<Rc<Version>>>,
    ready: Cell<bool>,
    wake: RefCell<Option<Waker>>,
}
struct Code(Rc<Cell<bool>>);
impl Drop for Code {
    fn drop(&mut self) {
        self.0.set(true);
    }
}

fn surface(name: &str) -> Surface {
    Surface {
        contract: Contract::new(name, 1),
        methods: Permissions::READ,
        input: Box::new([]),
        output: Box::new([]),
    }
}

fn source(state: Rc<Source>, released: Rc<Cell<bool>>) -> ResourceProvider {
    let views = state.clone();
    ResourceProvider::new(
        state,
        ResourceOps {
            unload: None,
            read: |state, _, input| {
                if input.contract.name.as_ref() == "logical.state" {
                    return Box::pin(core::future::ready(value(
                        &state.current.borrow(),
                        input,
                    )));
                }
                let state = state.clone();
                Box::pin(poll_fn(move |context| {
                    prepare(&state, &input, context)
                }))
            },
            write: |_, _, _| Box::pin(async { Err(Error::UNSUPPORTED) }),
        },
        Box::new([surface("resource.state.prepare"), surface("logical.state")]),
        Rc::new(Code(released)),
    )
    .with_read_view(move || {
        let version =
            views.prepared.borrow().clone().ok_or(Error::NOT_READY)?;
        Ok(view(version))
    })
}

fn prepare(
    source: &Source,
    input: &Message,
    context: &Context<'_>,
) -> Poll<Result<Message, Error>> {
    if !input.data.is_empty() {
        return Poll::Ready(Err(Error::INVALID));
    }
    if !source.ready.get() {
        *source.wake.borrow_mut() = Some(context.waker().clone());
        return Poll::Pending;
    }
    *source.prepared.borrow_mut() = Some(source.current.borrow().clone());
    Poll::Ready(Ok(Message::new(input.contract.clone(), &[])))
}

fn view(version: Rc<Version>) -> ResourceProvider {
    let again = version.clone();
    ResourceProvider::new(
        version,
        ResourceOps {
            unload: None,
            read: |version, _, input| {
                let result = value(version, input);
                Box::pin(core::future::ready(result))
            },
            write: |_, _, _| Box::pin(async { Err(Error::UNSUPPORTED) }),
        },
        Box::new([surface("logical.state"), surface("resource.base")]),
        Rc::new(()),
    )
    .with_read_view(move || Ok(view(again.clone())))
}

fn value(version: &Version, input: Message) -> Result<Message, Error> {
    if !input.data.is_empty() {
        return Err(Error::INVALID);
    }
    let data = match input.contract.name.as_ref() {
        "resource.base" => version.base,
        "logical.state" if version.active => b"active",
        "logical.state" => b"inactive",
        _ => return Err(Error::UNSUPPORTED),
    };
    Ok(Message::new(input.contract, data))
}

fn finish<T>(runtime: &mut Runtime, future: impl Future<Output = T>) -> T {
    run_with(future, runtime, &Runtime::DRIVER).unwrap()
}

fn read(runtime: &mut Runtime, kernel: &Connection, name: &str) -> Message {
    let root = kernel.root().into_inner().unwrap();
    finish(
        runtime,
        kernel.read(root, &Message::new(Contract::new(name, 1), &[])),
    )
    .unwrap()
}

fn prepare_version(
    runtime: &mut Runtime,
    kernel: &Connection,
    subject: ResourceId,
    source: &Source,
    version: Version,
) {
    source.ready.set(false);
    let mut waiting = kernel.read(
        subject,
        &Message::new(Contract::new("resource.state.prepare", 1), &[]),
    );
    let mut context = Context::from_waker(Waker::noop());
    assert!(waiting.as_mut().poll(&mut context).is_pending());
    for _ in 0..4 {
        if source.wake.borrow().is_some() {
            break;
        }
        runtime.advance(&mut context).unwrap();
    }
    assert!(waiting.as_mut().poll(&mut context).is_pending());
    *source.current.borrow_mut() = Rc::new(version);
    source.ready.set(true);
    source.wake.borrow_mut().take().unwrap().wake();
    assert_eq!(finish(runtime, waiting).unwrap().data.as_ref(), b"");
}

#[test]
fn prepared_data_and_base_share_one_version_under_independent_source_leases() {
    let mut runtime = Runtime::new(243, Limits::default()).unwrap();
    runtime.start().unwrap();
    let state = Rc::new(Source {
        current: RefCell::new(Rc::new(Version {
            base: b"initial",
            active: false,
        })),
        prepared: RefCell::new(None),
        ready: Cell::new(false),
        wake: RefCell::new(None),
    });
    let released = Rc::new(Cell::new(false));
    let id = runtime
        .register("source", source(state.clone(), released.clone()))
        .map_err(|(error, _)| error)
        .unwrap();
    let client =
        reovim_sdk::local(runtime.connect(&[(id, Permissions::READ)]).unwrap());
    let subject = client.resolve("source").into_inner().unwrap();
    let first = client.capture(subject, Permissions::READ).unwrap();
    let second = client.capture(subject, Permissions::READ).unwrap();
    assert_eq!(first.read_view(subject).err(), Some(Error::NOT_READY));
    prepare_version(
        &mut runtime,
        &first,
        subject,
        &state,
        Version { base: b"first", active: true },
    );
    let old = first.read_view(subject).unwrap();
    assert_eq!(
        read(&mut runtime, &old, "resource.base").data.as_ref(),
        b"first"
    );
    prepare_version(
        &mut runtime,
        &second,
        subject,
        &state,
        Version { base: b"second", active: false },
    );
    let new = second.read_view(subject).unwrap();
    assert_eq!(
        read(&mut runtime, &old, "logical.state").data.as_ref(),
        b"active"
    );
    assert_eq!(
        read(&mut runtime, &new, "resource.base").data.as_ref(),
        b"second"
    );
    assert_eq!(
        read(&mut runtime, &new, "logical.state").data.as_ref(),
        b"inactive"
    );
    let foreign = old.root().into_inner().unwrap();
    assert_eq!(
        finish(
            &mut runtime,
            client.read(
                foreign,
                &Message::new(Contract::new("logical.state", 1), &[])
            )
        ),
        Err(RequestError::Rejected(Error::DENIED))
    );
    drop(first);
    assert_eq!(runtime.unregister(id), Err(Error::BUSY));
    drop(second);
    runtime.unregister(id).unwrap();
    assert!(released.get());
    assert_eq!(
        read(&mut runtime, &old, "resource.base").data.as_ref(),
        b"first"
    );
    assert_eq!(
        read(&mut runtime, &new, "logical.state").data.as_ref(),
        b"inactive"
    );
    drop(old);
    drop(new);
    runtime.advance(&mut Context::from_waker(Waker::noop())).unwrap();
    runtime.close().unwrap();
}
