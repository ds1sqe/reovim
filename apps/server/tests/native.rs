#![cfg(all(feature = "std", target_os = "linux"))]

use reovim_host_reference::{native::load_resource, run};
use reovim_kernel_interface::{
    Contract, Message, buffer::integer, provider::Permissions,
};
use reovim_kernel_runtime::{Limits, Runtime};
use std::{path::Path, process::Command};

#[test]
#[allow(unsafe_code)]
fn loads_independent_c_provider_after_runtime_start() {
    let mut runtime = Runtime::new(71, Limits::default()).unwrap();
    runtime.start().unwrap();
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let directory = std::env::temp_dir()
        .join(format!("reovim-native-{}", std::process::id()));
    std::fs::create_dir_all(&directory).unwrap();
    let library = directory.join("counter.so");
    let output = Command::new("cc")
        .args(["-std=c11", "-Wall", "-Wextra", "-Werror", "-shared", "-fPIC"])
        .arg("-I")
        .arg(root.join("host/reference/include"))
        .arg(root.join("modules/native-counter/counter.c"))
        .arg("-o")
        .arg(&library)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    // This fixture implements the header's ownership and no-unwind contract.
    let provider = unsafe { load_resource(&library) }.unwrap();
    let registration = runtime
        .register("modules/native/add", provider)
        .map_err(|(error, _)| error)
        .unwrap();
    let connection =
        runtime.connect(&[(registration, Permissions::ALL)]).unwrap();
    let result = run(
        async {
            let id = connection.resolve("modules/native/add").await.unwrap();
            let request =
                Message::new(Contract::new("native.counter", 1), &[7]);
            connection.write(id, &request).await.unwrap()
        },
        |context| runtime.advance(context).map(|_| ()),
    )
    .unwrap();
    assert_eq!(integer(&result.data), Ok(7));
    runtime.unregister(registration).unwrap();
    runtime.close().unwrap();
    std::fs::remove_dir_all(directory).unwrap();
}
