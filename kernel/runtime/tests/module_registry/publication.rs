use super::*;
use reovim_kernel_interface::{
    Outcome, Value,
    caller::{Call, Operation, Status},
    resource::AccessId,
};
use reovim_kernel_runtime::{Grant, PublicationGrant};

const fn grant(namespace: ResourceId) -> PublicationGrant {
    PublicationGrant {
        namespace,
        scope: ModuleScope::Global,
        access: 1,
        create_access: 0,
        retain_loans: false,
    }
}
fn resource(runtime: &mut Runtime, path: &[u8]) -> ResourceId {
    let provider = ResourceProvider::new(
        (),
        ResourceOps {
            open: None,
            namespace: None,
            io: None,
            stop: |()| Ok(()),
        },
        Box::new([]),
        Rc::new(()),
    )
    .ok()
    .unwrap();
    runtime.register(path, provider, None).ok().unwrap()
}
fn call(
    binding: &Binding,
    operation: Operation,
    subject: AccessId,
    input: Value,
) -> Result<Value, Error> {
    let caller = binding.caller();
    let request = caller.reserve()?;
    if let Err(error) = caller.submit(Call {
        request,
        operation,
        subject,
        command: 0,
        expected: None,
        input,
    }) {
        caller.release(request)?;
        return Err(error);
    }
    let Status::Outcome(outcome) =
        caller.result(request, &mut Context::from_waker(Waker::noop()))?
    else {
        panic!("common call pending");
    };
    let Outcome::Ok(value) = outcome.as_ref() else {
        panic!("call failed: {outcome:?}")
    };
    let value = value.clone();
    caller.adopt(request)?;
    caller.release(request)?;
    Ok(value)
}
fn acquire(binding: &Binding, identity: ResourceId) -> Result<Value, Error> {
    let mut bytes = 1_u32.to_le_bytes().to_vec();
    bytes.extend_from_slice(&identity.epoch);
    bytes.extend_from_slice(&identity.serial.to_le_bytes());
    call(
        binding,
        Operation::Acquire,
        AccessId(0),
        Value::copy(&bytes, &[]).unwrap(),
    )
}
#[allow(clippy::result_large_err, reason = "inert rejection returns context")]
fn published(
    runtime: &mut Runtime,
    namespace: ResourceId,
    parent: &Binding,
) -> (ResourceId, ResourceId, reovim_kernel_interface::caller::Caller) {
    let captured = Rc::new(std::cell::RefCell::new(None));
    let observed = captured.clone();
    let definition = Definition::new(
        Declaration::new(ModuleId([0xe1; 16]), "published", 1, 1, Box::new([]))
            .unwrap(),
        vec![Box::<[u8]>::from(&b"member"[..])].into_boxed_slice(),
        1,
        Rc::new(move |context| {
            *observed.borrow_mut() = Some(context.imports.clone());
            Module::construct(
                ModuleImplementation {
                    construct: |context| {
                        Ok((Lifecycle::Constructed, context.exports))
                    },
                    driver: DriverOps {
                        status: |(phase, _)| *phase,
                        advance: |state, _| publish(state),
                        close: |(phase, _)| {
                            *phase = Lifecycle::Closed;
                            Ok(())
                        },
                    },
                },
                context,
                Rc::new(()),
            )
        }),
    )
    .unwrap();
    runtime
        .register_module(namespace, namespace, Rc::new(definition), parent, &[])
        .unwrap();
    let selection = runtime
        .select_module(namespace, "published", 1, ModuleScope::Global)
        .unwrap();
    let instance = runtime.activate_selected(&selection.modules()[0]).unwrap();
    for _ in 0..4 {
        runtime.advance(&mut Context::from_waker(Waker::noop())).unwrap();
    }
    let caller = captured.borrow_mut().take().unwrap();
    (instance, runtime.module_exports(instance).unwrap()[0], caller)
}
fn publish(
    state: &mut (Lifecycle, reovim_kernel_interface::provider::Registrar),
) -> Result<Progress, Error> {
    if state.0 == Lifecycle::Constructed {
        state.0 = Lifecycle::Starting;
        return Ok(Progress::More);
    }
    if state.0 == Lifecycle::Starting {
        let provider = ResourceProvider::new(
            (),
            ResourceOps {
                open: None,
                namespace: None,
                io: None,
                stop: |()| Ok(()),
            },
            Box::new([]),
            Rc::new(()),
        )
        .map_err(|(error, ())| error)?;
        state.1.stage(b"member", provider).map_err(|(error, _)| error)?;
        state.0 = Lifecycle::Ready;
    }
    Ok(Progress::Idle)
}

