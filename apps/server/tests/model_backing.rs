#![cfg(all(feature = "std", target_os = "linux"))]

use reovim_host_reference::{native, run};
use reovim_kernel_interface::{
    Contract, Error, Message, RequestError, ResourceId,
    buffer::{self, ClosePolicy},
    provider::{Permissions, ResourceOps, ResourceProvider, Surface},
};
use reovim_kernel_runtime::{Limits, Runtime};
use std::task::{Context, Poll, Waker};
use std::{
    cell::Cell,
    future::{Future, poll_fn},
    path::Path,
    rc::Rc,
};

fn finish<T>(runtime: &mut Runtime, future: impl Future<Output = T>) -> T {
    run(future, |context| runtime.advance(context).map(|_| ())).unwrap()
}

fn backing(gate: Rc<Cell<bool>>, failed: Rc<Cell<bool>>) -> ResourceProvider {
    ResourceProvider::new(
        (gate, failed),
        ResourceOps {
            unload: None,
            read: |(gate, failed), _, input| {
                let (gate, failed) = (gate.clone(), failed.clone());
                Box::pin(poll_fn(move |_| read(&gate, &failed, &input)))
            },
            write: |_, _, _| Box::pin(async { Err(Error::DENIED) }),
        },
        Box::new([Surface {
            contract: Contract::new("resource.bytes", 1),
            methods: Permissions::READ,
            input: Box::new([]),
            output: Box::new([]),
        }]),
        Rc::new(()),
    )
}

fn read(
    gate: &Cell<bool>,
    failed: &Cell<bool>,
    input: &Message,
) -> Poll<Result<Message, Error>> {
    if !gate.get() {
        return Poll::Pending;
    }
    if failed.get() {
        return Poll::Ready(Err(Error::UNAVAILABLE));
    }
    Poll::Ready(Ok(Message::new(input.contract.clone(), &[73])))
}

#[test]
#[allow(unsafe_code)]
fn native_model_receives_scoped_backing_and_retains_granted_reads() {
    let directory = build();
    let mut runtime = Runtime::new(209, Limits::default()).unwrap();
    runtime.start().unwrap();
    let gate = Rc::new(Cell::new(false));
    let failed = Rc::new(Cell::new(false));
    let source = runtime
        .register("source", backing(gate.clone(), failed.clone()))
        .map_err(|(error, _)| error)
        .unwrap();
    let binding = runtime.bind(&[]).unwrap();
    let kernel = binding.connection();
    let contribution = unsafe {
        native::load_with(&directory.join("backed.so"), "model", Some(&kernel))
    }
    .unwrap();
    let model = runtime
        .register_bound(&binding, vec![contribution])
        .map_err(|(error, _)| error)
        .unwrap()[0];
    denied(&mut runtime, source, model);
    cancelled(&mut runtime, source, model, &gate);
    assert!(kernel.resolve("source").into_inner().is_err());
    gate.set(false);
    let client = reovim_sdk::local(
        runtime
            .connect(&[
                (source, Permissions::READ),
                (model, Permissions::WRITE),
            ])
            .unwrap(),
    );
    let source_id = client.resolve("source").into_inner().unwrap();
    let model_id = client.resolve("model").into_inner().unwrap();
    let request =
        Message::reference(Contract::new("backed.initial", 1), source_id);
    let mut opening = client.write(model_id, &request);
    let mut context = Context::from_waker(Waker::noop());
    for _ in 0..8 {
        runtime.advance(&mut context).unwrap();
    }
    assert!(opening.as_mut().poll(&mut context).is_pending());
    assert!(client.resolve("buffers").into_inner().is_err());
    assert_eq!(runtime.unregister(model), Err(Error::BUSY));
    assert_eq!(runtime.unregister(source), Err(Error::BUSY));
    gate.set(true);
    let observed =
        buffer::observation(finish(&mut runtime, opening).unwrap()).unwrap();
    assert_eq!(observed.value.data.as_ref(), &[73]);
    let received = kernel.resolve("source").into_inner().unwrap();
    assert_ne!(received, source_id);
    let held = buffer::retained(
        &finish(&mut runtime, buffer::retain(&client, observed.state)).unwrap(),
    )
    .unwrap();
    finish(
        &mut runtime,
        buffer::close(&client, observed.state, ClosePolicy::DiscardLocal),
    )
    .unwrap();
    assert_eq!(runtime.unregister(source), Err(Error::BUSY));
    let value = finish(
        &mut runtime,
        client.read(
            held.id,
            &Message::new(Contract::new("backed.value", 1), &[]),
        ),
    )
    .unwrap();
    assert_eq!(value.data.as_ref(), &[73]);
    client.close(held.id).into_inner().unwrap();
    assert!(kernel.resolve("source").into_inner().is_err());
    failed.set(true);
    let buffers = client.resolve("buffers").into_inner().unwrap();
    let before = client.children(buffers).into_inner().unwrap();
    assert_eq!(
        finish(&mut runtime, client.write(model_id, &request)),
        Err(RequestError::Failed(Error::UNAVAILABLE))
    );
    assert_eq!(client.children(buffers).into_inner().unwrap(), before);
    assert!(kernel.resolve("source").into_inner().is_err());
    runtime.unregister(model).unwrap();
    runtime.unregister(source).unwrap();
    runtime.close().unwrap();
    std::fs::remove_dir_all(directory).unwrap();
}

