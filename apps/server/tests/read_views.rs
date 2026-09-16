#[path = "support/memory.rs"]
mod memory;

use reovim_host_reference::run_with;
use reovim_kernel_interface::{
    Connection, Contract, Error, Message, RequestError, ResourceId,
    provider::{Permissions, ResourceOps, ResourceProvider, Surface},
};
use reovim_kernel_runtime::{Limits, Runtime};
use std::{
    cell::Cell,
    future::Future,
    rc::Rc,
    task::{Context, Waker},
};

struct Code(Rc<Cell<bool>>);
impl Drop for Code {
    fn drop(&mut self) {
        self.0.set(true);
    }
}

fn reader(code: Rc<Code>) -> ResourceProvider {
    let view_code = code.clone();
    ResourceProvider::new(
        (),
        ResourceOps {
            unload: None,
            read: |(), _, input| {
                Box::pin(async move {
                    Ok(Message::new(input.contract, b"retained data"))
                })
            },
            write: |(), _, _| Box::pin(async { Err(Error::DENIED) }),
        },
        Box::new([Surface {
            contract: Contract::new("bytes", 1),
            methods: Permissions::READ,
            input: Box::new([]),
            output: Box::new([]),
        }]),
        code,
    )
    .with_read_view(move || Ok(reader(view_code.clone())))
}

fn finish<T>(runtime: &mut Runtime, future: impl Future<Output = T>) -> T {
    run_with(future, runtime, &Runtime::DRIVER).unwrap()
}
fn read(
    runtime: &mut Runtime,
    client: &Connection,
    id: ResourceId,
) -> Result<Message, RequestError> {
    finish(
        runtime,
        client.read(id, &Message::new(Contract::new("bytes", 1), &[])),
    )
}

#[test]
fn native_view_owns_data_after_original_withdrawal_and_clones_drain() {
    let mut runtime = Runtime::new(237, Limits::default()).unwrap();
    runtime.start().unwrap();
    let released = Rc::new(Cell::new(false));
    let source = runtime
        .register("source", reader(Rc::new(Code(released.clone()))))
        .map_err(|(error, _)| error)
        .unwrap();
    let client = reovim_sdk::local(
        runtime.connect(&[(source, Permissions::READ)]).unwrap(),
    );
    let original = client.resolve("source").into_inner().unwrap();
    let view = client.read_view(original).unwrap();
    let root = view.root().into_inner().unwrap();
    assert_ne!(root, original);
    assert_eq!(
        view.capture(root, Permissions::WRITE).err(),
        Some(Error::DENIED)
    );
    assert_eq!(
        finish(
            &mut runtime,
            view.write(root, &Message::new(Contract::new("bytes", 1), &[]))
        ),
        Err(RequestError::Rejected(Error::DENIED))
    );
    runtime.unregister(source).unwrap();
    assert!(!released.get());
    assert_eq!(
        read(&mut runtime, &view, root).unwrap().data.as_ref(),
        b"retained data"
    );
    let alias = view.clone();
    drop(view);
    let pending =
        alias.read(root, &Message::new(Contract::new("bytes", 1), &[]));
    drop(alias);
    assert_eq!(
        finish(&mut runtime, pending).unwrap().data.as_ref(),
        b"retained data"
    );
    runtime.advance(&mut Context::from_waker(Waker::noop())).unwrap();
    assert!(released.get());
    runtime.close().unwrap();
}

#[test]
fn view_reconnect_retains_its_root_and_explicit_close_fences_clones() {
    let mut runtime = Runtime::new(238, Limits::default()).unwrap();
    runtime.start().unwrap();
    let source = runtime
        .register("source", reader(Rc::new(Code(Rc::new(Cell::new(false))))))
        .map_err(|(error, _)| error)
        .unwrap();
    let client = reovim_sdk::local(
        runtime.connect(&[(source, Permissions::READ)]).unwrap(),
    );
    let id = client.resolve("source").into_inner().unwrap();
    let view = client.read_view(id).unwrap();
    let root = view.root().into_inner().unwrap();
    let next = view.reconnect().unwrap();
    assert_eq!(
        read(&mut runtime, &view, root),
        Err(RequestError::Rejected(Error::STALE))
    );
    let root = next.root().into_inner().unwrap();
    let clone = next.clone();
    let captured = next.capture(root, Permissions::READ).unwrap();
    next.close(root).into_inner().unwrap();
    assert_eq!(
        read(&mut runtime, &clone, root),
        Err(RequestError::Rejected(Error::CLOSED))
    );
    assert_eq!(
        read(&mut runtime, &captured, root),
        Err(RequestError::Rejected(Error::CLOSED))
    );
    drop(captured);
    drop(clone);
    drop(next);
    drop(view);
    runtime.advance(&mut Context::from_waker(Waker::noop())).unwrap();
    runtime.unregister(source).unwrap();
    runtime.close().unwrap();
}

