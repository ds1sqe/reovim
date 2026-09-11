use alloc::{boxed::Box, rc::Rc};
use core::{
    cell::RefCell,
    future::poll_fn,
    task::{Context, Poll, Waker},
};
use reovim_kernel_interface::{
    Contract, Error, Message,
    provider::{Permissions, ResourceOps, ResourceProvider, Surface},
};

use crate::{Limits, Progress, Runtime};

#[derive(Default)]
struct Gate {
    ready: bool,
    wake: Option<Waker>,
}

#[test]
fn pending_provider_sleeps_until_woken() {
    let gate = Rc::new(RefCell::new(Gate::default()));
    let provider = ResourceProvider::new(
        Rc::clone(&gate),
        ResourceOps {
            unload: None,
            read: |gate, _, input| {
                let gate = Rc::clone(gate);
                Box::pin(poll_fn(move |context| {
                    poll_gate(&gate, &input, context)
                }))
            },
            write: |_, _, _| Box::pin(core::future::ready(Err(Error::DENIED))),
        },
        Box::new([Surface {
            contract: Contract::new("gate", 1),
            methods: Permissions::READ,
            input: Box::new([]),
            output: Box::new([]),
        }]),
        Rc::new(()),
    );
    let mut runtime = Runtime::new(1, Limits::default()).unwrap();
    runtime.start().unwrap();
    let id = runtime.register("gate", provider).map_err(|(e, _)| e).unwrap();
    let caller = runtime.connect(&[(id, Permissions::READ)]).unwrap();
    let id = caller.resolve("gate").into_inner().unwrap();
    let input = Message::new(Contract::new("gate", 1), b"ready");
    let mut reply = caller.read(id, &input);
    let mut context = Context::from_waker(Waker::noop());
    runtime.advance(&mut context).unwrap();
    runtime.advance(&mut context).unwrap();
    assert_eq!(runtime.advance(&mut context), Ok(Progress::Idle));
    assert!(reply.as_mut().poll(&mut context).is_pending());
    gate.borrow_mut().ready = true;
    gate.borrow_mut().wake.take().unwrap().wake();
    assert_eq!(runtime.advance(&mut context), Ok(Progress::More));
    let Poll::Ready(Ok(result)) = reply.as_mut().poll(&mut context) else {
        panic!("woken provider did not complete");
    };
    assert_eq!(result.data.as_ref(), b"ready");
}

fn poll_gate(
    gate: &RefCell<Gate>,
    input: &Message,
    context: &Context<'_>,
) -> Poll<Result<Message, Error>> {
    let mut gate = gate.borrow_mut();
    if gate.ready {
        return Poll::Ready(Ok(input.clone()));
    }
    assert!(gate.wake.is_none(), "polled without a wake");
    gate.wake = Some(context.waker().clone());
    Poll::Pending
}
