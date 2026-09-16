use super::*;
use reovim_kernel_interface::{
    abi,
    driver::Lifecycle,
    module::ModuleScope,
    provider::{ResourceOps, ResourceProvider},
};
use reovim_kernel_runtime::Activation;
use reovim_sdk::{NativeModule, module_implementation};

fn mapped(library: &std::rc::Rc<Library>) -> NativeModule {
    let entry: unsafe extern "C" fn() -> *const abi::Module =
        unsafe { library.symbol(c"rv_module_entry") };
    unsafe { NativeModule::import(entry(), library.clone()) }.unwrap()
}

fn base_runtime() -> (Runtime, ResourceId, reovim_kernel_runtime::Binding) {
    let mut runtime = Runtime::new([0x81; 16], Limits::default()).unwrap();
    let root = ResourceProvider::new(
        (),
        ResourceOps {
            open: None,
            namespace: None,
            io: None,
            stop: |()| Ok(()),
        },
        Box::new([]),
        std::rc::Rc::new(()),
    )
    .ok()
    .unwrap();
    let namespace = runtime.register(b"/module", root, None).ok().unwrap();
    let binding = runtime.bind(&[]).unwrap();
    runtime.start().unwrap();
    (runtime, namespace, binding)
}

fn activate(
    runtime: &mut Runtime,
    namespace: ResourceId,
    binding: &reovim_kernel_runtime::Binding,
    module: &NativeModule,
) -> Result<ResourceId, Error> {
    let paths: Vec<&[u8]> =
        module.required().iter().map(AsRef::as_ref).collect();
    runtime.activate(
        Activation {
            namespace,
            scope: ModuleScope::Global,
            required: &paths,
            exports: module.export_limit(),
        },
        module_implementation(),
        module.imports(binding.caller(), LIMITS, 16),
        binding,
        std::rc::Rc::new(module.clone()),
    )
}

#[test]
fn native_module_entry_drives_embedded_provider_data_and_final_destruction() {
    let (mut runtime, namespace, binding) = base_runtime();
    let library = Library::compile_source("module.c");
    let module = mapped(&library);
    assert_eq!(library.count(c"fixture_module_constructed"), 0);
    let instance =
        activate(&mut runtime, namespace, &binding, &module).unwrap();
    let cx = &mut Context::from_waker(Waker::noop());
    assert_eq!(runtime.module_status(instance), Ok(Lifecycle::Constructed));
    runtime.advance(cx).unwrap();
    assert_eq!(runtime.module_exports(instance), Err(Error::NOT_READY));
    runtime.advance(cx).unwrap();
    let exports = runtime.module_exports(instance).unwrap().to_vec();
    let caller = runtime.bind(&[grant(exports[1], 1)]).unwrap().caller();
    let handle = acquire(&caller, exports[1], cx);
    let declaration = discovery::first(&caller, handle, cx);
    let bound = success(
        &caller,
        call(&caller, Operation::Bind, handle, bytes(declaration.bytes())),
        cx,
    )
    .references[0];
    success(
        &caller,
        call(
            &caller,
            Operation::SetAccess,
            handle,
            Value::copy(&1_u32.to_le_bytes(), &[]).unwrap(),
        ),
        cx,
    );
    let mut request = call(&caller, Operation::Ioctl, bound, Value::unit());
    request.command = match declaration
        .methods()
        .iter()
        .find(|m| {
            matches!(
                m.operation(),
                reovim_kernel_interface::declaration::Operation::Ioctl(_)
            )
        })
        .unwrap()
        .operation()
    {
        reovim_kernel_interface::declaration::Operation::Ioctl(command) => {
            command
        }
        reovim_kernel_interface::declaration::Operation::Exec => unreachable!(),
    };
    caller.submit(request.clone()).unwrap();
    runtime.advance(cx).unwrap();
    runtime.advance(cx).unwrap();
    let result = outcome(&caller, request.request, cx);
    let Outcome::Ok(value) = result.as_ref() else { panic!("missing value") };
    assert_eq!(number(value), 82);
    caller.adopt(request.request).unwrap();
    caller.release(request.request).unwrap();
    assert_eq!(runtime.close(), Err(Error::BUSY));
    assert_eq!(library.count(c"fixture_module_destroyed"), 0);
    drop(result);
    runtime.advance(cx).unwrap();
    runtime.close().unwrap();
    assert_eq!(library.count(c"fixture_destroyed"), 2);
    assert_eq!(library.count(c"fixture_module_destroyed"), 1);
    assert_eq!(library.count(c"fixture_late_closed"), 1);
}

