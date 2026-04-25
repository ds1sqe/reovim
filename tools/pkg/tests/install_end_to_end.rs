//! End-to-end integration: `pkg install` → `pkg list` → `pkg remove`
//! round-trip against a staged fixture project.

use std::{fs, path::PathBuf};

use {assert_cmd::Command, predicates::str::contains, tempfile::TempDir};

fn driver_poc_so() -> PathBuf {
    // Resolve at test-run time from the test binary's `deps/`
    // sibling — build-time env vars race cargo's build order under
    // `cargo test --workspace`.
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

fn stage_fixture(kind: &str) -> TempDir {
    let dir = tempfile::tempdir().expect("tempdir");
    let root = dir.path();

    // Root user-config manifest.
    fs::write(
        root.join("pkg.toml"),
        "[package]\nname = \"root-setup\"\nreovim-version = \"^0.15\"\n\n[dependencies]\nalpha = { path = \"alpha\" }\n",
    )
    .unwrap();

    // Path-dep manifest with kind.
    fs::create_dir_all(root.join("alpha/dist")).unwrap();
    fs::write(
        root.join("alpha/pkg.toml"),
        format!(
            "[package]\nname = \"alpha\"\nversion = \"1.0.0\"\nkind = \"{kind}\"\nreovim-version = \"^0.15\"\n",
        ),
    )
    .unwrap();
    fs::copy(driver_poc_so(), root.join("alpha/dist/libreovim_pkg_alpha.so")).unwrap();

    dir
}

fn pkg() -> Command {
    Command::cargo_bin("pkg").expect("cargo bin `pkg` available")
}

#[test]
fn install_then_list_then_remove_round_trip() {
    let dir = stage_fixture("module");
    let lib = dir.path().join("library-root");

    pkg()
        .arg("install")
        .arg("--manifest-dir")
        .arg(dir.path())
        .arg("--library-root")
        .arg(&lib)
        .assert()
        .success();

    let installed_so = lib.join("modules/libreovim_pkg_alpha.so");
    assert!(installed_so.is_file(), "cdylib not placed at {}", installed_so.display());

    let list_output = pkg()
        .arg("list")
        .arg("--library-root")
        .arg(&lib)
        .arg("--lockfile")
        .arg(dir.path().join("pkg.lock"))
        .assert()
        .success()
        .get_output()
        .clone();
    let stdout = String::from_utf8(list_output.stdout).expect("utf-8");
    assert!(stdout.contains("alpha 1.0.0 module"), "list output: {stdout}");
    assert!(stdout.contains(&installed_so.display().to_string()));

    pkg()
        .arg("remove")
        .arg("alpha")
        .arg("--library-root")
        .arg(&lib)
        .arg("--lockfile")
        .arg(dir.path().join("pkg.lock"))
        .assert()
        .success();
    assert!(!installed_so.exists(), "cdylib still on disk after remove");
}

#[test]
fn install_exits_one_on_missing_dist_dir() {
    let dir = tempfile::tempdir().unwrap();
    fs::write(
        dir.path().join("pkg.toml"),
        "[package]\nname = \"root-setup\"\nreovim-version = \"^0.15\"\n\n[dependencies]\nalpha = { path = \"alpha\" }\n",
    )
    .unwrap();
    fs::create_dir_all(dir.path().join("alpha")).unwrap();
    fs::write(
        dir.path().join("alpha/pkg.toml"),
        "[package]\nname = \"alpha\"\nversion = \"1.0.0\"\nkind = \"module\"\nreovim-version = \"^0.15\"\n",
    )
    .unwrap();
    let lib = dir.path().join("library-root");
    pkg()
        .arg("install")
        .arg("--manifest-dir")
        .arg(dir.path())
        .arg("--library-root")
        .arg(&lib)
        .assert()
        .code(1)
        .stderr(contains("dist"));
}

#[test]
fn remove_exits_one_when_package_not_installed() {
    let dir = tempfile::tempdir().unwrap();
    fs::write(dir.path().join("pkg.lock"), "version = 1\n").unwrap();
    let lib = dir.path().join("library-root");
    pkg()
        .arg("remove")
        .arg("ghost")
        .arg("--library-root")
        .arg(&lib)
        .arg("--lockfile")
        .arg(dir.path().join("pkg.lock"))
        .assert()
        .code(1)
        .stderr(contains("ghost"));
}

#[test]
fn install_lazy_dep_emits_trigger_in_lockfile() {
    let dir = tempfile::tempdir().expect("tempdir");
    let root = dir.path();

    fs::write(
        root.join("pkg.toml"),
        "[package]\n\
         name = \"root-setup\"\n\
         reovim-version = \"^0.15\"\n\
         \n\
         [dependencies]\n\
         alpha = { path = \"alpha\" }\n\
         \n\
         [lazy]\n\
         alpha = { on-domain = \"text\" }\n",
    )
    .unwrap();
    fs::create_dir_all(root.join("alpha/dist")).unwrap();
    fs::write(
        root.join("alpha/pkg.toml"),
        "[package]\nname = \"alpha\"\nversion = \"1.0.0\"\nkind = \"module\"\nreovim-version = \"^0.15\"\n",
    )
    .unwrap();
    fs::copy(driver_poc_so(), root.join("alpha/dist/libreovim_pkg_alpha.so")).unwrap();
    let lib = root.join("library-root");

    pkg()
        .arg("install")
        .arg("--manifest-dir")
        .arg(root)
        .arg("--library-root")
        .arg(&lib)
        .assert()
        .success();

    let lock = fs::read_to_string(root.join("pkg.lock")).expect("read lock");
    assert!(
        lock.contains("trigger = \"on-domain:text\""),
        "trigger not emitted in lockfile:\n{lock}",
    );
}

#[test]
fn list_on_missing_lockfile_is_empty_not_error() {
    let dir = tempfile::tempdir().unwrap();
    let lib = dir.path().join("library-root");
    pkg()
        .arg("list")
        .arg("--library-root")
        .arg(&lib)
        .arg("--lockfile")
        .arg(dir.path().join("pkg.lock"))
        .assert()
        .success()
        .stdout("");
}
