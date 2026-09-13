use super::*;
use reovim_kernel_interface::{
    buffer::{self, ModelOps, ModelProvider, State},
    provider::Contribution,
};

fn validate_initial(input: &Message) -> Result<(), Error> {
    if input.data.as_ref() == b"reject" {
        return Err(Error::INVALID);
    }
    Ok(())
}

fn model(runtime: &mut Runtime) -> ResourceId {
    let binding = runtime.bind(&[]).unwrap();
    let mut provider = ModelProvider::new(
        reovim_sdk::local(binding.connection()),
        ModelOps {
            unload: None,
            initialize: |kernel, input| {
                let kernel = kernel.clone();
                Box::pin(async move {
                    let reply = kernel
                        .read(
                            input.resources[0],
                            &Message::new(
                                Contract::new("logical.state", 1),
                                &[],
                            ),
                        )
                        .await
                        .map_err(|_| Error::PROVIDER)?;
                    validate_initial(&input)?;
                    Ok(State::new(reply.data.as_ref() == b"active", 1))
                })
            },
            prepare: |_, _, input| {
                Box::pin(async move {
                    Ok(State::new(input.data.as_ref() == b"active", 1))
                })
            },
            observe: |_, state, input| {
                Ok(Message::new(
                    input.contract,
                    if *state.get::<bool>().unwrap() {
                        b"active"
                    } else {
                        b"inactive"
                    },
                ))
            },
            inspect: None,
        },
        Contract::new("logical.initial", 1),
        Contract::new("logical.value", 1),
        Contract::new("logical.set", 1),
        Rc::new(()),
    );
    provider.initial_input = Box::new([Permissions::READ]);
    runtime
        .register_bound(
            &binding,
            vec![Contribution::Model { path: "model".into(), provider }],
        )
        .map_err(|(error, _)| error)
        .unwrap()[0]
}

struct Fixture {
    runtime: Runtime,
    kernel: Connection,
    source: ResourceId,
    model: ResourceId,
    subject: ResourceId,
    target: ResourceId,
    state: Rc<Source>,
    released: Rc<Cell<bool>>,
}

fn setup() -> Fixture {
    let mut runtime = Runtime::new(244, Limits::default()).unwrap();
    runtime.start().unwrap();
    let state = Rc::new(Source {
        current: RefCell::new(Rc::new(Version {
            base: b"initial",
            active: true,
        })),
        prepared: RefCell::new(None),
        ready: Cell::new(false),
        wake: RefCell::new(None),
    });
    let released = Rc::new(Cell::new(false));
    let source = runtime
        .register("source", super::source(state.clone(), released.clone()))
        .map_err(|(error, _)| error)
        .unwrap();
    let model = model(&mut runtime);
    let kernel = reovim_sdk::local(
        runtime
            .connect(&[
                (source, Permissions::READ),
                (model, Permissions::WRITE),
            ])
            .unwrap(),
    );
    let subject = kernel.resolve("source").into_inner().unwrap();
    let target = kernel.resolve("model").into_inner().unwrap();
    prepare_version(
        &mut runtime,
        &kernel,
        subject,
        &state,
        Version { base: b"first", active: true },
    );
    Fixture { runtime, kernel, source, model, subject, target, state, released }
}

fn open(f: &mut Fixture) -> buffer::BufferDescriptor {
    buffer::descriptor(
        &finish(
            &mut f.runtime,
            buffer::open(&f.kernel, f.target, f.subject, &[]),
        )
        .unwrap(),
    )
    .unwrap()
}

fn describe(f: &mut Fixture, buffer: ResourceId) -> buffer::BufferDescriptor {
    buffer::descriptor(
        &finish(&mut f.runtime, buffer::describe(&f.kernel, buffer)).unwrap(),
    )
    .unwrap()
}

