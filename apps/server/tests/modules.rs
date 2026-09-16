#[path = "support/memory.rs"]
mod memory;

use reovim_host_reference::run;
use reovim_kernel_interface::{
    Contract, Error, Message,
    driver::{DriverOps, Lifecycle, Progress},
    module::Module,
    provider::{Contribution, Permissions},
};
use reovim_kernel_runtime::{Limits, Runtime};
use std::{
    cell::Cell,
    rc::Rc,
    task::{Context, Waker},
};

struct State {
    status: Lifecycle,
    advances: Rc<Cell<u64>>,
    destroyed: Rc<Cell<bool>>,
}
impl Drop for State {
    fn drop(&mut self) {
        assert_eq!(self.status, Lifecycle::Closed);
        self.destroyed.set(true);
    }
}
fn advance(state: &mut State, _: &mut Context<'_>) -> Progress {
    state.advances.set(state.advances.get() + 1);
    state.status = match state.status {
        Lifecycle::Constructed => Lifecycle::Starting,
        Lifecycle::Starting => Lifecycle::Ready,
        Lifecycle::Closing => Lifecycle::Closed,
        value => value,
    };
    if state.status == Lifecycle::Closed {
        Progress::Stopped
    } else if state.status == Lifecycle::Ready {
        Progress::Idle
    } else {
        Progress::More
    }
}
const fn close(state: &mut State) {
    state.status = Lifecycle::Closing;
}
fn module(advances: Rc<Cell<u64>>, destroyed: Rc<Cell<bool>>) -> Module {
    Module::new(
        State { status: Lifecycle::Constructed, advances, destroyed },
        DriverOps {
            status: |state| state.status,
            advance: |state, wake| Ok(advance(state, wake)),
            close: |state| {
                close(state);
                Ok(())
            },
        },
        vec![Contribution::Resource {
            path: "service/value".into(),
            provider: memory::provider(b"ready value"),
        }],
        Rc::new(()),
    )
}
#[test]
fn runtime_drives_activation_and_retains_state_until_stopped() {
    let mut runtime = Runtime::new(118, Limits::default()).unwrap();
    runtime.start().unwrap();
    let advances = Rc::new(Cell::new(0));
    let destroyed = Rc::new(Cell::new(false));
    let instance = runtime
        .activate_module(
            "modules/example",
            module(Rc::clone(&advances), Rc::clone(&destroyed)),
        )
        .map_err(|(error, _)| error)
        .unwrap();
    assert_eq!(advances.get(), 0); // Adoption executes no module code.
    assert_eq!(runtime.resource("service/value"), Err(Error::UNAVAILABLE));
    let mut wake = Context::from_waker(Waker::noop());
    let observer = reovim_sdk::local(
        runtime.connect(&[(instance, Permissions::READ)]).unwrap(),
    );
    let local = observer.resolve("modules/example").into_inner().unwrap();
    let mut waiting =
        Box::pin(reovim_kernel_interface::module::ready(&observer, local));
    assert!(
        core::future::Future::poll(waiting.as_mut(), &mut wake).is_pending()
    );
    assert_eq!(advances.get(), 0); // Polling never enters the instance driver.
    run(waiting, |wake| runtime.advance(wake).map(|_| ())).unwrap().unwrap();
    let export = runtime.resource("service/value").unwrap();
    let client = runtime
        .connect(&[(instance, Permissions::READ), (export, Permissions::READ)])
        .unwrap();
    let subject = client.resolve("service/value").into_inner().unwrap();
    let reply = run(
        client.read(
            subject,
            &Message::new(Contract::new("resource.bytes", 1), &[]),
        ),
        |wake| runtime.advance(wake).map(|_| ()),
    )
    .unwrap()
    .unwrap();
    assert_eq!(reply.data.as_ref(), b"ready value");
    assert_eq!(runtime.unregister(export), Err(Error::BUSY));
    assert_eq!(runtime.close_module(instance), Err(Error::BUSY));
    assert!(!destroyed.get());
    assert_eq!(runtime.resource("service/value"), Err(Error::UNAVAILABLE));
    runtime.advance(&mut wake).unwrap();
    assert!(destroyed.get());
    let subject = client.resolve("modules/example").into_inner().unwrap();
    let reply = run(
        client.read(
            subject,
            &Message::new(Contract::new("kernel.module.status", 1), &[]),
        ),
        |wake| runtime.advance(wake).map(|_| ()),
    )
    .unwrap()
    .unwrap();
    assert_eq!(reply.data.as_ref(), b"closed");
    runtime.close_module(instance).unwrap();
    runtime.close().unwrap();
}

