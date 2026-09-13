use super::*;
use reovim_kernel_interface::buffer::{ModelOps, ModelProvider, State};

fn model(runtime: &mut Runtime, calls: Rc<Cell<u32>>) {
    let root = runtime.resource("").unwrap();
    let binding = runtime.bind(&[(root, Permissions::ALL)]).unwrap();
    let mut model = ModelProvider::new(
        calls,
        ModelOps {
            unload: None,
            initialize: |calls, _| {
                calls.set(calls.get() + 1);
                Box::pin(async { Ok(State::new((), 0)) })
            },
            prepare: |_, _, _| Box::pin(async { Err(Error::UNSUPPORTED) }),
            observe: |_, _, input| {
                Ok(Message::new(input.contract, &9_u64.to_le_bytes()))
            },
            inspect: None,
        },
        reovim_kernel_interface::Contract::new("source.open", 1),
        reovim_kernel_interface::Contract::new("source.bytes", 1),
        reovim_kernel_interface::Contract::new("source.splice", 1),
        Rc::new(()),
    );
    model.initial_input = Box::new([Permissions::READ]);
    runtime
        .register_bound(
            &binding,
            alloc::vec![Contribution::Model {
                path: "model".into(),
                provider: model,
            }],
        )
        .map_err(|(error, _)| error)
        .unwrap();
}

fn capture(reads: Rc<Cell<u32>>) -> ResourceProvider {
    ResourceProvider::new(
        reads,
        ResourceOps {
            unload: None,
            read: |reads, subject, input| {
                reads.set(reads.get() + 1);
                Box::pin(async move {
                    Ok(Message::reference(input.contract, subject))
                })
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
    .with_read_view(|| Ok(base()))
}

fn base() -> ResourceProvider {
    let mut provider = reader();
    provider.surfaces[0].contract =
        reovim_kernel_interface::Contract::new("resource.base", 1);
    provider.with_read_view(|| Ok(base()))
}

fn run(after_capture: bool) {
    let reads = Rc::new(Cell::new(0));
    let calls = Rc::new(Cell::new(0));
    let (mut runtime, kernel, snapshot) = setup(capture(reads.clone()));
    model(&mut runtime, calls.clone());
    let target = kernel.resolve("model").into_inner().unwrap();
    let waiting = if after_capture {
        buffer::open(&kernel, target, snapshot, &[])
    } else {
        kernel.read(
            snapshot,
            &Message::new(
                reovim_kernel_interface::Contract::new("capture", 1),
                &[],
            ),
        )
    };
    let kernel = kernel.reconnect().unwrap();
    let snapshot = kernel.resolve("factory/snapshot").into_inner().unwrap();
    let target = kernel.resolve("model").into_inner().unwrap();
    let lost = finish(&mut runtime, waiting);
    let Err(RequestError::Unknown { recovery, .. }) = &lost else {
        panic!("lost stage result");
    };
    let recovery = recovery.clone();
    let command = Command::new(fault::once(kernel));
    let active = command.reserve().unwrap();
    *active.0.model.borrow_mut() = Some((command.kernel.clone(), target));
    if after_capture {
        *active.0.source.borrow_mut() =
            Some(command.kernel.capture(snapshot, Permissions::READ).unwrap());
    }
    assert!(matches!(
        finish(&mut runtime, command.settle(&active.0, lost)),
        Err(RequestError::Unknown { .. })
    ));
    drop(active);
    assert_eq!(command.unload(), Err(Error::BUSY));
    let work = command.kernel.resolve(&recovery).into_inner().unwrap();
    let input = Message::reference(
        reovim_kernel_interface::Contract::new("command.open.resume", 1),
        work,
    );
    let mut resumed =
        finish(&mut runtime, command.resume(ResourceId(0), input.clone()))
            .unwrap();
    resumed.contract =
        reovim_kernel_interface::Contract::new("kernel.buffer.open", 1);
    let opened = buffer::descriptor(&resumed).unwrap();
    assert_eq!(opened.backing, Some(snapshot));
    assert_eq!(calls.get(), 1, "resume must not create another Buffer");
    assert_eq!(reads.get(), u32::from(!after_capture));
    assert!(
        finish(&mut runtime, command.resume(ResourceId(0), input)).is_err()
    );
    assert_eq!(calls.get(), 1);
    finish(
        &mut runtime,
        buffer::close(
            &command.kernel,
            opened.state,
            buffer::ClosePolicy::DiscardLocal,
        ),
    )
    .unwrap();
    command.unload().unwrap();
    stop(&mut runtime, &command.kernel);
    runtime.close().unwrap();
}

#[test]
fn capture_recovery_continues_opening_without_capturing_again() {
    run(false);
}

#[test]
fn initialization_recovery_returns_the_original_buffer() {
    run(true);
}
