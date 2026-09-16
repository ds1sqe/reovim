use super::*;
use reovim_kernel_interface::{
    Pending,
    call::{Cancellation, Completion, PendingOps},
    declaration::Declaration as Api,
    module::{Import, ModuleContext},
    provider::{ProviderInterface, Registrar},
};

pub fn api() -> Api {
    let mut bytes = vec![0xa8; 16];
    bytes.extend_from_slice(&1_u32.to_le_bytes());
    bytes.extend_from_slice(&[
        2, 1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 5, 0, b'v', b'a', b'l', b'u', b'e', 0,
        3, 0,
    ]);
    Api::decode(&bytes).unwrap()
}
#[derive(Default)]
pub struct Observation {
    pub caller: RefCell<Option<Caller>>,
    pub value: Cell<Option<u32>>,
    pub destroyed: Cell<bool>,
    pub service: Cell<Option<ResourceId>>,
    pub reject: Cell<bool>,
    pub consumer_live: Cell<bool>,
    pub shutdown: Cell<bool>,
}
struct Service {
    phase: Lifecycle,
    exports: Registrar,
    value: u32,
    observation: Rc<Observation>,
}
#[allow(
    clippy::result_large_err,
    reason = "inert rejection returns the complete context"
)]
pub fn service(observation: &Rc<Observation>) -> Rc<Definition> {
    let observation = observation.clone();
    Rc::new(
        Definition::new(
            Declaration::new(ModuleId([1; 16]), "service", 1, 1, Box::new([]))
                .unwrap(),
            Box::new([Box::from(b"device".as_slice())]),
            1,
            Rc::new(move |context| {
                observation.service.set(Some(context.instance));
                Module::construct(
                    ModuleImplementation {
                        construct: |context: ModuleContext<(
                            Caller,
                            Rc<Observation>,
                        )>| {
                            Ok(Service {
                                phase: Lifecycle::Constructed,
                                exports: context.exports,
                                value: 37,
                                observation: context.imports.1,
                            })
                        },
                        driver: DriverOps {
                            status: |state| state.phase,
                            advance: service_advance,
                            close: |state| {
                                state.phase = Lifecycle::Closed;
                                Ok(())
                            },
                        },
                    },
                    ModuleContext {
                        instance: context.instance,
                        namespace: context.namespace,
                        scope: context.scope,
                        imports: (context.imports, observation.clone()),
                        exports: context.exports,
                    },
                    Rc::new(()),
                )
            }),
        )
        .unwrap(),
    )
}
fn service_advance(
    state: &mut Service,
    _: &mut Context<'_>,
) -> Result<Progress, Error> {
    if state.phase != Lifecycle::Constructed {
        return Ok(Progress::Idle);
    }
    let provider = ResourceProvider::new(
        (state.value, state.observation.clone()),
        ResourceOps {
            open: None,
            namespace: None,
            io: None,
            stop: |(_, observation)| {
                assert!(!observation.consumer_live.get());
                Ok(())
            },
        },
        Box::new([ProviderInterface {
            prepare: None,
            declaration: api(),
            begin: Some(begin),
            create: None,
        }]),
        Rc::new(()),
    )
    .ok()
    .unwrap();
    state.exports.stage(b"device", provider).map_err(|(error, _)| error)?;
    state.phase = Lifecycle::Ready;
    Ok(Progress::More)
}
fn begin(
    value: &mut (u32, Rc<Observation>),
    _: usize,
    _: reovim_kernel_interface::declaration::Operation,
    _: &Value,
) -> Result<Pending, Error> {
    let value = Value::copy(&value.0.to_le_bytes(), &[])?;
    Ok(Pending::new(
        Some(value),
        PendingOps {
            poll: |_, _| Ok(Poll::Ready(())),
            take: |value| {
                Ok(Completion {
                    outcome: Outcome::Ok(value.take().ok_or(Error::CLOSED)?),
                    activity: 0,
                    owner: None,
                })
            },
            cancel: |_| Ok(Cancellation::TooLate),
            reconcile: |_| Err(Error::UNSUPPORTED),
            finish: |_| Ok(()),
        },
    ))
}
struct Consumer {
    phase: Lifecycle,
    caller: Caller,
    request: Option<u64>,
    observation: Rc<Observation>,
}
impl Drop for Consumer {
    fn drop(&mut self) {
        // The imported grant must still be live during the consumer destructor.
        if !self.observation.shutdown.get() {
            acquire(&self.caller, b"/imports/value").unwrap();
        }
        self.observation.consumer_live.set(false);
        self.observation.destroyed.set(true);
    }
}
#[allow(
    clippy::result_large_err,
    reason = "inert rejection returns the complete context"
)]
pub fn consumer(
    observation: &Rc<Observation>,
    requested: Api,
    path: &[u8],
) -> Rc<Definition> {
    let mut dependency = dependency(1, 1, DependencyScope::Global, false);
    dependency.imports =
        Box::new([Import::new("value", path, requested, 1, 0, false).unwrap()]);
    let observation = observation.clone();
    Rc::new(
        Definition::new(
            Declaration::new(
                ModuleId([2; 16]),
                "consumer",
                1,
                1,
                Box::new([dependency]),
            )
            .unwrap(),
            Box::new([]),
            0,
            Rc::new(move |context| {
                assert_eq!(
                    acquire(&context.imports, b"/imports/value"),
                    Err(Error::NOT_READY)
                );
                observation.caller.replace(Some(context.imports.clone()));
                if observation.reject.get() {
                    context.imports.reserve().unwrap();
                    return Err(Error::PROVIDER);
                }
                Module::construct(
                    ModuleImplementation {
                        construct: |context: ModuleContext<(
                            Caller,
                            Rc<Observation>,
                        )>| {
                            context.imports.1.consumer_live.set(true);
                            Ok(Consumer {
                                phase: Lifecycle::Constructed,
                                caller: context.imports.0,
                                request: None,
                                observation: context.imports.1,
                            })
                        },
                        driver: DriverOps {
                            status: |state| state.phase,
                            advance: consumer_advance,
                            close: |state| {
                                state.phase = Lifecycle::Closed;
                                Ok(())
                            },
                        },
                    },
                    ModuleContext {
                        instance: context.instance,
                        namespace: context.namespace,
                        scope: context.scope,
                        imports: (context.imports, observation.clone()),
                        exports: context.exports,
                    },
                    Rc::new(()),
                )
            }),
        )
        .unwrap(),
    )
}
fn consumer_advance(
    state: &mut Consumer,
    _: &mut Context<'_>,
) -> Result<Progress, Error> {
    if state.phase == Lifecycle::Constructed {
        return start(state);
    }
    let Some(request) = state.request else { return Ok(Progress::Idle) };
    let Status::Outcome(outcome) = state.caller.snapshot(request)? else {
        return Ok(Progress::Idle);
    };
    let Outcome::Ok(value) = outcome.as_ref() else {
        return Err(Error::PROVIDER);
    };
    state
        .observation
        .value
        .set(Some(u32::from_le_bytes(value.data.as_ref().try_into().unwrap())));
    state.caller.adopt(request)?;
    state.caller.release(request)?;
    state.request = None;
    state.phase = Lifecycle::Ready;
    Ok(Progress::More)
}
fn start(state: &mut Consumer) -> Result<Progress, Error> {
    let access = acquire(&state.caller, b"/imports/value")?;
    let denied = call(
        &state.caller,
        Operation::SetAccess,
        access,
        0,
        &4_u32.to_le_bytes(),
    );
    assert_eq!(state.caller.submit(denied.clone()), Err(Error::DENIED));
    state.caller.release(denied.request)?;
    common(&state.caller, Operation::SetAccess, access, &1_u32.to_le_bytes())?;
    let definition = api();
    let mut encoded =
        u32::try_from(definition.bytes().len()).unwrap().to_le_bytes().to_vec();
    encoded.extend_from_slice(definition.bytes());
    let access =
        common(&state.caller, Operation::Bind, access, &encoded)?.references[0];
    let request = call(&state.caller, Operation::Ioctl, access, 1, &[]);
    state.request = Some(request.request);
    state.caller.submit(request)?;
    state.phase = Lifecycle::Starting;
    Ok(Progress::More)
}
pub fn acquire(caller: &Caller, path: &[u8]) -> Result<AccessId, Error> {
    let mut locator = 0_u32.to_le_bytes().to_vec();
    locator
        .extend_from_slice(&u32::try_from(path.len()).unwrap().to_le_bytes());
    locator.extend_from_slice(path);
    Ok(common(caller, Operation::Acquire, AccessId(0), &locator)?.references[0])
}
fn common(
    caller: &Caller,
    operation: Operation,
    subject: AccessId,
    data: &[u8],
) -> Result<Value, Error> {
    let request = call(caller, operation, subject, 0, data);
    if let Err(error) = caller.submit(request.clone()) {
        caller.release(request.request)?;
        return Err(error);
    }
    let Status::Outcome(outcome) = caller.snapshot(request.request)? else {
        return Err(Error::NOT_READY);
    };
    let Outcome::Ok(value) = outcome.as_ref() else {
        return Err(Error::PROVIDER);
    };
    caller.adopt(request.request)?;
    caller.release(request.request)?;
    Ok(value.clone())
}
