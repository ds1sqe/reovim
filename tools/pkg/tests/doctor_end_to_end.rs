//! End-to-end integration: `pkg doctor` audit and `--fix` paths
//! against a staged library root + lockfile.

use std::{
    fmt::Write as _,
    fs,
    path::{Path, PathBuf},
};

use {
    assert_cmd::Command,
    predicates::str::contains,
    sha2::{Digest, Sha256},
    tempfile::TempDir,
};

fn driver_poc_so() -> PathBuf {
    let exe = std::env::current_exe().expect("test binary path");
    let deps = exe.parent().expect("test binary dir");
    for name in [
        "libreovim_driver_abi_poc.so",
        "libreovim_driver_abi_poc.dylib",
        "reovim_driver_abi_poc.dll",
    ] {
        let candidate = deps.join(name);
        if candidate.is_file() {
            return candidate;
        }
    }
    panic!("driver-abi-poc cdylib not found next to {}", exe.display());
}

fn pkg() -> Command {
    Command::cargo_bin("pkg").expect("cargo bin `pkg` available")
}

fn sha_of_file(path: &Path) -> String {
    let bytes = fs::read(path).unwrap();
    let mut s = String::with_capacity(64);
    for b in Sha256::digest(&bytes) {
        write!(s, "{b:02x}").unwrap();
    }
    s
}

fn stage_one_module(name: &str) -> (TempDir, PathBuf, PathBuf, PathBuf) {
    let dir = tempfile::tempdir().expect("tempdir");
    let root = dir.path().to_path_buf();
    fs::write(
        root.join("pkg.toml"),
        format!(
            "[package]\nname = \"root-setup\"\nreovim-version = \"^0.15\"\n\n\
             [dependencies]\n{name} = {{ path = \"{name}\" }}\n",
        ),
    )
    .unwrap();
    fs::create_dir_all(root.join(name).join("dist")).unwrap();
    fs::write(
        root.join(name).join("pkg.toml"),
        format!(
            "[package]\nname = \"{name}\"\nversion = \"1.0.0\"\nkind = \"module\"\nreovim-version = \"^0.15\"\n",
        ),
    )
    .unwrap();
    fs::copy(driver_poc_so(), root.join(format!("{name}/dist/libreovim_pkg_{name}.so"))).unwrap();
    let lib = root.join("library-root");
    pkg()
        .arg("install")
        .arg("--manifest-dir")
        .arg(&root)
        .arg("--library-root")
        .arg(&lib)
        .assert()
        .success();
    let installed = lib.join(format!("modules/libreovim_pkg_{name}.so"));
    let lockfile = root.join("pkg.lock");
    (dir, lib, installed, lockfile)
}

#[test]
fn doctor_clean_root_exits_0() {
    let (_dir, lib, _installed, lockfile) = stage_one_module("alpha");
    pkg()
        .arg("doctor")
        .arg("--library-root")
        .arg(&lib)
        .arg("--lockfile")
        .arg(&lockfile)
        .assert()
        .success()
        .stdout("");
}

#[test]
fn doctor_with_missing_emits_finding_and_exits_1() {
    let (_dir, lib, installed, lockfile) = stage_one_module("alpha");
    fs::remove_file(&installed).unwrap();
    pkg()
        .arg("doctor")
        .arg("--library-root")
        .arg(&lib)
        .arg("--lockfile")
        .arg(&lockfile)
        .assert()
        .code(1)
        .stdout(contains("missing alpha"));
}

#[test]
fn doctor_with_orphan_emits_finding_and_exits_1() {
    let (_dir, lib, _installed, lockfile) = stage_one_module("alpha");
    fs::copy(driver_poc_so(), lib.join("modules/libreovim_pkg_ghost.so")).unwrap();
    pkg()
        .arg("doctor")
        .arg("--library-root")
        .arg(&lib)
        .arg("--lockfile")
        .arg(&lockfile)
        .assert()
        .code(1)
        .stdout(contains("orphan"));
}

#[test]
fn doctor_with_drift_emits_finding_and_exits_1() {
    let (_dir, lib, installed, lockfile) = stage_one_module("alpha");
    let mut bytes = fs::read(&installed).unwrap();
    bytes.push(b'\n');
    fs::write(&installed, bytes).unwrap();
    pkg()
        .arg("doctor")
        .arg("--library-root")
        .arg(&lib)
        .arg("--lockfile")
        .arg(&lockfile)
        .assert()
        .code(1)
        .stdout(contains("drift alpha"));
}