#[test]
fn association_is_installed_with_state_and_preserves_independent_buffers() {
    let mut f = setup();
    let first = open(&mut f);
    assert_eq!(
        current(&mut f, first.state.buffer).value.data.as_ref(),
        b"active"
    );
    assert_eq!(describe(&mut f, first.state.buffer).backing, Some(f.subject));
    prepare_version(
        &mut f.runtime,
        &f.kernel,
        f.subject,
        &f.state,
        Version { base: b"second", active: false },
    );
    let second = open(&mut f);
    assert_ne!(first.state.buffer, second.state.buffer);
    assert_ne!(first.history, second.history);
    assert_eq!(
        current(&mut f, second.state.buffer).value.data.as_ref(),
        b"inactive"
    );
    assert_eq!(describe(&mut f, second.state.buffer).backing, Some(f.subject));
    let old = buffer::retained(
        &finish(&mut f.runtime, buffer::retain(&f.kernel, first.state))
            .unwrap(),
    )
    .unwrap();
    let mut update = first.state.revision.to_le_bytes().to_vec();
    update.extend_from_slice(b"inactive");
    let mut input =
        Message::reference(Contract::new("logical.set", 1), first.state.buffer);
    input.data = update.into();
    let changed = buffer::observation(
        finish(&mut f.runtime, f.kernel.write(first.state.buffer, &input))
            .unwrap(),
    )
    .unwrap();
    assert_eq!(
        finish(
            &mut f.runtime,
            buffer::close(
                &f.kernel,
                changed.state,
                buffer::ClosePolicy::RejectModified
            )
        ),
        Err(RequestError::Failed(Error::BUSY))
    );
    let mut undo = Message::reference(
        Contract::new("kernel.history.undo", 1),
        first.state.buffer,
    );
    undo.data = changed.state.revision.to_le_bytes().into();
    finish(&mut f.runtime, f.kernel.write(first.history, &undo)).unwrap();
    let restored = describe(&mut f, first.state.buffer);
    finish(
        &mut f.runtime,
        buffer::close(
            &f.kernel,
            restored.state,
            buffer::ClosePolicy::RejectModified,
        ),
    )
    .unwrap();
    assert_eq!(f.runtime.unregister(f.source), Err(Error::BUSY));
    finish(
        &mut f.runtime,
        buffer::close(
            &f.kernel,
            second.state,
            buffer::ClosePolicy::RejectModified,
        ),
    )
    .unwrap();
    f.runtime.unregister(f.source).unwrap();
    assert!(f.released.get());
    let value = finish(
        &mut f.runtime,
        f.kernel.read(
            old.id,
            &Message::new(Contract::new("logical.value", 1), &[]),
        ),
    )
    .unwrap();
    assert_eq!(value.data.as_ref(), b"active");
    f.kernel.close(old.id).into_inner().unwrap();
    f.runtime.unregister(f.model).unwrap();
    f.runtime.close().unwrap();
}

fn current(f: &mut Fixture, id: ResourceId) -> buffer::Observation {
    buffer::observation(
        finish(
            &mut f.runtime,
            f.kernel.read(
                id,
                &Message::new(Contract::new("logical.value", 1), &[]),
            ),
        )
        .unwrap(),
    )
    .unwrap()
}

#[test]
fn rejected_initialization_leaves_no_buffer_or_source_lease() {
    let mut f = setup();
    assert_eq!(
        finish(
            &mut f.runtime,
            buffer::open(&f.kernel, f.target, f.subject, b"reject")
        ),
        Err(RequestError::Failed(Error::INVALID))
    );
    no_buffers(&mut f);
    f.runtime.unregister(f.source).unwrap();
    assert!(f.released.get());
    f.runtime.unregister(f.model).unwrap();
    f.runtime.close().unwrap();
}

fn no_buffers(f: &mut Fixture) {
    let root = f.runtime.resource("buffers").unwrap();
    let observer = f.runtime.connect(&[(root, Permissions::READ)]).unwrap();
    let root = observer.resolve("buffers").into_inner().unwrap();
    assert_eq!(observer.children(root).into_inner().unwrap(), vec![]);
}

