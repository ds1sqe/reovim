use super::*;
use std::cell::Cell;

struct Saved {
    target: Rc<RefCell<Option<Open>>>,
    destroyed: Rc<Cell<bool>>,
}
impl Drop for Saved {
    fn drop(&mut self) {
        self.destroyed.set(true);
    }
}

#[test]
fn retaining_open_staging_does_not_cycle_back_to_the_provider() {
    let target = Rc::new(RefCell::new(None));
    let destroyed = Rc::new(Cell::new(false));
    let provider = ResourceProvider::new(
        Saved { target: target.clone(), destroyed: destroyed.clone() },
        reovim_kernel_interface::provider::ResourceOps {
            open: Some(|saved: &mut Saved, opened: &Open| {
                *saved.target.borrow_mut() = Some(opened.clone());
                opened.stage(empty()).map_err(|(error, _)| error)?;
                Ok(settled())
            }),
            namespace: None,
            io: None,
            stop: |_| Ok(()),
        },
        Box::new([]),
        Rc::new(()),
    )
    .ok()
    .unwrap();
    let mut runtime = Runtime::new([0x73; 16], Limits::default()).unwrap();
    let identity = runtime.register(b"/saved", provider, None).ok().unwrap();
    let caller = runtime
        .bind(&[Grant {
            resource: identity,
            access: 0,
            create_access: 0,
            retain_loans: false,
        }])
        .unwrap()
        .caller();
    runtime.start().unwrap();
    query(&mut runtime, &caller, acquire(&caller, identity));
    runtime.close().unwrap();
    assert!(destroyed.get());
    let late = target.borrow_mut().take().unwrap();
    let Err((error, mut provider)) = late.stage(empty()) else {
        panic!("expired staging unexpectedly accepted a provider");
    };
    assert_eq!(error, Error::CLOSED);
    provider.stop().unwrap();
}

#[test]
fn released_provisional_open_keeps_busy_cleanup_inside_the_access_bound() {
    let limits = Limits { callers: 1, handles: 1, ..Limits::default() };
    let (mut runtime, library, caller, identity) = installed(limits);
    let first = acquire(&caller, identity);
    caller.submit(first.clone()).unwrap();
    assert!(matches!(
        drive(&mut runtime, &caller, first.request).as_ref(),
        Outcome::Ok(_)
    ));
    toggle(&library, c"fixture_stop_busy");
    caller.release(first.request).unwrap();
    runtime.advance(&mut Context::from_waker(Waker::noop())).unwrap();
    let second = acquire(&caller, identity);
    assert_eq!(caller.submit(second.clone()), Err(Error::PRESSURE));
    caller.release(second.request).unwrap();
    assert_eq!(library.count(c"fixture_opens"), 1);
    assert_eq!(library.count(c"fixture_cursors_destroyed"), 0);
    toggle(&library, c"fixture_stop_ready");
    runtime.advance(&mut Context::from_waker(Waker::noop())).unwrap();
    assert_eq!(library.count(c"fixture_cursors_destroyed"), 1);
    let token =
        query(&mut runtime, &caller, acquire(&caller, identity)).references[0];
    access(&mut runtime, &caller, token, 1);
    assert_byte(&byte(&mut runtime, &caller, token, None), b'a');
    runtime.close().unwrap();
    assert_eq!(library.count(c"fixture_cursors_destroyed"), 2);
}

#[test]
fn opened_implementation_cannot_add_an_unoffered_io_direction() {
    let (mut runtime, library, caller, identity) =
        installed_with(Limits::default(), c"fixture_open_subject");
    let request = acquire(&caller, identity);
    caller.submit(request.clone()).unwrap();
    assert_eq!(
        drive(&mut runtime, &caller, request.request).as_ref(),
        &Outcome::Failed(Error::UNSUPPORTED)
    );
    caller.release(request.request).unwrap();
    runtime.advance(&mut Context::from_waker(Waker::noop())).unwrap();
    assert_eq!(library.count(c"fixture_opens"), 0);
    assert_eq!(library.count(c"fixture_cursors_destroyed"), 1);
    runtime.close().unwrap();
    assert_eq!(library.count(c"fixture_destroyed"), 1);
}

#[test]
fn close_waits_for_the_admitted_read_and_its_native_cleanup() {
    let (mut runtime, library, caller, identity) = installed(Limits::default());
    let token =
        query(&mut runtime, &caller, acquire(&caller, identity)).references[0];
    access(&mut runtime, &caller, token, 1);
    let read = call(
        &caller,
        Operation::Read,
        token,
        Value::copy(&1_u64.to_le_bytes(), &[]).unwrap(),
    );
    caller.submit(read.clone()).unwrap();
    runtime.advance(&mut Context::from_waker(Waker::noop())).unwrap();
    let close = call(&caller, Operation::Close, token, Value::unit());
    caller.submit(close.clone()).unwrap();
    let outcome = drive(&mut runtime, &caller, read.request);
    let Outcome::Ok(value) = outcome.as_ref() else { panic!("{outcome:?}") };
    assert_byte(value, b'a');
    assert_eq!(caller.snapshot(close.request), Ok(Status::Pending));
    assert_eq!(library.count(c"fixture_cursors_destroyed"), 0);
    caller.release(read.request).unwrap();
    completed(&mut runtime, &caller, close.request);
    assert_eq!(library.count(c"fixture_cursors_destroyed"), 1);
    runtime.close().unwrap();
}
