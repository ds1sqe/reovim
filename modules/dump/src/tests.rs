use super::*;
use alloc::{boxed::Box, rc::Rc, vec::Vec};
use core::cell::RefCell;
use reovim_kernel_interface::Contract;
use reovim_kernel_interface::provider::{
    ProviderReply, ResourceOps, ResourceProvider, Surface,
};
use reovim_kernel_runtime::{Limits, Runtime};

type Output = Rc<RefCell<Vec<u8>>>;

fn surface(name: &str, methods: Permissions) -> Surface {
    Surface {
        contract: Contract::new(name, 1),
        methods,
        input: Box::new([]),
        output: Box::new([]),
    }
}
fn read(reads: &mut usize, _: ResourceId, input: Message) -> ProviderReply {
    let data = if *reads == 0 { b"captured" } else { b"modified" };
    *reads += 1;
    Box::pin(ready(Ok(Message::new(input.contract, data))))
}
fn write(
    state: &mut (u8, Output),
    _: ResourceId,
    input: Message,
) -> ProviderReply {
    let mut reply = [0; 17];
    if input.contract.name.as_ref() == "host.export.file" {
        let split = input.data.iter().position(|b| *b == 0).unwrap();
        state.1.borrow_mut().extend_from_slice(&input.data[split + 1..]);
        reply[0] = state.0;
        reply[1..9].copy_from_slice(&8_u64.to_le_bytes());
        reply[9..].copy_from_slice(&5_u64.to_le_bytes());
        if state.0 == 0 {
            reply[9..].fill(0);
        }
    }
    Box::pin(ready(Ok(Message::new(input.contract, &reply))))
}
fn exercise(status: u8) -> (Message, Vec<u8>) {
    let mut runtime = Runtime::new(201, Limits::default()).unwrap();
    runtime.start().unwrap();
    let source = ResourceProvider::new(
        0,
        ResourceOps {
            unload: None,
            read,
            write: |_, _, _| panic!("dump must never write its source"),
        },
        Box::new([surface("resource.bytes", Permissions::READ)]),
        Rc::new(()),
    );
    let source =
        runtime.register("source", source).map_err(|(error, _)| error).unwrap();
    let output = Output::default();
    let destination = ResourceProvider::new(
        (status, output.clone()),
        ResourceOps {
            unload: None,
            write,
            read: |_, _, _| panic!("output is write-only"),
        },
        Box::new([
            surface("host.export.directory", Permissions::WRITE),
            surface("host.export.file", Permissions::WRITE),
        ]),
        Rc::new(()),
    );
    let destination = runtime
        .register("destination", destination)
        .map_err(|(error, _)| error)
        .unwrap();
    let kernel = runtime
        .connect(&[
            (source, Permissions::READ),
            (destination, Permissions::WRITE),
        ])
        .unwrap();
    let source = kernel.resolve("source").into_inner().unwrap();
    let destination = kernel.resolve("destination").into_inner().unwrap();
    let mut input = Message::new(Contract::new("command.dump", 1), b"dump");
    input.resources = Box::new([source, destination]);
    let command = Command::new(kernel);
    let result = reovim_host_reference::run(
        command.call(ResourceId(0), input),
        |context| runtime.advance(context).map(|_| ()),
    )
    .unwrap()
    .unwrap();
    let bytes = output.borrow().clone();
    (result, bytes)
}

#[test]
fn changed_source_keeps_captured_bytes_and_reports_partial() {
    let (result, bytes) = exercise(0);
    assert_eq!(bytes, b"captured");
    assert_eq!(result.data.as_ref(), b"partial\nchanged\t\tbytes=8 error=0\n");
}

#[test]
fn incomplete_and_unknown_output_never_report_complete() {
    let (result, bytes) = exercise(1);
    assert_eq!(bytes, b"captured");
    assert_eq!(
        result.data.as_ref(),
        b"partial\nincomplete\t\tbytes=8 error=5\n"
    );
    let (result, bytes) = exercise(2);
    assert_eq!(bytes, b"captured");
    assert_eq!(result.data.as_ref(), b"partial\nunknown\t\tbytes=8 error=5\n");
}
