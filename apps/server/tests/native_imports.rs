#![cfg(all(feature = "std", target_os = "linux"))]
use reovim_host_reference::{native::load_resource_with, run};
use reovim_kernel_interface::{
    Contract, Error, Message, buffer::integer, provider::Permissions,
};
use reovim_kernel_runtime::{Limits, Runtime};
use std::{
    cell::Cell,
    path::{Path, PathBuf},
    process::Command,
    rc::Rc,
    task::{Context, Waker},
};

fn build(name: &str) -> (PathBuf, PathBuf) {
    build_as(name, name)
}

fn build_as(name: &str, label: &str) -> (PathBuf, PathBuf) {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let directory = std::env::temp_dir()
        .join(format!("reovim-native-{label}-{}", std::process::id()));
    std::fs::create_dir_all(&directory).unwrap();
    let library = directory.join("forward.so");
    let output = Command::new("cc")
        .args(["-std=c11", "-Wall", "-Wextra", "-Werror", "-shared", "-fPIC"])
        .arg("-I")
        .arg(root.join("host/reference/include"))
        .arg(root.join(if name == "forward" {
            "modules/native-forward/forward.c".into()
        } else {
            format!("apps/server/tests/native/{name}.c")
        }))
        .arg("-o")
        .arg(&library)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    (directory, library)
}
#[test]
#[allow(unsafe_code)]
fn native_module_calls_granted_kernel_api_and_keeps_pending_work_owned() {
    let mut runtime = Runtime::new(109, Limits::default()).unwrap();
    runtime.start().unwrap();
    let value = Rc::new(Cell::new(0));
    let counter = runtime
        .register(
            "modules/counter/value",
            reovim_module_counter::provider(Rc::clone(&value)),
        )
        .map_err(|(error, _)| error)
        .unwrap();
    let imports = runtime.connect(&[(counter, Permissions::ALL)]).unwrap();
    let (directory, library) = build("forward");
    // The fixture retains its granted ABI connection and drains pending calls.
    let module = unsafe { load_resource_with(&library, &imports) }.unwrap();
    let forward = runtime
        .register("forward", module)
        .map_err(|(error, _)| error)
        .unwrap();
    let client = reovim_sdk::local(
        runtime.connect(&[(forward, Permissions::ALL)]).unwrap(),
    );
    let subject = client.resolve("forward").into_inner().unwrap();
    let request =
        Message::new(Contract::new("native.forward", 1), &13_u64.to_le_bytes());
    run(client.write(subject, &request), |context| {
        runtime.advance(context).map(|_| ())
    })
    .unwrap()
    .unwrap();
    // Native write submitted a separate Kernel request.
    assert_eq!(value.get(), 0);
    assert_eq!(runtime.unregister(forward), Err(Error::BUSY));
    let mut context = Context::from_waker(Waker::noop());
    runtime.advance(&mut context).unwrap();
    runtime.advance(&mut context).unwrap();
    let reply = run(
        client.read(
            subject,
            &Message::new(Contract::new("native.forward", 1), &[]),
        ),
        |context| runtime.advance(context).map(|_| ()),
    )
    .unwrap()
    .unwrap();
    assert_eq!(integer(&reply.data), Ok(13));
    assert_eq!(value.get(), 13);
    // A second pending call is drained by shutdown's unload retry.
    run(client.write(subject, &request), |context| {
        runtime.advance(context).map(|_| ())
    })
    .unwrap()
    .unwrap();
    assert_eq!(runtime.close(), Err(Error::BUSY));
    runtime.advance(&mut context).unwrap();
    runtime.advance(&mut context).unwrap();
    runtime.close().unwrap();
    assert_eq!(value.get(), 26);
    std::fs::remove_dir_all(directory).unwrap();
}

#[test]
#[allow(unsafe_code)]
fn incompatible_native_table_is_rejected_before_context_creation() {
    let (directory, library) = build("incompatible");
    // The invalid version's factory aborts if validation calls it.
    let result =
        unsafe { reovim_host_reference::native::load_resource(&library) };
    let Err(error) = result else {
        panic!("incompatible ABI must reject");
    };
    assert_eq!(error, Error::UNSUPPORTED);
    std::fs::remove_dir_all(directory).unwrap();
}

#[test]
#[allow(unsafe_code)]
fn reconnect_keeps_native_wakes_alive_until_child_completion() {
    let mut runtime = Runtime::new(110, Limits::default()).unwrap();
    runtime.start().unwrap();
    let value = Rc::new(Cell::new(0));
    let counter = runtime
        .register(
            "modules/counter/value",
            reovim_module_counter::provider(Rc::clone(&value)),
        )
        .map_err(|(error, _)| error)
        .unwrap();
    let imports = runtime.connect(&[(counter, Permissions::ALL)]).unwrap();
    let (directory, library) = build_as("forward", "forward-reconnect");
    // The fixture retains imports and reports outstanding native wake owners.
    let module = unsafe { load_resource_with(&library, &imports) }.unwrap();
    let forward = runtime
        .register("forward", module)
        .map_err(|(error, _)| error)
        .unwrap();
    let client = runtime.connect(&[(forward, Permissions::ALL)]).unwrap();
    let subject = client.resolve("forward").into_inner().unwrap();
    let request =
        Message::new(Contract::new("native.forward", 1), &7_u64.to_le_bytes());
    run(client.write(subject, &request), |context| {
        runtime.advance(context).map(|_| ())
    })
    .unwrap()
    .unwrap();
    assert_eq!(runtime.unregister(forward), Err(Error::BUSY));
    let recovered = imports.reconnect().unwrap();
    // The old wait is fenced, but its wake still points into native code.
    assert_eq!(runtime.unregister(forward), Err(Error::BUSY));
    assert_eq!(value.get(), 0);
    let mut context = Context::from_waker(Waker::noop());
    runtime.advance(&mut context).unwrap();
    runtime.advance(&mut context).unwrap();
    assert_eq!(value.get(), 7);
    runtime.unregister(forward).unwrap();
    assert!(recovered.resolve("modules/counter/value").into_inner().is_ok());
    let requests = recovered.resolve("requests").into_inner().unwrap();
    let retained = recovered.children(requests).into_inner().unwrap();
    let work = retained.first().expect("child result remains recoverable");
    let result = run(
        reovim_kernel_interface::requests::result(&recovered, work.resource),
        |context| runtime.advance(context).map(|_| ()),
    )
    .unwrap()
    .unwrap()
    .unwrap();
    recovered.close(work.resource).into_inner().unwrap();
    assert_eq!(result.contract, Contract::new("external.counter", 1));
    assert_eq!(integer(&result.data), Ok(7));
    assert_eq!(value.get(), 7); // Recovery does not replay the effect.
    runtime.close().unwrap();
    std::fs::remove_dir_all(directory).unwrap();
}