#[test]
fn unsupported_and_overgranted_factories_preserve_original_access() {
    let mut runtime = Runtime::new(239, Limits::default()).unwrap();
    runtime.start().unwrap();
    let source = runtime
        .register(
            "source",
            memory::provider(b"original").with_read_view(|| {
                let mut view = memory::provider(b"invalid view");
                view.surfaces[0].methods = Permissions::ALL;
                Ok(view)
            }),
        )
        .map_err(|(error, _)| error)
        .unwrap();
    let plain = runtime
        .register("plain", memory::provider(b"plain"))
        .map_err(|(error, _)| error)
        .unwrap();
    let client = reovim_sdk::local(
        runtime
            .connect(&[(source, Permissions::READ), (plain, Permissions::READ)])
            .unwrap(),
    );
    let source = client.resolve("source").into_inner().unwrap();
    let plain = client.resolve("plain").into_inner().unwrap();
    assert_eq!(client.read_view(source).err(), Some(Error::PROVIDER));
    assert_eq!(client.read_view(plain).err(), Some(Error::UNSUPPORTED));
    assert_eq!(
        finish(
            &mut runtime,
            client.read(
                source,
                &Message::new(Contract::new("resource.bytes", 1), &[])
            )
        )
        .unwrap()
        .data
        .as_ref(),
        b"original"
    );
    runtime.close().unwrap();
}

#[test]
fn released_view_roots_reclaim_capacity_without_reusing_identity() {
    let mut runtime = Runtime::new(
        240,
        Limits { resources: 16, callers: 4, ..Limits::default() },
    )
    .unwrap();
    runtime.start().unwrap();
    let source = runtime
        .register("source", reader(Rc::new(Code(Rc::new(Cell::new(false))))))
        .map_err(|(error, _)| error)
        .unwrap();
    let client = reovim_sdk::local(
        runtime.connect(&[(source, Permissions::READ)]).unwrap(),
    );
    let source = client.resolve("source").into_inner().unwrap();
    let mut previous = None;
    for _ in 0..48 {
        let view = client.read_view(source).unwrap();
        let root = view.root().into_inner().unwrap();
        assert_ne!(previous, Some(root));
        assert_eq!(
            read(&mut runtime, &view, root).unwrap().data.as_ref(),
            b"retained data"
        );
        previous = Some(root);
        drop(view);
        runtime.advance(&mut Context::from_waker(Waker::noop())).unwrap();
    }
    runtime.close().unwrap();
}

#[test]
fn view_close_waits_for_recovered_work_adoption() {
    let mut runtime = Runtime::new(241, Limits::default()).unwrap();
    runtime.start().unwrap();
    let source = runtime
        .register("source", reader(Rc::new(Code(Rc::new(Cell::new(false))))))
        .map_err(|(error, _)| error)
        .unwrap();
    let client = reovim_sdk::local(
        runtime.connect(&[(source, Permissions::READ)]).unwrap(),
    );
    let source = client.resolve("source").into_inner().unwrap();
    let view = client.read_view(source).unwrap();
    let root = view.root().into_inner().unwrap();
    let pending =
        view.read(root, &Message::new(Contract::new("bytes", 1), &[]));
    let next = view.reconnect().unwrap();
    let Err(RequestError::Unknown { recovery, .. }) =
        finish(&mut runtime, pending)
    else {
        panic!("retained view request");
    };
    let root = next.root().into_inner().unwrap();
    assert_eq!(
        next.close(root).into_inner(),
        Err(RequestError::Rejected(Error::BUSY))
    );
    let work = next.resolve(&recovery).into_inner().unwrap();
    let reply = finish(
        &mut runtime,
        reovim_kernel_interface::requests::result(&next, work),
    )
    .unwrap()
    .unwrap();
    assert_eq!(reply.data.as_ref(), b"retained data");
    next.close(work).into_inner().unwrap();
    next.close(root).into_inner().unwrap();
    drop(next);
    drop(view);
    runtime.advance(&mut Context::from_waker(Waker::noop())).unwrap();
    runtime.close().unwrap();
}

#[test]
fn denied_and_capacity_failures_do_not_leak_a_constructed_view() {
    let mut runtime =
        Runtime::new(242, Limits { callers: 2, ..Limits::default() }).unwrap();
    runtime.start().unwrap();
    let called = Rc::new(Cell::new(false));
    let released = Rc::new(Cell::new(false));
    let factory_called = called.clone();
    let factory_released = released.clone();
    let source = runtime
        .register(
            "source",
            memory::provider(b"original").with_read_view(move || {
                factory_called.set(true);
                factory_released.set(false);
                Ok(reader(Rc::new(Code(factory_released.clone()))))
            }),
        )
        .map_err(|(error, _)| error)
        .unwrap();
    let denied = reovim_sdk::local(
        runtime.connect(&[(source, Permissions::WRITE)]).unwrap(),
    );
    let id = denied.resolve("source").into_inner().unwrap();
    assert_eq!(denied.read_view(id).err(), Some(Error::DENIED));
    assert!(!called.get());
    let allowed = reovim_sdk::local(
        runtime.connect(&[(source, Permissions::READ)]).unwrap(),
    );
    let id = allowed.resolve("source").into_inner().unwrap();
    assert_eq!(allowed.read_view(id).err(), Some(Error::PRESSURE));
    assert!(called.get());
    assert!(released.get());
    drop(denied);
    let view = allowed.read_view(id).unwrap();
    assert!(!released.get());
    let root = view.root().into_inner().unwrap();
    assert_eq!(
        read(&mut runtime, &view, root).unwrap().data.as_ref(),
        b"retained data"
    );
    drop(view);
    runtime.advance(&mut Context::from_waker(Waker::noop())).unwrap();
    assert!(released.get());
    runtime.close().unwrap();
}
