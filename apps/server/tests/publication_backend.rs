use reovim_host_reference::run_with;
use reovim_kernel_interface::{
    Connection, Contract, Error, Message, RequestError, ResourceId, buffer,
    driver::{DriverOps, Lifecycle, Progress},
    module::Module,
    provider::{
        Contribution, Permissions, ResourceOps, ResourceProvider, Surface,
    },
    publication::backend as protocol,
};
use reovim_kernel_runtime::{Limits, Runtime};
use std::{
    future::Future,
    rc::Rc,
    task::{Context, Waker},
};
#[path = "publication_backend/device.rs"]
mod device;
#[path = "publication_backend/model.rs"]
mod model;
#[path = "publication_backend/save.rs"]
mod save;
#[path = "publication_backend/transfer.rs"]
mod transfer;
#[path = "publication_backend/violation.rs"]
mod violation;

const fn code(error: &RequestError) -> Error {
    match error {
        RequestError::Rejected(error) | RequestError::Failed(error) => *error,
        RequestError::Unknown { cause, .. } => *cause,
    }
}

fn surface(name: &str, methods: Permissions) -> Surface {
    Surface {
        contract: Contract::new(name, 1),
        methods,
        input: Box::new([]),
        output: Box::new([]),
    }
}

fn finish<T>(runtime: &mut Runtime, future: impl Future<Output = T>) -> T {
    run_with(future, runtime, &Runtime::DRIVER).unwrap()
}

struct Fixture {
    runtime: Runtime,
    client: Connection,
    target: ResourceId,
    base: ResourceId,
    buffer: buffer::Observation,
    selected: buffer::RetainedState,
    device: Rc<device::Device>,
}

fn setup() -> Fixture {
    setup_model(false)
}

fn setup_model(backed: bool) -> Fixture {
    let mut runtime = Runtime::new(248, Limits::default()).unwrap();
    runtime.start().unwrap();
    let model = if backed {
        model::backed(&mut runtime)
    } else {
        model::register(&mut runtime)
    };
    let device = device::Device::new();
    let base = runtime
        .register("base", device::view(device.current.borrow().clone()))
        .map_err(|(error, _)| error)
        .unwrap();
    let binding = runtime.bind(&[]).unwrap();
    let module = Module::new(
        Lifecycle::Ready,
        DriverOps {
            status: |state| *state,
            advance: |state, _| {
                Ok(if *state == Lifecycle::Closed {
                    Progress::Stopped
                } else {
                    Progress::Idle
                })
            },
            close: |state| {
                *state = Lifecycle::Closed;
                Ok(())
            },
        },
        vec![Contribution::Resource {
            path: "target".into(),
            provider: device::provider(
                reovim_sdk::local(binding.connection()),
                device.clone(),
            ),
        }],
        Rc::new(()),
    );
    runtime
        .activate_bound_module(&binding, "backend", module)
        .map_err(|(error, _)| error)
        .unwrap();
    let control = runtime.resource("backend").unwrap();
    ready_backend(&mut runtime, control);
    let target = runtime.resource("target").unwrap();
    let client = reovim_sdk::local(
        runtime
            .connect(&[
                (control, Permissions::ALL),
                (target, Permissions::ALL),
                (model, Permissions::ALL),
                (base, Permissions::READ),
            ])
            .unwrap(),
    );
    let target = client.resolve("target").into_inner().unwrap();
    let base = client.resolve("base").into_inner().unwrap();
    let model = client.resolve("model").into_inner().unwrap();
    let buffer = if backed {
        let opened = buffer::descriptor(
            &finish(&mut runtime, buffer::open(&client, model, target, &[]))
                .unwrap(),
        )
        .unwrap();
        buffer::observation(
            finish(
                &mut runtime,
                client.read(
                    opened.state.buffer,
                    &Message::new(Contract::new("logical.value", 1), &[]),
                ),
            )
            .unwrap(),
        )
        .unwrap()
    } else {
        buffer::observation(
            finish(
                &mut runtime,
                client.write(
                    model,
                    &Message::new(Contract::new("logical.initial", 1), &[1]),
                ),
            )
            .unwrap(),
        )
        .unwrap()
    };
    let selected = buffer::retained(
        &finish(&mut runtime, buffer::retain(&client, buffer.state)).unwrap(),
    )
    .unwrap();
    Fixture { runtime, client, target, base, buffer, selected, device }
}

fn ready_backend(runtime: &mut Runtime, control: ResourceId) {
    let bootstrap = reovim_sdk::local(
        runtime.connect(&[(control, Permissions::ALL)]).unwrap(),
    );
    let instance = bootstrap.resolve("backend").into_inner().unwrap();
    finish(
        runtime,
        reovim_kernel_interface::module::ready(&bootstrap, instance),
    )
    .unwrap();
}

