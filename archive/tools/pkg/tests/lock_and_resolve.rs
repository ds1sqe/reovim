//! Integration tests for `pkg lock` and `pkg resolve`.
//!
//! Drives the resolver end-to-end through the `pkg` binary using
//! fixtures copied from `lib/pkg-resolver/tests/fixtures/` into a
//! `tempfile::tempdir()`.

use std::{
    fs,
    path::{Path, PathBuf},
};

use {assert_cmd::Command, predicates::str::contains, tempfile::TempDir};

fn resolver_fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../lib/pkg-resolver/tests/fixtures")
        .join(name)
}

fn copy_tree(src: &Path, dst: &Path) {
    fs::create_dir_all(dst).unwrap();
    for entry in fs::read_dir(src).unwrap() {
        let entry = entry.unwrap();
        let from = entry.path();
        let to = dst.join(entry.file_name());
        if entry.file_type().unwrap().is_dir() {
            copy_tree(&from, &to);
        } else {
            fs::copy(&from, &to).unwrap();
        }
    }
}

fn stage(fixture: &str) -> TempDir {
    let dir = tempfile::tempdir().expect("tempdir");
    copy_tree(&resolver_fixture(fixture), dir.path());
    dir
}

fn pkg() -> Command {
    Command::cargo_bin("pkg").expect("cargo bin `pkg` available")
}

#[test]
fn lock_writes_deterministic_pkg_lock() {
    let dir = stage("03-diamond");

    pkg()
        .arg("lock")
        .arg("--manifest-dir")
        .arg(dir.path())
        .assert()
        .success();

    let first = fs::read(dir.path().join("pkg.lock")).expect("lockfile written");
    pkg()
        .arg("lock")
        .arg("--manifest-dir")
        .arg(dir.path())
        .assert()
        .success();
    let second = fs::read(dir.path().join("pkg.lock")).expect("lockfile still there");
    assert_eq!(first, second, "two runs of `pkg lock` produce different bytes");
}

#[test]
fn lock_honors_custom_output_path() {
    let dir = stage("01-single-dep");
    let out = dir.path().join("nested/pkg.lock");
    fs::create_dir_all(out.parent().unwrap()).unwrap();

    pkg()
        .arg("lock")
        .arg("--manifest-dir")
        .arg(dir.path())
        .arg("--lockfile")
        .arg(&out)
        .assert()
        .success();
    assert!(out.exists(), "custom lockfile path not written: {}", out.display());
}

#[test]
fn resolve_prints_sorted_package_list() {
    let dir = stage("02-transitive-chain");

    let output = pkg()
        .arg("resolve")
        .arg("--manifest-dir")
        .arg(dir.path())
        .assert()
        .success()
        .get_output()
        .clone();
    let stdout = String::from_utf8(output.stdout).expect("utf-8");
    let lines: Vec<&str> = stdout.lines().collect();
    assert_eq!(lines.len(), 3, "expected 3 packages, got {stdout:?}");
    assert!(lines[0].starts_with("a 1.0.0 "));
    assert!(lines[1].starts_with("b 2.0.0 "));
    assert!(lines[2].starts_with("c 3.0.0 "));
}

#[test]
fn lock_exits_one_on_cycle_with_readable_error() {
    let dir = stage("06-cycle-simple");

    pkg()
        .arg("lock")
        .arg("--manifest-dir")
        .arg(dir.path())
        .assert()
        .code(1)
        .stderr(contains("cycle"))
        .stderr(contains("foo"))
        .stderr(contains("bar"));
}

#[test]
fn resolve_exits_one_on_conflict() {
    let dir = stage("08-conflict-version");

    pkg()
        .arg("resolve")
        .arg("--manifest-dir")
        .arg(dir.path())
        .assert()
        .code(1)
        .stderr(contains("shared"));
}

#[test]
fn lock_exits_one_when_manifest_missing() {
    let dir = tempfile::tempdir().unwrap();

    pkg()
        .arg("lock")
        .arg("--manifest-dir")
        .arg(dir.path())
        .assert()
        .code(1)
        .stderr(contains("manifest not found"));
}
