//! End-to-end integration: `pkg trigger` opens the cdylibs whose
//! lockfile entry's trigger matches the requested event.

use std::{fs, path::PathBuf};

use {assert_cmd::Command, predicates::str::contains, tempfile::TempDir};

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

fn stage_lazy_fixture() -> (TempDir, PathBuf) {
    let dir = tempfile::tempdir().expect("tempdir");
    let root = dir.path().to_path_buf();

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
        .arg(&root)
        .arg("--library-root")
        .arg(&lib)
        .assert()
        .success();

    (dir, lib)
}

#[test]
fn trigger_on_domain_loads_expected_cdylib() {
    let (dir, lib) = stage_lazy_fixture();
    pkg()
        .arg("trigger")
        .arg("domain")
        .arg("text")
        .arg("--library-root")
        .arg(&lib)
        .arg("--lockfile")
        .arg(dir.path().join("pkg.lock"))
        .assert()
        .success()
        .stdout(contains("loaded alpha"));
}

#[test]
fn trigger_on_unknown_domain_is_zero_matches_but_exits_0() {
    let (dir, lib) = stage_lazy_fixture();
    pkg()
        .arg("trigger")
        .arg("domain")
        .arg("nonsense")
        .arg("--library-root")
        .arg(&lib)
        .arg("--lockfile")
        .arg(dir.path().join("pkg.lock"))
        .assert()
        .success()
        .stdout("");
}

#[test]
fn trigger_loads_every_package_matching_the_event() {
    let dir = tempfile::tempdir().expect("tempdir");
    let root = dir.path().to_path_buf();

    fs::write(
        root.join("pkg.toml"),
        "[package]\n\
         name = \"root-setup\"\n\
         reovim-version = \"^0.15\"\n\
         \n\
         [dependencies]\n\
         alpha = { path = \"alpha\" }\n\
         beta = { path = \"beta\" }\n\
         \n\
         [lazy]\n\
         alpha = { on-domain = \"text\" }\n\
         beta = { on-domain = \"text\" }\n",
    )
    .unwrap();
    for name in ["alpha", "beta"] {
        fs::create_dir_all(root.join(name).join("dist")).unwrap();
        fs::write(
            root.join(name).join("pkg.toml"),
            format!(
                "[package]\nname = \"{name}\"\nversion = \"1.0.0\"\nkind = \"module\"\nreovim-version = \"^0.15\"\n",
            ),
        )
        .unwrap();
        fs::copy(driver_poc_so(), root.join(format!("{name}/dist/libreovim_pkg_{name}.so")))
            .unwrap();
    }

    let lib = root.join("library-root");
    pkg()
        .arg("install")
        .arg("--manifest-dir")
        .arg(&root)
        .arg("--library-root")
        .arg(&lib)
        .assert()
        .success();

    pkg()
        .arg("trigger")
        .arg("domain")
        .arg("text")
        .arg("--library-root")
        .arg(&lib)
        .arg("--lockfile")
        .arg(root.join("pkg.lock"))
        .assert()
        .success()
        .stdout(contains("loaded alpha"))
        .stdout(contains("loaded beta"));
}

#[test]
fn trigger_with_missing_lockfile_exits_1() {
    let dir = tempfile::tempdir().unwrap();
    let lib = dir.path().join("library-root");
    pkg()
        .arg("trigger")
        .arg("domain")
        .arg("text")
        .arg("--library-root")
        .arg(&lib)
        .arg("--lockfile")
        .arg(dir.path().join("pkg.lock"))
        .assert()
        .code(1)
        .stderr(contains("lockfile"));
}
