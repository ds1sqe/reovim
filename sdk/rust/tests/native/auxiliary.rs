use super::*;

fn request(host: &Harness, mode: u32) -> Call {
    let mut request = host.make(mode, false);
    request.command = 8;
    request
}

fn tokens(host: &Harness) -> [AccessId; 2] {
    let token: unsafe extern "C" fn(u32) -> u64 =
        unsafe { host.library.symbol(c"fixture_auxiliary_token") };
    [AccessId(unsafe { token(0) }), AccessId(unsafe { token(1) })]
}

fn deny_provisional(host: &Harness) {
    let caller = host.imports.as_ref().unwrap();
    for token in tokens(host) {
        let request = call(
            caller,
            Operation::SetAccess,
            token,
            Value::copy(&0_u32.to_le_bytes(), &[]).unwrap(),
        );
        assert_eq!(caller.submit(request.clone()), Err(Error::DENIED));
        caller.release(request.request).unwrap();
    }
}

#[test]
fn native_auxiliaries_commit_with_primary_and_have_independent_access() {
    let mut host = Harness::capacity(4, true);
    let cx = &mut Context::from_waker(Waker::noop());
    let cancelled = request(&host, 0);
    host.caller.submit(cancelled.clone()).unwrap();
    host.caller.cancel(cancelled.request).unwrap();
    host.caller.release(cancelled.request).unwrap();
    host.runtime.advance(cx).unwrap();
    assert_eq!(host.library.count(c"fixture_created"), 0);
    let call = request(&host, 0);
    host.caller.submit(call.clone()).unwrap();
    let overflow = request(&host, 0);
    assert_eq!(host.caller.submit(overflow.clone()), Err(Error::PRESSURE));
    host.caller.release(overflow.request).unwrap();
    host.runtime.advance(cx).unwrap();
    host.runtime.advance(cx).unwrap();
    deny_provisional(&host);
    let result = outcome(&host.caller, call.request, cx);
    let Outcome::Ok(value) = result.as_ref() else { panic!("batch failed") };
    assert_eq!(
        value.data.as_ref(),
        &[2, 0, 0, 0, 2, 0, 0, 0, 0, 0, 0, 0, 1, 0, 0, 0]
    );
    host.caller.adopt(call.request).unwrap();
    host.caller.release(call.request).unwrap();
    assert_eq!(read_value(&mut host, value.references[0], cx), 0x00ab_c000);
    assert_eq!(read_value(&mut host, value.references[1], cx), 0x00ab_c001);
    read_child(&mut host, value.references[2], cx);
    host.runtime.advance(cx).unwrap();
    host.runtime.close().unwrap();
    assert_eq!(host.library.count(c"fixture_created"), 1);
    assert_eq!(host.library.count(c"fixture_child_destroyed"), 3);
    assert_eq!(host.library.count(c"fixture_factory_destroyed"), 1);
}

#[test]
fn unreturned_auxiliary_drains_without_publishing_or_losing_batch_cleanup() {
    let mut host = Harness::capacity(4, true);
    let cx = &mut Context::from_waker(Waker::noop());
    let call = request(&host, 6);
    host.caller.submit(call.clone()).unwrap();
    host.runtime.advance(cx).unwrap();
    host.runtime.advance(cx).unwrap();
    let result = outcome(&host.caller, call.request, cx);
    let Outcome::Ok(value) = result.as_ref() else { panic!("batch failed") };
    assert_eq!(value.data.as_ref(), &[1, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0]);
    host.cleanup_ready(false);
    host.caller.adopt(call.request).unwrap();
    host.caller.release(call.request).unwrap();
    host.runtime.advance(cx).unwrap();
    // The factory request is joined before initializing its private results;
    // unused staged subjects retain their separate cleanup custody.
    assert_eq!(host.library.count(c"fixture_finished"), 1);
    assert_eq!(host.library.count(c"fixture_child_destroyed"), 0);
    deny_provisional(&host);
    host.cleanup_ready(true);
    host.runtime.advance(cx).unwrap();
    assert_eq!(host.library.count(c"fixture_child_destroyed"), 1);
    assert_eq!(read_value(&mut host, value.references[0], cx), 0x00ab_c000);
    read_child(&mut host, value.references[1], cx);
    host.runtime.advance(cx).unwrap();
    host.runtime.close().unwrap();
    assert_eq!(host.library.count(c"fixture_child_destroyed"), 3);
}

