use reovim_host_reference::run;
use reovim_kernel_interface::{
    Connection, Contract, Error, Message, RequestError,
    buffer::{self, ClosePolicy},
    provider::{
        Contribution, Permissions, ResourceOps, ResourceProvider, Surface,
    },
    session,
};
use reovim_kernel_runtime::{Binding, Limits, Runtime};
use std::{future::Future, rc::Rc};

fn finish<T>(runtime: &mut Runtime, future: impl Future<Output = T>) -> T {
    run(future, |context| runtime.advance(context).map(|_| ())).unwrap()
}

fn relay(runtime: &mut Runtime, binding: &Binding) {
    let provider = ResourceProvider::new(
        reovim_sdk::local(binding.connection()),
        ResourceOps {
            unload: None,
            read: |_, _, _| Box::pin(async { Err(Error::UNSUPPORTED) }),
            write: |kernel: &mut Connection, _, input| {
                let kernel = kernel.clone();
                Box::pin(async move {
                    let model = kernel.resolve("model").await.unwrap();
                    Ok(kernel.write(model, &input).await.unwrap())
                })
            },
        },
        ["heading.initial", "kernel.session.create"]
            .into_iter()
            .map(|name| Surface {
                contract: Contract::new(name, 1),
                methods: Permissions::WRITE,
                input: Box::new([]),
                output: Box::new([]),
            })
            .collect(),
        Rc::new(()),
    )
    .forwarding();
    runtime
        .register_bound(
            binding,
            vec![Contribution::Resource { path: "relay".into(), provider }],
        )
        .map_err(|(error, _)| error)
        .unwrap();
}

fn setup() -> (Runtime, Connection) {
    let mut runtime = Runtime::new(183, Limits::default()).unwrap();
    runtime.start().unwrap();
    let model = runtime
        .register_model("model", reovim_representation_basic::heading())
        .map_err(|(error, _)| error)
        .unwrap();
    let root = runtime.resource("").unwrap();
    let owner = runtime.bind(&[(root, Permissions::ALL)]).unwrap();
    let module =
        runtime.bind_for(&owner, &[(model, Permissions::ALL)]).unwrap();
    relay(&mut runtime, &module);
    let client = reovim_sdk::local(owner.connection());
    (runtime, client)
}

#[test]
fn forwarded_creation_belongs_to_client_across_private_module_namespaces() {
    let (mut runtime, client) = setup();
    let root = runtime.resource("").unwrap();
    let subject = client.resolve("relay").into_inner().unwrap();
    let opened = finish(
        &mut runtime,
        client.write(
            subject,
            &Message::new(Contract::new("heading.initial", 1), b"# Owned\n"),
        ),
    )
    .unwrap();
    let observed = buffer::observation(opened).unwrap();
    assert_eq!(observed.value.data.as_ref(), b"# Owned\n");
    let buffers = client.resolve("buffers").into_inner().unwrap();
    let name = client
        .children(buffers)
        .into_inner()
        .unwrap()
        .into_iter()
        .find(|entry| entry.resource == observed.state.buffer)
        .unwrap()
        .name;
    let stranger = reovim_sdk::local(
        runtime.connect(&[(root, Permissions::ALL)]).unwrap(),
    );
    let foreign =
        stranger.resolve(&format!("buffers/{name}")).into_inner().unwrap();
    let mut foreign_state = observed.state;
    foreign_state.buffer = foreign;
    assert_eq!(
        finish(
            &mut runtime,
            buffer::close(&stranger, foreign_state, ClosePolicy::DiscardLocal,)
        ),
        Err(RequestError::Failed(Error::DENIED))
    );
    assert_eq!(
        finish(
            &mut runtime,
            buffer::close(&client, observed.state, ClosePolicy::DiscardLocal,)
        )
        .unwrap()
        .data
        .as_ref(),
        b"closed"
    );
    runtime.close().unwrap();
}

#[test]
fn client_can_release_a_forwarded_snapshot_and_close_its_session() {
    let (mut runtime, client) = setup();
    let subject = client.resolve("relay").into_inner().unwrap();
    let observed = finish(
        &mut runtime,
        session::create(
            &client,
            subject,
            &Message::new(Contract::new("heading.initial", 1), b"# Context\n"),
        ),
    )
    .unwrap();
    assert_eq!(observed.value.data.as_ref(), b"# Context\n");
    assert_eq!(
        finish(&mut runtime, session::release(&client, observed.snapshot)),
        Ok(())
    );
    assert_eq!(
        finish(&mut runtime, session::close(&client, observed.session)),
        Ok(())
    );
    runtime.close().unwrap();
}

#[test]
fn sharing_an_owner_does_not_widen_grants_or_accept_foreign_bindings() {
    let mut runtime = Runtime::new(184, Limits::default()).unwrap();
    runtime.start().unwrap();
    let root = runtime.resource("").unwrap();
    let owner = runtime.bind(&[(root, Permissions::READ)]).unwrap();
    assert!(matches!(
        runtime.bind_for(&owner, &[(root, Permissions::ALL)]),
        Err(Error::DENIED)
    ));
    let module = runtime.bind_for(&owner, &[]).unwrap();
    let imports = module.connection();
    assert!(matches!(
        imports.resolve("kernel").into_inner(),
        Err(RequestError::Rejected(_))
    ));
    let mut other = Runtime::new(185, Limits::default()).unwrap();
    assert!(matches!(other.bind_for(&owner, &[]), Err(Error::INVALID)));
}
