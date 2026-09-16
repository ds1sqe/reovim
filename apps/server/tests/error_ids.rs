#[path = "support/memory.rs"]
mod memory;

use reovim_host_reference::run;
use reovim_kernel_interface::{
    Contract, Error, Message, RequestError, ResourceId,
    provider::{
        Permissions, ProviderReply, ResourceOps, ResourceProvider, Surface,
    },
};
use reovim_kernel_runtime::{Limits, Runtime};
use std::{cell::Cell, future::ready, rc::Rc};

fn fail(
    error: &mut Rc<Cell<Error>>,
    _: ResourceId,
    _: Message,
) -> ProviderReply {
    Box::pin(ready(Err(error.get())))
}
#[test]
fn diagnostic_ids_are_scoped_and_do_not_disclose_hidden_subjects() {
    let mut runtime = Runtime::new(107, Limits::default()).unwrap();
    runtime.start().unwrap();
    let error = Rc::new(Cell::new(Error::INVALID));
    let provider = ResourceProvider::new(
        Rc::clone(&error),
        ResourceOps { unload: None, read: fail, write: fail },
        Box::new([Surface {
            contract: Contract::new("failure", 1),
            methods: Permissions::READ,
            input: Box::new([]),
            output: Box::new([]),
        }]),
        Rc::new(()),
    );
    let visible = runtime
        .register("visible", provider)
        .map_err(|(error, _)| error)
        .unwrap();
    let hidden = runtime
        .register("hidden", memory::provider(b"private"))
        .map_err(|(error, _)| error)
        .unwrap();
    let client = reovim_sdk::local(
        runtime.connect(&[(visible, Permissions::READ)]).unwrap(),
    );
    let local = client.resolve("visible").into_inner().unwrap();
    assert_ne!(visible, local);
    let request = Message::new(Contract::new("failure", 1), &[]);
    error.set(Error { id: Some(visible), ..Error::INVALID });
    let result = run(client.read(local, &request), |context| {
        runtime.advance(context).map(|_| ())
    })
    .unwrap();
    assert_eq!(
        result,
        Err(RequestError::Failed(Error { id: Some(local), ..Error::INVALID }))
    );
    error.set(Error { id: Some(hidden), ..Error::INVALID });
    let result = run(client.read(local, &request), |context| {
        runtime.advance(context).map(|_| ())
    })
    .unwrap();
    assert_eq!(result, Err(RequestError::Failed(Error::INVALID)));
    assert!(client.resolve("hidden").into_inner().is_err());
    runtime.close().unwrap();
}
