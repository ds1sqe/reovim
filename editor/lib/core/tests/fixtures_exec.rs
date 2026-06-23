//! Integration tests that build the `#![no_std] #![no_main]` editor-core fixtures,
//! exec the built binaries, and assert their exit codes and flushed LOG2
//! output (#796 Phase 5 acceptance).
//!
//! Bootstrap state 1 (1.2 §10): this is a std libtest integration binary —
//! the `no_std` test runner is the `editor-core-selftest` bin. This file only
//! orchestrates fixture builds and process execs; the fixtures themselves fly
//! the `no_std` flight code.
//!
//! Coverage note: the panic line's *exact bytes* vary per run (the LOG2
//! timestamp is the live monotonic clock), so the assertions PARSE the line
//! against the LOG2 grammar (9.5 §2) and match the level/emitter/address and
//! message fields — never exact bytes.
//!
//! # Native-only pin
//!
//! Unlike `arch/tests/fixtures_exec.rs`, which supports cross-target builds
//! via `ARCH_FIXTURE_TARGET`/`ARCH_FIXTURE_RUNNER` env vars, this harness
//! is pinned to native host execution. The editor-core fixtures link `reovim-editor-core`
//! which adds a non-trivial boot sequence; cross-target verification is deferred
//! until a second concrete use-case demands it (rule of three). A comment below
//! marks the extension point for when that need arises.

use std::{
    path::{Path, PathBuf},
    process::{Command, Stdio},
    sync::{Mutex, OnceLock},
};

// ---------------------------------------------------------------------------
// Serialization guard for the editor-core-selftest tests
//
// `editor_core_selftest_all_pass_exits_zero` and
// `editor_core_selftest_inject_failure_exits_nonzero` both build package
// `editor-core-selftest` with DIFFERENT feature sets (`[]` vs `["inject-failure"]`),
// and cargo writes both variants to the same artifact path. The guard must span
// build AND exec: serializing only the builds would still let one test relink
// the binary between the other test's build and exec, swapping in the wrong
// feature variant.
//
// A poisoned lock is recovered deliberately: the guard protects a cargo
// artifact that the next build regenerates, not in-memory state, so a panic
// in one test (a failed assertion) leaves nothing corrupt behind.
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

/// Builds the fixture crate `pkg` (with its `runtime` feature where applicable)
/// and returns the path to the built executable, parsed from cargo's JSON build
/// messages so no target dir / profile is hardcoded.
///
/// The fixtures live in a nested workspace (`editor/lib/core/tests/fixtures/`)
/// excluded from the parent, so the build is scoped by that manifest path —
/// keeping the `runtime` feature off the parent's `cargo test --workspace`
/// graph (see the nested workspace's `Cargo.toml` for the lang-item rationale).
///
/// Extension point: when cross-target support is needed, add
/// `EDITOR_CORE_FIXTURE_TARGET` / `EDITOR_CORE_FIXTURE_RUNNER` env var handling here,
/// mirroring `arch/tests/fixtures_exec.rs`. For now, native-only (see module
/// doc for the rule-of-three rationale).
fn build_fixture(pkg: &str) -> PathBuf {
    build_fixture_features(pkg, &[])
}

/// Builds the fixture crate `pkg` with the given cargo `features` enabled and
/// returns the built executable path. The features select bin variants (e.g.
/// the selftest runner's deliberately-failing test) without a second crate.
///
/// See [`build_fixture`] for the `CARGO_MANIFEST_DIR` and nested-workspace
/// rationale.
fn build_fixture_features(pkg: &str, features: &[&str]) -> PathBuf {
    // `CARGO_MANIFEST_DIR` is `editor/lib/core/` (the crate under test);
    // the nested fixture workspace lives at
    // `editor/lib/core/tests/fixtures/`.
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

// ---------------------------------------------------------------------------
// Exec helpers
// ---------------------------------------------------------------------------

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
    p.push(format!("reovim-editor-core-fixture-{tag}-{}.log", std::process::id()));
    p
}

/// Reads the flushed sink to a string and removes it.
fn read_and_remove(path: &Path) -> String {
    let s = std::fs::read_to_string(path)
        .unwrap_or_else(|e| panic!("read sink {}: {e}", path.display()));
    let _ = std::fs::remove_file(path);
    s
}

// ---------------------------------------------------------------------------
// Log-line parser — LOG2 grammar (9.5 §2)
// ---------------------------------------------------------------------------

/// Parses a editor-core-emitter LOG2 line out of `out` and asserts its fields
/// against the 9.5 §2 grammar: `[ts] emitter SP address ":" SP message`,
/// ts = `seconds.micros` (micros width 6), editor-core address has no `/`.
/// Asserts the emitter (`editor-core`), the `panic` subsystem address, and the
/// message substring — not exact bytes (the timestamp varies per run).
fn assert_log2_editor_core_panic(out: &str, msg_substr: &str) {
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
    assert_eq!(emitter, "editor-core", "editor-core-emitter form");
    let (address, message) = after.split_once(": ").expect("address ':' message");
    assert_eq!(address, "panic", "editor-core subsystem address");
    assert!(!address.contains('/'), "editor-core address has no '/'");
    assert!(message.contains(msg_substr), "message carries the panic text: {message:?}");
}

