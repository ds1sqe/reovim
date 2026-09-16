#[path = "support/memory.rs"]
mod memory;

use reovim_kernel_interface::{
    Error, RequestError, provider::Permissions, resource::ResourceFlags,
};
use reovim_kernel_runtime::{Limits, Runtime};

#[test]
fn flags_follow_visible_children_through_native_adapters() {
    let mut runtime = Runtime::new(312, Limits::default()).unwrap();
    runtime.start().unwrap();
    let parent = runtime.register_namespace("visible").unwrap();
    let binding = runtime.bind(&[(parent, Permissions::READ)]).unwrap();
    runtime.mount(&binding, "alias", parent).unwrap();
    let caller = reovim_sdk::local(binding.connection());
    let id = caller.resolve("visible").into_inner().unwrap();
    let empty = ResourceFlags::CONTAINER | ResourceFlags::EMPTY;
    assert_eq!(caller.flags(id).into_inner(), Ok(empty));
    let child = runtime
        .register("visible/data", memory::provider(b"content"))
        .map_err(|(e, _)| e)
        .unwrap();
    assert_eq!(caller.flags(id).into_inner(), Ok(ResourceFlags::CONTAINER));
    assert_eq!(caller.resolve("alias").into_inner(), Ok(id));
    let child_id = caller.resolve("visible/data").into_inner().unwrap();
    assert_eq!(caller.flags(child_id).into_inner(), Ok(empty));
    runtime.unregister(child).unwrap();
    assert_eq!(caller.flags(id).into_inner(), Ok(empty));
    assert_eq!(
        caller.flags(child_id).into_inner(),
        Err(RequestError::Rejected(Error::DENIED))
    );
    let peer = reovim_sdk::local(runtime.connect(&[]).unwrap());
    assert_eq!(
        peer.flags(id).into_inner(),
        Err(RequestError::Rejected(Error::DENIED))
    );
    let next = caller.reconnect().unwrap();
    assert_eq!(
        caller.flags(id).into_inner(),
        Err(RequestError::Rejected(Error::STALE))
    );
    let next_id = next.resolve("visible").into_inner().unwrap();
    assert_eq!(next.flags(next_id).into_inner(), Ok(empty));
    runtime.close().unwrap();
    assert_eq!(
        next.flags(next_id).into_inner(),
        Err(RequestError::Rejected(Error::CLOSED))
    );
}

#[test]
fn unnamed_read_view_is_a_leaf_even_when_its_data_is_empty() {
    let mut runtime = Runtime::new(313, Limits::default()).unwrap();
    runtime.start().unwrap();
    let provider = memory::provider(b"source")
        .with_read_view(|| Ok(memory::provider(b"")));
    let parent =
        runtime.register("data", provider).map_err(|(e, _)| e).unwrap();
    let caller = reovim_sdk::local(
        runtime.connect(&[(parent, Permissions::READ)]).unwrap(),
    );
    let id = caller.resolve("data").into_inner().unwrap();
    let view = caller.read_view(id).unwrap();
    let root = view.root().into_inner().unwrap();
    assert_eq!(view.flags(root).into_inner().unwrap().bits(), 0);
    view.close(root).into_inner().unwrap();
    assert_eq!(
        view.flags(root).into_inner(),
        Err(RequestError::Rejected(Error::CLOSED))
    );
    runtime.close().unwrap();
}

#[test]
fn inspection_is_passive_and_does_not_publish_hidden_siblings() {
    let mut runtime = Runtime::new(314, Limits::default()).unwrap();
    runtime.start().unwrap();
    let visible = runtime.register_namespace("view/visible").unwrap();
    runtime.register_namespace("view/private/secret").unwrap();
    let caller = reovim_sdk::local(
        runtime.connect(&[(visible, Permissions::READ)]).unwrap(),
    );
    let root = caller.resolve("view").into_inner().unwrap();
    let before = runtime.events(0);
    assert_eq!(caller.flags(root).into_inner(), Ok(ResourceFlags::CONTAINER));
    let children = caller.children(root).into_inner().unwrap();
    assert_eq!(
        children.iter().map(|entry| &*entry.name).collect::<Vec<_>>(),
        ["visible"]
    );
    assert!(caller.resolve("view/private").into_inner().is_err());
    assert_eq!(runtime.events(0), before);
    runtime.close().unwrap();
}

#[test]
#[allow(unsafe_code)]
fn native_absence_and_invalid_structure_are_explicit_errors() {
    use reovim_kernel_interface::abi;
    use std::{ffi::c_void, rc::Rc};
    unsafe extern "C" fn invalid(
        _: *mut c_void,
        _: u64,
        output: *mut u32,
    ) -> abi::Failure {
        unsafe { output.write(ResourceFlags::EMPTY.bits()) };
        abi::Failure { code: 0, resource: 0 }
    }
    let mut runtime = Runtime::new(315, Limits::default()).unwrap();
    runtime.start().unwrap();
    for available in [false, true] {
        let mut raw = reovim_sdk::export(runtime.connect(&[]).unwrap());
        // All fields are copied inert function/pointer values; this table
        // stays alive until the imported Connection releases its context.
        let mut table = Box::new(unsafe { std::ptr::read(raw.table) });
        table.flags = available.then_some(invalid);
        raw.table = &raw const *table;
        let caller = unsafe { reovim_sdk::import(raw, Rc::new(())) };
        let root = caller.root().into_inner().unwrap();
        let expected =
            if available { Error::PROVIDER } else { Error::UNSUPPORTED };
        assert_eq!(
            caller.flags(root).into_inner(),
            Err(RequestError::Rejected(expected))
        );
        drop(caller);
    }
    runtime.close().unwrap();
}
