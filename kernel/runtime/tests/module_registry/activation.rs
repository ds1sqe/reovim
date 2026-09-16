use super::control::{bind_decl, call, invoke, outcome};
use super::*;
use reovim_kernel_interface::{
    Outcome,
    call::Cancellation,
    caller::Operation,
    module::{activation, control},
};

fn drive(runtime: &mut Runtime, turns: usize) {
    for _ in 0..turns {
        runtime.advance(&mut Context::from_waker(Waker::noop())).unwrap();
    }
}
#[test]
fn caller_activation_returns_usable_exact_access_and_recovers_original_receipt()
{
    let (mut runtime, root, imports) = base();
    let constructors = Rc::new(Cell::new(0));
    let package = register(
        &mut runtime,
        root,
        &imports,
        definition(1, "one", 1, 1, vec![], &constructors),
    );
    let (caller, bound) =
        bind_decl(&mut runtime, package, &activation::declaration().unwrap());
    let denied = call(&caller, Operation::Exec, bound, 0, &[]);
    assert_eq!(caller.submit(denied.clone()), Err(Error::DENIED));
    caller.release(denied.request).unwrap();
    assert_eq!(constructors.get(), 0);
    invoke(
        &mut runtime,
        &caller,
        Operation::SetAccess,
        bound,
        0,
        &4_u32.to_le_bytes(),
    );
    let first = call(&caller, Operation::Exec, bound, 0, &[]);
    caller.submit(first.clone()).unwrap();
    caller.submit(first.clone()).unwrap();
    drive(&mut runtime, 8);
    assert_eq!(constructors.get(), 1);
    let result = outcome(&caller, first.request);
    let Outcome::Ok(value) = result.as_ref() else {
        panic!("activation failed");
    };
    let instance_access = value.references[0];
    caller.submit(first.clone()).unwrap();
    assert_eq!(outcome(&caller, first.request), result);
    assert_eq!(caller.cancel(first.request), Err(Error::CLOSED));
    caller.adopt(first.request).unwrap();
    caller.release(first.request).unwrap();
    assert_status(&mut runtime, &caller, instance_access, 2);
    // No composition grant for the instance exists: this access came only
    // from the activation result and works through the declared control API.
    invoke(
        &mut runtime,
        &caller,
        Operation::Ioctl,
        instance_access,
        control::UNLOAD,
        &[],
    );
    assert_status(&mut runtime, &caller, instance_access, 4);
    let replacement = call(&caller, Operation::Exec, bound, 0, &[]);
    caller.submit(replacement.clone()).unwrap();
    drive(&mut runtime, 8);
    let result = outcome(&caller, replacement.request);
    let Outcome::Ok(value) = result.as_ref() else {
        panic!("reactivation failed");
    };
    let replacement_access = value.references[0];
    caller.adopt(replacement.request).unwrap();
    caller.release(replacement.request).unwrap();
    assert_eq!(constructors.get(), 2);
    assert_status(&mut runtime, &caller, instance_access, 4);
    assert_status(&mut runtime, &caller, replacement_access, 2);
    drive(&mut runtime, 1);
    runtime.close().unwrap();
}

#[test]
fn cancellation_before_graph_admission_preserves_package_and_allows_retry() {
    let (mut runtime, root, imports) = base();
    let constructors = Rc::new(Cell::new(0));
    let package = register(
        &mut runtime,
        root,
        &imports,
        definition(1, "one", 1, 1, vec![], &constructors),
    );
    let (caller, bound) =
        bind_decl(&mut runtime, package, &activation::declaration().unwrap());
    invoke(
        &mut runtime,
        &caller,
        Operation::SetAccess,
        bound,
        0,
        &4_u32.to_le_bytes(),
    );
    let cancelled = call(&caller, Operation::Exec, bound, 0, &[]);
    caller.submit(cancelled.clone()).unwrap();
    drive(&mut runtime, 1);
    assert_eq!(caller.cancel(cancelled.request), Ok(Cancellation::Accepted));
    drive(&mut runtime, 4);
    assert_eq!(
        outcome(&caller, cancelled.request).as_ref(),
        &Outcome::Failed(Error::CANCELLED)
    );
    assert_eq!(constructors.get(), 0);
    caller.release(cancelled.request).unwrap();
    let retry = call(&caller, Operation::Exec, bound, 0, &[]);
    caller.submit(retry.clone()).unwrap();
    drive(&mut runtime, 8);
    assert!(matches!(outcome(&caller, retry.request).as_ref(), Outcome::Ok(_)));
    assert_eq!(constructors.get(), 1);
    // Release without adoption discards returned access under ordinary custody.
    caller.release(retry.request).unwrap();
    drive(&mut runtime, 2);
    runtime.close().unwrap();
}

