#![cfg(all(feature = "std", target_os = "linux"))]

#[path = "process/cli.rs"]
mod cli;
#[path = "process/support.rs"]
mod support;
use reovim_kernel_interface::{
    Value,
    caller::{Operation, Status},
};
use reovim_service::protocol::{Action, Reply};
use std::os::unix::ffi::OsStrExt;
use support::*;

#[test]
fn configured_process_serves_native_exports_and_drains_on_sigterm() {
    let artifacts = Artifacts::new();
    let library = artifacts.compile("initial.so");
    let mut server = Server::start(&artifacts, Some(&library));
    let (access, api) = server.peer.bind_path(b"/modules/device/first");
    let command = api
        .methods()
        .iter()
        .find_map(|method| match method.operation() {
            reovim_kernel_interface::declaration::Operation::Ioctl(command) => {
                Some(command)
            }
            reovim_kernel_interface::declaration::Operation::Exec => None,
        })
        .unwrap();
    let read =
        server.peer.call(Operation::Ioctl, access, command, Value::unit());
    assert_eq!(u64::from_le_bytes(read.data.as_ref().try_into().unwrap()), 81);
    let request = server.peer.reserve();
    server.terminate();
    server.peer.await_closed();
    assert_eq!(
        server.peer.exchange(Action::Result(request)),
        Reply::Status(Status::Reserved)
    );
    server.peer.send(Action::Release(request));
    assert!(server.finished().success());
}

#[test]
fn running_process_admits_and_activates_artifact_built_after_ready() {
    let artifacts = Artifacts::new();
    let mut server = Server::start(&artifacts, None);
    let library = artifacts.compile("late.so");
    let (load, _) = server.peer.bind_path(b"/load");
    let path = library.as_os_str().as_bytes();
    let mut input = u32::try_from(path.len()).unwrap().to_le_bytes().to_vec();
    input.extend_from_slice(path);
    let package = server.peer.call(
        Operation::Exec,
        load,
        0,
        Value::copy(&input, &[]).unwrap(),
    );
    std::fs::remove_file(library).unwrap();
    let mut locator = 1_u32.to_le_bytes().to_vec();
    locator.extend_from_slice(&package.data);
    let (activation, _) = server.peer.bind_locator(&locator);
    let instance =
        server.peer.call(Operation::Exec, activation, 0, Value::unit());
    let status = server.peer.call(
        Operation::Ioctl,
        instance.references[0],
        1,
        Value::unit(),
    );
    assert_eq!(u32::from_le_bytes(status.data.as_ref().try_into().unwrap()), 2);
    let entries = server.peer.entries(b"/loaded");
    let mut observed = Vec::new();
    for entry in entries {
        let mut locator = 1_u32.to_le_bytes().to_vec();
        locator.extend_from_slice(&entry.resource.epoch);
        locator.extend_from_slice(&entry.resource.serial.to_le_bytes());
        let (device, declaration) = server.peer.bind_locator(&locator);
        let command = read_command(&declaration);
        let value =
            server.peer.call(Operation::Ioctl, device, command, Value::unit());
        observed.push((
            entry.name,
            u64::from_le_bytes(value.data.as_ref().try_into().unwrap()),
        ));
    }
    assert_eq!(
        observed,
        vec![(Box::from(&b"first"[..]), 81), (Box::from(&b"second"[..]), 82)]
    );
    server.terminate();
    assert!(server.finished().success());
}

#[test]
fn missing_required_selection_fails_before_ready() {
    let artifacts = Artifacts::new();
    let output =
        std::process::Command::new(env!("CARGO_BIN_EXE_reovim-server"))
            .args(["127.0.0.1:0", "--trusted-module-root"])
            .arg(&artifacts.directory)
            .args(["--start", "absent"])
            .output()
            .unwrap();
    assert!(!output.status.success());
    assert_eq!(output.stdout.as_slice(), b"");
}

fn read_command(
    declaration: &reovim_kernel_interface::declaration::Declaration,
) -> u32 {
    declaration
        .methods()
        .iter()
        .find_map(|method| match method.operation() {
            reovim_kernel_interface::declaration::Operation::Ioctl(command) => {
                Some(command)
            }
            reovim_kernel_interface::declaration::Operation::Exec => None,
        })
        .unwrap()
}

