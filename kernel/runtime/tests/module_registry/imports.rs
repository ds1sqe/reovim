use super::control::call;
use super::*;
use reovim_kernel_interface::{
    Outcome, Value,
    caller::{Caller, Operation, Status},
    resource::AccessId,
};
use std::{cell::RefCell, task::Poll};
#[path = "imports/fixture.rs"]
mod fixture;
use fixture::*;

fn drive(runtime: &mut Runtime, turns: usize) {
    for _ in 0..turns {
        runtime.advance(&mut Context::from_waker(Waker::noop())).unwrap();
    }
}
fn activate(
    runtime: &mut Runtime,
    root: ResourceId,
) -> reovim_kernel_runtime::ModuleProbe {
    let selected = runtime
        .select_module(root, "consumer", 1, ModuleScope::Global)
        .unwrap();
    runtime.probe_modules(selected).unwrap()
}
#[test]
fn imported_calls_retain_grants_through_destructor_and_receipts() {
    let (mut runtime, root, parent) =
        base_limits(Limits { callers: 3, ..Limits::default() });
    let observed = Rc::new(Observation::default());
    register(&mut runtime, root, &parent, service(&observed));
    runtime
        .register_module(
            root,
            root,
            consumer(&observed, api(), b"device"),
            &parent,
            &["value"],
        )
        .unwrap();
    let probe = activate(&mut runtime, root);
    drive(&mut runtime, 12);
    let Poll::Ready(Ok(instance)) = probe.status() else {
        panic!("consumer did not become Ready");
    };
    assert_eq!(observed.value.get(), Some(37));
    let imported = observed.caller.borrow().as_ref().unwrap().clone();
    assert_eq!(
        acquire(&parent.caller(), b"/imports/value"),
        Err(Error::DENIED)
    );
    assert_eq!(runtime.retire_binding(&parent), Err(Error::BUSY));
    assert!(matches!(runtime.bind(&[]), Err(Error::PRESSURE)));
    assert_eq!(
        runtime.unload_module(observed.service.get().unwrap()),
        Err(Error::BUSY)
    );
    let retained = imported.reserve().unwrap();
    assert_eq!(runtime.unload_module(instance), Ok(Lifecycle::Closing));
    drive(&mut runtime, 6);
    assert!(observed.destroyed.get());
    assert_eq!(runtime.module_status(instance), Ok(Lifecycle::Closing));
    assert_eq!(
        runtime.unload_module(observed.service.get().unwrap()),
        Err(Error::BUSY)
    );
    assert_eq!(runtime.retire_binding(&parent), Err(Error::BUSY));
    imported.release(retained).unwrap();
    drive(&mut runtime, 3);
    assert_eq!(runtime.module_status(instance), Ok(Lifecycle::Closed));
    assert_eq!(imported.reserve(), Err(Error::CLOSED));
    let freed = runtime.bind(&[]).unwrap();
    runtime.retire_binding(&freed).unwrap();
    let previous_service = observed.service.get().unwrap();
    assert_eq!(runtime.unload_module(previous_service), Ok(Lifecycle::Closing));
    drive(&mut runtime, 3);
    let replacement = activate(&mut runtime, root);
    drive(&mut runtime, 12);
    let Poll::Ready(Ok(fresh)) = replacement.status() else {
        panic!("reactivation failed");
    };
    assert_ne!(fresh, instance);
    assert_ne!(observed.service.get(), Some(previous_service));
    assert_eq!(observed.value.get(), Some(37));
    assert_eq!(imported.reserve(), Err(Error::CLOSED));
    runtime.unload_module(fresh).unwrap();
    drive(&mut runtime, 6);
    runtime.close().unwrap();
}

