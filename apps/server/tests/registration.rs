#[path = "support/memory.rs"]
mod memory;

use reovim_host_reference::run;
use reovim_kernel_interface::{
    Contract, Error, Message, provider::Permissions,
};
use reovim_kernel_runtime::{Limits, Runtime};

#[test]
fn ambiguous_registration_returns_usable_provider_ownership() {
    let mut runtime = Runtime::new(104, Limits::default()).unwrap();
    let mut provider = memory::provider(b"\xffkept");
    let surface = provider.surfaces[0].clone();
    provider.surfaces = Box::new([surface.clone(), surface.clone()]);
    let Err((error, mut provider)) = runtime.register("source", provider)
    else {
        panic!("overlapping surfaces must reject before publication");
    };
    assert_eq!(error, Error::INVALID);
    provider.surfaces = Box::new([surface]);
    let resource = runtime
        .register("source", provider)
        .map_err(|(error, _)| error)
        .unwrap();
    runtime.start().unwrap();
    let client = runtime.connect(&[(resource, Permissions::READ)]).unwrap();
    let subject = client.resolve("source").into_inner().unwrap();
    let result = run(
        client.read(
            subject,
            &Message::new(Contract::new("resource.bytes", 1), &[]),
        ),
        |context| runtime.advance(context).map(|_| ()),
    )
    .unwrap()
    .unwrap();
    assert_eq!(result.data.as_ref(), b"\xffkept");
    runtime.close().unwrap();
}
#[test]
fn contract_metadata_is_bounded_before_registration() {
    let limits = Limits { path_bytes: 8, ..Limits::default() };
    let mut runtime = Runtime::new(105, limits).unwrap();
    let provider = memory::provider(b"kept");
    let Err((error, mut provider)) = runtime.register("source", provider)
    else {
        panic!("oversized contract metadata must reject");
    };
    assert_eq!(error, Error::PRESSURE);
    provider.surfaces[0].contract = Contract::new("", 1);
    let Err((error, mut provider)) = runtime.register("source", provider)
    else {
        panic!("an empty contract name must reject");
    };
    assert_eq!(error, Error::INVALID);
    provider.surfaces[0].contract = Contract::new("bytes", 1);
    let resource = runtime
        .register("source", provider)
        .map_err(|(error, _)| error)
        .unwrap();
    runtime.start().unwrap();
    let client = runtime.connect(&[(resource, Permissions::READ)]).unwrap();
    let subject = client.resolve("source").into_inner().unwrap();
    let result = run(
        client.read(subject, &Message::new(Contract::new("bytes", 1), &[])),
        |context| runtime.advance(context).map(|_| ()),
    )
    .unwrap()
    .unwrap();
    assert_eq!(result.data.as_ref(), b"kept");
    runtime.close().unwrap();
}

#[test]
fn model_edit_cannot_replace_the_shared_close_contract() {
    let mut runtime = Runtime::new(106, Limits::default()).unwrap();
    let mut model = reovim_representation_basic::heading();
    model.edit = Contract::new("kernel.buffer.close", 1);
    let Err((error, mut model)) = runtime.register_model("model", model) else {
        panic!("model code cannot replace Kernel close authority");
    };
    assert_eq!(error, Error::INVALID);
    model.edit = Contract::new("heading.replace", 1);
    let registration = runtime
        .register_model("model", *model)
        .map_err(|(error, _)| error)
        .unwrap();
    assert_eq!(runtime.resource("model"), Ok(registration));
    runtime.unregister(registration).unwrap();
    runtime.close().unwrap();
}
