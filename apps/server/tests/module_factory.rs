#[path = "support/memory.rs"]
mod memory;

use reovim_host_reference::run_with;
use reovim_kernel_interface::{
    Connection, Contract, Error, Message, ResourceId,
    driver::{DriverOps, Lifecycle, Progress},
    module::{self, Factory, Module},
    provider::{Contribution, Permissions},
};
use reovim_kernel_runtime::{Limits, Runtime};
use std::{
    cell::Cell,
    rc::Rc,
    task::{Context, Waker},
};

struct Imports {
    initial: Lifecycle,
    kernel: Connection,
    constructed: Rc<Cell<Option<ResourceId>>>,
    destroyed: Rc<Cell<bool>>,
}
struct State {
    status: Lifecycle,
    destroyed: Rc<Cell<bool>>,
}
impl Drop for State {
    fn drop(&mut self) {
        assert_eq!(self.status, Lifecycle::Closed);
        self.destroyed.set(true);
    }
}
struct Code {
    destroyed: Rc<Cell<bool>>,
    released: Rc<Cell<bool>>,
}
impl Drop for Code {
    fn drop(&mut self) {
        // State destruction precedes code release.
        assert!(self.destroyed.get());
        self.released.set(true);
    }
}
fn construct(imports: &Imports, instance: ResourceId) -> Result<Module, Error> {
    // Passive metadata access proves construction is outside registry guards.
    imports.kernel.resolve("input").into_inner().map_err(|_| Error::DENIED)?;
    assert_eq!(imports.constructed.replace(Some(instance)), None);
    Ok(Module::new(
        State {
            status: imports.initial,
            destroyed: Rc::clone(&imports.destroyed),
        },
        DriverOps {
            status: |state| state.status,
            advance: |state, _| {
                if state.status == Lifecycle::Closing {
                    state.status = Lifecycle::Closed;
                    Ok(Progress::Stopped)
                } else {
                    state.status = Lifecycle::Ready;
                    Ok(Progress::Idle)
                }
            },
            close: |state| {
                state.status = Lifecycle::Closing;
                Ok(())
            },
        },
        vec![Contribution::Resource {
            path: "module/value".into(),
            provider: memory::provider(b"constructed once"),
        }],
        Rc::new(()), // Factory ownership must supply the executable pin.
    ))
}

#[test]
fn runtime_constructs_once_and_keeps_factory_code_through_cleanup() {
    let mut runtime = Runtime::new(127, Limits::default()).unwrap();
    runtime.start().unwrap();
    let input = runtime
        .register("input", memory::provider(b"in"))
        .map_err(|(error, _)| error)
        .unwrap();
    let imports = runtime.connect(&[(input, Permissions::READ)]).unwrap();
    let constructed = Rc::new(Cell::new(None));
    let destroyed = Rc::new(Cell::new(false));
    let released = Rc::new(Cell::new(false));
    let factory = Factory::new(
        Imports {
            initial: Lifecycle::Constructed,
            kernel: imports,
            constructed: Rc::clone(&constructed),
            destroyed: Rc::clone(&destroyed),
        },
        construct,
        Rc::new(Code {
            destroyed: Rc::clone(&destroyed),
            released: Rc::clone(&released),
        }),
    );
    let instance = runtime
        .activate_factory("modules/factory", factory)
        .map_err(|(error, _)| error)
        .unwrap();
    assert_eq!(constructed.get(), None);
    let client = reovim_sdk::local(
        runtime
            .connect(&[(runtime.resource("").unwrap(), Permissions::ALL)])
            .unwrap(),
    );
    let local = client.resolve("modules/factory").into_inner().unwrap();
    let mut wait = Box::pin(module::ready(&client, local));
    let mut wake = Context::from_waker(Waker::noop());
    assert!(core::future::Future::poll(wait.as_mut(), &mut wake).is_pending());
    assert_eq!(constructed.get(), None);
    run_with(wait, &mut runtime, &Runtime::DRIVER).unwrap().unwrap();
    assert_eq!(constructed.get(), Some(instance));
    assert!(!released.get());
    let value = client.resolve("module/value").into_inner().unwrap();
    let reply = run_with(
        client.read(
            value,
            &Message::new(Contract::new("resource.bytes", 1), &[]),
        ),
        &mut runtime,
        &Runtime::DRIVER,
    )
    .unwrap()
    .unwrap();
    assert_eq!(reply.data.as_ref(), b"constructed once");
    assert_eq!(runtime.close_module(instance), Err(Error::BUSY));
    assert!(!released.get());
    runtime.advance(&mut wake).unwrap();
    assert!(destroyed.get());
    assert!(released.get());
    runtime.close().unwrap();
}