#[test]
fn publication_is_recorded_membership_not_path_ancestry_or_scope_guessing() {
    let (mut runtime, root, parent) = base();
    let recipient = runtime.bind(&[]).unwrap();
    runtime.grant_publication(&recipient, grant(root)).unwrap();
    let impostor = resource(&mut runtime, b"/catalog/not-a-member");
    assert_eq!(acquire(&recipient, root), Err(Error::DENIED));
    assert_eq!(acquire(&recipient, impostor), Err(Error::DENIED));
    runtime.grant_publication(&parent, grant(root)).unwrap();
    let (instance, member, inherited) = published(&mut runtime, root, &parent);
    check_inherited(&mut runtime, &inherited, member);
    let wrong_scope = runtime.bind(&[]).unwrap();
    runtime
        .grant_publication(
            &wrong_scope,
            PublicationGrant {
                scope: ModuleScope::Session(root),
                ..grant(root)
            },
        )
        .unwrap();
    assert_eq!(acquire(&wrong_scope, member), Err(Error::DENIED));
    let value = acquire(&recipient, member).unwrap();
    let handle = value.references[0];
    assert_eq!(
        call(
            &recipient,
            Operation::SetAccess,
            handle,
            Value::copy(&2_u32.to_le_bytes(), &[]).unwrap()
        ),
        Err(Error::DENIED)
    );
    call(
        &recipient,
        Operation::SetAccess,
        handle,
        Value::copy(&1_u32.to_le_bytes(), &[]).unwrap(),
    )
    .unwrap();
    assert_eq!(runtime.unload_module(instance), Err(Error::BUSY));
    runtime.advance(&mut Context::from_waker(Waker::noop())).unwrap();
    runtime.retire_binding(&recipient).unwrap();
    assert_eq!(runtime.unload_module(instance), Ok(Lifecycle::Closing));
    for _ in 0..4 {
        runtime.advance(&mut Context::from_waker(Waker::noop())).unwrap();
    }
    runtime.close().unwrap();
}

#[test]
fn publication_grants_validate_owner_and_freeze_registered_imports() {
    let (mut runtime, root, parent) = base();
    let recipient = runtime.bind(&[]).unwrap();
    let (mut foreign, _, _) = base();
    assert_eq!(
        foreign.grant_publication(&recipient, grant(root)),
        Err(Error::DENIED)
    );
    runtime.grant_publication(&recipient, grant(root)).unwrap();
    runtime.grant_publication(&recipient, grant(root)).unwrap();
    assert_eq!(
        runtime.grant_publication(
            &recipient,
            PublicationGrant { access: 7, ..grant(root) }
        ),
        Err(Error::CONFLICT)
    );
    register(
        &mut runtime,
        root,
        &parent,
        definition(1, "empty", 1, 1, vec![], &Rc::new(Cell::new(0))),
    );
    assert_eq!(
        runtime.grant_publication(&parent, grant(root)),
        Err(Error::BUSY)
    );
    runtime
        .grant(
            &recipient,
            Grant {
                resource: root,
                access: 0,
                create_access: 0,
                retain_loans: false,
            },
        )
        .unwrap();
    assert!(acquire(&recipient, root).is_ok());
    runtime.advance(&mut Context::from_waker(Waker::noop())).unwrap();
    runtime.retire_binding(&recipient).unwrap();
    assert_eq!(
        runtime.grant_publication(&recipient, grant(root)),
        Err(Error::CLOSED)
    );
    runtime.close().unwrap();
    foreign.close().unwrap();
}

#[test]
fn ordinary_and_publication_grants_share_the_configured_budget() {
    let (mut runtime, root, _) =
        base_limits(Limits { resources: 2, ..Limits::default() });
    let recipient = runtime
        .bind(&[Grant {
            resource: root,
            access: 0,
            create_access: 0,
            retain_loans: false,
        }])
        .unwrap();
    runtime.grant_publication(&recipient, grant(root)).unwrap();
    runtime.grant_publication(&recipient, grant(root)).unwrap();
    assert_eq!(
        runtime.grant_publication(
            &recipient,
            PublicationGrant {
                scope: ModuleScope::Session(root),
                ..grant(root)
            }
        ),
        Err(Error::PRESSURE)
    );
    let other = resource(&mut runtime, b"/other");
    assert_eq!(
        runtime.grant(
            &recipient,
            Grant {
                resource: other,
                access: 0,
                create_access: 0,
                retain_loans: false,
            }
        ),
        Err(Error::PRESSURE)
    );
    runtime.close().unwrap();
}

fn check_inherited(
    runtime: &mut Runtime,
    caller: &reovim_kernel_interface::caller::Caller,
    member: ResourceId,
) {
    let mut bytes = 1_u32.to_le_bytes().to_vec();
    bytes.extend_from_slice(&member.epoch);
    bytes.extend_from_slice(&member.serial.to_le_bytes());
    let request = caller.reserve().unwrap();
    caller
        .submit(Call {
            request,
            operation: Operation::Acquire,
            subject: AccessId(0),
            command: 0,
            expected: None,
            input: Value::copy(&bytes, &[]).unwrap(),
        })
        .unwrap();
    let mut cx = Context::from_waker(Waker::noop());
    let Status::Outcome(result) = caller.result(request, &mut cx).unwrap()
    else {
        panic!("acquisition pending");
    };
    let Outcome::Ok(value) = result.as_ref() else {
        panic!("inherited publication grant missing");
    };
    let access = value.references[0];
    caller.adopt(request).unwrap();
    caller.release(request).unwrap();
    drop(result);
    let close = caller.reserve().unwrap();
    caller
        .submit(Call {
            request: close,
            operation: Operation::Close,
            subject: access,
            command: 0,
            expected: None,
            input: Value::unit(),
        })
        .unwrap();
    runtime.advance(&mut cx).unwrap();
    assert!(matches!(
        caller.result(close, &mut cx).unwrap(),
        Status::Outcome(_)
    ));
    caller.release(close).unwrap();
    runtime.advance(&mut cx).unwrap();
}