#[test]
fn failed_publication_retains_the_instance_through_cleanup() {
    let mut runtime = Runtime::new(119, Limits::default()).unwrap();
    runtime.start().unwrap();
    let advances = Rc::new(Cell::new(0));
    let destroyed = Rc::new(Cell::new(false));
    let mut module = module(advances, Rc::clone(&destroyed));
    let Contribution::Resource { provider, .. } = &mut module.contributions[0]
    else {
        panic!("expected Resource");
    };
    provider.surfaces[0].contract = Contract::new("", 1);
    let instance = runtime
        .activate_module("modules/invalid", module)
        .map_err(|(error, _)| error)
        .unwrap();
    let mut wake = Context::from_waker(Waker::noop());
    runtime.advance(&mut wake).unwrap();
    runtime.advance(&mut wake).unwrap();
    assert_eq!(runtime.resource("service"), Err(Error::UNAVAILABLE));
    assert!(!destroyed.get());
    runtime.advance(&mut wake).unwrap();
    assert!(!destroyed.get()); // Requesting close did not prove Stopped.
    runtime.advance(&mut wake).unwrap();
    assert!(destroyed.get());
    let client = runtime.connect(&[(instance, Permissions::READ)]).unwrap();
    let subject = client.resolve("modules/invalid").into_inner().unwrap();
    let reply = run(
        client.read(
            subject,
            &Message::new(Contract::new("kernel.module.status", 1), &[]),
        ),
        |wake| runtime.advance(wake).map(|_| ()),
    )
    .unwrap()
    .unwrap();
    let code = reply.data.strip_prefix(b"failed").unwrap();
    assert_eq!(
        reovim_kernel_interface::buffer::integer(code),
        Ok(Error::INVALID.code.0),
    );
    assert_eq!(
        run(reovim_kernel_interface::module::ready(&client, subject), |wake| {
            runtime.advance(wake).map(|_| ())
        },)
        .unwrap(),
        Err(reovim_kernel_interface::RequestError::Failed(Error::INVALID)),
    );
    runtime.close_module(instance).unwrap();
    runtime.close().unwrap();
}

#[test]
fn root_driver_cleans_an_inert_module_without_starting_the_runtime() {
    let mut runtime = Runtime::new(120, Limits::default()).unwrap();
    let destroyed = Rc::new(Cell::new(false));
    runtime
        .activate_module(
            "modules/inert",
            module(Rc::new(Cell::new(0)), Rc::clone(&destroyed)),
        )
        .map_err(|(error, _)| error)
        .unwrap();
    let driver = Runtime::DRIVER;
    assert_eq!((driver.close)(&mut runtime), Err(Error::BUSY));
    assert!(!destroyed.get());
    let mut wake = Context::from_waker(Waker::noop());
    assert_eq!(
        (driver.advance)(&mut runtime, &mut wake),
        Ok(Progress::Stopped)
    );
    assert!(destroyed.get());
}

#[test]
fn idle_instance_does_not_hide_another_instances_startup() {
    let mut runtime = Runtime::new(121, Limits::default()).unwrap();
    runtime.start().unwrap();
    runtime
        .activate_module(
            "modules/first",
            module(Rc::new(Cell::new(0)), Rc::new(Cell::new(false))),
        )
        .map_err(|(error, _)| error)
        .unwrap();
    let mut wake = Context::from_waker(Waker::noop());
    runtime.advance(&mut wake).unwrap();
    runtime.advance(&mut wake).unwrap();
    let advances = Rc::new(Cell::new(0));
    let mut second = module(Rc::clone(&advances), Rc::new(Cell::new(false)));
    let Contribution::Resource { path, .. } = &mut second.contributions[0]
    else {
        panic!("expected Resource");
    };
    *path = "service/second".into();
    runtime
        .activate_module("modules/second", second)
        .map_err(|(error, _)| error)
        .unwrap();
    assert_eq!(runtime.advance(&mut wake), Ok(Progress::More));
    assert_eq!(advances.get(), 1); // Every active driver received its turn.
    runtime.advance(&mut wake).unwrap();
    assert!(runtime.resource("service/second").is_ok());
    assert_eq!(runtime.close(), Err(Error::BUSY));
    runtime.advance(&mut wake).unwrap();
    runtime.close().unwrap();
}

