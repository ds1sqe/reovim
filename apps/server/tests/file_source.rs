#![cfg(all(feature = "std", target_os = "linux"))]

use reovim_host_reference::{run, snapshot};
use reovim_kernel_interface::{
    Connection, Contract, Error, Message, RequestError, ResourceId,
    buffer::{self, ClosePolicy, ModelOps, ModelProvider, State},
    provider::{Contribution, Permissions},
};
use reovim_kernel_runtime::{Limits, Runtime};
use std::{fs, future::Future, path::PathBuf, rc::Rc};

struct Files(PathBuf);
impl Files {
    fn new(name: &str) -> Self {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../tmp")
            .join(format!("file-source-{name}-{}", std::process::id()));
        fs::create_dir_all(&root).unwrap();
        Self(root)
    }
}
impl Drop for Files {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).unwrap();
    }
}

fn finish<T>(runtime: &mut Runtime, future: impl Future<Output = T>) -> T {
    run(future, |context| runtime.advance(context).map(|_| ())).unwrap()
}

fn query(name: &str, data: &[u8]) -> Message {
    Message::new(Contract::new(name, 1), data)
}

fn window(name: &str) -> Message {
    let data = [0_u64.to_le_bytes(), 9_u64.to_le_bytes()].concat();
    query(name, &data)
}

fn prepare(runtime: &mut Runtime, client: &Connection, source: ResourceId) {
    finish(runtime, client.read(source, &query("resource.state.prepare", &[])))
        .unwrap();
}

fn bytes(
    runtime: &mut Runtime,
    view: &Connection,
    contract: &str,
) -> Box<[u8]> {
    let root = view.root().into_inner().unwrap();
    finish(runtime, view.read(root, &window(contract))).unwrap().data
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
                        .read(input.resources[0], &window("resource.snapshot"))
                        .await
                        .map_err(|_| Error::PROVIDER)?;
                    Ok(State::new(reply.data, 1))
                })
            },
            prepare: |_, _, _| Box::pin(async { Err(Error::UNSUPPORTED) }),
            observe: |_, state, input| {
                Ok(Message::new(
                    input.contract,
                    state.get::<Box<[u8]>>().unwrap(),
                ))
            },
            inspect: None,
        },
        Contract::new("file.initial", 1),
        Contract::new("file.value", 1),
        Contract::new("file.edit", 1),
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

#[test]
fn pinned_file_prepares_coherent_bases_and_independent_backed_buffers() {
    let files = Files::new("backing");
    let path = files.0.join("draft");
    fs::write(&path, b"before\xff\r\n").unwrap();
    let mut runtime = Runtime::new(244, Limits::default()).unwrap();
    runtime.start().unwrap();
    let source = runtime
        .register("file", snapshot::source(&path, &files.0).unwrap())
        .map_err(|(error, _)| error)
        .unwrap();
    let model = model(&mut runtime);
    let client = reovim_sdk::local(
        runtime
            .connect(&[
                (source, Permissions::READ),
                (model, Permissions::WRITE),
            ])
            .unwrap(),
    );
    let subject = client.resolve("file").into_inner().unwrap();
    let target = client.resolve("model").into_inner().unwrap();
    assert_eq!(client.read_view(subject).err(), Some(Error::NOT_READY));
    prepare(&mut runtime, &client, subject);
    let old = client.read_view(subject).unwrap();
    let first = buffer::descriptor(
        &finish(&mut runtime, buffer::open(&client, target, subject, &[]))
            .unwrap(),
    )
    .unwrap();
    fs::write(&path, b"after!\xfe\r\n").unwrap();
    prepare(&mut runtime, &client, subject);
    let new = client.read_view(subject).unwrap();
    let second = buffer::descriptor(
        &finish(&mut runtime, buffer::open(&client, target, subject, &[]))
            .unwrap(),
    )
    .unwrap();
    assert_eq!(first.backing, Some(subject));
    assert_eq!(second.backing, Some(subject));
    assert_ne!(first.state.buffer, second.state.buffer);
    assert_ne!(first.history, second.history);
    assert_eq!(
        bytes(&mut runtime, &old, "resource.base").as_ref(),
        b"before\xff\r\n"
    );
    assert_eq!(
        bytes(&mut runtime, &new, "resource.base").as_ref(),
        b"after!\xfe\r\n"
    );
    for (opened, expected) in
        [(first, b"before\xff\r\n"), (second, b"after!\xfe\r\n")]
    {
        let observed = finish(
            &mut runtime,
            client.read(opened.state.buffer, &query("file.value", &[])),
        )
        .unwrap();
        assert_eq!(
            buffer::observation(observed).unwrap().value.data.as_ref(),
            expected
        );
        assert_eq!(runtime.unregister(source), Err(Error::BUSY));
        finish(
            &mut runtime,
            buffer::close(&client, opened.state, ClosePolicy::RejectModified),
        )
        .unwrap();
    }
    check_identity(&mut runtime, &client, subject, &files);
    assert_eq!(
        bytes(&mut runtime, &old, "resource.snapshot").as_ref(),
        b"before\xff\r\n"
    );
    runtime.unregister(source).unwrap();
    assert_eq!(
        bytes(&mut runtime, &new, "resource.snapshot").as_ref(),
        b"after!\xfe\r\n"
    );
    drop((old, new));
    runtime.unregister(model).unwrap();
    runtime.close().unwrap();
}

fn check_identity(
    runtime: &mut Runtime,
    client: &Connection,
    subject: ResourceId,
    files: &Files,
) {
    // Source identity follows the pinned file, not a replacement pathname.
    fs::rename(files.0.join("draft"), files.0.join("moved")).unwrap();
    fs::write(files.0.join("draft"), b"new file!").unwrap();
    fs::write(files.0.join("moved"), b"pinned!!!").unwrap();
    prepare(runtime, client, subject);
    let pinned = client.read_view(subject).unwrap();
    assert_eq!(
        bytes(runtime, &pinned, "resource.snapshot").as_ref(),
        b"pinned!!!"
    );
    let bad = query("resource.state.prepare", b"invalid");
    assert_eq!(
        finish(runtime, client.read(subject, &bad)),
        Err(RequestError::Failed(Error::INVALID))
    );
    let unchanged = client.read_view(subject).unwrap();
    assert_eq!(
        bytes(runtime, &unchanged, "resource.base").as_ref(),
        b"pinned!!!"
    );
}

#[test]
fn unsupported_clone_leaves_source_unprepared() {
    let files = Files::new("unsupported");
    let path = files.0.join("draft");
    fs::write(&path, b"original").unwrap();
    let mut runtime = Runtime::new(245, Limits::default()).unwrap();
    runtime.start().unwrap();
    let source = runtime
        .register(
            "file",
            snapshot::source(&path, std::path::Path::new("/dev/shm")).unwrap(),
        )
        .map_err(|(error, _)| error)
        .unwrap();
    let client = reovim_sdk::local(
        runtime.connect(&[(source, Permissions::READ)]).unwrap(),
    );
    let subject = client.resolve("file").into_inner().unwrap();
    assert_eq!(
        finish(
            &mut runtime,
            client.read(subject, &query("resource.state.prepare", &[]))
        ),
        Err(RequestError::Failed(Error::UNSUPPORTED))
    );
    assert_eq!(client.read_view(subject).err(), Some(Error::NOT_READY));
    assert_eq!(fs::read(&path).unwrap(), b"original");
    runtime.unregister(source).unwrap();
    runtime.close().unwrap();
}
