#![cfg(all(feature = "std", target_os = "linux"))]

use reovim_host_reference::{native::load_model, run};
use reovim_kernel_interface::{
    Contract, Error, Message,
    buffer::{edit, observation, retain, retained},
    provider::Permissions,
};
use reovim_kernel_runtime::{Limits, Runtime};
use std::{future::Future, path::Path, process::Command};

fn finish<T>(runtime: &mut Runtime, future: impl Future<Output = T>) -> T {
    run(future, |context| runtime.advance(context).map(|_| ())).unwrap()
}
#[test]
#[allow(unsafe_code)]
fn native_nontext_model_edits_and_undoes_through_the_same_authority() {
    let mut runtime = Runtime::new(92, Limits::default()).unwrap();
    runtime.start().unwrap();
    let (directory, library) = build_model();
    // Fixture uses immutable states and matching native destructors.
    let model = unsafe { load_model(&library) }.unwrap();
    let id = runtime
        .register_model("models/native", model)
        .map_err(|(error, _)| error)
        .unwrap();
    let client =
        reovim_sdk::local(runtime.connect(&[(id, Permissions::ALL)]).unwrap());
    let model = finish(&mut runtime, client.resolve("models/native")).unwrap();
    let initial = finish(
        &mut runtime,
        client.write(
            model,
            &Message::new(Contract::new("native.keys.create", 1), &[]),
        ),
    )
    .unwrap();
    let initial = observation(initial).unwrap();
    let keys = reovim_domain_basic::decode_keys(&initial.value.data).unwrap();
    assert_eq!(keys.0.get(&17), Some(&true));
    assert_eq!(keys.0.get(&9001), Some(&false));
    let snapshot = retained(
        &finish(&mut runtime, retain(&client, initial.state)).unwrap(),
    )
    .unwrap();
    assert_eq!(snapshot.state, initial.state);
    assert_eq!(snapshot.node, initial.node);
    let request = edit(
        initial.state,
        Message::new(
            Contract::new("native.keys.toggle", 1),
            &9001_u64.to_le_bytes(),
        ),
    );
    let changed =
        finish(&mut runtime, client.write(initial.state.buffer, &request))
            .unwrap();
    let changed = observation(changed).unwrap();
    let keys = reovim_domain_basic::decode_keys(&changed.value.data).unwrap();
    assert_eq!(keys.0.get(&17), Some(&true));
    assert_eq!(keys.0.get(&9001), Some(&true));
    let selected = Message::new(Contract::new("native.keys", 1), &[1]);
    let old =
        finish(&mut runtime, client.read(snapshot.id, &selected)).unwrap();
    assert_eq!(old.data.as_ref(), &[0]);
    assert_eq!(
        finish(&mut runtime, retain(&client, initial.state)),
        Err(reovim_kernel_interface::RequestError::Failed(Error::STALE))
    );
    assert_eq!(runtime.unregister(id), Err(Error::BUSY));
    let undo = edit(
        changed.state,
        Message::new(Contract::new("kernel.history.undo", 1), &[]),
    );
    finish(&mut runtime, client.write(changed.history, &undo)).unwrap();
    let restored = finish(
        &mut runtime,
        client.read(
            initial.state.buffer,
            &Message::new(Contract::new("native.keys", 1), &[]),
        ),
    )
    .unwrap();
    let restored = observation(restored).unwrap();
    assert_eq!(restored.value, initial.value);
    assert_eq!(restored.node, initial.node);
    assert!(restored.state.revision > changed.state.revision);
    finish(
        &mut runtime,
        reovim_kernel_interface::buffer::close(
            &client,
            restored.state,
            reovim_kernel_interface::buffer::ClosePolicy::DiscardLocal,
        ),
    )
    .unwrap();
    // Shared close leaves the independently owned state and native code alive.
    assert_eq!(runtime.unregister(id), Err(Error::BUSY));
    let mut pending = client.read(snapshot.id, &selected);
    let mut context = std::task::Context::from_waker(std::task::Waker::noop());
    assert!(pending.as_mut().poll(&mut context).is_pending());
    runtime.advance(&mut context).unwrap();
    runtime.advance(&mut context).unwrap();
    client.close(snapshot.id).into_inner().unwrap();
    assert_eq!(runtime.unregister(id), Err(Error::BUSY));
    assert_eq!(finish(&mut runtime, pending).unwrap().data.as_ref(), &[0]);
    client.close(snapshot.id).into_inner().unwrap();
    assert_eq!(
        finish(&mut runtime, client.read(snapshot.id, &selected)),
        Err(reovim_kernel_interface::RequestError::Rejected(Error::CLOSED))
    );
    // C destroy asserts states died before context release; Runtime stays live.
    runtime.unregister(id).unwrap();
    still_live(&mut runtime);
    runtime.close().unwrap();
    std::fs::remove_dir_all(directory).unwrap();
}

fn still_live(runtime: &mut Runtime) {
    let fresh = runtime
        .register("still-live", reovim_resource_local::memory(b"alive"))
        .map_err(|(error, _)| error)
        .unwrap();
    let reader = runtime.connect(&[(fresh, Permissions::READ)]).unwrap();
    let resource = reader.resolve("still-live").into_inner().unwrap();
    let value = finish(
        runtime,
        reader.read(
            resource,
            &Message::new(Contract::new("resource.bytes", 1), &[]),
        ),
    )
    .unwrap();
    assert_eq!(value.data.as_ref(), b"alive");
}

fn build_model() -> (std::path::PathBuf, std::path::PathBuf) {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let directory = std::env::temp_dir()
        .join(format!("reovim-native-model-{}", std::process::id()));
    std::fs::create_dir_all(&directory).unwrap();
    let library = directory.join("keys.so");
    let output = Command::new("cc")
        .args(["-std=c11", "-Wall", "-Wextra", "-Werror", "-shared", "-fPIC"])
        .arg("-I")
        .arg(root.join("host/reference/include"))
        .arg(root.join("modules/native-keys/keys.c"))
        .arg("-o")
        .arg(&library)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    (directory, library)
}