#[test]
fn doctor_with_unloadable_emits_finding_and_exits_1() {
    let (_dir, lib, installed, lockfile) = stage_one_module("alpha");
    let garbage = b"hi\n";
    fs::write(&installed, garbage).unwrap();
    let new_sha = sha_of_file(&installed);
    let mut lock = fs::read_to_string(&lockfile).unwrap();
    let old_sha_line = lock
        .lines()
        .find(|l| l.starts_with("sha256 = "))
        .unwrap()
        .to_string();
    let new_sha_line = format!("sha256 = \"{new_sha}\"");
    lock = lock.replace(&old_sha_line, &new_sha_line);
    fs::write(&lockfile, lock).unwrap();
    pkg()
        .arg("doctor")
        .arg("--library-root")
        .arg(&lib)
        .arg("--lockfile")
        .arg(&lockfile)
        .assert()
        .code(1)
        .stdout(contains("unloadable alpha"));
}

#[test]
fn doctor_fix_resolves_orphan_then_exits_0() {
    let (_dir, lib, _installed, lockfile) = stage_one_module("alpha");
    let orphan_path = lib.join("modules/libreovim_pkg_ghost.so");
    fs::copy(driver_poc_so(), &orphan_path).unwrap();

    pkg()
        .arg("doctor")
        .arg("--fix")
        .arg("--library-root")
        .arg(&lib)
        .arg("--lockfile")
        .arg(&lockfile)
        .assert()
        .success()
        .stdout(contains("removed orphan"));
    assert!(!orphan_path.exists(), "orphan should be deleted");
}

#[test]
fn doctor_fix_resolves_missing_then_exits_0() {
    let (_dir, lib, installed, lockfile) = stage_one_module("alpha");
    fs::remove_file(&installed).unwrap();

    pkg()
        .arg("doctor")
        .arg("--fix")
        .arg("--library-root")
        .arg(&lib)
        .arg("--lockfile")
        .arg(&lockfile)
        .assert()
        .success()
        .stdout(contains("dropped stale inventory entry"));
    let lock = fs::read_to_string(&lockfile).unwrap();
    assert!(!lock.contains("alpha"), "alpha entry should be gone:\n{lock}");
}

#[test]
fn doctor_fix_leaves_drift_and_exits_1() {
    let (_dir, lib, installed, lockfile) = stage_one_module("alpha");
    let mut bytes = fs::read(&installed).unwrap();
    bytes.push(b'\n');
    fs::write(&installed, bytes).unwrap();

    pkg()
        .arg("doctor")
        .arg("--fix")
        .arg("--library-root")
        .arg(&lib)
        .arg("--lockfile")
        .arg(&lockfile)
        .assert()
        .code(1)
        .stdout(contains("drift alpha"));
}

#[test]
fn doctor_fix_leaves_unloadable_and_exits_1() {
    let (_dir, lib, installed, lockfile) = stage_one_module("alpha");
    let garbage = b"hi\n";
    fs::write(&installed, garbage).unwrap();
    let new_sha = sha_of_file(&installed);
    let mut lock = fs::read_to_string(&lockfile).unwrap();
    let old_sha_line = lock
        .lines()
        .find(|l| l.starts_with("sha256 = "))
        .unwrap()
        .to_string();
    lock = lock.replace(&old_sha_line, &format!("sha256 = \"{new_sha}\""));
    fs::write(&lockfile, lock).unwrap();

    pkg()
        .arg("doctor")
        .arg("--fix")
        .arg("--library-root")
        .arg(&lib)
        .arg("--lockfile")
        .arg(&lockfile)
        .assert()
        .code(1)
        .stdout(contains("unloadable alpha"));
}

#[test]
fn doctor_fix_partial_when_drift_and_orphan_both_present() {
    let (_dir, lib, installed, lockfile) = stage_one_module("alpha");
    let mut bytes = fs::read(&installed).unwrap();
    bytes.push(b'\n');
    fs::write(&installed, bytes).unwrap();
    let orphan_path = lib.join("modules/libreovim_pkg_ghost.so");
    fs::copy(driver_poc_so(), &orphan_path).unwrap();

    pkg()
        .arg("doctor")
        .arg("--fix")
        .arg("--library-root")
        .arg(&lib)
        .arg("--lockfile")
        .arg(&lockfile)
        .assert()
        .code(1)
        .stdout(contains("drift alpha"));
    assert!(!orphan_path.exists(), "orphan should be deleted even on partial fix");
}

#[test]
fn doctor_help_documents_fix_flag() {
    pkg()
        .arg("doctor")
        .arg("--help")
        .assert()
        .success()
        .stdout(contains("--fix"))
        .stdout(contains("audit"));
}
