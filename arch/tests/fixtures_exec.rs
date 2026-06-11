//! Integration tests that build the `#![no_std] #![no_main]` arch fixtures,
//! exec the built binaries, and assert their exit codes and flushed LOG2
//! output (#785 Phase 4 acceptance).
//!
//! Bootstrap state 1 (1.2 §10): this is a std libtest integration binary —
//! the `no_std` test runner is Phase 5. It only orchestrates fixture builds
//! and process execs; the fixtures themselves fly the `no_std` flight code.
//!
//! Coverage note: the panic line's *exact bytes* vary per run (the LOG2
//! timestamp is the live monotonic clock), so the assertions PARSE the line
//! against the LOG2 grammar (9.5 §2) and match the level/emitter/address and
//! message fields — never exact bytes.

use std::{
    path::{Path, PathBuf},
    process::{Command, Stdio},
};

/// Builds the fixture crate `pkg` (with its `runtime` feature) and returns the
/// path to the built executable, parsed from cargo's JSON build messages so no
/// target dir / profile is hardcoded.
///
/// The fixtures live in a nested workspace (`arch/tests/fixtures/`) excluded
/// from the parent, so the build is scoped by that manifest path — keeping the
/// `runtime` feature off the parent's `cargo test --workspace` graph (see the
/// nested workspace's `Cargo.toml` for the lang-item rationale).
fn build_fixture(pkg: &str) -> PathBuf {
    build_fixture_features(pkg, &[])
}

/// Builds the fixture crate `pkg` with the given cargo `features` enabled and
/// returns the built executable path. The features select bin variants (e.g.
/// the test-runner pilot's deliberately-failing test) without a second crate.
fn build_fixture_features(pkg: &str, features: &[&str]) -> PathBuf {
    // `CARGO_MANIFEST_DIR` is `arch/` (the crate under test); the nested
    // fixture workspace lives at `arch/tests/fixtures/`. Run the build with
    // that directory as the CWD so cargo resolves the nested workspace
    // `Cargo.toml`. Each fixture's `build.rs` emits the `-nostartfiles` link
    // arg the arch `_start` needs (build-script link args survive a `RUSTFLAGS`
    // override, unlike `.cargo/config.toml` rustflags).
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
        .stderr(Stdio::inherit())
        .output()
        .expect("cargo build runs");
    assert!(output.status.success(), "fixture {pkg} builds");

    // Each JSON line is a build message; the `compiler-artifact` for the bin
    // target carries `"executable":"<path>"`. Find the last one for `pkg`.
    let stdout = String::from_utf8(output.stdout).expect("cargo json is utf8");
    let exe = stdout
        .lines()
        .filter(|l| l.contains("\"compiler-artifact\""))
        .filter(|l| l.contains(pkg))
        .filter_map(extract_executable)
        .next_back()
        .unwrap_or_else(|| panic!("no executable artifact for {pkg}"));
    PathBuf::from(exe)
}

/// Extracts the `"executable":"..."` value from one cargo JSON message line,
/// or `None` when the field is absent or null. A minimal hand parser keeps the
/// test free of a JSON dependency (L9: zero third-party).
fn extract_executable(line: &str) -> Option<String> {
    let key = "\"executable\":";
    let start = line.find(key)? + key.len();
    let rest = line[start..].trim_start();
    let rest = rest.strip_prefix('"')?;
    let end = rest.find('"')?;
    Some(rest[..end].to_owned())
}

/// Runs `exe` with `args`, returns its exit code (the value the process exited
/// with; the test asserts the disposition codes).
fn run(exe: &Path, args: &[&str]) -> i32 {
    let status = Command::new(exe)
        .args(args)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .expect("fixture execs");
    status
        .code()
        .expect("fixture exited with a code, not a signal")
}

/// A unique scratch path under the OS temp dir for a fixture's flushed sink.
fn scratch_path(tag: &str) -> PathBuf {
    let mut p = std::env::temp_dir();
    p.push(format!("reovim-arch-fixture-{tag}-{}.log", std::process::id()));
    p
}

#[test]
fn smoke_fixture_boots_allocates_threads_exits_zero() {
    // The charter smoke: boot via `_start`, allocate a DS, spawn + join a
    // thread, exit 0. This is the phase's end-to-end proof.
    let exe = build_fixture("arch-fixture-smoke");
    assert_eq!(run(&exe, &[]), 0, "smoke fixture boots and exits 0");
}

#[test]
fn testrt_pilot_all_pass_exits_zero() {
    // The no_std runner charter smoke (pass side): the full arch test suite
    // runs on the in-repo runner and exits 0 when every test passes.
    let exe = build_fixture("arch-selftest");
    assert_eq!(run(&exe, &[]), 0, "all-pass test run exits 0");
}