#[test]
fn missing_dependency_returns_original_error_without_construction() {
    let (mut runtime, root, imports) = base();
    let constructors = Rc::new(Cell::new(0));
    let package = register(
        &mut runtime,
        root,
        &imports,
        definition(
            1,
            "one",
            1,
            1,
            vec![dependency(2, 1, DependencyScope::Global, false)],
            &constructors,
        ),
    );
    let (caller, bound) =
        bind_decl(&mut runtime, package, &activation::declaration().unwrap());
    invoke(
        &mut runtime,
        &caller,
        Operation::SetAccess,
        bound,
        0,
        &4_u32.to_le_bytes(),
    );
    let request = call(&caller, Operation::Exec, bound, 0, &[]);
    caller.submit(request.clone()).unwrap();
    drive(&mut runtime, 4);
    assert_eq!(
        outcome(&caller, request.request).as_ref(),
        &Outcome::Failed(Error::UNAVAILABLE)
    );
    assert_eq!(constructors.get(), 0);
    caller.release(request.request).unwrap();
    drive(&mut runtime, 1);
    runtime.close().unwrap();
}

fn assert_status(
    runtime: &mut Runtime,
    caller: &reovim_kernel_interface::caller::Caller,
    access: reovim_kernel_interface::resource::AccessId,
    status: u32,
) {
    assert_eq!(
        invoke(runtime, caller, Operation::Ioctl, access, control::STATUS, &[])
            .data
            .as_ref(),
        &status.to_le_bytes()
    );
}

#[test]
fn output_capacity_rejects_before_construction_and_recovers_after_cancel() {
    let (mut runtime, root, imports) =
        base_limits(Limits { handles: 3, ..Limits::default() });
    let constructors = Rc::new(Cell::new(0));
    let package = register(
        &mut runtime,
        root,
        &imports,
        definition(1, "one", 1, 1, vec![], &constructors),
    );
    let (caller, bound) =
        bind_decl(&mut runtime, package, &activation::declaration().unwrap());
    invoke(
        &mut runtime,
        &caller,
        Operation::SetAccess,
        bound,
        0,
        &4_u32.to_le_bytes(),
    );
    let first = call(&caller, Operation::Exec, bound, 0, &[]);
    let second = call(&caller, Operation::Exec, bound, 0, &[]);
    caller.submit(first.clone()).unwrap();
    assert_eq!(caller.submit(second.clone()), Err(Error::PRESSURE));
    assert_eq!(constructors.get(), 0);
    caller.release(second.request).unwrap();
    assert_eq!(caller.cancel(first.request), Ok(Cancellation::Accepted));
    drive(&mut runtime, 3);
    caller.release(first.request).unwrap();
    let retry = call(&caller, Operation::Exec, bound, 0, &[]);
    caller.submit(retry.clone()).unwrap();
    drive(&mut runtime, 8);
    let result = outcome(&caller, retry.request);
    let Outcome::Ok(value) = result.as_ref() else {
        panic!("activation failed");
    };
    let instance = value.references[0];
    caller.adopt(retry.request).unwrap();
    caller.release(retry.request).unwrap();
    assert_status(&mut runtime, &caller, instance, 2);
    assert_eq!(constructors.get(), 1);
    drive(&mut runtime, 1);
    runtime.close().unwrap();
}

#[test]
fn unauthorized_declared_imports_reject_before_any_graph_constructor() {
    let (mut runtime, root, imports) = base();
    let constructors = Rc::new(Cell::new(0));
    register(
        &mut runtime,
        root,
        &imports,
        definition(1, "service", 1, 1, vec![], &constructors),
    );
    let mut required = dependency(1, 1, DependencyScope::Global, false);
    required.imports =
        Box::new([reovim_kernel_interface::module::Import::new(
            "service",
            b"control",
            control::declaration().unwrap(),
            1,
            0,
            false,
        )
        .unwrap()]);
    let package = register(
        &mut runtime,
        root,
        &imports,
        definition(2, "consumer", 1, 1, vec![required], &constructors),
    );
    let selected = runtime
        .select_module(root, "consumer", 1, ModuleScope::Global)
        .unwrap();
    assert_eq!(
        runtime.activate_selected(selected.modules().last().unwrap()),
        Err(Error::DENIED)
    );
    assert!(matches!(runtime.probe_modules(selected), Err(Error::DENIED)));
    let (caller, bound) =
        bind_decl(&mut runtime, package, &activation::declaration().unwrap());
    invoke(
        &mut runtime,
        &caller,
        Operation::SetAccess,
        bound,
        0,
        &4_u32.to_le_bytes(),
    );
    let request = call(&caller, Operation::Exec, bound, 0, &[]);
    caller.submit(request.clone()).unwrap();
    drive(&mut runtime, 4);
    assert_eq!(
        outcome(&caller, request.request).as_ref(),
        &Outcome::Failed(Error::DENIED)
    );
    assert_eq!(constructors.get(), 0);
    caller.release(request.request).unwrap();
    drive(&mut runtime, 1);
    runtime.close().unwrap();
}