fn publish(f: &mut Fixture, base: ResourceId) -> protocol::Receipt {
    protocol::receipt(
        &finish(
            &mut f.runtime,
            protocol::publish(&f.client, f.target, f.selected.id, base, &[]),
        )
        .unwrap(),
    )
    .unwrap()
}

fn status(f: &mut Fixture, receipt: &protocol::Receipt) -> protocol::Status {
    protocol::status(
        &finish(&mut f.runtime, protocol::result(&f.client, receipt.evidence))
            .unwrap(),
    )
    .unwrap()
}

fn release(f: &mut Fixture, receipt: &protocol::Receipt) {
    finish(&mut f.runtime, protocol::release(&f.client, f.target, receipt))
        .unwrap();
}

fn stop(mut f: Fixture) {
    f.client.close(f.selected.id).into_inner().unwrap();
    finish(
        &mut f.runtime,
        buffer::close(
            &f.client,
            f.buffer.state,
            buffer::ClosePolicy::DiscardLocal,
        ),
    )
    .unwrap();
    let backend = f.client.resolve("backend").into_inner().unwrap();
    finish(
        &mut f.runtime,
        reovim_kernel_interface::module::unload(&f.client, backend),
    )
    .unwrap();
    f.runtime.close().unwrap();
}

#[test]
fn retained_model_data_and_conditional_base_survive_caller_snapshot_close() {
    let mut f = setup();
    f.device.ready.set(false);
    let waiting =
        protocol::publish(&f.client, f.target, f.selected.id, f.base, &[]);
    let mut context = Context::from_waker(Waker::noop());
    for _ in 0..64 {
        f.runtime.advance(&mut context).unwrap();
        if f.device.wake.borrow().is_some() {
            break;
        }
    }
    assert!(f.device.wake.borrow().is_some());
    f.client.close(f.selected.id).into_inner().unwrap();
    let mut edit = Message::reference(
        Contract::new("logical.set", 1),
        f.buffer.state.buffer,
    );
    edit.data = [0_u64.to_le_bytes().as_slice(), &[0]].concat().into();
    f.buffer = buffer::observation(
        finish(&mut f.runtime, f.client.write(f.buffer.state.buffer, &edit))
            .unwrap(),
    )
    .unwrap();
    assert_eq!(f.buffer.value.data.as_ref(), [0]);
    assert!(!f.device.current.borrow().active);
    finish(
        &mut f.runtime,
        buffer::close(
            &f.client,
            f.buffer.state,
            buffer::ClosePolicy::DiscardLocal,
        ),
    )
    .unwrap();
    f.device.ready.set(true);
    f.device.wake.borrow_mut().take().unwrap().wake();
    let receipt =
        protocol::receipt(&finish(&mut f.runtime, waiting).unwrap()).unwrap();
    assert_eq!(status(&mut f, &receipt), protocol::Status::Applied);
    assert!(f.device.current.borrow().active);
    let committed = f.client.read_view(receipt.evidence).unwrap();
    release(&mut f, &receipt);
    let root = committed.root().into_inner().unwrap();
    let value = finish(
        &mut f.runtime,
        committed
            .read(root, &Message::new(Contract::new("logical.value", 1), &[])),
    )
    .unwrap();
    assert_eq!(value.data.as_ref(), [1]);
    drop(committed);
    stop(f);
}