#[test]
fn unstarted_factory_is_discarded_without_construction() {
    let mut runtime = Runtime::new(128, Limits::default()).unwrap();
    let factory =
        Factory::new((), |(), _| panic!("must remain inert"), Rc::new(()));
    runtime
        .activate_factory("modules/inert", factory)
        .map_err(|(error, _)| error)
        .unwrap();
    runtime.close().unwrap();
}

#[test]
fn missing_import_grant_fails_construction_without_exports() {
    let mut runtime = Runtime::new(129, Limits::default()).unwrap();
    runtime.start().unwrap();
    runtime
        .register("input", memory::provider(b"private"))
        .map_err(|(error, _)| error)
        .unwrap();
    let constructed = Rc::new(Cell::new(None));
    let factory = Factory::new(
        Imports {
            initial: Lifecycle::Constructed,
            kernel: runtime.connect(&[]).unwrap(),
            constructed: Rc::clone(&constructed),
            destroyed: Rc::new(Cell::new(false)),
        },
        construct,
        Rc::new(()),
    );
    let instance = runtime
        .activate_factory("modules/denied", factory)
        .map_err(|(error, _)| error)
        .unwrap();
    let client = runtime.connect(&[(instance, Permissions::READ)]).unwrap();
    let local = client.resolve("modules/denied").into_inner().unwrap();
    assert_eq!(
        run_with(module::ready(&client, local), &mut runtime, &Runtime::DRIVER)
            .unwrap(),
        Err(reovim_kernel_interface::RequestError::Failed(Error::DENIED))
    );
    assert_eq!(constructed.get(), None);
    assert_eq!(runtime.resource("module/value"), Err(Error::UNAVAILABLE));
    runtime.close().unwrap();
}

#[test]
fn noninert_constructor_is_rejected_but_state_still_gets_cleanup() {
    let mut runtime = Runtime::new(130, Limits::default()).unwrap();
    runtime.start().unwrap();
    let input = runtime
        .register("input", memory::provider(b"in"))
        .map_err(|(error, _)| error)
        .unwrap();
    let destroyed = Rc::new(Cell::new(false));
    let released = Rc::new(Cell::new(false));
    let factory = Factory::new(
        Imports {
            initial: Lifecycle::Ready,
            kernel: runtime.connect(&[(input, Permissions::READ)]).unwrap(),
            constructed: Rc::new(Cell::new(None)),
            destroyed: Rc::clone(&destroyed),
        },
        construct,
        Rc::new(Code {
            destroyed: Rc::clone(&destroyed),
            released: Rc::clone(&released),
        }),
    );
    let instance = runtime
        .activate_factory("modules/noninert", factory)
        .map_err(|(error, _)| error)
        .unwrap();
    let client = runtime.connect(&[(instance, Permissions::READ)]).unwrap();
    let local = client.resolve("modules/noninert").into_inner().unwrap();
    assert_eq!(
        run_with(module::ready(&client, local), &mut runtime, &Runtime::DRIVER)
            .unwrap(),
        Err(reovim_kernel_interface::RequestError::Failed(Error::PROVIDER))
    );
    assert_eq!(runtime.resource("module/value"), Err(Error::UNAVAILABLE));
    assert!(!destroyed.get());
    assert!(!released.get());
    let mut wake = Context::from_waker(Waker::noop());
    runtime.advance(&mut wake).unwrap();
    assert!(destroyed.get());
    assert!(released.get());
    runtime.close().unwrap();
}
