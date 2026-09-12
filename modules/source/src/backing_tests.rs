use alloc::{boxed::Box, rc::Rc, vec::Vec};
use core::{
    cell::{Cell, RefCell},
    future::{Future, poll_fn},
    task::{Context, Poll, Waker},
};
use reovim_kernel_interface::{
    Contract, Error, Message,
    provider::{Permissions, ResourceOps, ResourceProvider, Surface},
};
use reovim_kernel_runtime::{Limits, Runtime};

type Reads = Rc<RefCell<Vec<core::ops::Range<u64>>>>;
fn finish<T>(runtime: &mut Runtime, future: impl Future<Output = T>) -> T {
    reovim_host_reference::run(future, |cx| runtime.advance(cx).map(|_| ()))
        .unwrap()
}

fn provider(reads: Reads, mode: Rc<Cell<u8>>) -> ResourceProvider {
    ResourceProvider::new(
        (reads, mode),
        ResourceOps {
            unload: None,
            read: |(reads, mode), _, input| {
                if input.data.is_empty() {
                    return metadata(input);
                }
                let range = super::api::decode_range(&input.data).unwrap();
                reads.borrow_mut().push(range.clone());
                let mode = mode.clone();
                Box::pin(poll_fn(move |_| window(&mode, &range, &input)))
            },
            write: |_, _, _| Box::pin(async { Err(Error::DENIED) }),
        },
        Box::new([Surface {
            contract: Contract::new("resource.snapshot", 1),
            methods: Permissions::READ,
            input: Box::new([]),
            output: Box::new([]),
        }]),
        Rc::new(()),
    )
}
fn metadata(
    input: Message,
) -> reovim_kernel_interface::provider::ProviderReply {
    Box::pin(async move {
        Ok(Message::new(input.contract, &100_000_u64.to_le_bytes()))
    })
}
fn window(
    mode: &Cell<u8>,
    range: &core::ops::Range<u64>,
    input: &Message,
) -> Poll<Result<Message, Error>> {
    match mode.get() {
        0 => Poll::Pending,
        1 => Poll::Ready(Err(Error::UNAVAILABLE)),
        _ => Poll::Ready(Ok(Message::new(
            input.contract.clone(),
            &range
                .clone()
                .map(|offset| u8::try_from(offset % 251).unwrap())
                .collect::<Vec<_>>(),
        ))),
    }
}

#[test]
fn source_preserves_lazy_ranges_and_missing_window_failures() {
    let mut runtime = Runtime::new(211, Limits::default()).unwrap();
    runtime.start().unwrap();
    let reads = Reads::default();
    let mode = Rc::new(Cell::new(2));
    let resource = runtime
        .register("backing", provider(reads.clone(), mode.clone()))
        .map_err(|(error, _)| error)
        .unwrap();
    let kernel = runtime.connect(&[(resource, Permissions::READ)]).unwrap();
    let backing = kernel.resolve("backing").into_inner().unwrap();
    let input = Message::reference(Contract::new("source.open", 1), backing);
    let initial =
        finish(&mut runtime, super::open(Some(kernel), input)).unwrap();
    let source = initial.get::<super::storage::Source>().unwrap();
    assert_eq!(source.len(), 100_000);
    assert!(reads.borrow().is_empty());
    let next = source.splice(40_000..40_004, b"new").unwrap();
    assert!(reads.borrow().is_empty());
    let mut bytes = Vec::new();
    finish(&mut runtime, next.read_async(39_998..40_005, &mut bytes)).unwrap();
    let expected = [89, 90, b'n', b'e', b'w', 95, 96];
    assert_eq!(bytes, expected);
    assert_eq!(reads.borrow().as_slice(), &[39_998..40_000, 40_004..40_006]);
    mode.set(0);
    let mut missing = Vec::new();
    let mut waiting = Box::pin(next.read_async(0..2, &mut missing));
    let mut context = Context::from_waker(Waker::noop());
    assert!(waiting.as_mut().poll(&mut context).is_pending());
    runtime.advance(&mut context).unwrap();
    assert!(waiting.as_mut().poll(&mut context).is_pending());
    mode.set(1);
    assert_eq!(finish(&mut runtime, waiting), Err(Error::UNAVAILABLE));
    assert_eq!(missing, [] as [u8; 0]);
    mode.set(2);
    let mut original = Vec::new();
    finish(&mut runtime, source.read_async(40_000..40_004, &mut original))
        .unwrap();
    assert_eq!(original, [91, 92, 93, 94]);
    drop(next);
    drop(initial);
    runtime.unregister(resource).unwrap();
    runtime.close().unwrap();
}
