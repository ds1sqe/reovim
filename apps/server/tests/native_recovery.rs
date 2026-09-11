#![cfg(target_os = "linux")]

use reovim_host_reference::run;
use reovim_kernel_interface::{
    Connection, Contract, Error, Message, RequestError,
    buffer::integer,
    provider::{Permissions, ResourceOps, ResourceProvider, Surface},
    requests,
};
use reovim_kernel_runtime::{Limits, Runtime};
use std::{
    cell::Cell,
    future::Future,
    path::PathBuf,
    rc::Rc,
    task::{Context, Waker},
};

fn finish<T>(runtime: &mut Runtime, future: impl Future<Output = T>) -> T {
    run(future, |context| runtime.advance(context).map(|_| ())).unwrap()
}
fn locator(error: RequestError) -> Box<str> {
    let RequestError::Unknown { recovery, .. } = error else {
        panic!("expected lost-reply evidence, got {error:?}")
    };
    recovery
}
fn build() -> PathBuf {
    let target = std::env::temp_dir()
        .join(format!("reovim-native-recovery-{}", std::process::id()));
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../product/Cargo.toml");
    let output = std::process::Command::new("cargo")
        .args(["build", "--offline", "--locked", "--manifest-path"])
        .arg(manifest)
        .args([
            "-p",
            "reovim-module-write",
            "--example",
            "write",
            "--target-dir",
        ])
        .arg(&target)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    target
}
fn source(value: Rc<Cell<u64>>) -> ResourceProvider {
    ResourceProvider::new(
        value,
        ResourceOps {
            unload: None,
            read: |_, _, _| Box::pin(async { Err(Error::UNSUPPORTED) }),
            write: |value, _, input| {
                let next = value.get() + integer(&input.data).unwrap();
                value.set(next);
                Box::pin(core::future::ready(Ok(Message::new(
                    input.contract,
                    &next.to_le_bytes(),
                ))))
            },
        },
        Box::new([Surface {
            contract: Contract::new("sum", 1),
            methods: Permissions::WRITE,
            input: Box::new([]),
            output: Box::new([]),
        }]),
        Rc::new(()),
    )
}
fn admitted(runtime: &mut Runtime, imports: &Connection) -> String {
    let root = imports.resolve("requests").into_inner().unwrap();
    let mut context = Context::from_waker(Waker::noop());
    for _ in 0..8 {
        runtime.advance(&mut context).unwrap();
        let work = imports.children(root).into_inner().unwrap();
        if let Some(work) = work.first() {
            return format!("requests/{}", work.name);
        }
    }
    panic!("loaded command did not admit its inner request")
}

#[test]
#[allow(unsafe_code)]
fn loaded_write_preserves_lost_reply_and_recovers_without_repeating_effect() {
    let target = build();
    let mut runtime = Runtime::new(187, Limits::default()).unwrap();
    runtime.start().unwrap();
    let value = Rc::new(Cell::new(0));
    let source = runtime
        .register("source", source(value.clone()))
        .map_err(|(error, _)| error)
        .unwrap();
    let root = runtime.resource("").unwrap();
    let owner = runtime.bind(&[(root, Permissions::ALL)]).unwrap();
    let binding =
        runtime.bind_for(&owner, &[(source, Permissions::WRITE)]).unwrap();
    let imports = binding.connection();
    // This is the actual built-in artifact, using the declared C module table.
    let providers = unsafe {
        reovim_host_reference::native::load_contributions(
            &target.join("debug/examples/libwrite.so"),
            "command",
            Some(&imports),
        )
    }
    .unwrap();
    let ids = runtime
        .register_bound(&binding, providers)
        .map_err(|(error, _)| error)
        .unwrap();
    let client = reovim_sdk::local(owner.connection());
    let command = client.resolve("command").into_inner().unwrap();
    let waiting = client.write(
        command,
        &Message::new(
            Contract::new("command.write", 1),
            b"/source\0--contract\0sum\x001\0--hex\x000700000000000000",
        ),
    );
    let inner_path = admitted(&mut runtime, &imports);
    assert_eq!(value.get(), 0);
    let _resumed = imports.reconnect().unwrap();
    let outer_path = locator(finish(&mut runtime, waiting).unwrap_err());
    assert_ne!(&*outer_path, inner_path);
    let outer = client.resolve(&outer_path).into_inner().unwrap();
    let original = finish(&mut runtime, requests::result(&client, outer))
        .unwrap()
        .unwrap_err();
    assert_eq!(&*locator(original), inner_path);
    let inner = client.resolve(&inner_path).into_inner().unwrap();
    let first = finish(&mut runtime, requests::result(&client, inner))
        .unwrap()
        .unwrap();
    let second = finish(&mut runtime, requests::result(&client, inner))
        .unwrap()
        .unwrap();
    assert_eq!(first, second);
    assert_eq!(integer(&first.data), Ok(7));
    assert_eq!(
        client.close(inner).into_inner(),
        Err(RequestError::Rejected(Error::BUSY))
    );
    assert_eq!(runtime.unregister_contributions(&ids), Err(Error::BUSY));
    client.close(outer).into_inner().unwrap();
    client.close(inner).into_inner().unwrap();
    runtime.unregister_contributions(&ids).unwrap();
    assert_eq!(value.get(), 7);
    runtime.close().unwrap();
    std::fs::remove_dir_all(target).unwrap();
}
