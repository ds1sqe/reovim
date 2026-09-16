use super::*;
use core::ffi::CStr;
use reovim_kernel_interface::call::Cancellation;

fn mask(host: &Harness, symbol: &CStr, value: u32) {
    let set: unsafe extern "C" fn(u32) = unsafe { host.library.symbol(symbol) };
    unsafe { set(value) };
}

fn result(host: &mut Harness, request: &Call) -> Rc<Outcome> {
    let cx = &mut Context::from_waker(Waker::noop());
    for _ in 0..24 {
        host.runtime.advance(cx).unwrap();
        if let Status::Outcome(value) =
            host.caller.result(request.request, cx).unwrap()
        {
            return value;
        }
    }
    panic!("factory did not settle")
}

fn batch(host: &Harness, mode: u32) -> Call {
    let mut request = host.make(mode, false);
    request.command = 8;
    host.caller.submit(request.clone()).unwrap();
    request
}

fn read_batch(host: &mut Harness, request: &Call, value: &Value) {
    let cx = &mut Context::from_waker(Waker::noop());
    host.caller.adopt(request.request).unwrap();
    host.caller.release(request.request).unwrap();
    assert_eq!(read_value(host, value.references[0], cx), 0x00ab_c100);
    assert_eq!(read_value(host, value.references[1], cx), 0x00ab_c101);
    assert_eq!(read_value(host, value.references[2], cx), 0xfeed_1334);
    host.runtime.advance(cx).unwrap();
    host.runtime.close().unwrap();
    assert_eq!(host.library.count(c"fixture_child_destroyed"), 3);
    assert_eq!(host.library.count(c"fixture_factory_destroyed"), 1);
}

#[test]
fn factory_initializes_primary_and_returned_auxiliary_access_together() {
    let mut host = Harness::source(4, true, "factory_open.c");
    let request = batch(&host, 0);
    let outcome = result(&mut host, &request);
    let Outcome::Ok(value) = outcome.as_ref() else { panic!("{outcome:?}") };
    read_batch(&mut host, &request, value);
    assert_eq!(host.library.count(c"fixture_created"), 1);
    assert_eq!(host.library.count(c"fixture_primary_opens"), 1);
    assert_eq!(host.library.count(c"fixture_first_opens"), 1);
    assert_eq!(host.library.count(c"fixture_second_opens"), 1);
    assert_eq!(host.library.count(c"fixture_opened_destroyed"), 3);
}

#[test]
fn failed_auxiliary_open_retries_only_that_initialization_after_cleanup() {
    let mut host = Harness::source(4, true, "factory_open.c");
    mask(&host, c"fixture_fail_mask", 4);
    let request = batch(&host, 0);
    let unknown = result(&mut host, &request);
    assert_eq!(
        unknown.as_ref(),
        &Outcome::Unknown { error: Error::UNAVAILABLE, progress: None }
    );
    assert_eq!(host.caller.cancel(request.request), Ok(Cancellation::TooLate));
    host.caller.submit(request.clone()).unwrap();
    host.cleanup_ready(false);
    assert_eq!(host.caller.reconcile(request.request), Err(Error::BUSY));
    assert_eq!(host.library.count(c"fixture_opened_destroyed"), 0);
    mask(&host, c"fixture_fail_mask", 0);
    host.cleanup_ready(true);
    host.caller.reconcile(request.request).unwrap();
    let outcome = result(&mut host, &request);
    let Outcome::Ok(value) = outcome.as_ref() else { panic!("{outcome:?}") };
    read_batch(&mut host, &request, value);
    assert_eq!(host.library.count(c"fixture_created"), 1);
    assert_eq!(host.library.count(c"fixture_primary_opens"), 1);
    assert_eq!(host.library.count(c"fixture_first_opens"), 1);
    assert_eq!(host.library.count(c"fixture_second_opens"), 2);
    assert_eq!(host.library.count(c"fixture_opened_destroyed"), 4);
}

#[test]
fn uncertain_auxiliary_open_reconciles_original_pending_without_reopening() {
    let mut host = Harness::source(4, true, "factory_open.c");
    mask(&host, c"fixture_unknown_mask", 2);
    let request = batch(&host, 0);
    assert!(matches!(
        result(&mut host, &request).as_ref(),
        Outcome::Unknown { .. }
    ));
    assert_eq!(host.library.count(c"fixture_second_opens"), 0);
    host.caller.reconcile(request.request).unwrap();
    let outcome = result(&mut host, &request);
    let Outcome::Ok(value) = outcome.as_ref() else { panic!("{outcome:?}") };
    read_batch(&mut host, &request, value);
    assert_eq!(host.library.count(c"fixture_created"), 1);
    assert_eq!(host.library.count(c"fixture_first_opens"), 1);
}

#[test]
fn missing_selected_result_api_rolls_back_all_opened_contexts() {
    let mut host = Harness::source(4, true, "factory_open.c");
    mask(&host, c"fixture_omit_mask", 4);
    let request = batch(&host, 0);
    assert_eq!(
        result(&mut host, &request).as_ref(),
        &Outcome::Failed(Error::UNSUPPORTED)
    );
    host.cleanup_ready(false);
    host.caller.release(request.request).unwrap();
    let cx = &mut Context::from_waker(Waker::noop());
    host.runtime.advance(cx).unwrap();
    assert_eq!(host.library.count(c"fixture_child_destroyed"), 0);
    host.cleanup_ready(true);
    for _ in 0..4 {
        host.runtime.advance(cx).unwrap();
    }
    host.runtime.close().unwrap();
    assert_eq!(host.library.count(c"fixture_opened_destroyed"), 3);
    assert_eq!(host.library.count(c"fixture_child_destroyed"), 3);
}

#[test]
fn unreturned_auxiliary_is_destroyed_without_opening_an_access() {
    let mut host = Harness::source(4, true, "factory_open.c");
    let request = batch(&host, 6);
    let outcome = result(&mut host, &request);
    let Outcome::Ok(value) = outcome.as_ref() else { panic!("{outcome:?}") };
    let cx = &mut Context::from_waker(Waker::noop());
    host.caller.adopt(request.request).unwrap();
    host.caller.release(request.request).unwrap();
    assert_eq!(read_value(&mut host, value.references[0], cx), 0x00ab_c100);
    assert_eq!(read_value(&mut host, value.references[1], cx), 0xfeed_1334);
    host.runtime.advance(cx).unwrap();
    host.runtime.close().unwrap();
    assert_eq!(host.library.count(c"fixture_second_opens"), 0);
    assert_eq!(host.library.count(c"fixture_opened_destroyed"), 2);
    assert_eq!(host.library.count(c"fixture_child_destroyed"), 3);
}

#[test]
fn released_uncertain_batch_finishes_without_exposing_or_leaking_access() {
    let mut host = Harness::source(4, true, "factory_open.c");
    mask(&host, c"fixture_unknown_mask", 2);
    let request = batch(&host, 0);
    assert!(matches!(
        result(&mut host, &request).as_ref(),
        Outcome::Unknown { .. }
    ));
    host.caller.reconcile(request.request).unwrap();
    host.caller.release(request.request).unwrap();
    let cx = &mut Context::from_waker(Waker::noop());
    for _ in 0..4 {
        host.runtime.advance(cx).unwrap();
    }
    host.runtime.close().unwrap();
    assert_eq!(host.library.count(c"fixture_created"), 1);
    assert_eq!(host.library.count(c"fixture_opened_destroyed"), 3);
    assert_eq!(host.library.count(c"fixture_child_destroyed"), 3);
}
