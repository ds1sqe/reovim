#![cfg(feature = "std")]

use std::process::{Command, Output};

#[test]
fn hosted_command_completes_both_native_scenarios() {
    for (scenario, expected) in [
        ("heading", "Some([72, 1, 101])"),
        ("keyed", "Some([75, 17, 1, 93, 0])"),
    ] {
        let output = run(scenario);
        assert!(output.status.success(), "{output:?}");
        let stdout = String::from_utf8(output.stdout).unwrap();
        let final_state = stdout.lines().last().unwrap();
        assert!(final_state.contains(&format!("client0={expected}")));
        assert!(final_state.contains(&format!("client1={expected}")));
        let events = String::from_utf8(output.stderr).unwrap();
        assert!(events.contains("kernel.editing/v1"));
        assert!(events.contains("stage=committed"));
        assert!(events.contains("stage=closed"));
    }
}

#[test]
fn unsupported_scenario_reports_usage_without_starting_editing() {
    let output = run("unknown");
    assert!(!output.status.success());
    assert_eq!(output.stdout.as_slice(), b"");
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(stderr.contains("usage: reovim-server"));
    assert!(!stderr.contains("kernel.editing/v1"));
}

fn run(scenario: &str) -> Output {
    Command::new(env!("CARGO_BIN_EXE_reovim-server"))
        .arg(scenario)
        .output()
        .expect("execute the packaged local reference")
}
