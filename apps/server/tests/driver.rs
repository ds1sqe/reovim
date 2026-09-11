use reovim_host_reference::run_with;
use reovim_kernel_interface::{
    Contract, Error, Message, RequestError,
    driver::{Lifecycle, Progress},
    provider::Permissions,
};
use reovim_kernel_runtime::{Limits, Runtime};
use std::task::{Context, Waker};

#[test]
fn interface_driver_starts_executes_and_retains_results_while_closing() {
    let mut runtime =
        Runtime::new(116, Limits::default(), reovim_kernel_core::create)
            .unwrap();
    let source = runtime
        .register("source", reovim_resource_local::memory(b"\xffretained"))
        .map_err(|(error, _)| error)
        .unwrap();
    let client = reovim_sdk::local(
        runtime.connect(&[(source, Permissions::READ)]).unwrap(),
    );
    let driver = Runtime::DRIVER;
    let mut context = Context::from_waker(Waker::noop());
    assert_eq!((driver.status)(&runtime), Lifecycle::Constructed);
    assert_eq!(
        client.resolve("source").into_inner(),
        Err(RequestError::Rejected(Error::NOT_READY))
    );
    assert_eq!(
        (driver.advance)(&mut runtime, &mut context),
        Ok(Progress::More)
    );
    assert_eq!((driver.status)(&runtime), Lifecycle::Ready);
    let subject = client.resolve("source").into_inner().unwrap();
    let input = Message::new(Contract::new("resource.bytes", 1), &[]);
    let waiting = client.read(subject, &input);
    assert_eq!((driver.close)(&mut runtime), Err(Error::BUSY));
    assert_eq!((driver.status)(&runtime), Lifecycle::Closing);
    assert_eq!(
        run_with(client.read(subject, &input), &mut runtime, &driver).unwrap(),
        Err(RequestError::Rejected(Error::CLOSED))
    );
    // Execution can finish, but an unobserved result still prevents Stopped.
    (driver.advance)(&mut runtime, &mut context).unwrap();
    (driver.advance)(&mut runtime, &mut context).unwrap();
    assert_eq!((driver.status)(&runtime), Lifecycle::Closing);
    let reply = run_with(waiting, &mut runtime, &driver).unwrap().unwrap();
    assert_eq!(reply.data.as_ref(), b"\xffretained");
    assert_eq!(
        (driver.advance)(&mut runtime, &mut context),
        Ok(Progress::Stopped)
    );
    assert_eq!((driver.status)(&runtime), Lifecycle::Closed);
    (driver.close)(&mut runtime).unwrap();
    assert_eq!(
        (driver.advance)(&mut runtime, &mut context),
        Ok(Progress::Stopped)
    );
}

#[test]
fn driver_can_close_an_owner_that_never_started() {
    let mut runtime =
        Runtime::new(117, Limits::default(), reovim_kernel_core::create)
            .unwrap();
    let driver = Runtime::DRIVER;
    (driver.close)(&mut runtime).unwrap();
    let mut context = Context::from_waker(Waker::noop());
    assert_eq!(
        (driver.advance)(&mut runtime, &mut context),
        Ok(Progress::Stopped)
    );
    assert_eq!((driver.status)(&runtime), Lifecycle::Closed);
    assert_eq!(runtime.start(), Err(Error::CLOSED));
}
