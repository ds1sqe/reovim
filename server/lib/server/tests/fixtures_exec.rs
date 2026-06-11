//! Integration test that builds the `#![no_std] #![no_main]` server-runtime
//! selftest fixture, execs it, and asserts its exit code (#797 acceptance).
//!
//! Bootstrap state 1 (1.2 §10): this is a std libtest integration binary —
//! the `no_std` test runner is the `server-selftest` bin, which carries the
//! end-to-end UDS smoke (boot → register text Domain → listener → raw framed
//! client). This file only orchestrates the fixture build and process exec.

use std::{
    path::{Path, PathBuf},
    process::Command,
};

/// Builds the fixture package in the nested workspace and returns the
/// executable path parsed from cargo's JSON output.
fn build_fixture(pkg: &str) -> PathBuf {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/Cargo.toml");
    let out = Command::new(env!("CARGO"))
        .args([
            "build",
            "--manifest-path",
            manifest.to_str().expect("manifest path is UTF-8"),
            "--package",
            pkg,
            "--message-format=json-render-diagnostics",
        ])
        .output()
        .expect("cargo build runs");
    assert!(
        out.status.success(),
        "fixture build failed:\n{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let stdout = String::from_utf8_lossy(&out.stdout);
    let exe = stdout
        .lines()
        .filter(|l| l.contains(&format!("\"{pkg}\"")))
        .filter_map(|l| {
            let key = "\"executable\":\"";
            let start = l.find(key)? + key.len();
            let rest = &l[start..];
            let end = rest.find('"')?;
            Some(rest[..end].to_owned())
        })
        .next_back()
        .expect("cargo JSON names the built executable");
    PathBuf::from(exe)
}

#[test]
fn server_selftest_all_pass_exits_zero() {
    // The full server-runtime suite (incl. the UDS hello/attach/input smoke)
    // on the no_std runner: every test green is exit 0.
    let exe = build_fixture("server-selftest");
    let status = Command::new(&exe).status().expect("fixture executes");
    assert_eq!(status.code(), Some(0), "server selftest must pass");
}