#[test]
fn unknown_evidence_reconciles_once_and_old_base_rejects_without_refresh() {
    let mut f = setup();
    f.device.uncertain.set(true);
    let base = f.base;
    let first = publish(&mut f, base);
    assert_eq!(
        status(&mut f, &first),
        protocol::Status::OutcomeUnknown(Error::UNAVAILABLE.code)
    );
    assert!(f.device.current.borrow().active);
    assert_eq!(
        f.client.read_view(first.evidence).err(),
        Some(Error::NOT_READY)
    );
    assert_eq!(
        finish(&mut f.runtime, protocol::release(&f.client, f.target, &first)),
        Err(RequestError::Failed(Error::BUSY))
    );
    let module = f.client.resolve("backend").into_inner().unwrap();
    assert_eq!(
        finish(
            &mut f.runtime,
            reovim_kernel_interface::module::unload(&f.client, module)
        ),
        Err(RequestError::Failed(Error::BUSY))
    );
    f.device.reconcile.set(false);
    assert_eq!(
        finish(&mut f.runtime, protocol::reconcile(&f.client, first.evidence)),
        Err(RequestError::Failed(Error::UNSUPPORTED))
    );
    assert_eq!(
        status(&mut f, &first),
        protocol::Status::OutcomeUnknown(Error::UNAVAILABLE.code)
    );
    f.device.reconcile.set(true);
    let applied =
        finish(&mut f.runtime, protocol::reconcile(&f.client, first.evidence))
            .unwrap();
    assert_eq!(protocol::status(&applied), Ok(protocol::Status::Applied));
    let original = f.device.current.borrow().clone();
    finish(&mut f.runtime, protocol::reconcile(&f.client, first.evidence))
        .unwrap();
    assert!(Rc::ptr_eq(&original, &f.device.current.borrow()));
    let conflict = publish(&mut f, base);
    assert_eq!(
        status(&mut f, &conflict),
        protocol::Status::DefinitelyNotApplied(Error::STALE.code)
    );
    assert!(Rc::ptr_eq(&original, &f.device.current.borrow()));
    let held = f.client.capture(first.evidence, Permissions::READ).unwrap();
    assert_eq!(
        finish(&mut f.runtime, protocol::release(&f.client, f.target, &first)),
        Err(RequestError::Failed(Error::BUSY))
    );
    drop(held);
    release(&mut f, &conflict);
    release(&mut f, &first);
    stop(f);
}

#[test]
fn lost_delivery_recovers_one_evidence_owner_and_an_independent_committed_base()
{
    let mut f = setup();
    let root = f.client.resolve("requests").into_inner().unwrap();
    let before = f.client.children(root).into_inner().unwrap();
    let waiting =
        protocol::publish(&f.client, f.target, f.selected.id, f.base, &[]);
    let after = f.client.children(root).into_inner().unwrap();
    let work = after
        .iter()
        .find(|entry| !before.iter().any(|old| old.resource == entry.resource))
        .unwrap()
        .resource;
    drop(waiting);
    let first = finish(
        &mut f.runtime,
        reovim_kernel_interface::requests::result(&f.client, work),
    )
    .unwrap()
    .unwrap();
    let second = finish(
        &mut f.runtime,
        reovim_kernel_interface::requests::result(&f.client, work),
    )
    .unwrap()
    .unwrap();
    let receipt = protocol::receipt(&first).unwrap();
    assert_eq!(protocol::receipt(&second).unwrap(), receipt);
    let original = f.device.current.borrow().clone();
    f.client.close(work).into_inner().unwrap();
    assert_eq!(status(&mut f, &receipt), protocol::Status::Applied);
    let committed = f.client.read_view(receipt.evidence).unwrap();
    *f.device.current.borrow_mut() = Rc::new(device::Version {
        generation: original.generation + 1,
        active: false,
    });
    release(&mut f, &receipt);
    let root = committed.root().into_inner().unwrap();
    let base = finish(
        &mut f.runtime,
        committed
            .read(root, &Message::new(Contract::new("resource.base", 1), &[])),
    )
    .unwrap();
    assert_eq!(base.data.as_ref(), original.generation.to_le_bytes());
    let value = finish(
        &mut f.runtime,
        committed
            .read(root, &Message::new(Contract::new("logical.value", 1), &[])),
    )
    .unwrap();
    assert_eq!(value.data.as_ref(), [1]);
    assert!(!f.device.current.borrow().active);
    drop(committed);
    stop(f);
}

#[test]
fn target_write_authority_is_checked_before_backend_input_or_effect() {
    let mut f = setup();
    let target = f.runtime.resource("target").unwrap();
    let reader = reovim_sdk::local(
        f.runtime.connect(&[(target, Permissions::READ)]).unwrap(),
    );
    let target = reader.resolve("target").into_inner().unwrap();
    assert_eq!(
        finish(
            &mut f.runtime,
            protocol::publish(&reader, target, target, target, &[])
        ),
        Err(RequestError::Rejected(Error::DENIED))
    );
    assert!(!f.device.current.borrow().active);
    drop(reader);
    stop(f);
}

#[test]
fn partial_status_preserves_atomicity_violation_and_rejects_unknown_flags() {
    let status = protocol::Status::PartiallyApplied {
        cause: Error::UNAVAILABLE.code,
        atomicity_violated: true,
    };
    let mut message =
        protocol::encode(Contract::new("resource.effect", 1), status).unwrap();
    assert_eq!(protocol::status(&message), Ok(status));
    message.data[9] = 2;
    assert_eq!(protocol::status(&message), Err(Error::INVALID));
    message.data[9] = 1;
    message.resources = Box::new([ResourceId(1)]);
    assert_eq!(protocol::status(&message), Err(Error::INVALID));
}