/// Asserts that `out` contains at least one well-formed LOG2 line (a line
/// matching the `[ts] emitter address: message` grammar), proving the flush
/// mirror carried ring content at panic time.
fn assert_has_log2_lines(out: &str) {
    let has_log2 = out.lines().any(|l| {
        // A LOG2 line starts with '[' (timestamp bracket) and contains ':'
        // preceded by an emitter and address token. This is a structural check,
        // not a grammar re-parser; the golden test in render_tests.rs owns the
        // exact grammar proof.
        l.starts_with('[') && l.contains(": ")
    });
    assert!(
        has_log2,
        "flushed sink must contain at least one LOG2 line (ring had content): {out:?}"
    );
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[test]
fn editor_core_selftest_all_pass_exits_zero() {
    // The editor-core test-suite charter smoke (pass side): all editor-core
    // `*_tests.rs` tests run on the in-repo no_std runner and exit 0.
    //
    // Serialized against `editor_core_selftest_inject_failure_exits_nonzero` via
    // `selftest_lock()` — see the guard's comment for the shared-artifact
    // hazard. The guard spans build AND exec.
    let _guard = selftest_lock();
    let exe = build_fixture("editor-core-selftest");
    assert_eq!(run(&exe, &[]), 0, "all-pass editor-core test run exits 0");
}

#[test]
fn editor_core_selftest_inject_failure_exits_nonzero() {
    // The editor-core test-suite charter smoke (fail side): the inject-failure
    // variant adds a deliberately-failing test; the arch panic handler exits
    // the halt disposition code (70) — the runner's fail-fast contract.
    //
    // Serialized against `editor_core_selftest_all_pass_exits_zero` via
    // `selftest_lock()` — see the guard's comment for the shared-artifact
    // hazard. The guard spans build AND exec.
    let _guard = selftest_lock();
    let exe = build_fixture_features("editor-core-selftest", &["inject-failure"]);
    assert_eq!(run(&exe, &[]), 70, "inject-failure exits the halt code");
}

#[test]
fn editor_core_panic_halt_flushes_log2_and_exits_70() {
    // AB12 integration smoke (halt path): the fixture boots the editor core,
    // opens a FileSink-equivalent flush fd, emits events into the ring, then
    // panics. The out-of-process assertions are:
    //   (a) exit code = 70 (Disposition::Halt)
    //   (b) flushed file contains LOG2 lines + the panic line
    //
    // AC (c) — "state-record stub observed the matching PanicRecord" — is
    // covered in-process by `init_tests.rs::record_panic_state_*` tests
    // (the `STATE_RECORD` AtomicU32 slot is inaccessible after process exit).
    // This comment intentionally documents the split so reviewers understand
    // why (c) is not asserted here.
    let exe = build_fixture("editor-core-panic-halt");
    let sink = scratch_path("halt");
    let code = run(&exe, &[sink.to_str().unwrap()]);
    let out = read_and_remove(&sink);
    assert_eq!(code, 70, "halt disposition exits EX_SOFTWARE (70)");
    assert_has_log2_lines(&out);
    assert_log2_editor_core_panic(&out, "halt-path editor-core fixture panic");
}

#[test]
fn editor_core_panic_recover_flushes_log2_and_exits_75() {
    // AB12 integration smoke (recover path): same structure as the halt
    // fixture but with Disposition::Recover — exits 75 (EX_TEMPFAIL).
    //
    // AC (c) split: same reasoning as `editor_core_panic_halt_flushes_log2_and_exits_70`.
    // The `init_tests.rs` selftest suite covers the state-record hook in-process.
    let exe = build_fixture("editor-core-panic-recover");
    let sink = scratch_path("recover");
    let code = run(&exe, &[sink.to_str().unwrap()]);
    let out = read_and_remove(&sink);
    assert_eq!(code, 75, "recover disposition exits EX_TEMPFAIL (75)");
    assert_has_log2_lines(&out);
    assert_log2_editor_core_panic(&out, "recover-path editor-core fixture panic");
}

// ---------------------------------------------------------------------------
// No-catch_unwind assertion (AB12: no `catch_unwind` in editor core)
// ---------------------------------------------------------------------------

#[test]
fn no_catch_unwind_in_editor_core_src() {
    // DAG6: no unwinder; AB12 — a panic reaches arch's handler directly.
    // `catch_unwind` in the editor-core crate would be unsound (no unwinder to
    // unwind through). Walk `editor/lib/core/src/` and fail if any `.rs`
    // file contains the string.
    //
    // The fixture harness (this file) is `std` and may use unwinding freely;
    // we restrict the scan to `src/` only (not `tests/`, which is std land).
    let editor_core_src = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src");
    let violations = walk_rs_files_for_pattern(&editor_core_src, "catch_unwind");
    assert!(
        violations.is_empty(),
        "editor/lib/core/src contains catch_unwind (DAG6/AB12 violation): {violations:?}"
    );
}

/// Recursively walks `dir` for `.rs` files and returns the paths of files
/// whose content contains `pattern`. Pure-Rust; no external grep dependency.
fn walk_rs_files_for_pattern(dir: &Path, pattern: &str) -> Vec<PathBuf> {
    let mut hits = Vec::new();
    walk_rs_files(dir, pattern, &mut hits);
    hits
}

/// Recursive helper for `walk_rs_files_for_pattern`.
fn walk_rs_files(dir: &Path, pattern: &str, hits: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            walk_rs_files(&path, pattern, hits);
        } else if path.extension().is_some_and(|e| e == "rs")
            && let Ok(content) = std::fs::read_to_string(&path)
            && content.contains(pattern)
        {
            hits.push(path);
        }
    }
}
