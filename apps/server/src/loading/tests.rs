use super::*;
use reovim_kernel_interface::requests;
use reovim_kernel_runtime::Limits;
use std::{path::PathBuf, process::Command, task::Waker};

fn build(name: &str) -> PathBuf {
    let directory = std::env::temp_dir()
        .join(format!("reovim-loader-{name}-{}", std::process::id()));
    std::fs::create_dir_all(&directory).unwrap();
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let result = Command::new("cc")
        .args([
            "-std=c11", "-Wall", "-Wextra", "-Werror", "-shared", "-fPIC", "-I",
        ])
        .arg(root.join("host/reference/include"))
        .arg(root.join("modules/native-counter/counter.c"))
        .arg("-o")
        .arg(directory.join("counter.so"))
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    directory
}

fn finish<T>(
    runtime: &mut Runtime,
    future: impl core::future::Future<Output = T>,
) -> T {
    reovim_host_reference::run(future, |context| {
        runtime.advance(context).map(|_| ())
    })
    .unwrap()
}
fn locator(error: RequestError) -> Box<str> {
    let RequestError::Unknown { recovery, .. } = error else {
        panic!("expected retained load result")
    };
    recovery
}
fn setup() -> (Runtime, Connection, Loader) {
    let mut runtime = Runtime::new(189, Limits::default()).unwrap();
    runtime.start().unwrap();
    let root = runtime.resource("").unwrap();
    let grants = alloc::vec![(root, Permissions::ALL)];
    let owner = runtime.bind(&grants).unwrap();
    let kernel = reovim_sdk::local(owner.connection());
    let loader = Loader::new(owner, root, grants);
    runtime.register("load", loader.provider()).map_err(|(e, _)| e).unwrap();
    (runtime, kernel, loader)
}
fn submit_load(
    runtime: &mut Runtime,
    kernel: &Connection,
    loader: &Loader,
    library: &std::path::Path,
) -> Reply {
    let load = kernel.resolve("load").into_inner().unwrap();
    let data = format!("{}\0counter", library.display());
    let waiting = kernel.write(
        load,
        &Message::new(Contract::new("host.module.load", 1), data.as_bytes()),
    );
    let mut context = Context::from_waker(Waker::noop());
    for _ in 0..8 {
        runtime.advance(&mut context).unwrap();
        if loader.is_pending() {
            return waiting;
        }
    }
    panic!("load was not admitted")
}
fn release(
    runtime: &mut Runtime,
    kernel: &Connection,
    work: ResourceId,
    module: ResourceId,
) {
    kernel.close(work).into_inner().unwrap();
    finish(
        runtime,
        kernel.write(
            module,
            &Message::new(Contract::new("kernel.module.unload", 1), &[]),
        ),
    )
    .unwrap();
    runtime.close().unwrap();
}

#[test]
fn lost_readiness_reply_returns_the_installed_instance_and_evidence() {
    let directory = build("lost-ready");
    let (mut runtime, kernel, mut loader) = setup();
    let waiting = submit_load(
        &mut runtime,
        &kernel,
        &loader,
        &directory.join("counter.so"),
    );
    let mut context = Context::from_waker(Waker::noop());
    loader.advance(&mut runtime, &kernel, &mut context);
    let next = kernel.reconnect().unwrap();
    loader.advance(&mut runtime, &next, &mut context);
    let path = locator(finish(&mut runtime, waiting).unwrap_err());
    let work = next.resolve(&path).into_inner().unwrap();
    let result =
        finish(&mut runtime, requests::result(&next, work)).unwrap().unwrap();
    let data = core::str::from_utf8(&result.data).unwrap();
    let fields: Vec<_> = data.split('\t').collect();
    assert_eq!(fields[0], "readiness-unknown");
    assert_eq!(fields[1].parse::<u64>().unwrap(), Error::STALE.code.0);
    let readiness = next.resolve(fields[2]).into_inner().unwrap();
    let ready = finish(&mut runtime, requests::result(&next, readiness))
        .unwrap()
        .unwrap();
    assert_eq!(ready.data.as_ref(), b"ready");
    assert!(next.resolve("counter").into_inner().is_ok());
    next.close(readiness).into_inner().unwrap();
    release(&mut runtime, &next, work, result.resources[0]);
    std::fs::remove_dir_all(directory).unwrap();
}

#[test]
fn readiness_lookup_failure_cannot_discard_the_installed_instance() {
    let directory = build("lost-lookup");
    let (mut runtime, kernel, mut loader) = setup();
    let waiting = submit_load(
        &mut runtime,
        &kernel,
        &loader,
        &directory.join("counter.so"),
    );
    let next = kernel.reconnect().unwrap();
    let mut context = Context::from_waker(Waker::noop());
    loader.advance(&mut runtime, &kernel, &mut context);
    loader.advance(&mut runtime, &next, &mut context);
    let path = locator(finish(&mut runtime, waiting).unwrap_err());
    let work = next.resolve(&path).into_inner().unwrap();
    let result =
        finish(&mut runtime, requests::result(&next, work)).unwrap().unwrap();
    assert_eq!(result.data.as_ref(), b"readiness-error\t7");
    let module = result.resources[0];
    let ready = finish(
        &mut runtime,
        next.read(
            module,
            &Message::new(Contract::new("kernel.module.ready", 1), &[]),
        ),
    )
    .unwrap();
    assert_eq!(ready.data.as_ref(), b"ready");
    release(&mut runtime, &next, work, module);
    std::fs::remove_dir_all(directory).unwrap();
}