#[test]
fn missing_export_rolls_back_before_consumer_construction() {
    rejected(api(), b"missing", Limits::default(), false, Error::UNAVAILABLE);
}
#[test]
fn complete_api_mismatch_rejects_even_when_the_api_identity_matches() {
    let mut bytes = api().bytes().to_vec();
    let output = bytes.len() - 2;
    bytes[output] = 4; // U64 does not match the offered U32 result.
    let required =
        reovim_kernel_interface::declaration::Declaration::decode(&bytes)
            .unwrap();
    rejected(required, b"device", Limits::default(), false, Error::UNSUPPORTED);
}
#[test]
fn rejected_constructor_fences_derived_clones_and_returns_caller_capacity() {
    rejected(
        api(),
        b"device",
        Limits { callers: 3, ..Limits::default() },
        true,
        Error::PROVIDER,
    );
}
#[test]
fn import_context_pressure_rolls_back_before_consumer_construction() {
    rejected(
        api(),
        b"device",
        Limits { callers: 2, ..Limits::default() },
        false,
        Error::PRESSURE,
    );
}
fn rejected(
    requested: reovim_kernel_interface::declaration::Declaration,
    path: &[u8],
    limits: Limits,
    reject: bool,
    expected: Error,
) {
    let (mut runtime, root, parent) = base_limits(limits);
    let observed = Rc::new(Observation::default());
    observed.reject.set(reject);
    register(&mut runtime, root, &parent, service(&observed));
    runtime
        .register_module(
            root,
            root,
            consumer(&observed, requested, path),
            &parent,
            &["value"],
        )
        .unwrap();
    let probe = activate(&mut runtime, root);
    drive(&mut runtime, 12);
    assert_eq!(probe.status(), Poll::Ready(Err(expected)));
    assert_eq!(
        runtime.module_status(observed.service.get().unwrap()),
        Ok(Lifecycle::Closed)
    );
    if reject {
        let rejected = observed.caller.borrow().as_ref().unwrap().clone();
        assert_eq!(rejected.reserve(), Err(Error::CLOSED));
        let recovered = runtime.bind(&[]).unwrap();
        runtime.retire_binding(&recovered).unwrap();
    } else {
        assert!(observed.caller.borrow().is_none());
    }
    assert_eq!(
        acquire(&parent.caller(), b"/imports/value"),
        Err(Error::DENIED)
    );
    drive(&mut runtime, 1);
    runtime.close().unwrap();
}

#[test]
fn approvals_are_exact_and_cannot_change_an_existing_registration() {
    let (mut runtime, root, parent) = base();
    let observed = Rc::new(Observation::default());
    let definition = consumer(&observed, api(), b"device");
    assert_eq!(
        runtime.register_module(
            root,
            root,
            definition.clone(),
            &parent,
            &["missing"]
        ),
        Err(Error::INVALID)
    );
    assert_eq!(
        runtime.register_module(
            root,
            root,
            definition.clone(),
            &parent,
            &["value", "value"]
        ),
        Err(Error::CONFLICT)
    );
    let registered = runtime
        .register_module(root, root, definition.clone(), &parent, &["value"])
        .unwrap();
    assert_eq!(
        runtime.register_module(
            root,
            root,
            definition.clone(),
            &parent,
            &["value"]
        ),
        Ok(registered)
    );
    assert_eq!(
        runtime.register_module(root, root, definition, &parent, &[]),
        Err(Error::CONFLICT)
    );
    assert!(observed.caller.borrow().is_none());
    runtime.close().unwrap();
}

#[test]
fn shutdown_destroys_consumer_before_stopping_imported_provider() {
    let (mut runtime, root, parent) = base();
    let observed = Rc::new(Observation::default());
    register(&mut runtime, root, &parent, service(&observed));
    runtime
        .register_module(
            root,
            root,
            consumer(&observed, api(), b"device"),
            &parent,
            &["value"],
        )
        .unwrap();
    let probe = activate(&mut runtime, root);
    drive(&mut runtime, 12);
    assert!(matches!(probe.status(), Poll::Ready(Ok(_))));
    assert!(observed.consumer_live.get());
    observed.shutdown.set(true);
    runtime.close().unwrap();
    assert!(observed.destroyed.get());
    assert_eq!(
        observed.caller.borrow().as_ref().unwrap().reserve(),
        Err(Error::CLOSED)
    );
}