#[test]
fn backing_identity_in_a_descriptor_grants_no_source_operations() {
    let mut f = setup();
    let opened = open(&mut f);
    let directory = f.kernel.resolve("buffers").into_inner().unwrap();
    let name = f
        .kernel
        .children(directory)
        .into_inner()
        .unwrap()
        .into_iter()
        .find(|entry| entry.resource == opened.state.buffer)
        .unwrap()
        .name;
    let path = format!("buffers/{name}");
    let subject = f.runtime.resource(&path).unwrap();
    let observer = reovim_sdk::local(
        f.runtime.connect(&[(subject, Permissions::READ)]).unwrap(),
    );
    let buffer = observer.resolve(&path).into_inner().unwrap();
    let descriptor = buffer::descriptor(
        &finish(&mut f.runtime, buffer::describe(&observer, buffer)).unwrap(),
    )
    .unwrap();
    let source = descriptor.backing.expect("backing metadata");
    assert_eq!(
        finish(
            &mut f.runtime,
            observer.read(
                source,
                &Message::new(Contract::new("logical.state", 1), &[])
            )
        ),
        Err(RequestError::Rejected(Error::DENIED))
    );
    finish(
        &mut f.runtime,
        buffer::close(
            &f.kernel,
            opened.state,
            buffer::ClosePolicy::RejectModified,
        ),
    )
    .unwrap();
    f.runtime.unregister(f.source).unwrap();
    f.runtime.unregister(f.model).unwrap();
    f.runtime.close().unwrap();
}

#[test]
fn cancelled_opening_releases_the_prepared_association() {
    let mut f = setup();
    let waiting = buffer::open(&f.kernel, f.target, f.subject, &[]);
    f.runtime.advance(&mut Context::from_waker(Waker::noop())).unwrap();
    let root = f.kernel.resolve("requests").into_inner().unwrap();
    let work = f
        .kernel
        .children(root)
        .into_inner()
        .unwrap()
        .into_iter()
        .next()
        .expect("opening work")
        .resource;
    finish(
        &mut f.runtime,
        f.kernel.write(
            work,
            &Message::new(Contract::new("kernel.request.cancel", 1), &[]),
        ),
    )
    .unwrap();
    assert_eq!(
        finish(&mut f.runtime, waiting),
        Err(RequestError::Failed(Error::CANCELLED))
    );
    no_buffers(&mut f);
    assert_eq!(f.runtime.unregister(f.source), Err(Error::BUSY));
    f.kernel.close(work).into_inner().unwrap();
    f.runtime.unregister(f.source).unwrap();
    f.runtime.unregister(f.model).unwrap();
    f.runtime.close().unwrap();
}

#[test]
fn ordinary_creation_remains_unbound() {
    let mut f = setup();
    let created = buffer::observation(
        finish(
            &mut f.runtime,
            f.kernel.write(
                f.target,
                &Message::reference(
                    Contract::new("logical.initial", 1),
                    f.subject,
                ),
            ),
        )
        .unwrap(),
    )
    .unwrap();
    assert_eq!(describe(&mut f, created.state.buffer).backing, None);
    assert_eq!(
        finish(
            &mut f.runtime,
            buffer::close(
                &f.kernel,
                created.state,
                buffer::ClosePolicy::RejectModified
            )
        ),
        Err(RequestError::Failed(Error::BUSY))
    );
    finish(
        &mut f.runtime,
        buffer::close(
            &f.kernel,
            created.state,
            buffer::ClosePolicy::DiscardLocal,
        ),
    )
    .unwrap();
    f.runtime.unregister(f.source).unwrap();
    f.runtime.unregister(f.model).unwrap();
    f.runtime.close().unwrap();
}

#[test]
fn a_data_snapshot_without_a_base_cannot_become_a_backing() {
    let mut f = setup();
    let snapshot = f
        .runtime
        .register(
            "input",
            reovim_resource_local::memory(b"input")
                .with_read_view(|| Ok(reovim_resource_local::memory(b"input"))),
        )
        .map_err(|(error, _)| error)
        .unwrap();
    let client = reovim_sdk::local(
        f.runtime
            .connect(&[
                (snapshot, Permissions::READ),
                (f.model, Permissions::WRITE),
            ])
            .unwrap(),
    );
    let target = client.resolve("model").into_inner().unwrap();
    let input = client.resolve("input").into_inner().unwrap();
    assert_eq!(
        finish(&mut f.runtime, buffer::open(&client, target, input, &[])),
        Err(RequestError::Failed(Error::UNSUPPORTED))
    );
    no_buffers(&mut f);
    f.runtime.unregister(snapshot).unwrap();
    f.runtime.unregister(f.source).unwrap();
    f.runtime.unregister(f.model).unwrap();
    f.runtime.close().unwrap();
}