#[test]
fn native_module_missing_export_rolls_back_and_busy_close_keeps_instance() {
    let (mut runtime, namespace, binding) = base_runtime();
    let library = Library::compile_source("module.c");
    let mode: unsafe extern "C" fn(u32) =
        unsafe { library.symbol(c"fixture_module_mode") };
    let gate: unsafe extern "C" fn(u32) =
        unsafe { library.symbol(c"fixture_close_ready") };
    unsafe {
        mode(1);
        gate(0);
    }
    let module = mapped(&library);
    let instance =
        activate(&mut runtime, namespace, &binding, &module).unwrap();
    let cx = &mut Context::from_waker(Waker::noop());
    runtime.advance(cx).unwrap();
    runtime.advance(cx).unwrap();
    assert_eq!(runtime.module_exports(instance), Err(Error::UNAVAILABLE));
    assert_eq!(library.count(c"fixture_destroyed"), 1);
    assert_eq!(runtime.close(), Err(Error::BUSY));
    assert_eq!(library.count(c"fixture_module_destroyed"), 0);
    unsafe {
        gate(1);
    }
    runtime.advance(cx).unwrap();
    runtime.close().unwrap();
    assert_eq!(library.count(c"fixture_module_destroyed"), 1);
}

#[test]
fn native_module_rejects_bad_metadata_and_inert_construction_without_ownership()
{
    let (mut runtime, namespace, binding) = base_runtime();
    let library = Library::compile_source("module.c");
    let entry: unsafe extern "C" fn() -> *const abi::Module =
        unsafe { library.symbol(c"rv_module_entry") };
    let mut descriptor = unsafe { *entry() };
    descriptor.version.reserved = 1;
    assert!(matches!(
        unsafe { NativeModule::import(&raw const descriptor, library.clone()) },
        Err(Error::UNSUPPORTED)
    ));
    assert_eq!(library.count(c"fixture_module_constructed"), 0);
    let mode: unsafe extern "C" fn(u32) =
        unsafe { library.symbol(c"fixture_module_mode") };
    unsafe {
        mode(2);
    }
    let module = mapped(&library);
    let context = reovim_kernel_interface::module::ModuleContext {
        instance: ResourceId { epoch: [0; 16], serial: 0 },
        namespace,
        scope: ModuleScope::Global,
        imports: module.imports(binding.caller(), LIMITS, 16),
        exports: reovim_kernel_interface::provider::Registrar::new(
            (),
            reovim_kernel_interface::provider::RegistrarOps {
                stage: |(), _, _| panic!("invalid context staged"),
            },
        ),
    };
    let (returned, error) =
        (module_implementation().construct)(context).err().unwrap();
    assert_eq!(error, Error::INVALID);
    assert_eq!(returned.namespace, namespace);
    assert_eq!(
        activate(&mut runtime, namespace, &binding, &module),
        Err(Error::DENIED)
    );
    assert_eq!(library.count(c"fixture_module_constructed"), 0);
    assert_eq!(library.count(c"fixture_module_destroyed"), 0);
    runtime.close().unwrap();
}

#[test]
fn native_module_closed_during_idle_drive_releases_its_instance() {
    let (mut runtime, namespace, binding) = base_runtime();
    let library = Library::compile_source("module.c");
    let mode: unsafe extern "C" fn(u32) =
        unsafe { library.symbol(c"fixture_module_mode") };
    let gate: unsafe extern "C" fn(u32) =
        unsafe { library.symbol(c"fixture_close_ready") };
    unsafe {
        mode(3);
        gate(0);
    }
    let module = mapped(&library);
    activate(&mut runtime, namespace, &binding, &module).unwrap();
    let cx = &mut Context::from_waker(Waker::noop());
    runtime.advance(cx).unwrap();
    runtime.advance(cx).unwrap();
    assert_eq!(runtime.close(), Err(Error::BUSY));
    assert_eq!(library.count(c"fixture_module_destroyed"), 0);
    unsafe { gate(1) };
    runtime.advance(cx).unwrap();
    runtime.close().unwrap();
    assert_eq!(library.count(c"fixture_module_destroyed"), 1);
    assert_eq!(library.count(c"fixture_late_closed"), 1);
}

#[test]
fn native_module_copies_passive_identity_and_exact_dependency_metadata() {
    use reovim_kernel_interface::module::{DependencyScope, ModuleId};
    let library = Library::compile_source("module.c");
    let manifest: unsafe extern "C" fn(*mut abi::Module) =
        unsafe { library.symbol(c"fixture_module_declaration") };
    let mut output = core::mem::MaybeUninit::uninit();
    unsafe { manifest(output.as_mut_ptr()) };
    let output = unsafe { output.assume_init() };
    let module =
        unsafe { NativeModule::import(&raw const output, library.clone()) }
            .unwrap();
    assert_eq!(library.count(c"fixture_module_constructed"), 0);
    let declaration = module.declaration().clone();
    drop(module);
    drop(library);
    let mut id = [0; 16];
    id[0] = 0xa3;
    assert_eq!(declaration.id(), ModuleId(id));
    assert_eq!(declaration.name(), "fixture");
    assert_eq!(declaration.revision(), 1);
    assert_eq!(declaration.scopes(), 1);
    id[0] = 0xb1;
    let dependency = &declaration.dependencies()[0];
    assert_eq!(dependency.module, ModuleId(id));
    assert_eq!(dependency.revision, 7);
    assert_eq!(dependency.scope, DependencyScope::Global);
    assert!(dependency.optional);
    let import = &dependency.imports[0];
    assert_eq!(import.name(), "counter");
    assert_eq!(import.export(), b"device");
    assert_eq!(import.access(), 5);
    assert_eq!(import.create_access(), 1);
    assert!(import.retain_loans());
    assert_eq!(import.declaration().api(), &[0x71; 16]);
    let read = &import.declaration().methods()[1];
    assert_eq!(read.name(), "read");
    assert_eq!(
        read.operation(),
        reovim_kernel_interface::declaration::Operation::Ioctl(1)
    );
    assert_eq!(read.access(), 1);
    assert_eq!(
        read.output(),
        &reovim_kernel_interface::declaration::ValueType::decode(&[4]).unwrap()
    );
}

