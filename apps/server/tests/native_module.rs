#![cfg(target_os = "linux")]
#![allow(unsafe_code)]

use reovim_host_reference::native;
use reovim_kernel_interface::{
    Error, Outcome, ResourceId, Value,
    caller::{Caller, Operation},
    declaration::{Declaration, Operation as Declared, ValueLimits},
    driver::Lifecycle,
    module::ModuleScope,
    provider::{ResourceOps, ResourceProvider},
    resource::AccessId,
};
use reovim_kernel_runtime::{Activation, Grant, Limits, Runtime};
use reovim_sdk::{Request, module_implementation};
use std::{
    path::{Path, PathBuf},
    process::Command,
    rc::Rc,
    sync::atomic::{AtomicU64, Ordering},
    task::{Context, Waker},
};

static NEXT: AtomicU64 = AtomicU64::new(0);

struct Artifact {
    directory: PathBuf,
    library: PathBuf,
}

impl Artifact {
    fn compile(source: &str) -> Self {
        let directory = std::env::temp_dir().join(format!(
            "reovim-host-module-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir(&directory).unwrap();
        let library = directory.join("module.so");
        let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
        let status = Command::new("cc")
            .args([
                "-std=c11", "-Wall", "-Wextra", "-Werror", "-shared", "-fPIC",
            ])
            .arg("-I")
            .arg(manifest.join("../../host/reference/include"))
            .arg(manifest.join("../../sdk/rust/tests/native").join(source))
            .arg("-o")
            .arg(&library)
            .status()
            .unwrap();
        assert!(status.success());
        Self { directory, library }
    }
}

impl Artifact {
    fn rust(example: &str) -> Self {
        let directory = std::env::temp_dir().join(format!(
            "reovim-rust-module-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
        let output = Command::new(env!("CARGO"))
            .args(["build", "--offline", "--manifest-path"])
            .arg(manifest.join("../../product/Cargo.toml"))
            .args([
                "-p",
                &format!("reovim-module-{example}"),
                "--example",
                example,
            ])
            .arg("--target-dir")
            .arg(&directory)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let library = directory.join(format!("debug/examples/lib{example}.so"));
        Self { directory, library }
    }
}

#[test]
fn server_loads_rust_counter_built_after_runtime_start() {
    let mut runtime =
        reovim_server::assemble([0x93; 16], Limits::default()).unwrap();
    let resource = rust_module(&mut runtime, b"/rust", "counter");
    let cx = &mut Context::from_waker(Waker::noop());
    let (caller, bound, declaration) = discover(&mut runtime, resource, 0);
    assert!(
        declaration.methods().iter().any(|m| m.operation() == Declared::Exec)
    );
    let output = invoke(
        &mut runtime,
        &caller,
        Operation::Exec,
        bound,
        0,
        &37_u64.to_le_bytes(),
    );
    assert_eq!(
        u64::from_le_bytes(output.data.as_ref().try_into().unwrap()),
        37
    );
    assert_eq!(read_discovered(&mut runtime, resource), 37);
    runtime.advance(cx).unwrap();
    runtime.close().unwrap();
}

fn rust_module(
    runtime: &mut Runtime,
    path: &[u8],
    example: &str,
) -> ResourceId {
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
    let namespace = runtime.register(path, root, None).ok().unwrap();
    let binding = runtime.bind(&[]).unwrap();
    let artifact = Artifact::rust(example);
    let module = unsafe { native::load(&artifact.library) }.unwrap();
    let definition = Rc::new(
        module
            .definition(
                ValueLimits { bytes: 8192, elements: 256, references: 8 },
                16,
            )
            .unwrap(),
    );
    let registered = runtime
        .register_module(
            namespace,
            namespace,
            definition.clone(),
            &binding,
            &[],
        )
        .unwrap();
    assert_eq!(
        runtime.register_module(
            namespace,
            namespace,
            definition,
            &binding,
            &[]
        ),
        Ok(registered)
    );
    drop(module);
    drop(artifact);
    let (caller, package, declaration) = discover(runtime, registered, 0);
    let limits = ValueLimits { bytes: 8192, elements: 256, references: 8 };
    let exported =
        Rc::new(reovim_sdk::ExportedCaller::new(caller, limits, 8).unwrap());
    let caller = unsafe {
        reovim_sdk::import_caller(exported.raw(), exported, limits, 16)
    }
    .unwrap();
    assert_eq!(
        declaration.api(),
        &reovim_kernel_interface::module::activation::API
    );
    let result = invoke(runtime, &caller, Operation::Exec, package, 0, &[]);
    let info =
        invoke(runtime, &caller, Operation::Info, result.references[0], 0, &[]);
    let instance = ResourceId {
        epoch: info.data[..16].try_into().unwrap(),
        serial: u64::from_le_bytes(info.data[16..24].try_into().unwrap()),
    };
    assert_eq!(runtime.module_status(instance), Ok(Lifecycle::Ready));
    runtime.module_exports(instance).unwrap()[0]
}

impl Drop for Artifact {
    fn drop(&mut self) {
        std::fs::remove_dir_all(&self.directory).unwrap();
    }
}

#[test]
fn server_loads_after_start_and_keeps_exact_code_through_activation() {
    let mut runtime =
        reovim_server::assemble([0x92; 16], Limits::default()).unwrap();
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
    let namespace = runtime.register(b"/device", root, None).ok().unwrap();
    let binding = runtime.bind(&[]).unwrap();
    let artifact = Artifact::compile("module.c");
    let module = unsafe { native::load(&artifact.library) }.unwrap();
    let paths: Vec<&[u8]> =
        module.required().iter().map(AsRef::as_ref).collect();
    let instance = runtime
        .activate(
            Activation {
                namespace,
                scope: ModuleScope::Global,
                required: &paths,
                exports: module.export_limit(),
            },
            module_implementation(),
            module.imports(
                binding.caller(),
                ValueLimits { bytes: 8192, elements: 256, references: 8 },
                16,
            ),
            &binding,
            Rc::new(module.clone()),
        )
        .unwrap();
    drop(module);
    drop(artifact);
    let cx = &mut Context::from_waker(Waker::noop());
    assert_eq!(runtime.module_status(instance), Ok(Lifecycle::Constructed));
    runtime.advance(cx).unwrap();
    assert_eq!(runtime.module_status(instance), Ok(Lifecycle::Starting));
    assert_eq!(runtime.module_exports(instance), Err(Error::NOT_READY));
    runtime.advance(cx).unwrap();
    assert_eq!(runtime.module_status(instance), Ok(Lifecycle::Ready));
    let exports = runtime.module_exports(instance).unwrap().to_vec();
    assert_ne!(exports[0], exports[1]);
    assert_eq!(read_discovered(&mut runtime, exports[0]), 81);
    assert_eq!(read_discovered(&mut runtime, exports[1]), 82);
    runtime.advance(cx).unwrap();
    // The C destructor checks that embedded providers died first and that its
    // retained registrar rejects late staging while callbacks are still live.
    runtime.close().unwrap();
}

#[test]
fn host_rejects_missing_artifact_entry_and_nul_path() {
    let artifact = Artifact::compile("operations.c");
    assert!(matches!(
        unsafe { native::load(&artifact.library) },
        Err(Error::UNSUPPORTED)
    ));
    assert!(matches!(
        unsafe { native::load(&artifact.directory.join("missing.so")) },
        Err(Error::UNAVAILABLE)
    ));
    assert!(matches!(
        unsafe { native::load(Path::new("invalid\0path")) },
        Err(Error::INVALID)
    ));
}

fn invoke(
    runtime: &mut Runtime,
    caller: &Caller,
    operation: Operation,
    subject: AccessId,
    command: u32,
    input: &[u8],
) -> Value {
    let request = Request::reserve(
        caller,
        operation,
        subject,
        command,
        None,
        Value::copy(input, &[]).unwrap(),
    )
    .unwrap();
    request.submit().unwrap();
    let result = reovim_host_reference::run(request.outcome(), |cx| {
        runtime.advance(cx).map(|_| ())
    })
    .unwrap()
    .unwrap();
    let Outcome::Ok(value) = result.as_ref() else { panic!("call failed") };
    request.adopt().unwrap();
    request.release().unwrap();
    value.clone()
}

fn discover(
    runtime: &mut Runtime,
    resource: ResourceId,
    create_access: u32,
) -> (Caller, AccessId, Declaration) {
    let caller = runtime
        .bind(&[Grant {
            resource,
            access: 5,
            create_access,
            retain_loans: false,
        }])
        .unwrap()
        .caller();
    discover_using(runtime, resource, caller)
}

fn discover_using(
    runtime: &mut Runtime,
    resource: ResourceId,
    caller: Caller,
) -> (Caller, AccessId, Declaration) {
    let mut locator = 1_u32.to_le_bytes().to_vec();
    locator.extend_from_slice(&resource.epoch);
    locator.extend_from_slice(&resource.serial.to_le_bytes());
    let opened =
        invoke(runtime, &caller, Operation::Acquire, AccessId(0), 0, &locator)
            .references[0];
    let mut page_input = 0_u64.to_le_bytes().to_vec();
    page_input.extend_from_slice(&65536_u64.to_le_bytes());
    let page =
        invoke(runtime, &caller, Operation::Interfaces, opened, 0, &page_input);
    let length = u32::from_le_bytes(page.data[4..8].try_into().unwrap());
    let encoded = &page.data[8..8 + length as usize];
    let declaration = Declaration::decode(encoded).unwrap();
    let mut bind_input = length.to_le_bytes().to_vec();
    bind_input.extend_from_slice(encoded);
    let bound =
        invoke(runtime, &caller, Operation::Bind, opened, 0, &bind_input)
            .references[0];
    invoke(
        runtime,
        &caller,
        Operation::SetAccess,
        opened,
        0,
        &5_u32.to_le_bytes(),
    );
    (caller, bound, declaration)
}

fn read_discovered(runtime: &mut Runtime, resource: ResourceId) -> u64 {
    let (caller, bound, declaration) = discover(runtime, resource, 0);
    let command = declaration
        .methods()
        .iter()
        .find_map(|method| match method.operation() {
            Declared::Ioctl(command) => Some(command),
            Declared::Exec => None,
        })
        .unwrap();
    let value = invoke(runtime, &caller, Operation::Ioctl, bound, command, &[]);
    u64::from_le_bytes(value.data.as_ref().try_into().unwrap())
}

#[test]
fn server_loads_memory_factory_and_reads_independent_opaque_resources() {
    let mut runtime =
        reovim_server::assemble([0x94; 16], Limits::default()).unwrap();
    let resource = rust_module(&mut runtime, b"/memory", "memory");
    let (denied, factory, _) = discover(&mut runtime, resource, 0);
    let request = Request::reserve(
        &denied,
        Operation::Exec,
        factory,
        0,
        None,
        Value::copy(&0_u32.to_le_bytes(), &[]).unwrap(),
    )
    .unwrap();
    assert_eq!(request.submit(), Err(Error::DENIED));
    request.release().unwrap();
    let (caller, bound, declaration) = discover(&mut runtime, resource, 1);
    assert!(declaration.methods()[0].created().is_some());
    let first = create_memory(&mut runtime, &caller, bound, b"\0\xff\t\n");
    let second = create_memory(&mut runtime, &caller, bound, b"x");
    assert_ne!(first, second);
    assert_eq!(
        read_memory(&mut runtime, &caller, first, 0, 8),
        (b"\0\xff\t\n".to_vec(), true)
    );
    assert_eq!(
        read_memory(&mut runtime, &caller, first, 1, 2),
        (b"\xff\t".to_vec(), false)
    );
    assert_eq!(
        read_memory(&mut runtime, &caller, second, 0, 8),
        (b"x".to_vec(), true)
    );
    let limit_bytes = vec![0xff; 4096];
    let maximum = create_memory(&mut runtime, &caller, bound, &limit_bytes);
    assert_eq!(
        read_memory(&mut runtime, &caller, maximum, 0, 4096),
        (limit_bytes, true)
    );
    let write = Request::reserve(
        &caller,
        Operation::SetAccess,
        first,
        0,
        None,
        Value::copy(&3_u32.to_le_bytes(), &[]).unwrap(),
    )
    .unwrap();
    assert_eq!(write.submit(), Err(Error::DENIED));
    write.release().unwrap();
    invoke(&mut runtime, &caller, Operation::Close, maximum, 0, &[]);
    invoke(&mut runtime, &caller, Operation::Close, first, 0, &[]);
    invoke(&mut runtime, &caller, Operation::Close, second, 0, &[]);
    runtime.advance(&mut Context::from_waker(Waker::noop())).unwrap();
    runtime.close().unwrap();
}

fn create_memory(
    runtime: &mut Runtime,
    caller: &Caller,
    factory: AccessId,
    bytes: &[u8],
) -> AccessId {
    let mut input = u32::try_from(bytes.len()).unwrap().to_le_bytes().to_vec();
    input.extend_from_slice(bytes);
    let value = invoke(runtime, caller, Operation::Exec, factory, 0, &input);
    let opened = value.references[0];
    invoke(
        runtime,
        caller,
        Operation::SetAccess,
        opened,
        0,
        &1_u32.to_le_bytes(),
    );
    opened
}

fn read_memory(
    runtime: &mut Runtime,
    caller: &Caller,
    opened: AccessId,
    offset: u64,
    maximum: u64,
) -> (Vec<u8>, bool) {
    let mut input = offset.to_le_bytes().to_vec();
    input.extend_from_slice(&maximum.to_le_bytes());
    let value = invoke(runtime, caller, Operation::ReadAt, opened, 0, &input);
    let length =
        u32::from_le_bytes(value.data[..4].try_into().unwrap()) as usize;
    let bytes = value.data[4..4 + length].to_vec();
    (bytes, value.data[4 + length] == 1)
}

#[path = "native_module/dependencies.rs"]
mod dependencies;

#[path = "native_module/creation.rs"]
mod creation;
