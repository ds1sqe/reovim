use super::*;
use reovim_kernel_interface::caller::{Caller, CallerOps};
use reovim_service::protocol::Endpoint;
use std::{cell::RefCell, rc::Weak};

struct Forward {
    caller: Caller,
    endpoint: RefCell<Weak<Endpoint>>,
    observed: RefCell<Option<Reply>>,
}

#[test]
fn reentrant_reservation_rejects_without_allocating_or_replacing_outer_ack() {
    let mut runtime = reovim_server::assemble(
        [0xb9; 16],
        Limits { requests: 1, ..Limits::default() },
    )
    .unwrap();
    let state = Rc::new(Forward {
        caller: runtime.bind(&[]).unwrap().caller(),
        endpoint: RefCell::new(Weak::new()),
        observed: RefCell::new(None),
    });
    let caller = Caller::new(
        state.clone(),
        CallerOps {
            reserve: |state| {
                let endpoint = state.endpoint.borrow().upgrade().unwrap();
                let reply = endpoint.dispatch(frame(12));
                *state.observed.borrow_mut() = Some(reply);
                state.caller.reserve()
            },
            submit: |_, _| Err(Error::UNSUPPORTED),
            result: |_, _, _| Err(Error::UNSUPPORTED),
            cancel: |_, _| Err(Error::UNSUPPORTED),
            reconcile: |_, _| Err(Error::UNSUPPORTED),
            adopt: |_, _| Err(Error::UNSUPPORTED),
            release: |_, _| Err(Error::UNSUPPORTED),
        },
    );
    let endpoint = Rc::new(Endpoint::new(TOKEN, caller));
    *state.endpoint.borrow_mut() = Rc::downgrade(&endpoint);
    let original = endpoint.dispatch(frame(11));
    assert_eq!(*state.observed.borrow(), Some(Reply::Error(Error::BUSY)));
    assert_eq!(endpoint.dispatch(frame(11)), original);
    state.caller.release(id(&original)).unwrap();
    runtime.advance(&mut Context::from_waker(Waker::noop())).unwrap();
    let next = endpoint.dispatch(frame(12));
    assert_ne!(id(&next), id(&original));
    state.caller.release(id(&next)).unwrap();
    runtime.advance(&mut Context::from_waker(Waker::noop())).unwrap();
    runtime.close().unwrap();
}
