use reovim_host_reference::run;
use reovim_kernel_interface::{
    Connection, Contract, Error, Message, ResourceId,
    provider::{
        Permissions, ProviderReply, ResourceOps, ResourceProvider, Surface,
    },
};
use reovim_kernel_runtime::{Limits, Runtime};
use std::{cell::Cell, future::ready, rc::Rc};

struct State {
    ready: Rc<Cell<bool>>,
    destroyed: Rc<Cell<bool>>,
    kernel: Connection,
}
impl Drop for State {
    fn drop(&mut self) {
        // Destructor re-entry proves the registry guard has been released.
        self.kernel.root().into_inner().unwrap();
        self.destroyed.set(true);
    }
}
fn unload(state: &mut State) -> Result<(), Error> {
    state.kernel.root().into_inner().unwrap();
    if state.ready.get() { Ok(()) } else { Err(Error::BUSY) }
}
fn read(_: &mut State, _: ResourceId, input: Message) -> ProviderReply {
    Box::pin(ready(Ok(Message::new(input.contract, b"usable"))))
}
fn setup() -> (Runtime, Connection, ResourceId, Rc<Cell<bool>>, Rc<Cell<bool>>)
{
    let mut runtime =
        Runtime::new(108, Limits::default(), reovim_kernel_core::create)
            .unwrap();
    runtime.start().unwrap();
    let ready = Rc::new(Cell::new(false));
    let destroyed = Rc::new(Cell::new(false));
    let kernel = runtime.connect(&[]).unwrap();
    let provider = ResourceProvider::new(
        State {
            ready: Rc::clone(&ready),
            destroyed: Rc::clone(&destroyed),
            kernel,
        },
        ResourceOps { read, write: read, unload: Some(unload) },
        Box::new([Surface {
            contract: Contract::new("state", 1),
            methods: Permissions::READ,
            input: Box::new([]),
            output: Box::new([]),
        }]),
        Rc::new(()),
    );
    let id = runtime
        .register("state", provider)
        .map_err(|(error, _)| error)
        .unwrap();
    let client = runtime.connect(&[(id, Permissions::READ)]).unwrap();
    (runtime, client, id, ready, destroyed)
}
#[test]
fn busy_unload_keeps_the_provider_usable_and_destroys_outside_guards() {
    let (mut runtime, client, id, ready, destroyed) = setup();
    assert_eq!(runtime.unregister(id), Err(Error::BUSY));
    assert!(!destroyed.get());
    let subject = client.resolve("state").into_inner().unwrap();
    let result = run(
        client.read(subject, &Message::new(Contract::new("state", 1), &[])),
        |context| runtime.advance(context).map(|_| ()),
    )
    .unwrap()
    .unwrap();
    assert_eq!(result.data.as_ref(), b"usable");
    ready.set(true);
    runtime.unregister(id).unwrap();
    assert!(destroyed.get());
    assert!(client.resolve("state").into_inner().is_err());
    runtime.close().unwrap();
}
#[test]
fn root_shutdown_preserves_busy_provider_ownership_for_retry() {
    let (mut runtime, _, _, ready, destroyed) = setup();
    assert_eq!(runtime.close(), Err(Error::BUSY));
    assert!(!destroyed.get());
    ready.set(true);
    runtime.close().unwrap();
    assert!(destroyed.get());
}

#[test]
fn busy_member_restores_the_entire_contribution_set() {
    let (mut runtime, client, state, ready, destroyed) = setup();
    let source = runtime
        .register("source", reovim_resource_local::memory(b"still available"))
        .map_err(|(error, _)| error)
        .unwrap();
    assert_eq!(
        runtime.unregister_contributions(&[source, state]),
        Err(Error::BUSY)
    );
    assert_eq!(runtime.resource("source"), Ok(source));
    assert_eq!(runtime.resource("state"), Ok(state));
    assert!(!destroyed.get());
    let subject = client.resolve("state").into_inner().unwrap();
    let reply = run(
        client.read(subject, &Message::new(Contract::new("state", 1), &[])),
        |context| runtime.advance(context).map(|_| ()),
    )
    .unwrap()
    .unwrap();
    assert_eq!(reply.data.as_ref(), b"usable");
    ready.set(true);
    runtime.unregister_contributions(&[source, state]).unwrap();
    assert!(destroyed.get());
    assert_eq!(runtime.resource("source"), Err(Error::UNAVAILABLE));
    assert_eq!(runtime.resource("state"), Err(Error::UNAVAILABLE));
    runtime.close().unwrap();
}

#[test]
fn duplicate_member_preserves_the_registration() {
    let (mut runtime, _, state, ready, destroyed) = setup();
    ready.set(true);
    assert_eq!(
        runtime.unregister_contributions(&[state, state]),
        Err(Error::INVALID)
    );
    assert!(!destroyed.get());
    assert_eq!(runtime.resource("state"), Ok(state));
    runtime.unregister(state).unwrap();
    assert!(destroyed.get());
    runtime.close().unwrap();
}