#[test]
fn uapi_selftest_all_pass_exits_zero() {
    // uapi selftest charter smoke (pass side, #786 Phase 5): the full uapi
    // test suite — layout goldens, codec round-trips + 57-byte Hello frame
    // golden, CF5 cross-check, macro vtable smoke — all run on the no_std
    // runner and exit 0 when every test passes.
    let exe = build_fixture("uapi-selftest");
    assert_eq!(run(&exe, &[]), 0, "uapi all-pass test run exits 0");
}

#[test]
fn uapi_selftest_one_failure_exits_nonzero() {
    // uapi selftest charter smoke (fail side): the inject-failure variant
    // adds a deliberately-failing test; the arch panic handler exits the
    // halt disposition code (70) — the runner's fail-fast contract.
    let exe = build_fixture_features("uapi-selftest", &["inject-failure"]);
    assert_eq!(run(&exe, &[]), 70, "uapi inject-failure exits the halt code");
}

#[test]
fn testrt_pilot_one_failure_exits_nonzero() {
    // The no_std runner charter smoke (fail side): a deliberately-failing test
    // (the `inject-failure` variant) panics, so the arch panic handler exits
    // the halt disposition code (70) — the runner's fail-fast contract.
    let exe = build_fixture_features("arch-selftest", &["inject-failure"]);
    assert_eq!(run(&exe, &[]), 70, "a failing test exits the halt code");
}

#[test]
fn panic_halt_fixture_flushes_log2_and_exits_70() {
    let exe = build_fixture("arch-fixture-panic-halt");
    let sink = scratch_path("halt");
    let code = run(&exe, &[sink.to_str().unwrap()]);
    let line = read_and_remove(&sink);
    assert_eq!(code, 70, "default disposition halts with EX_SOFTWARE");
    assert_log2_kernel_panic(&line, "halt-path fixture panic");
    assert!(!line.contains("rollback=failed"), "non-cleanup panic has no rollback marker");
}

#[test]
fn panic_recover_fixture_fires_hook_and_exits_75() {
    let exe = build_fixture("arch-fixture-panic-recover");
    let sink = scratch_path("recover");
    let code = run(&exe, &[sink.to_str().unwrap()]);
    let out = read_and_remove(&sink);
    assert_eq!(code, 75, "recover disposition exits EX_TEMPFAIL");
    assert!(
        out.contains("state-hook: disposition=recover rollback=ok"),
        "state-record hook fired with the expected PanicRecord: {out:?}",
    );
    assert_log2_kernel_panic(&out, "recover-path fixture panic");
}

#[test]
fn panic_ab13_fixture_marks_rollback_failed() {
    let exe = build_fixture("arch-fixture-panic-ab13");
    let sink = scratch_path("ab13");
    let code = run(&exe, &[sink.to_str().unwrap()]);
    let line = read_and_remove(&sink);
    assert_eq!(code, 70, "cleanup panic with no disposition halts");
    assert!(line.contains("rollback=failed"), "AB13 marker in flushed line: {line:?}");
    assert_log2_kernel_panic(&line, "ab13 cleanup-context panic");
}

/// Reads the flushed sink to a string and removes it.
fn read_and_remove(path: &Path) -> String {
    let s = std::fs::read_to_string(path)
        .unwrap_or_else(|e| panic!("read sink {}: {e}", path.display()));
    let _ = std::fs::remove_file(path);
    s
}

/// Parses a kernel-emitter LOG2 line out of `out` and asserts its fields
/// against the 9.5 §2 grammar: `[ts] emitter SP address ":" SP message`,
/// ts = `seconds.micros` (micros width 6), kernel address has no `/`.
/// Asserts the emitter (`kernel`), the `panic` subsystem address, and the
/// message substring — not exact bytes (the timestamp varies per run).
fn assert_log2_kernel_panic(out: &str, msg_substr: &str) {
    let line = out
        .lines()
        .find(|l| l.contains("kernel panic:"))
        .unwrap_or_else(|| panic!("a panic line in {out:?}"));
    assert!(line.starts_with('['), "ts opens with '['");
    let close = line.find(']').expect("ts closes with ']'");
    let ts = &line[1..close];
    let (secs, micros) = ts.split_once('.').expect("ts is seconds.micros");
    assert!(secs.trim().parse::<u64>().is_ok(), "ts seconds numeric: {secs:?}");
    assert_eq!(micros.len(), 6, "micros zero-padded width 6");
    assert!(micros.parse::<u32>().is_ok(), "ts micros numeric: {micros:?}");
    let rest = line[close + 1..].trim_start();
    let (emitter, after) = rest.split_once(' ').expect("emitter token");
    assert_eq!(emitter, "kernel", "kernel-emitter form");
    let (address, message) = after.split_once(": ").expect("address ':' message");
    assert_eq!(address, "panic", "kernel subsystem address");
    assert!(!address.contains('/'), "kernel address has no '/'");
    assert!(message.contains(msg_substr), "message carries the panic text: {message:?}");
}
