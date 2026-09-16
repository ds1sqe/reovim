use super::*;
use std::task::Poll;

fn drive(runtime: &mut Runtime, turns: usize) {
    for _ in 0..turns {
        runtime.advance(&mut Context::from_waker(Waker::noop())).unwrap();
    }
}

#[test]
fn optional_availability_cannot_retarget_an_existing_instance() {
    let (mut runtime, root, binding) = base();
    let root_calls = Rc::new(Cell::new(0));
    let dep_calls = Rc::new(Cell::new(0));
    register(
        &mut runtime,
        root,
        &binding,
        definition(
            1,
            "root",
            1,
            1,
            vec![dependency(2, 1, DependencyScope::Same, true)],
            &root_calls,
        ),
    );
    let absent =
        runtime.select_module(root, "root", 1, ModuleScope::Global).unwrap();
    let original = runtime.activate_selected(&absent.modules()[0]).unwrap();
    drive(&mut runtime, 1);
    register(
        &mut runtime,
        root,
        &binding,
        definition(2, "optional", 1, 1, vec![], &dep_calls),
    );
    let present =
        runtime.select_module(root, "root", 1, ModuleScope::Global).unwrap();
    assert!(matches!(runtime.probe_modules(present), Err(Error::CONFLICT)));
    assert_eq!(dep_calls.get(), 0);
    assert_eq!(root_calls.get(), 1);
    assert_eq!(runtime.module_status(original), Ok(Lifecycle::Ready));
    assert_eq!(runtime.activate_selected(&absent.modules()[0]), Ok(original));
    let present =
        runtime.select_module(root, "root", 1, ModuleScope::Global).unwrap();
    let optional = runtime.activate_selected(&present.modules()[0]).unwrap();
    drive(&mut runtime, 1);
    assert_eq!(
        runtime.activate_selected(&present.modules()[1]),
        Err(Error::CONFLICT)
    );
    assert_eq!(runtime.module_status(optional), Ok(Lifecycle::Ready));
    assert_eq!(runtime.unload_module(original), Ok(Lifecycle::Closing));
    drive(&mut runtime, 1);
    let joined = runtime.probe_modules(present).unwrap();
    drive(&mut runtime, 8);
    let Poll::Ready(Ok(replacement)) = joined.status() else {
        panic!("replacement activation did not complete");
    };
    assert_ne!(replacement, original);
    assert_eq!(root_calls.get(), 2);
    assert_eq!(dep_calls.get(), 1);
    assert_eq!(
        runtime.activate_selected(&absent.modules()[0]),
        Err(Error::CONFLICT)
    );
    assert_eq!(runtime.module_status(replacement), Ok(Lifecycle::Ready));
    assert_eq!(runtime.module_status(original), Ok(Lifecycle::Closed));
    runtime.close().unwrap();
}

#[test]
fn captured_declarations_construct_fresh_dependencies_after_unload() {
    let (mut runtime, root, binding) = base();
    let calls = Rc::new(Cell::new(0));
    register(
        &mut runtime,
        root,
        &binding,
        definition(1, "dep", 1, 1, vec![], &calls),
    );
    register(
        &mut runtime,
        root,
        &binding,
        definition(
            2,
            "root",
            1,
            1,
            vec![dependency(1, 1, DependencyScope::Same, false)],
            &calls,
        ),
    );
    let selection =
        runtime.select_module(root, "root", 1, ModuleScope::Global).unwrap();
    let dependency =
        runtime.activate_selected(&selection.modules()[0]).unwrap();
    drive(&mut runtime, 1);
    let original = runtime.activate_selected(&selection.modules()[1]).unwrap();
    drive(&mut runtime, 1);
    assert_eq!(runtime.unload_module(original), Ok(Lifecycle::Closing));
    drive(&mut runtime, 1);
    assert_eq!(runtime.unload_module(dependency), Ok(Lifecycle::Closing));
    drive(&mut runtime, 1);
    let replacement =
        runtime.activate_selected(&selection.modules()[0]).unwrap();
    assert_ne!(replacement, dependency);
    drive(&mut runtime, 1);
    let fresh_root =
        runtime.activate_selected(&selection.modules()[1]).unwrap();
    assert_ne!(fresh_root, original);
    drive(&mut runtime, 1);
    assert_eq!(
        runtime.activate_selected(&selection.modules()[1]),
        Ok(fresh_root)
    );
    assert_eq!(runtime.unload_module(replacement), Err(Error::BUSY));
    assert_eq!(runtime.module_status(dependency), Ok(Lifecycle::Closed));
    runtime.close().unwrap();
}