fn denied(runtime: &mut Runtime, source: ResourceId, model: ResourceId) {
    let client = runtime
        .connect(&[(source, Permissions::WRITE), (model, Permissions::WRITE)])
        .unwrap();
    let source = client.resolve("source").into_inner().unwrap();
    let model = client.resolve("model").into_inner().unwrap();
    let request =
        Message::reference(Contract::new("backed.initial", 1), source);
    assert_eq!(
        finish(runtime, client.write(model, &request)),
        Err(RequestError::Rejected(Error::DENIED))
    );
    assert_eq!(
        finish(
            runtime,
            client.write(
                model,
                &Message::new(Contract::new("backed.initial", 1), &[])
            )
        ),
        Err(RequestError::Rejected(Error::INVALID))
    );
}

fn cancelled(
    runtime: &mut Runtime,
    source: ResourceId,
    model: ResourceId,
    gate: &Cell<bool>,
) {
    let client = runtime
        .connect(&[(source, Permissions::READ), (model, Permissions::WRITE)])
        .unwrap();
    let source = client.resolve("source").into_inner().unwrap();
    let model = client.resolve("model").into_inner().unwrap();
    let input = Message::reference(Contract::new("backed.initial", 1), source);
    let waiting = client.write(model, &input);
    let mut context = Context::from_waker(Waker::noop());
    for _ in 0..8 {
        runtime.advance(&mut context).unwrap();
    }
    let next = client.reconnect().unwrap();
    let RequestError::Unknown { recovery, .. } =
        finish(runtime, waiting).unwrap_err()
    else {
        panic!("recovery");
    };
    let work = next.resolve(&recovery).into_inner().unwrap();
    finish(
        runtime,
        next.write(
            work,
            &Message::new(Contract::new("kernel.request.cancel", 1), &[]),
        ),
    )
    .unwrap();
    gate.set(true);
    assert_eq!(
        finish(runtime, reovim_kernel_interface::requests::result(&next, work)),
        Ok(Err(RequestError::Failed(Error::CANCELLED)))
    );
    next.close(work).into_inner().unwrap();
    assert!(next.resolve("buffers").into_inner().is_err());
}

fn build() -> std::path::PathBuf {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let directory = std::env::temp_dir()
        .join(format!("reovim-model-backing-{}", std::process::id()));
    std::fs::create_dir_all(&directory).unwrap();
    let output = std::process::Command::new("cc")
        .args(["-std=c11", "-Wall", "-Wextra", "-Werror", "-shared", "-fPIC"])
        .arg("-I")
        .arg(root.join("host/reference/include"))
        .arg(root.join("apps/server/tests/native/backed.c"))
        .arg("-o")
        .arg(directory.join("backed.so"))
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    directory
}
