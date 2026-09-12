use reovim_host_reference::run;
use reovim_kernel_interface::{
    Connection, Contract, Error, Message, RequestError, ResourceId,
    provider::{
        Contribution, Permissions, ResourceOps, ResourceProvider, Surface,
    },
};
use reovim_kernel_runtime::{Limits, Runtime};
use std::{cell::RefCell, future::Future, rc::Rc};

type Held = Rc<RefCell<Option<(Connection, ResourceId)>>>;

fn finish<T>(runtime: &mut Runtime, future: impl Future<Output = T>) -> T {
    run(future, |context| runtime.advance(context).map(|_| ())).unwrap()
}

fn receiver(kernel: Connection, held: Held) -> ResourceProvider {
    ResourceProvider::new(
        (kernel, held),
        ResourceOps {
            unload: None,
            read: |_, _, _| Box::pin(async { Err(Error::UNSUPPORTED) }),
            write: |(kernel, held), _, input| {
                let rights = if input.data.is_empty() {
                    Permissions::READ
                } else {
                    Permissions::ALL
                };
                let id = input.resources[0];
                let captured = kernel.capture(id, rights);
                let result = captured.map(|connection| {
                    *held.borrow_mut() = Some((connection, id));
                    Message::new(input.contract, &[])
                });
                Box::pin(async move { result })
            },
        },
        Box::new([Surface {
            contract: Contract::new("capture", 1),
            methods: Permissions::WRITE,
            input: Box::new([Permissions::READ]),
            output: Box::new([]),
        }]),
        Rc::new(()),
    )
}

#[test]
fn borrowed_capture_retains_exact_rights_and_admitted_work_through_c_abi() {
    let mut runtime = Runtime::new(203, Limits::default()).unwrap();
    runtime.start().unwrap();
    let source = runtime
        .register("source", reovim_resource_local::memory(b"exact"))
        .map_err(|(error, _)| error)
        .unwrap();
    let binding = runtime.bind(&[]).unwrap();
    let unprivileged = reovim_sdk::local(binding.connection());
    let held = Held::default();
    let ids = runtime
        .register_bound(
            &binding,
            vec![Contribution::Resource {
                path: "receiver".into(),
                provider: receiver(unprivileged.clone(), held.clone()),
            }],
        )
        .map_err(|(error, _)| error)
        .unwrap();
    let client = reovim_sdk::local(
        runtime
            .connect(&[
                (source, Permissions::READ),
                (ids[0], Permissions::WRITE),
            ])
            .unwrap(),
    );
    let source_id = client.resolve("source").into_inner().unwrap();
    let receiver = client.resolve("receiver").into_inner().unwrap();
    let mut request =
        Message::reference(Contract::new("capture", 1), source_id);
    request.data = Box::new([1]);
    assert_eq!(
        finish(&mut runtime, client.write(receiver, &request)),
        Err(RequestError::Failed(Error::DENIED))
    );
    assert!(held.borrow().is_none());
    request.data = Box::new([]);
    finish(&mut runtime, client.write(receiver, &request)).unwrap();
    let (captured, id) = held.borrow_mut().take().unwrap();
    let read = Message::new(Contract::new("resource.bytes", 1), &[]);
    assert_eq!(
        finish(&mut runtime, unprivileged.read(id, &read))
            .unwrap()
            .data
            .as_ref(),
        b"exact"
    );
    assert_eq!(runtime.unregister(source), Err(Error::BUSY));
    let alias = captured.clone();
    let reconnected = captured.reconnect().unwrap();
    assert_eq!(
        finish(&mut runtime, alias.read(id, &read)),
        Err(RequestError::Rejected(Error::STALE))
    );
    let id = reconnected.resolve("source").into_inner().unwrap();
    let unprivileged = reovim_sdk::local(binding.connection());
    let admitted = reconnected.read(id, &read);
    drop(alias);
    drop(captured);
    drop(reconnected);
    assert_eq!(runtime.unregister(source), Err(Error::BUSY));
    assert_eq!(finish(&mut runtime, admitted).unwrap().data.as_ref(), b"exact");
    assert_eq!(
        finish(&mut runtime, unprivileged.read(id, &read)),
        Err(RequestError::Rejected(Error::DENIED))
    );
    runtime.unregister(source).unwrap();
    runtime.close().unwrap();
}