#[test]
fn native_module_rejects_invalid_metadata_without_constructing() {
    let library = Library::compile_source("module.c");
    let entry: unsafe extern "C" fn() -> *const abi::Module =
        unsafe { library.symbol(c"rv_module_entry") };
    let base = unsafe { *entry() };
    let invalid = [
        abi::Module { id: [0; 16], ..base },
        abi::Module { revision: 0, ..base },
        abi::Module { scopes: 4, ..base },
        abi::Module {
            name: abi::Bytes { ptr: b"\xff".as_ptr(), len: 1 },
            ..base
        },
    ];
    for native in invalid {
        assert!(matches!(
            unsafe { NativeModule::import(&raw const native, library.clone()) },
            Err(Error::INVALID)
        ));
    }
    let dependency = abi::ModuleDependency {
        module: [0xb2; 16],
        revision: 1,
        scope: 0,
        optional: 0,
        imports: core::ptr::null(),
        import_count: 0,
    };
    for dependency in [
        abi::ModuleDependency { scope: 2, ..dependency },
        abi::ModuleDependency { optional: 2, ..dependency },
    ] {
        let native = abi::Module {
            dependencies: &raw const dependency,
            dependency_count: 1,
            ..base
        };
        assert!(matches!(
            unsafe { NativeModule::import(&raw const native, library.clone()) },
            Err(Error::INVALID)
        ));
    }
    let duplicates = [dependency; 2];
    let repeated = abi::Module {
        dependencies: duplicates.as_ptr(),
        dependency_count: duplicates.len(),
        ..base
    };
    assert!(matches!(
        unsafe { NativeModule::import(&raw const repeated, library.clone()) },
        Err(Error::CONFLICT)
    ));
    let only_session = abi::Module { scopes: 2, ..base };
    let module = unsafe {
        NativeModule::import(&raw const only_session, library.clone())
    }
    .unwrap();
    let (mut runtime, namespace, binding) = base_runtime();
    assert_eq!(
        activate(&mut runtime, namespace, &binding, &module),
        Err(Error::UNSUPPORTED)
    );
    runtime.close().unwrap();
    assert_eq!(library.count(c"fixture_module_constructed"), 0);
}

#[test]
fn native_module_rejects_invalid_import_requests_before_construction() {
    let library = Library::compile_source("module.c");
    let entry: unsafe extern "C" fn() -> *const abi::Module =
        unsafe { library.symbol(c"rv_module_entry") };
    let base = unsafe { *entry() };
    let api = reovim_kernel_interface::module::control::declaration().unwrap();
    let import = abi::ModuleImport {
        name: abi::Bytes { ptr: b"device".as_ptr(), len: 6 },
        export_path: abi::Bytes { ptr: core::ptr::null(), len: 0 },
        declaration: abi::Bytes {
            ptr: api.bytes().as_ptr(),
            len: api.bytes().len(),
        },
        access: 1,
        create_access: 0,
        retain_loans: 0,
    };
    for import in [
        abi::ModuleImport { access: 8, ..import },
        abi::ModuleImport { create_access: 8, ..import },
        abi::ModuleImport { retain_loans: 2, ..import },
        abi::ModuleImport {
            name: abi::Bytes { ptr: b"a/b".as_ptr(), len: 3 },
            ..import
        },
        abi::ModuleImport {
            declaration: abi::Bytes { ptr: core::ptr::null(), len: 1 },
            ..import
        },
    ] {
        let dependency = abi::ModuleDependency {
            module: [2; 16],
            revision: 1,
            scope: 0,
            optional: 0,
            imports: &raw const import,
            import_count: 1,
        };
        let descriptor = abi::Module {
            dependencies: &raw const dependency,
            dependency_count: 1,
            ..base
        };
        assert!(matches!(
            unsafe {
                NativeModule::import(&raw const descriptor, library.clone())
            },
            Err(Error::INVALID)
        ));
    }
    let oversized = abi::ModuleDependency {
        module: [2; 16],
        revision: 1,
        scope: 0,
        optional: 0,
        imports: core::ptr::null(),
        import_count: 257,
    };
    let descriptor = abi::Module {
        dependencies: &raw const oversized,
        dependency_count: 1,
        ..base
    };
    assert!(matches!(
        unsafe { NativeModule::import(&raw const descriptor, library.clone()) },
        Err(Error::PRESSURE)
    ));
    assert_eq!(library.count(c"fixture_module_constructed"), 0);
}
