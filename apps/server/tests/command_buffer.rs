use reovim_host_reference::run_with;
use reovim_kernel_interface::{
    Connection, Contract, Error, Message, RequestError,
    buffer::{self, ClosePolicy},
    provider::{
        Contribution, Permissions, ResourceOps, ResourceProvider, Surface,
    },
};
use reovim_kernel_runtime::{Limits, Runtime};
use std::{cell::Cell, future::Future, rc::Rc};

struct Code(Rc<Cell<bool>>);
impl Drop for Code {
    fn drop(&mut self) {
        self.0.set(true);
    }
}

fn command(kernel: Connection, released: Rc<Cell<bool>>) -> ResourceProvider {
    ResourceProvider::new(
        kernel,
        ResourceOps {
            unload: None,
            read: |_, _, _| {
                Box::pin(async {
                    Err(RequestError::Rejected(Error::UNSUPPORTED))
                })
            },
            write: |kernel, _, input| {
                let kernel = kernel.clone();
                Box::pin(async move {
                    let model = kernel.resolve("model").await?;
                    let mut reply = kernel
                        .write(
                            model,
                            &Message::new(
                                Contract::new("heading.initial", 1),
                                &input.data,
                            ),
                        )
                        .await?;
                    reply.contract = input.contract;
                    Ok(reply)
                })
            },
        },
        Box::new([Surface {
            contract: Contract::new("command.open.test", 1),
            methods: Permissions::WRITE,
            input: Box::new([]),
            output: Box::new([Permissions::ALL, Permissions::ALL]),
        }]),
        Rc::new(Code(released)),
    )
}

fn finish<T>(runtime: &mut Runtime, future: impl Future<Output = T>) -> T {
    run_with(future, runtime, &Runtime::DRIVER).unwrap()
}

#[test]
fn client_owned_command_returns_editable_state_without_transferring_owner() {
    let mut runtime = Runtime::new(243, Limits::default()).unwrap();
    runtime.start().unwrap();
    let root = runtime.resource("").unwrap();
    let model = runtime
        .register_model("model", reovim_representation_basic::heading())
        .map_err(|(error, _)| error)
        .unwrap();
    let owner = runtime.bind(&[(root, Permissions::ALL)]).unwrap();
    let binding =
        runtime.bind_for(&owner, &[(model, Permissions::WRITE)]).unwrap();
    let released = Rc::new(Cell::new(false));
    let ids = runtime
        .register_bound(
            &binding,
            vec![Contribution::Resource {
                path: "open".into(),
                provider: command(
                    reovim_sdk::local(binding.connection()),
                    released.clone(),
                ),
            }],
        )
        .map_err(|(error, _)| error)
        .unwrap();
    let caller =
        runtime.bind_for(&owner, &[(ids[0], Permissions::WRITE)]).unwrap();
    let client = reovim_sdk::local(caller.connection());
    let open = client.resolve("open").into_inner().unwrap();
    let reply = finish(
        &mut runtime,
        client.write(
            open,
            &Message::new(Contract::new("command.open.test", 1), b"# Open"),
        ),
    )
    .unwrap();
    let observed = buffer::observation(reply).unwrap();
    assert_eq!(observed.value.data.as_ref(), b"# Open");
    runtime.unregister(ids[0]).unwrap();
    drop(binding);
    assert!(released.get());
    let changed = finish(
        &mut runtime,
        reovim_module_editing::replace(&client, &observed, b"Changed"),
    )
    .unwrap();
    assert_eq!(changed.value.data.as_ref(), b"# Changed");

    // Even full operation grants do not make another client the close owner.
    let peer = reovim_sdk::local(
        runtime.connect(&[(root, Permissions::ALL)]).unwrap(),
    );
    let parent = client.resolve("buffers").into_inner().unwrap();
    let name = client
        .children(parent)
        .into_inner()
        .unwrap()
        .into_iter()
        .find(|entry| entry.resource == changed.state.buffer)
        .unwrap()
        .name;
    let subject =
        peer.resolve(&format!("buffers/{name}")).into_inner().unwrap();
    let foreign = buffer::BufferState {
        buffer: subject,
        revision: changed.state.revision,
    };
    assert_eq!(
        finish(
            &mut runtime,
            buffer::close(&peer, foreign, ClosePolicy::DiscardLocal),
        ),
        Err(RequestError::Failed(Error::DENIED))
    );
    finish(
        &mut runtime,
        buffer::close(&client, changed.state, ClosePolicy::DiscardLocal),
    )
    .unwrap();
    runtime.close().unwrap();
}
