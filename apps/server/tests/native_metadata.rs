#![cfg(all(feature = "std", target_os = "linux"))]

use reovim_host_reference::{native::Artifact, run};
use reovim_kernel_interface::{
    Contract, Message, buffer::integer, module, provider::Permissions,
};
use reovim_kernel_runtime::{Limits, Runtime};
use std::{fs, path::Path, process::Command};

fn finish<T>(
    runtime: &mut Runtime,
    future: impl core::future::Future<Output = T>,
) -> T {
    run(future, |context| runtime.advance(context).map(|_| ())).unwrap()
}

#[test]
#[allow(unsafe_code)]
fn native_contracts_use_the_existing_bound_catalog_dependency_lifecycle() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let installed = std::env::temp_dir()
        .join(format!("reovim-native-graph-{}", std::process::id()));
    fs::create_dir(&installed).unwrap();
    fs::copy(
        root.join("host/reference/include/reovim.h"),
        installed.join("reovim.h"),
    )
    .unwrap();
    fs::copy(
        root.join("apps/server/tests/native/metadata.c"),
        installed.join("module.c"),
    )
    .unwrap();
    let mut runtime = Runtime::new(195, Limits::default()).unwrap();
    let scope = runtime.register_namespace("clients/native").unwrap();
    let owner = runtime.bind_at(scope, &[(scope, Permissions::ALL)]).unwrap();
    for name in ["feature", "storage", "index"] {
        let path = compile(&installed, name);
        let artifact = unsafe { Artifact::open(&path) }.unwrap();
        let binding =
            runtime.bind_for(&owner, &[(scope, Permissions::ALL)]).unwrap();
        let mount =
            owner.publication_path(&format!("services/{name}")).unwrap();
        let factory = artifact.factory(&mount, binding.connection());
        let catalog =
            owner.publication_path(&format!("catalog/{name}")).unwrap();
        runtime
            .register_bound_factory(
                &binding,
                &catalog,
                artifact.identity().clone(),
                factory,
            )
            .map_err(|(error, _, _)| error)
            .unwrap();
        fs::remove_file(path).unwrap();
    }
    runtime.start().unwrap();
    let kernel = reovim_sdk::local(owner.connection());
    let root = kernel.resolve("catalog/feature").into_inner().unwrap();
    let feature =
        finish(&mut runtime, module::activate(&kernel, root)).unwrap();
    for name in ["feature", "storage", "index"] {
        let subject =
            kernel.resolve(&format!("services/{name}")).into_inner().unwrap();
        let read = kernel.read(
            subject,
            &Message::new(Contract::new("example.constructions", 1), &[]),
        );
        assert_eq!(integer(&finish(&mut runtime, read).unwrap().data), Ok(1));
    }
    finish(&mut runtime, module::unload(&kernel, feature)).unwrap();
    for name in ["storage", "index"] {
        let catalog =
            kernel.resolve(&format!("catalog/{name}")).into_inner().unwrap();
        let instance =
            finish(&mut runtime, module::activate(&kernel, catalog)).unwrap();
        finish(&mut runtime, module::unload(&kernel, instance)).unwrap();
    }
    runtime.retire_client(&owner).unwrap();
    assert!(runtime.resource("clients/native").is_err());
    runtime.close().unwrap();
    fs::remove_dir_all(installed).unwrap();
}

fn compile(directory: &Path, name: &str) -> std::path::PathBuf {
    let library = format!("{name}.so");
    let mut command = Command::new("cc");
    command
        .current_dir(directory)
        .args([
            "-std=c11", "-Wall", "-Wextra", "-Werror", "-shared", "-fPIC",
            "-I.", "module.c", "-o", &library,
        ])
        .arg(format!("-DMODULE_NAME=\"example.{name}\""));
    command.arg(if name == "feature" { "-DREQUIRE_STORAGE" } else { "-DLEAF" });
    let result = command.output().unwrap();
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    directory.join(library)
}

#[test]
#[allow(unsafe_code)]
fn discovery_is_passive_and_construction_uses_the_retained_artifact() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let installed = std::env::temp_dir()
        .join(format!("reovim-native-metadata-{}", std::process::id()));
    fs::create_dir(&installed).unwrap();
    // The independently compiled source needs only the installed public header.
    fs::copy(
        root.join("host/reference/include/reovim.h"),
        installed.join("reovim.h"),
    )
    .unwrap();
    fs::copy(
        root.join("apps/server/tests/native/metadata.c"),
        installed.join("module.c"),
    )
    .unwrap();
    let library = installed.join("feature.so");
    let built = Command::new("cc")
        .current_dir(&installed)
        .args([
            "-std=c11",
            "-Wall",
            "-Wextra",
            "-Werror",
            "-shared",
            "-fPIC",
            "-I.",
            "module.c",
            "-o",
            "feature.so",
        ])
        .output()
        .unwrap();
    assert!(
        built.status.success(),
        "{}",
        String::from_utf8_lossy(&built.stderr)
    );
    let artifact = unsafe { Artifact::open(&library) }.unwrap();
    assert!(matches!(
        unsafe {
            reovim_host_reference::native::load_contributions(
                &library, "bypass", None,
            )
        },
        Err(reovim_kernel_interface::Error::UNSUPPORTED)
    ));
    assert_eq!(artifact.identity(), &Contract::new("example.feature", 1));
    assert_eq!(
        artifact.dependencies(),
        [
            Contract::new("example.storage", 1),
            Contract::new("example.index", 1)
        ]
    );
    fs::remove_file(library).unwrap();
    let contributions = artifact.construct("feature", None).unwrap();
    drop(artifact);
    let mut runtime = Runtime::new(194, Limits::default()).unwrap();
    let registered = runtime
        .register_contributions(contributions)
        .map_err(|(error, _)| error)
        .unwrap();
    runtime.start().unwrap();
    let kernel =
        runtime.connect(&[(registered[0], Permissions::READ)]).unwrap();
    let subject = kernel.resolve("feature").into_inner().unwrap();
    let reply = run(
        kernel.read(
            subject,
            &Message::new(Contract::new("example.constructions", 1), &[]),
        ),
        |context| runtime.advance(context).map(|_| ()),
    )
    .unwrap()
    .unwrap();
    assert_eq!(integer(&reply.data), Ok(1));
    runtime.close().unwrap();
    fs::remove_dir_all(installed).unwrap();
}