#[test]
fn rejected_auxiliary_batch_cleans_every_staged_native_owner() {
    for mode in [8, 11] {
        let mut host = Harness::capacity(4, true);
        let cx = &mut Context::from_waker(Waker::noop());
        let call = request(&host, mode);
        host.caller.submit(call.clone()).unwrap();
        host.runtime.advance(cx).unwrap();
        host.runtime.advance(cx).unwrap();
        let cause =
            if mode == 8 { Error::UNAVAILABLE } else { Error::PROVIDER };
        assert_eq!(
            outcome(&host.caller, call.request, cx).as_ref(),
            &Outcome::Failed(cause)
        );
        host.caller.release(call.request).unwrap();
        host.runtime.advance(cx).unwrap();
        host.runtime.close().unwrap();
        assert_eq!(
            host.library.count(c"fixture_child_destroyed"),
            if mode == 8 { 3 } else { 2 }
        );
        assert_eq!(host.library.count(c"fixture_factory_destroyed"), 1);
    }
}

#[test]
fn unknown_auxiliary_batch_retains_original_staging_until_reconciliation() {
    let mut host = Harness::capacity(4, true);
    let cx = &mut Context::from_waker(Waker::noop());
    let call = request(&host, 9);
    host.caller.submit(call.clone()).unwrap();
    host.runtime.advance(cx).unwrap();
    host.runtime.advance(cx).unwrap();
    let unknown = outcome(&host.caller, call.request, cx);
    assert!(matches!(unknown.as_ref(), Outcome::Unknown { .. }));
    deny_provisional(&host);
    host.caller.submit(call.clone()).unwrap();
    host.caller.reconcile(call.request).unwrap();
    host.caller.release(call.request).unwrap();
    host.runtime.advance(cx).unwrap();
    host.runtime.advance(cx).unwrap();
    host.runtime.close().unwrap();
    assert_eq!(host.library.count(c"fixture_created"), 1);
    assert_eq!(host.library.count(c"fixture_child_destroyed"), 3);
    assert!(matches!(unknown.as_ref(), Outcome::Unknown { .. }));
}

#[test]
fn empty_auxiliary_result_releases_all_unreturned_native_resources() {
    let mut host = Harness::capacity(4, true);
    let cx = &mut Context::from_waker(Waker::noop());
    let call = request(&host, 7);
    host.caller.submit(call.clone()).unwrap();
    host.runtime.advance(cx).unwrap();
    host.runtime.advance(cx).unwrap();
    let result = outcome(&host.caller, call.request, cx);
    let Outcome::Ok(value) = result.as_ref() else { panic!("batch failed") };
    assert_eq!(value.data.as_ref(), &[0; 8]);
    host.caller.adopt(call.request).unwrap();
    host.caller.release(call.request).unwrap();
    host.runtime.advance(cx).unwrap();
    assert_eq!(host.library.count(c"fixture_child_destroyed"), 2);
    read_child(&mut host, value.references[0], cx);
    host.runtime.advance(cx).unwrap();
    host.runtime.close().unwrap();
    assert_eq!(host.library.count(c"fixture_child_destroyed"), 3);
}

#[test]
fn invalid_auxiliary_output_retains_every_unpublished_provider_and_result() {
    let mut host = Harness::capacity(4, true);
    let cx = &mut Context::from_waker(Waker::noop());
    let call = request(&host, 10);
    host.caller.submit(call.clone()).unwrap();
    host.runtime.advance(cx).unwrap();
    host.runtime.advance(cx).unwrap();
    assert_eq!(
        outcome(&host.caller, call.request, cx).as_ref(),
        &Outcome::Unknown { error: Error::PROVIDER, progress: None }
    );
    assert_eq!(host.library.count(c"fixture_released"), 0);
    assert_eq!(host.library.count(c"fixture_child_destroyed"), 0);
    host.caller.submit(call.clone()).unwrap();
    assert_eq!(host.library.count(c"fixture_created"), 1);
    host.caller.release(call.request).unwrap();
    assert_eq!(host.runtime.close(), Err(Error::INCOMPLETE));
}
