use super::*;
use reovim_kernel_interface::provider::{ResourceOps, ResourceProvider};
use reovim_kernel_runtime::Limits;
use std::{cell::Cell, path::Path, process::Command};

fn values() -> ValueLimits {
    ValueLimits { bytes: 8192, elements: 256, references: 8 }
}
fn config(label: &str, library: std::path::PathBuf) -> Config {
    Config {
        sources: alloc::vec![Source { label: label.into(), library }],
        start: alloc::vec![label.into()],
    }
}
fn namespace(runtime: &mut Runtime, path: &[u8]) -> ResourceId {
    let provider = ResourceProvider::new(
        (),
        ResourceOps {
            open: None,
            namespace: None,
            io: None,
            stop: |()| Ok(()),
        },
        alloc::boxed::Box::new([]),
        Rc::new(()),
    )
    .ok()
    .unwrap();
    runtime.register(path, provider, None).ok().unwrap()
}

#[test]
fn invalid_boot_rejects_before_host_admission() {
    let calls = Rc::new(Cell::new(0));
    let observed = calls.clone();
    let admit: Admit = Rc::new(move |_| {
        observed.set(observed.get() + 1);
        Err(Error::UNAVAILABLE)
    });
    let mut invalid = config("device", "unused.so".into());
    invalid.start.push("missing".into());
    assert!(matches!(
        Modules::discover(invalid, &admit, values(), 16),
        Err(Error::UNAVAILABLE)
    ));
    assert_eq!(calls.get(), 0);
    assert!(
        Config::parse(
            ["--module", "device", "device.so", "--start", "device"]
                .map(alloc::string::String::from)
                .into_iter()
        )
        .is_ok()
    );
    for arguments in [
        alloc::vec!["--start", "missing"],
        alloc::vec!["--module", "a", "a.so", "--module", "a", "b.so"],
        alloc::vec!["--module", "a", "a.so", "--start", "a", "--start", "a"],
        alloc::vec!["--client-module", "a", "a.so"],
        alloc::vec!["--session", "text"],
    ] {
        assert!(Config::parse(arguments.into_iter().map(Into::into)).is_err());
    }
}

#[test]
#[allow(unsafe_code)]
fn configured_definition_survives_artifact_deletion_and_failed_grant() {
    let mut runtime = crate::assemble([0xbd; 16], Limits::default()).unwrap();
    let directory = std::env::temp_dir()
        .join(alloc::format!("reovim-boot-{}", std::process::id()));
    std::fs::create_dir(&directory).unwrap();
    let library =
        directory.join(std::ffi::OsStr::from_bytes(b"fixture-\xff.so"));
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let status = Command::new("cc")
        .args(["-std=c11", "-Wall", "-Wextra", "-Werror", "-shared", "-fPIC"])
        .arg("-I")
        .arg(manifest.join("../../host/reference/include"))
        .arg(manifest.join("../../sdk/rust/tests/native/module.c"))
        .arg("-o")
        .arg(&library)
        .status()
        .unwrap();
    assert!(status.success());
    let allowed = library.clone();
    let admit: Admit = Rc::new(move |path| {
        assert_eq!(path, allowed.as_os_str().as_bytes());
        unsafe { reovim_host_reference::native::load(&allowed) }
    });
    let modules =
        Modules::discover(config("device", library), &admit, values(), 16)
            .unwrap();
    std::fs::remove_dir_all(directory).unwrap();
    assert_eq!(modules.config().start[0], "device");
    let target = Target {
        registry: namespace(&mut runtime, b"/catalog"),
        namespace: namespace(&mut runtime, b"/device"),
        imports: Rc::new(runtime.bind(&[]).unwrap()),
        recipient: Rc::new(runtime.bind(&[]).unwrap()),
        approved: alloc::boxed::Box::new([]),
    };
    runtime.retire_binding(&target.recipient).unwrap();
    let first = modules.register("device", &mut runtime, &target).unwrap_err();
    assert_eq!(first.code, Error::CLOSED.code);
    let repeated =
        modules.register("device", &mut runtime, &target).unwrap_err();
    assert_eq!(repeated, first);
    let target =
        Target { recipient: Rc::new(runtime.bind(&[]).unwrap()), ..target };
    let package = modules.register("device", &mut runtime, &target).unwrap();
    assert_eq!(first, Error::with_id(Error::CLOSED.code.0, package));
    assert_eq!(
        modules.register("device", &mut runtime, &target).unwrap(),
        package
    );
    let selection = runtime
        .select_module(
            target.registry,
            "fixture",
            1,
            reovim_kernel_interface::module::ModuleScope::Global,
        )
        .unwrap();
    assert_eq!(selection.modules()[0].resource(), package);
    let mut startup = Startup::new(target.recipient.caller(), package).unwrap();
    let mut cx = core::task::Context::from_waker(core::task::Waker::noop());
    for _ in 0..128 {
        startup.advance(&mut cx).unwrap();
        runtime.advance(&mut cx).unwrap();
        if startup.result().unwrap().is_some() {
            break;
        }
    }
    let instance = startup.result().unwrap().unwrap();
    assert!(startup.request().is_none());
    assert_eq!(
        runtime.module_status(instance),
        Ok(reovim_kernel_interface::driver::Lifecycle::Ready)
    );
    runtime.retire_binding(&target.recipient).unwrap();
    runtime.close().unwrap();
}

#[test]
fn startup_retains_denied_submission_for_retry_under_the_same_receipt() {
    let mut runtime = crate::assemble([0xbe; 16], Limits::default()).unwrap();
    let package = namespace(&mut runtime, b"/subject");
    let binding = runtime.bind(&[]).unwrap();
    let caller = binding.caller();
    let mut startup = Startup::new(caller.clone(), package).unwrap();
    let mut cx = core::task::Context::from_waker(core::task::Waker::noop());
    assert_eq!(startup.advance(&mut cx), Err(Error::DENIED));
    let original = startup.request().unwrap().id();
    assert_eq!(startup.result(), Ok(None));
    assert_eq!(startup.advance(&mut cx), Err(Error::DENIED));
    assert_eq!(startup.request().unwrap().id(), original);
    runtime
        .grant(
            &binding,
            Grant {
                resource: package,
                access: 5,
                create_access: 0,
                retain_loans: false,
            },
        )
        .unwrap();
    startup.advance(&mut cx).unwrap();
    assert_eq!(startup.request().unwrap().id(), original);
    startup.advance(&mut cx).unwrap();
    assert!(startup.request().is_none());
    assert!(caller.result(original, &mut cx).is_err());
    runtime.advance(&mut cx).unwrap();
    runtime.close().unwrap();
}