#[test]
fn mixed_module_stays_usable_until_its_buffer_and_history_are_closed() {
    use reovim_host_reference::run_with;
    use reovim_kernel_interface::buffer::{self, ClosePolicy};

    let mut runtime = Runtime::new(122, Limits::default()).unwrap();
    runtime.start().unwrap();
    let destroyed = Rc::new(Cell::new(false));
    let mut editing = module(Rc::new(Cell::new(0)), Rc::clone(&destroyed));
    editing.contributions = vec![
        Contribution::Resource {
            path: "resource/notes".into(),
            provider: memory::provider(b"# a\n\xffopaque"),
        },
        Contribution::Model {
            path: "models/heading".into(),
            provider: reovim_representation_basic::heading(),
        },
    ];
    runtime
        .activate_module("modules/editing", editing)
        .map_err(|(error, _)| error)
        .unwrap();
    let root = runtime.resource("").unwrap();
    let client = reovim_sdk::local(
        runtime.connect(&[(root, Permissions::ALL)]).unwrap(),
    );
    let mut wake = Context::from_waker(Waker::noop());
    runtime.advance(&mut wake).unwrap();
    runtime.advance(&mut wake).unwrap();
    let edited = run_with(
        reovim_server::client::heading(&client),
        &mut runtime,
        &Runtime::DRIVER,
    )
    .unwrap()
    .unwrap();
    assert_eq!(edited.value.data.as_ref(), b"# b\n\xffopaque");
    let instance = client.resolve("modules/editing").into_inner().unwrap();
    assert_eq!(
        run_with(
            reovim_kernel_interface::module::unload(&client, instance),
            &mut runtime,
            &Runtime::DRIVER
        )
        .unwrap(),
        Err(reovim_kernel_interface::RequestError::Failed(Error::BUSY))
    );
    assert!(!destroyed.get());
    let source = client.resolve("resource/notes").into_inner().unwrap();
    let reply = run_with(
        client.read(
            source,
            &Message::new(Contract::new("resource.bytes", 1), &[]),
        ),
        &mut runtime,
        &Runtime::DRIVER,
    )
    .unwrap()
    .unwrap();
    assert_eq!(reply.data.as_ref(), b"# a\n\xffopaque");
    run_with(
        buffer::close(&client, edited.state, ClosePolicy::DiscardLocal),
        &mut runtime,
        &Runtime::DRIVER,
    )
    .unwrap()
    .unwrap();
    run_with(
        reovim_kernel_interface::module::unload(&client, instance),
        &mut runtime,
        &Runtime::DRIVER,
    )
    .unwrap()
    .unwrap();
    assert!(destroyed.get());
    assert_eq!(runtime.resource("models/heading"), Err(Error::UNAVAILABLE));
    assert_eq!(runtime.resource("resource/notes"), Err(Error::UNAVAILABLE));
    runtime.close().unwrap();
}

#[test]
fn stopped_driver_cannot_publish_ready_contributions() {
    let mut runtime = Runtime::new(123, Limits::default()).unwrap();
    runtime.start().unwrap();
    let destroyed = Rc::new(Cell::new(false));
    let module = Module::new(
        State {
            status: Lifecycle::Constructed,
            advances: Rc::new(Cell::new(0)),
            destroyed: Rc::clone(&destroyed),
        },
        DriverOps {
            status: |state| state.status,
            advance: |state, wake| {
                let progress = advance(state, wake);
                Ok(if state.status == Lifecycle::Ready {
                    Progress::Stopped
                } else {
                    progress
                })
            },
            close: |state| {
                close(state);
                Ok(())
            },
        },
        vec![Contribution::Resource {
            path: "invalid/export".into(),
            provider: memory::provider(b"must stay hidden"),
        }],
        Rc::new(()),
    );
    runtime
        .activate_module("modules/stopped", module)
        .map_err(|(error, _)| error)
        .unwrap();
    let mut wake = Context::from_waker(Waker::noop());
    runtime.advance(&mut wake).unwrap();
    runtime.advance(&mut wake).unwrap();
    assert_eq!(runtime.resource("invalid/export"), Err(Error::UNAVAILABLE));
    assert!(!destroyed.get());
    runtime.advance(&mut wake).unwrap();
    runtime.advance(&mut wake).unwrap();
    assert!(destroyed.get());
    runtime.close().unwrap();
}

#[test]
fn reconnect_recovers_readiness_of_the_original_instance() {
    use reovim_kernel_interface::{RequestError, requests};
    let mut runtime = Runtime::new(124, Limits::default()).unwrap();
    runtime.start().unwrap();
    let destroyed = Rc::new(Cell::new(false));
    let instance = runtime
        .activate_module(
            "modules/recovery",
            module(Rc::new(Cell::new(0)), Rc::clone(&destroyed)),
        )
        .map_err(|(error, _)| error)
        .unwrap();
    let client = reovim_sdk::local(
        runtime.connect(&[(instance, Permissions::READ)]).unwrap(),
    );
    let local = client.resolve("modules/recovery").into_inner().unwrap();
    let waiting = client.read(
        local,
        &Message::new(Contract::new("kernel.module.ready", 1), &[]),
    );
    let next = client.reconnect().unwrap();
    let error = run(waiting, |wake| runtime.advance(wake).map(|_| ()))
        .unwrap()
        .unwrap_err();
    let RequestError::Unknown { recovery, .. } = error else {
        panic!("reconnect must preserve the accepted readiness request");
    };
    let work = next.resolve(&recovery).into_inner().unwrap();
    let result = run(requests::result(&next, work), |wake| {
        runtime.advance(wake).map(|_| ())
    })
    .unwrap()
    .unwrap()
    .unwrap();
    next.close(work).into_inner().unwrap();
    assert_eq!(result.contract, Contract::new("kernel.module.ready", 1));
    assert_eq!(result.data.as_ref(), b"ready");
    assert_eq!(runtime.resource("modules/recovery"), Ok(instance));
    assert_eq!(runtime.close(), Err(Error::BUSY));
    let mut wake = Context::from_waker(Waker::noop());
    runtime.advance(&mut wake).unwrap();
    runtime.close().unwrap();
    assert!(destroyed.get());
}
