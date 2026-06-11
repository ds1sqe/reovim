//! Integration test that builds the `#![no_std] #![no_main]` TUI platform
//! selftest fixture, execs it, and asserts its exit code (#797 Phase 5).
//!
//! Bootstrap state 1 (1.2 §10): this is a std libtest integration binary —
//! the `no_std` test runner is the `tui-selftest` bin, which carries the
//! L12 unit tests for `carrier.rs` (`is_disconnect`) and `frame.rs`
//! (`compose_ansi_frame` pure-function suite). This file only orchestrates
//! the fixture build and process exec.

use std::{
    path::{Path, PathBuf},
    process::Command,
    sync::{Mutex, OnceLock},
};

// ---------------------------------------------------------------------------
// Serialization guard for the tui-selftest tests
//
// `tui_selftest_all_pass_exits_zero` and `tui_selftest_inject_failure_exits_nonzero`
// both build package `tui-selftest` with DIFFERENT feature sets (`[]` vs
// `["inject-failure"]`), and cargo writes both variants to the same artifact
// path. The guard must span build AND exec: serializing only the builds would
// still let one test relink the binary between the other test's build and exec,
// swapping in the wrong feature variant.
//
// A poisoned lock is recovered deliberately: the guard protects a cargo artifact
// that the next build regenerates, not in-memory state, so a panic in one test
// (a failed assertion) leaves nothing corrupt behind.
// ---------------------------------------------------------------------------
fn selftest_lock() -> std::sync::MutexGuard<'static, ()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

// ---------------------------------------------------------------------------
// Build helpers
// ---------------------------------------------------------------------------

/// Builds the fixture crate `pkg` and returns the path to the built executable,
/// parsed from cargo's JSON build messages so no target dir / profile is
/// hardcoded.
///
/// The fixtures live in a nested workspace
/// (`ext/client/platforms/tui/tests/fixtures/`) excluded from the parent, so
/// the build is scoped by that manifest path — keeping the `runtime` feature
/// off the parent's `cargo test --workspace` graph.
fn build_fixture(pkg: &str) -> PathBuf {
    build_fixture_features(pkg, &[])
}

/// Builds the fixture crate `pkg` with the given cargo `features` enabled.
fn build_fixture_features(pkg: &str, features: &[&str]) -> PathBuf {
    let fixtures_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures");
    let mut args = vec![
        "build".to_owned(),
        "--package".to_owned(),
        pkg.to_owned(),
        "--message-format=json-render-diagnostics".to_owned(),
    ];
    if !features.is_empty() {
        args.push("--features".to_owned());
        args.push(features.join(","));
    }
    let output = Command::new(env!("CARGO"))
        .current_dir(&fixtures_dir)
        .args(&args)
        .stderr(std::process::Stdio::inherit())
        .output()
        .expect("cargo build runs");
    assert!(output.status.success(), "fixture {pkg} builds");

    let stdout = String::from_utf8(output.stdout).expect("cargo json is UTF-8");
    let exe = stdout
        .lines()
        .filter(|l| l.contains("\"compiler-artifact\""))
        .filter(|l| l.contains(pkg))
        .filter_map(extract_executable)
        .next_back()
        .unwrap_or_else(|| panic!("no executable artifact for {pkg}"));
    PathBuf::from(exe)
}

/// Extracts the `"executable":"..."` value from one cargo JSON message line.
/// Returns `None` when the field is absent or null.
fn extract_executable(line: &str) -> Option<String> {
    let key = "\"executable\":";
    let start = line.find(key)? + key.len();
    let rest = line[start..].trim_start();
    let rest = rest.strip_prefix('"')?;
    let end = rest.find('"')?;
    Some(rest[..end].to_owned())
}

// ---------------------------------------------------------------------------
// Exec helpers
// ---------------------------------------------------------------------------

/// Runs `exe`, returns its exit code.
fn run(exe: &Path) -> i32 {
    let status = Command::new(exe)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .expect("fixture execs");
    status
        .code()
        .expect("fixture exited with a code, not a signal")
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[test]
fn tui_selftest_all_pass_exits_zero() {
    // The TUI platform test suite (carrier + frame unit tests) on the no_std
    // runner: every test green → exit 0.
    //
    // Serialized against `tui_selftest_inject_failure_exits_nonzero` via
    // `selftest_lock()` — see the guard's comment for the shared-artifact
    // hazard. The guard spans build AND exec.
    let _guard = selftest_lock();
    let exe = build_fixture("tui-selftest");
    assert_eq!(run(&exe), 0, "all-pass TUI test run exits 0");
}

#[test]
fn tui_selftest_inject_failure_exits_nonzero() {
    // The inject-failure variant adds one deliberately-failing test; the arch
    // panic handler exits the halt disposition code (70) — the runner's
    // fail-fast contract.
    //
    // Serialized against `tui_selftest_all_pass_exits_zero` via
    // `selftest_lock()` — see the guard's comment for the shared-artifact
    // hazard. The guard spans build AND exec.
    let _guard = selftest_lock();
    let exe = build_fixture_features("tui-selftest", &["inject-failure"]);
    assert_eq!(run(&exe), 70, "inject-failure exits the halt code");
}
