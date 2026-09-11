use reovim_host_reference::run;
use reovim_kernel_interface::{
    Connection, Contract, Error, Message, RequestError, ResourceId,
    buffer::{self, ClosePolicy},
    provider::{
        Contribution, Permissions, ResourceOps, ResourceProvider, Surface,
    },
    requests,
};
use reovim_kernel_runtime::{Binding, Limits, Runtime};
use std::{future::Future, rc::Rc};

fn finish<T>(runtime: &mut Runtime, future: impl Future<Output = T>) -> T {
    run(future, |context| runtime.advance(context).map(|_| ())).unwrap()
}

fn locator(error: RequestError) -> Box<str> {
    let RequestError::Unknown { recovery, .. } = error else {
        panic!("old connection must preserve recovery")
    };
    recovery
}

fn surface(name: &str, rights: Permissions) -> Surface {
    Surface {
        contract: Contract::new(name, 1),
        methods: Permissions::WRITE,
        input: Box::new([]),
        output: Box::new([rights]),
    }
}

fn publisher(binding: &Binding) -> ResourceProvider {
    ResourceProvider::new(
        reovim_sdk::local(binding.connection()),
        ResourceOps {
            unload: None,
            read: |_, _, _| Box::pin(async { Err(Error::UNSUPPORTED) }),
            write: |kernel: &mut Connection, _, input| {
                let model = kernel.resolve("model").into_inner().unwrap();
                let pending = kernel.write(
                    model,
                    &Message::new(
                        Contract::new("heading.initial", 1),
                        b"# Recovered\n",
                    ),
                );
                *kernel = kernel.reconnect().unwrap();
                let kernel = kernel.clone();
                Box::pin(async move {
                    let recovery = locator(pending.await.unwrap_err());
                    let work = kernel.resolve(&recovery).await.unwrap();
                    Ok(Message::reference(input.contract, work))
                })
            },
        },
        Box::new([
            surface("handoff", Permissions::ALL),
            surface("handoff.read", Permissions::READ),
        ]),
        Rc::new(()),
    )
}

fn setup() -> (Runtime, Connection, Connection, Binding) {
    let mut runtime = Runtime::new(186, Limits::default()).unwrap();
    runtime.start().unwrap();
    let root = runtime.resource("").unwrap();
    let model = runtime
        .register_model("model", reovim_representation_basic::heading())
        .map_err(|(error, _)| error)
        .unwrap();
    let owner = runtime.bind(&[(root, Permissions::ALL)]).unwrap();
    let module =
        runtime.bind_for(&owner, &[(model, Permissions::ALL)]).unwrap();
    let sibling =
        runtime.bind_for(&owner, &[(root, Permissions::ALL)]).unwrap();
    runtime
        .register_bound(
            &module,
            vec![Contribution::Resource {
                path: "handoff".into(),
                provider: publisher(&module),
            }],
        )
        .map_err(|(error, _)| error)
        .unwrap();
    (
        runtime,
        reovim_sdk::local(owner.connection()),
        reovim_sdk::local(sibling.connection()),
        module,
    )
}

fn transfer(
    runtime: &mut Runtime,
    client: &Connection,
    contract: &str,
) -> ResourceId {
    let subject = client.resolve("handoff").into_inner().unwrap();
    let reply = finish(
        runtime,
        client.write(subject, &Message::new(Contract::new(contract, 1), &[])),
    )
    .unwrap();
    let [work] = reply.resources.as_ref() else { panic!("one work reference") };
    *work
}

#[test]
fn explicit_handoff_recovers_stable_owned_results_in_the_receivers_namespace() {
    let (mut runtime, client, sibling, _) = setup();
    let work = transfer(&mut runtime, &client, "handoff");
    let root = client.resolve("requests").into_inner().unwrap();
    let locator = client
        .children(root)
        .into_inner()
        .unwrap()
        .into_iter()
        .find(|entry| entry.resource == work)
        .unwrap()
        .name;
    assert_eq!(
        sibling.resolve(&format!("requests/{locator}")).into_inner(),
        Err(RequestError::Rejected(Error::DENIED))
    );
    let first =
        finish(&mut runtime, requests::result(&client, work)).unwrap().unwrap();
    let second =
        finish(&mut runtime, requests::result(&client, work)).unwrap().unwrap();
    assert_eq!(first, second);
    let observed = buffer::observation(first).unwrap();
    assert_eq!(observed.value.data.as_ref(), b"# Recovered\n");
    let client = client.reconnect().unwrap();
    let work =
        client.resolve(&format!("requests/{locator}")).into_inner().unwrap();
    let again = buffer::observation(
        finish(&mut runtime, requests::result(&client, work)).unwrap().unwrap(),
    )
    .unwrap();
    assert_eq!(again.value, observed.value);
    assert_ne!(again.state.buffer, observed.state.buffer);
    client.close(work).into_inner().unwrap();
    finish(
        &mut runtime,
        buffer::close(&client, again.state, ClosePolicy::DiscardLocal),
    )
    .unwrap();
    runtime.close().unwrap();
}

