//! Integration tests for the `pkg` CLI scaffold.
//!
//! Covers: `--version`, `--help`, and each unimplemented subcommand
//! stub (exit code 2 with a phase-specific stderr message).

use {assert_cmd::Command, predicates::str::contains};

fn pkg() -> Command {
    Command::cargo_bin("pkg").expect("cargo bin `pkg` available")
}

#[test]
fn version_prints_workspace_version() {
    pkg()
        .arg("--version")
        .assert()
        .success()
        .stdout(contains(env!("CARGO_PKG_VERSION")));
}

#[test]
fn help_lists_every_subcommand() {
    let output = pkg().arg("--help").assert().success().get_output().clone();
    let stdout = String::from_utf8(output.stdout).expect("utf-8 stdout");
    for sub in ["install", "remove", "list", "lock", "resolve", "doctor"] {
        assert!(stdout.contains(sub), "help missing subcommand `{sub}`:\n{stdout}");
    }
}

#[test]
fn no_args_exits_successfully() {
    pkg().assert().success();
}

#[test]
fn install_stub_exits_two_and_mentions_phase_two() {
    pkg()
        .arg("install")
        .assert()
        .code(2)
        .stderr(contains("install"))
        .stderr(contains("#771 Phase 2"));
}

#[test]
fn remove_stub_exits_two_and_mentions_phase_two() {
    pkg()
        .args(["remove", "x"])
        .assert()
        .code(2)
        .stderr(contains("remove"))
        .stderr(contains("#771 Phase 2"));
}

#[test]
fn list_stub_exits_two_and_mentions_phase_two() {
    pkg()
        .arg("list")
        .assert()
        .code(2)
        .stderr(contains("list"))
        .stderr(contains("#771 Phase 2"));
}

#[test]
fn lock_stub_exits_two_and_mentions_phase_one() {
    pkg()
        .arg("lock")
        .assert()
        .code(2)
        .stderr(contains("lock"))
        .stderr(contains("#771 Phase 1"));
}

#[test]
fn resolve_stub_exits_two_and_mentions_phase_one() {
    pkg()
        .arg("resolve")
        .assert()
        .code(2)
        .stderr(contains("resolve"))
        .stderr(contains("#771 Phase 1"));
}

#[test]
fn doctor_stub_exits_two_and_mentions_phase_four() {
    pkg()
        .arg("doctor")
        .assert()
        .code(2)
        .stderr(contains("doctor"))
        .stderr(contains("#771 Phase 4"));
}

#[test]
fn unknown_subcommand_exits_with_clap_error() {
    pkg().arg("nonsense").assert().failure();
}
