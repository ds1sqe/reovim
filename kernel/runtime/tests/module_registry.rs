use reovim_kernel_interface::{
    Error, ResourceId,
    driver::{DriverOps, Lifecycle, Progress},
    module::{
        Declaration, Definition, Dependency, DependencyScope, Module, ModuleId,
        ModuleImplementation, ModuleScope,
    },
    provider::{ResourceOps, ResourceProvider},
};
use reovim_kernel_runtime::{Binding, Limits, Runtime};
use std::{
    cell::Cell,
    rc::Rc,
    task::{Context, Waker},
};

fn base() -> (Runtime, ResourceId, Binding) {
    base_limits(Limits::default())
}
fn base_limits(limits: Limits) -> (Runtime, ResourceId, Binding) {
    let mut runtime = Runtime::new([0x91; 16], limits).unwrap();
    let root = ResourceProvider::new(
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
    let registry = runtime.register(b"/catalog", root, None).ok().unwrap();
    let binding = runtime.bind(&[]).unwrap();
    runtime.start().unwrap();
    (runtime, registry, binding)
}
fn dependency(
    id: u8,
    revision: u32,
    scope: DependencyScope,
    optional: bool,
) -> Dependency {
    Dependency {
        module: ModuleId([id; 16]),
        revision,
        scope,
        optional,
        imports: Box::new([]),
    }
}

#[allow(clippy::result_large_err, reason = "inert constructor returns context")]
fn definition(
    id: u8,
    name: &str,
    revision: u32,
    scopes: u32,
    dependencies: Vec<Dependency>,
    calls: &Rc<Cell<u32>>,
) -> Rc<Definition> {
    let declaration = Declaration::new(
        ModuleId([id; 16]),
        name,
        revision,
        scopes,
        dependencies.into_boxed_slice(),
    )
    .unwrap();
    let calls = calls.clone();
    Rc::new(
        Definition::new(
            declaration,
            Box::new([]),
            0,
            Rc::new(move |context| {
                calls.set(calls.get() + 1);
                Module::construct(
                    ModuleImplementation {
                        construct: |_| Ok(Lifecycle::Constructed),
                        driver: DriverOps {
                            status: |phase| *phase,
                            advance,
                            close: |phase| {
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
        .unwrap(),
    )
}
fn register(
    runtime: &mut Runtime,
    root: ResourceId,
    binding: &Binding,
    definition: Rc<Definition>,
) -> ResourceId {
    runtime.register_module(root, root, definition, binding, &[]).unwrap()
}

#[test]
fn selection_captures_shared_dependency_order_and_exact_scopes_without_calls() {
    let (mut runtime, root, binding) = base();
    let calls = Rc::new(Cell::new(0));
    register(
        &mut runtime,
        root,
        &binding,
        definition(1, "base", 1, 1, vec![], &calls),
    );
    let shared = dependency(1, 1, DependencyScope::Global, false);
    register(
        &mut runtime,
        root,
        &binding,
        definition(2, "left", 1, 2, vec![shared.clone()], &calls),
    );
    register(
        &mut runtime,
        root,
        &binding,
        definition(3, "right", 1, 2, vec![shared], &calls),
    );
    register(
        &mut runtime,
        root,
        &binding,
        definition(
            4,
            "top",
            1,
            2,
            vec![
                dependency(2, 1, DependencyScope::Same, false),
                dependency(3, 1, DependencyScope::Same, false),
            ],
            &calls,
        ),
    );
    let session = ModuleScope::Session(root);
    let selected = runtime.select_module(root, "top", 1, session).unwrap();
    let actual: Vec<_> = selected
        .modules()
        .iter()
        .map(|m| (m.declaration().name(), m.scope()))
        .collect();
    assert_eq!(
        actual,
        vec![
            ("base", ModuleScope::Global),
            ("left", session),
            ("right", session),
            ("top", session)
        ]
    );
    assert_eq!(calls.get(), 0);
    runtime.close().unwrap();
    assert_eq!(selected.modules().last().unwrap().declaration().name(), "top");
}

#[test]
fn identical_admission_reuses_identity_and_selection_constructs_exact_owner() {
    let (mut runtime, root, binding) = base();
    let calls = Rc::new(Cell::new(0));
    let definition = definition(1, "one", 1, 1, vec![], &calls);
    let id = register(&mut runtime, root, &binding, definition.clone());
    assert_eq!(register(&mut runtime, root, &binding, definition.clone()), id);
    let selection =
        runtime.select_module(root, "one", 1, ModuleScope::Global).unwrap();
    drop(definition);
    assert_eq!(calls.get(), 0);
    let selected = &selection.modules()[0];
    assert_eq!(selected.resource(), id);
    let instance = runtime.activate_selected(selected).unwrap();
    assert_eq!(calls.get(), 1);
    runtime.advance(&mut Context::from_waker(Waker::noop())).unwrap();
    assert_eq!(runtime.module_status(instance), Ok(Lifecycle::Ready));
    runtime.close().unwrap();
}

#[test]
fn missing_optional_is_omitted_but_incompatible_available_revision_is_not() {
    let (mut runtime, root, binding) = base();
    let calls = Rc::new(Cell::new(0));
    register(
        &mut runtime,
        root,
        &binding,
        definition(
            1,
            "top",
            1,
            1,
            vec![dependency(2, 2, DependencyScope::Same, true)],
            &calls,
        ),
    );
    let selection =
        runtime.select_module(root, "top", 1, ModuleScope::Global).unwrap();
    assert_eq!(selection.modules()[0].declaration().name(), "top");
    register(
        &mut runtime,
        root,
        &binding,
        definition(2, "dep", 1, 1, vec![], &calls),
    );
    assert!(matches!(
        runtime.select_module(root, "top", 1, ModuleScope::Global),
        Err(Error::UNSUPPORTED)
    ));
    assert_eq!(calls.get(), 0);
    runtime.close().unwrap();
}

#[test]
fn cyclic_and_conflicting_revisions_reject_before_any_constructor() {
    let (mut runtime, root, binding) = base();
    let calls = Rc::new(Cell::new(0));
    register(
        &mut runtime,
        root,
        &binding,
        definition(
            1,
            "cycle",
            1,
            1,
            vec![dependency(1, 1, DependencyScope::Same, false)],
            &calls,
        ),
    );
    assert!(matches!(
        runtime.select_module(root, "cycle", 1, ModuleScope::Global),
        Err(Error::INVALID)
    ));
    register(
        &mut runtime,
        root,
        &binding,
        definition(2, "dep", 1, 1, vec![], &calls),
    );
    register(
        &mut runtime,
        root,
        &binding,
        definition(2, "dep", 2, 1, vec![], &calls),
    );
    register(
        &mut runtime,
        root,
        &binding,
        definition(
            3,
            "other",
            1,
            1,
            vec![dependency(2, 2, DependencyScope::Same, false)],
            &calls,
        ),
    );
    register(
        &mut runtime,
        root,
        &binding,
        definition(
            4,
            "top",
            1,
            1,
            vec![
                dependency(2, 1, DependencyScope::Same, false),
                dependency(3, 1, DependencyScope::Same, false),
            ],
            &calls,
        ),
    );
    assert!(matches!(
        runtime.select_module(root, "top", 1, ModuleScope::Global),
        Err(Error::CONFLICT)
    ));
    assert_eq!(calls.get(), 0);
    runtime.close().unwrap();
}

fn advance(
    phase: &mut Lifecycle,
    _: &mut Context<'_>,
) -> Result<Progress, Error> {
    if let Lifecycle::Failed(error) = *phase {
        return Err(error);
    }
    if *phase == Lifecycle::Closed {
        return Ok(Progress::Stopped);
    }
    *phase = Lifecycle::Ready;
    Ok(Progress::Idle)
}

#[test]
fn activation_waits_for_ready_dependencies_and_reuses_the_exact_attempt() {
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
            "top",
            1,
            1,
            vec![dependency(1, 1, DependencyScope::Same, false)],
            &calls,
        ),
    );
    let selection =
        runtime.select_module(root, "top", 1, ModuleScope::Global).unwrap();
    let modules = selection.modules();
    assert_eq!(runtime.activate_selected(&modules[1]), Err(Error::NOT_READY));
    assert_eq!(calls.get(), 0);
    let dependency = runtime.activate_selected(&modules[0]).unwrap();
    assert_eq!(runtime.activate_selected(&modules[0]), Ok(dependency));
    assert_eq!(calls.get(), 1);
    assert_eq!(runtime.activate_selected(&modules[1]), Err(Error::NOT_READY));
    runtime.advance(&mut Context::from_waker(Waker::noop())).unwrap();
    let consumer = runtime.activate_selected(&modules[1]).unwrap();
    let joined =
        runtime.select_module(root, "top", 1, ModuleScope::Global).unwrap();
    assert_eq!(runtime.activate_selected(&joined.modules()[1]), Ok(consumer));
    assert_eq!(calls.get(), 2);
    runtime.advance(&mut Context::from_waker(Waker::noop())).unwrap();
    assert_eq!(runtime.module_status(consumer), Ok(Lifecycle::Ready));
    runtime.close().unwrap();
}

#[test]
fn captured_optional_absence_does_not_follow_later_registry_admission() {
    let (mut runtime, root, binding) = base();
    let calls = Rc::new(Cell::new(0));
    register(
        &mut runtime,
        root,
        &binding,
        definition(
            1,
            "top",
            1,
            1,
            vec![dependency(2, 1, DependencyScope::Same, true)],
            &calls,
        ),
    );
    let selected =
        runtime.select_module(root, "top", 1, ModuleScope::Global).unwrap();
    register(
        &mut runtime,
        root,
        &binding,
        definition(2, "later", 1, 1, vec![], &calls),
    );
    runtime.activate_selected(&selected.modules()[0]).unwrap();
    assert_eq!(calls.get(), 1);
    let (mut foreign, _, _) = base();
    assert_eq!(
        foreign.activate_selected(&selected.modules()[0]),
        Err(Error::DENIED)
    );
    foreign.close().unwrap();
    runtime.advance(&mut Context::from_waker(Waker::noop())).unwrap();
    runtime.close().unwrap();
}

#[test]
fn admission_conflicts_and_foreign_bindings_cannot_replace_selected_code() {
    let (mut runtime, root, binding) = base();
    let first = Rc::new(Cell::new(0));
    let replacement = Rc::new(Cell::new(0));
    register(
        &mut runtime,
        root,
        &binding,
        definition(1, "one", 1, 1, vec![], &first),
    );
    assert_eq!(
        runtime.register_module(
            root,
            root,
            definition(1, "one", 1, 1, vec![], &replacement),
            &binding,
            &[]
        ),
        Err(Error::CONFLICT)
    );
    let (mut foreign, _, other_binding) = base();
    assert_eq!(
        runtime.register_module(
            root,
            root,
            definition(2, "two", 1, 1, vec![], &replacement),
            &other_binding,
            &[]
        ),
        Err(Error::DENIED)
    );
    assert!(matches!(
        runtime.select_module(root, "missing", 1, ModuleScope::Global),
        Err(Error::UNAVAILABLE)
    ));
    assert!(matches!(
        runtime.select_module(root, "one", 2, ModuleScope::Global),
        Err(Error::UNSUPPORTED)
    ));
    let selected =
        runtime.select_module(root, "one", 1, ModuleScope::Global).unwrap();
    runtime.activate_selected(&selected.modules()[0]).unwrap();
    assert_eq!(first.get(), 1);
    assert_eq!(replacement.get(), 0);
    runtime.advance(&mut Context::from_waker(Waker::noop())).unwrap();
    runtime.close().unwrap();
    foreign.close().unwrap();
}

#[path = "module_registry/lifetime.rs"]
mod lifetime;

#[test]
fn reactivation_uses_a_new_identity_and_old_unload_cannot_retarget_it() {
    let (mut runtime, root, binding) = base();
    let calls = Rc::new(Cell::new(0));
    register(
        &mut runtime,
        root,
        &binding,
        definition(1, "one", 1, 1, vec![], &calls),
    );
    let selection =
        runtime.select_module(root, "one", 1, ModuleScope::Global).unwrap();
    let old = runtime.activate_selected(&selection.modules()[0]).unwrap();
    runtime.advance(&mut Context::from_waker(Waker::noop())).unwrap();
    assert_eq!(runtime.unload_module(old), Ok(Lifecycle::Closing));
    runtime.advance(&mut Context::from_waker(Waker::noop())).unwrap();
    assert_eq!(runtime.unload_module(old), Ok(Lifecycle::Closed));
    let fresh = runtime.activate_selected(&selection.modules()[0]).unwrap();
    assert_ne!(fresh, old);
    assert_eq!(calls.get(), 2);
    runtime.advance(&mut Context::from_waker(Waker::noop())).unwrap();
    assert_eq!(runtime.unload_module(old), Ok(Lifecycle::Closed));
    assert_eq!(runtime.module_status(fresh), Ok(Lifecycle::Ready));
    assert_eq!(
        runtime.unload_module(ResourceId { epoch: [0x23; 16], serial: 1 }),
        Err(Error::UNAVAILABLE)
    );
    runtime.close().unwrap();
}

#[path = "module_registry/control.rs"]
mod control;

#[path = "module_registry/probe.rs"]
mod probe;

#[path = "module_registry/reuse.rs"]
mod reuse;

#[path = "module_registry/activation.rs"]
mod activation;

#[path = "module_registry/imports.rs"]
mod imports;

#[test]
fn registration_freezes_composition_import_grants() {
    use reovim_kernel_runtime::Grant;
    let (mut runtime, root, binding) = base();
    let calls = Rc::new(Cell::new(0));
    let package = register(
        &mut runtime,
        root,
        &binding,
        definition(1, "base", 1, 1, vec![], &calls),
    );
    assert_eq!(
        runtime.grant(
            &binding,
            Grant {
                resource: package,
                access: 5,
                create_access: 0,
                retain_loans: false,
            },
        ),
        Err(Error::BUSY)
    );
    assert_eq!(calls.get(), 0);
    runtime.close().unwrap();
}

#[path = "module_registry/publication.rs"]
mod publication;
