use reovim_host_reference::run;
use reovim_kernel_interface::{
    Connection, Contract, Error, Message, RequestError,
    access::Method,
    buffer::integer,
    driver::{DriverOps, Lifecycle, Progress},
    module::Module,
    provider::{
        Contribution, Permissions, ResourceOps, ResourceProvider, Surface,
    },
};
use reovim_kernel_runtime::{Binding, Limits, Runtime};
use std::{cell::Cell, future::Future, rc::Rc};

fn finish<T>(runtime: &mut Runtime, future: impl Future<Output = T>) -> T {
    run(future, |context| runtime.advance(context).map(|_| ())).unwrap()
}
fn value() -> ResourceProvider {
    reovim_module_counter::provider(Rc::new(Cell::new(42)))
}
fn module(binding: &Binding, path: &str) -> Module {
    let provider = ResourceProvider::new(
        reovim_sdk::local(binding.connection()),
        ResourceOps {
            unload: None,
            read: |_, _, _| {
                Box::pin(async {
                    Err(RequestError::Failed(Error::UNSUPPORTED))
                })
            },
            write: |kernel: &mut Connection, _, input| {
                let kernel = kernel.clone();
                Box::pin(async move {
                    let path = core::str::from_utf8(&input.data)
                        .map_err(|_| Error::INVALID)?;
                    let parent = kernel.resolve(path).await?;
                    let id = kernel
                        .publish(parent, "child", value())
                        .map_err(|(e, _)| e)?;
                    Ok(Message::reference(input.contract, id))
                })
            },
        },
        Box::new([Surface {
            contract: Contract::new("publish", 1),
            methods: Permissions::WRITE,
            input: Box::new([]),
            output: Box::new([Permissions::READ]),
        }]),
        Rc::new(()),
    );
    Module::new(
        Lifecycle::Constructed,
        DriverOps {
            status: |state| *state,
            advance: |state, _| {
                if *state == Lifecycle::Closed {
                    return Ok(Progress::Stopped);
                }
                *state = Lifecycle::Ready;
                Ok(Progress::Idle)
            },
            close: |state| {
                *state = Lifecycle::Closed;
                Ok(())
            },
        },
        vec![Contribution::Resource { path: path.into(), provider }],
        Rc::new(()),
    )
}

#[test]
fn rejected_publication_returns_usable_provider_ownership_through_c_abi() {
    let mut runtime = Runtime::new(190, Limits::default()).unwrap();
    runtime.start().unwrap();
    let root = runtime.resource("").unwrap();
    let client = reovim_sdk::local(
        runtime.connect(&[(root, Permissions::ALL)]).unwrap(),
    );
    let parent = client.root().into_inner().unwrap();
    let (error, mut provider) = client
        .publish(parent, "not-published", value())
        .expect_err("unbound callers cannot publish");
    assert_eq!(error, Error::DENIED);
    let result = finish(
        &mut runtime,
        provider.call(
            Method::Read,
            parent,
            Message::new(Contract::new("external.counter", 1), &[]),
        ),
    )
    .unwrap();
    assert_eq!(integer(&result.data), Ok(42));
    assert_eq!(
        client.resolve("not-published").into_inner(),
        Err(RequestError::Rejected(Error::UNAVAILABLE))
    );
    runtime.close().unwrap();
}

#[test]
fn publication_is_limited_to_the_executing_modules_own_contributions() {
    let mut runtime = Runtime::new(191, Limits::default()).unwrap();
    runtime.start().unwrap();
    let root = runtime.resource("").unwrap();
    let owner = runtime.bind(&[(root, Permissions::ALL)]).unwrap();
    let first = runtime.bind_for(&owner, &[(root, Permissions::ALL)]).unwrap();
    let second = runtime.bind_for(&owner, &[(root, Permissions::ALL)]).unwrap();
    runtime
        .activate_bound_module(&first, "instances/a", module(&first, "a"))
        .map_err(|(e, _)| e)
        .unwrap();
    runtime
        .activate_bound_module(&second, "instances/b", module(&second, "b"))
        .map_err(|(e, _)| e)
        .unwrap();
    let client = reovim_sdk::local(owner.connection());
    for path in ["instances/a", "instances/b"] {
        let id = client.resolve(path).into_inner().unwrap();
        finish(
            &mut runtime,
            client.read(
                id,
                &Message::new(Contract::new("kernel.module.ready", 1), &[]),
            ),
        )
        .unwrap();
    }
    let endpoint = client.resolve("a").into_inner().unwrap();
    let denied = finish(
        &mut runtime,
        client
            .write(endpoint, &Message::new(Contract::new("publish", 1), b"b")),
    );
    assert_eq!(denied, Err(RequestError::Failed(Error::DENIED)));
    let created = finish(
        &mut runtime,
        client
            .write(endpoint, &Message::new(Contract::new("publish", 1), b"a")),
    )
    .unwrap();
    let child = client.resolve("a/child").into_inner().unwrap();
    assert_eq!(created.resources.as_ref(), [child]);
    let reply = finish(
        &mut runtime,
        client.read(
            child,
            &Message::new(Contract::new("external.counter", 1), &[]),
        ),
    )
    .unwrap();
    assert_eq!(integer(&reply.data), Ok(42));
    for path in ["instances/a", "instances/b"] {
        let module = client.resolve(path).into_inner().unwrap();
        finish(
            &mut runtime,
            client.write(
                module,
                &Message::new(Contract::new("kernel.module.unload", 1), &[]),
            ),
        )
        .unwrap();
    }
    runtime.close().unwrap();
}
