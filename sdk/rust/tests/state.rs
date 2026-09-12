#![allow(unsafe_code)]

use reovim_kernel_interface::{Error, abi, buffer::State};
use reovim_sdk::{export_state_reply, import_state_reply};
use std::task::{Context, Poll, Waker};
use std::{cell::Cell, ffi::c_void, future::poll_fn, ptr, rc::Rc};

struct Marker(Rc<Cell<bool>>);
impl Drop for Marker {
    fn drop(&mut self) {
        assert!(!self.0.replace(true));
    }
}

const unsafe extern "C" fn noop(_: *const c_void) {}
fn wake() -> abi::Wake {
    abi::Wake { context: ptr::null(), retain: noop, release: noop, wake: noop }
}

#[test]
fn readiness_does_not_transfer_and_take_is_once() {
    let dropped = Rc::new(Cell::new(false));
    let state = State::new(Marker(dropped.clone()), 17);
    let pending = export_state_reply(Box::pin(async move { Ok(state) }));
    unsafe {
        assert_eq!(
            (pending.take)(pending.context).error,
            Error::NOT_READY.code.0
        );
        assert_eq!((pending.poll)(pending.context, wake()).status, 1);
        assert_eq!((pending.poll)(pending.context, wake()).status, 1);
        assert!(!dropped.get());
        let reply = (pending.take)(pending.context);
        assert_eq!(reply.error, 0);
        assert_eq!(reply.state_bytes, 17);
        assert_eq!((pending.take)(pending.context).error, Error::CLOSED.code.0);
        (pending.release)(pending.context);
        assert!(!dropped.get());
        drop(Box::from_raw(reply.state.cast::<State>()));
    }
    assert!(dropped.get());
}

#[test]
fn release_drops_untaken_ready_state() {
    let dropped = Rc::new(Cell::new(false));
    let state = State::new(Marker(dropped.clone()), 0);
    let pending = export_state_reply(Box::pin(async move { Ok(state) }));
    unsafe {
        assert_eq!((pending.poll)(pending.context, wake()).status, 1);
        (pending.release)(pending.context);
    }
    assert!(dropped.get());
}

#[test]
fn cancellation_releases_future_and_owner_without_taking() {
    let dropped = Rc::new(Cell::new(false));
    let state = State::new(Marker(dropped.clone()), 0);
    let pending = export_state_reply(Box::pin(poll_fn(move |_| {
        assert!(state.get::<Marker>().is_some());
        Poll::Pending
    })));
    let owner = Rc::new(());
    let weak = Rc::downgrade(&owner);
    let mut future =
        unsafe { import_state_reply(pending, owner, |_| panic!("not ready")) };
    assert!(
        future
            .as_mut()
            .poll(&mut Context::from_waker(Waker::noop()))
            .is_pending()
    );
    assert!(!dropped.get());
    assert!(weak.upgrade().is_some());
    drop(future);
    assert!(dropped.get());
    assert!(weak.upgrade().is_none());
}

#[test]
fn failed_preparation_preserves_error() {
    let pending =
        export_state_reply(Box::pin(async { Err(Error::UNAVAILABLE) }));
    let mut future = unsafe {
        import_state_reply(pending, Rc::new(()), |_| panic!("failed"))
    };
    let result = future.as_mut().poll(&mut Context::from_waker(Waker::noop()));
    assert!(matches!(result, Poll::Ready(Err(Error::UNAVAILABLE))));
}