#[test]
fn read_only_work_access_cannot_cancel_or_close_the_original_request() {
    let (mut runtime, client, _, owner) = setup();
    let work = transfer(&mut runtime, &client, "handoff.read");
    let observed =
        finish(&mut runtime, requests::result(&client, work)).unwrap().unwrap();
    assert_eq!(
        buffer::observation(observed).unwrap().value.data.as_ref(),
        b"# Recovered\n"
    );
    assert_eq!(
        client.close(work).into_inner(),
        Err(RequestError::Rejected(Error::DENIED))
    );
    assert_eq!(
        finish(
            &mut runtime,
            client.write(
                work,
                &Message::new(Contract::new("kernel.request.cancel", 1), &[])
            )
        ),
        Err(RequestError::Rejected(Error::DENIED))
    );
    let root = client.resolve("requests").into_inner().unwrap();
    let name = client
        .children(root)
        .into_inner()
        .unwrap()
        .into_iter()
        .find(|entry| entry.resource == work)
        .unwrap()
        .name;
    let owner = reovim_sdk::local(owner.connection());
    let work = owner.resolve(&format!("requests/{name}")).into_inner().unwrap();
    owner.close(work).into_inner().unwrap();
    runtime.close().unwrap();
}

#[test]
fn an_admitted_borrow_keeps_work_available_until_the_reader_finishes() {
    let (mut runtime, client, _, _) = setup();
    let binding = runtime.bind(&[]).unwrap();
    let reader = ResourceProvider::new(
        reovim_sdk::local(binding.connection()),
        ResourceOps {
            unload: None,
            read: |_, _, _| Box::pin(async { Err(Error::UNSUPPORTED) }),
            write: |kernel: &mut Connection, _, input| {
                let kernel = kernel.clone();
                Box::pin(async move {
                    let result = requests::result(&kernel, input.resources[0])
                        .await
                        .unwrap()
                        .unwrap();
                    let observed = buffer::observation(result).unwrap();
                    Ok(Message::new(input.contract, &observed.value.data))
                })
            },
        },
        Box::new([Surface {
            contract: Contract::new("inspect", 1),
            methods: Permissions::WRITE,
            input: Box::new([Permissions::READ]),
            output: Box::new([]),
        }]),
        Rc::new(()),
    );
    runtime
        .register_bound(
            &binding,
            vec![Contribution::Resource {
                path: "inspect".into(),
                provider: reader,
            }],
        )
        .map_err(|(error, _)| error)
        .unwrap();
    let work = transfer(&mut runtime, &client, "handoff");
    let reader = client.resolve("inspect").into_inner().unwrap();
    let waiting = client
        .write(reader, &Message::reference(Contract::new("inspect", 1), work));
    assert_eq!(
        client.close(work).into_inner(),
        Err(RequestError::Rejected(Error::BUSY))
    );
    assert_eq!(
        finish(&mut runtime, waiting).unwrap().data.as_ref(),
        b"# Recovered\n"
    );
    client.close(work).into_inner().unwrap();
    runtime.close().unwrap();
}

#[test]
fn a_retained_handoff_result_pins_the_work_it_returns() {
    let (mut runtime, client, _, _) = setup();
    let subject = client.resolve("handoff").into_inner().unwrap();
    let waiting =
        client.write(subject, &Message::new(Contract::new("handoff", 1), &[]));
    let client = client.reconnect().unwrap();
    let recovery = locator(finish(&mut runtime, waiting).unwrap_err());
    let outer = client.resolve(&recovery).into_inner().unwrap();
    let result = finish(&mut runtime, requests::result(&client, outer))
        .unwrap()
        .unwrap();
    let inner = result.resources[0];
    let value = finish(&mut runtime, requests::result(&client, inner))
        .unwrap()
        .unwrap();
    assert_eq!(
        buffer::observation(value).unwrap().value.data.as_ref(),
        b"# Recovered\n"
    );
    assert_eq!(
        client.close(inner).into_inner(),
        Err(RequestError::Rejected(Error::BUSY))
    );
    client.close(outer).into_inner().unwrap();
    client.close(inner).into_inner().unwrap();
    runtime.close().unwrap();
}
