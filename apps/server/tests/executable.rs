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
        let final_state = stdout
            .lines()
            .rfind(|line| line.starts_with("request="))
            .expect("actual public state output");
        assert!(final_state.contains(&format!("client0={expected}")));
        assert!(final_state.contains(&format!("client1={expected}")));
        let events = String::from_utf8(output.stderr).unwrap();
        assert!(events.contains("kernel.editing/v2"));
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
    assert!(!stderr.contains("kernel.editing/v2"));
}

#[test]
fn measurement_smoke_keeps_both_profiles_and_intentional_failures() {
    use std::collections::BTreeSet;

    let output = run("--measure-smoke");
    assert!(output.status.success(), "{output:?}");
    let stdout = String::from_utf8(output.stdout).unwrap();
    let mut lines = stdout.lines();
    let header = lines.next().expect("raw measurement header");
    let observed: BTreeSet<_> =
        lines.map(|line| measurement_case(header, line)).collect();
    let mut expected = BTreeSet::new();
    for workload in ["heading-small", "heading-limit", "keyed-sparse"] {
        for enabled in ["false", "true"] {
            expected.extend(
                ["edit", "undo", "redo", "invalid-edit"]
                    .into_iter()
                    .map(|operation| (workload, enabled, operation)),
            );
        }
    }
    assert_eq!(observed, expected);
    let events = String::from_utf8(output.stderr).unwrap();
    assert!(events.contains("trace_cut=Complete"));
    assert!(events.contains("trace_cut=NotCaptured"));
}

fn measurement_case<'a>(
    header: &str,
    line: &'a str,
) -> (&'a str, &'a str, &'a str) {
    use std::collections::BTreeMap;

    let row: BTreeMap<_, _> = header.split(',').zip(line.split(',')).collect();
    assert_eq!(row["oracle"], "true", "{line}");
    assert_eq!(row["error"], "\"\"", "{line}");
    assert_eq!(row["warmup"], "false");
    if row["operation"] == "invalid-edit" {
        assert_eq!(row["cause"], "Some(PolicyFailure)");
        assert_eq!(row["peer_total_ns"], "NA");
    } else {
        assert_eq!(row["status"], "Some(Accepted)");
        row["peer_total_ns"].parse::<u64>().expect("observed nanoseconds");
    }
    (row["workload"], row["enabled"], row["operation"])
}

fn run(scenario: &str) -> Output {
    Command::new(env!("CARGO_BIN_EXE_reovim-server"))
        .arg(scenario)
        .output()
        .expect("execute the packaged local reference")
}