#[test]
fn process_namespace_mutations_preserve_captured_subject() {
    let artifacts = Artifacts::new();
    let mut server = Server::start(&artifacts, None);
    let library =
        artifacts.compile_source("namespace.so", "namespace_module.c");
    activate_library(&mut server, &library);
    let path = b"/loaded/directory";
    let mut input = 0_u32.to_le_bytes().to_vec();
    input.extend_from_slice(&u32::try_from(path.len()).unwrap().to_le_bytes());
    input.extend_from_slice(path);
    let opened = server.peer.call(
        Operation::Acquire,
        reovim_kernel_interface::resource::AccessId(0),
        0,
        Value::copy(&input, &[]).unwrap(),
    );
    let access = opened.references[0];
    let info = server.peer.call(Operation::Info, access, 0, Value::unit());
    assert_eq!(u32::from_le_bytes(info.data[24..28].try_into().unwrap()), 0);
    assert_eq!(u32::from_le_bytes(info.data[28..32].try_into().unwrap()), 1);
    let (child, api) = server.peer.bind_path(b"/loaded/directory/disk");
    server.peer.call(
        Operation::SetAccess,
        access,
        0,
        Value::copy(&2_u32.to_le_bytes(), &[]).unwrap(),
    );
    for name in [&b"disk"[..], &b"alias"[..]] {
        let mut input =
            u32::try_from(name.len()).unwrap().to_le_bytes().to_vec();
        input.extend_from_slice(name);
        server.peer.call(
            Operation::Delete,
            access,
            0,
            Value::copy(&input, &[]).unwrap(),
        );
    }
    let info = server.peer.call(Operation::Info, access, 0, Value::unit());
    assert_eq!(u32::from_le_bytes(info.data[28..32].try_into().unwrap()), 3);
    let entries = server.peer.entries(path);
    let empty: Vec<reovim_kernel_interface::namespace::ResourceEntry> =
        Vec::new();
    assert_eq!(entries, empty);
    let value = server.peer.call(
        Operation::Ioctl,
        child,
        read_command(&api),
        Value::unit(),
    );
    assert_eq!(u64::from_le_bytes(value.data.as_ref().try_into().unwrap()), 41);
    let source = server.peer.call(Operation::Info, child, 0, Value::unit());
    let mut input = 0_u32.to_le_bytes().to_vec();
    input.extend_from_slice(&7_u32.to_le_bytes());
    input.extend_from_slice(b"revived");
    server.peer.call(
        Operation::Link,
        access,
        0,
        Value::copy(&input, &[child]).unwrap(),
    );
    let entries = server.peer.entries(path);
    assert_eq!(entries[0].name.as_ref(), b"revived");
    let mut input = 7_u32.to_le_bytes().to_vec();
    input.extend_from_slice(b"revived");
    input.extend_from_slice(&0_u32.to_le_bytes());
    input.extend_from_slice(&5_u32.to_le_bytes());
    input.extend_from_slice(b"moved");
    server.peer.call(
        Operation::Rename,
        access,
        0,
        Value::copy(&input, &[access]).unwrap(),
    );
    let entries = server.peer.entries(path);
    assert_eq!(entries[0].name.as_ref(), b"moved");
    assert_eq!(entries[0].resource.epoch, source.data[..16]);
    assert_eq!(
        entries[0].resource.serial,
        u64::from_le_bytes(source.data[16..24].try_into().unwrap())
    );
    server.peer.call(Operation::Close, child, 0, Value::unit());
    server.peer.call(Operation::Close, access, 0, Value::unit());
    server.terminate();
    assert!(server.finished().success());
}

fn activate_library(server: &mut Server, library: &std::path::Path) {
    let (load, _) = server.peer.bind_path(b"/load");
    let path = library.as_os_str().as_bytes();
    let mut input = u32::try_from(path.len()).unwrap().to_le_bytes().to_vec();
    input.extend_from_slice(path);
    let package = server.peer.call(
        Operation::Exec,
        load,
        0,
        Value::copy(&input, &[]).unwrap(),
    );
    let mut locator = 1_u32.to_le_bytes().to_vec();
    locator.extend_from_slice(&package.data);
    let (activation, _) = server.peer.bind_locator(&locator);
    server.peer.call(Operation::Exec, activation, 0, Value::unit());
}
