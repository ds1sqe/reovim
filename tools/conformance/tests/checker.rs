#![forbid(unsafe_code)]

use std::{
    collections::{BTreeMap, BTreeSet},
    ffi::OsString,
    fs, io,
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
    time::Duration,
};

use reovim_conformance::process::{CommandSpec, ProcessLimits, TerminationReason, run};

fn shell(script: &str, limits: ProcessLimits) -> reovim_conformance::process::ProcessResult {
    run(
        &CommandSpec {
            program: PathBuf::from("/bin/sh"),
            arguments: vec![OsString::from("-c"), OsString::from(script)],
            current_dir: PathBuf::from("/"),
            environment: BTreeMap::new(),
            clear_environment: true,
        },
        limits,
    )
    .expect("bounded helper process")
}

const fn limits(stdout_bytes: usize, stderr_bytes: usize, deadline_ms: u64) -> ProcessLimits {
    ProcessLimits {
        stdout_bytes,
        stderr_bytes,
        deadline: Duration::from_millis(deadline_ms),
        supervision_deadline: Duration::from_millis(deadline_ms + 2_000),
    }
}

const fn tight_supervision_limits(
    stdout_bytes: usize,
    stderr_bytes: usize,
    deadline_ms: u64,
) -> ProcessLimits {
    ProcessLimits {
        stdout_bytes,
        stderr_bytes,
        deadline: Duration::from_millis(deadline_ms),
        supervision_deadline: Duration::from_millis(deadline_ms + 100),
    }
}

fn assert_process_ownership(result: &reovim_conformance::process::ProcessResult) {
    assert!(result.reaped && result.readers_joined);
    #[cfg(target_os = "linux")]
    {
        assert_eq!(result.pgid, Some(result.leader_pid));
        assert!(result.group_identity_validated);
        assert!(result.group_absent_after_reap);
        assert!(result.descendants_owned);
        assert!(result.descendants_reaped);
    }
    #[cfg(not(target_os = "linux"))]
    {
        assert_eq!(result.pgid, None);
        assert!(!result.group_identity_validated);
        assert!(!result.group_absent_after_reap);
        assert!(!result.descendants_owned);
        assert!(!result.descendants_reaped);
    }
}

#[test]
fn exact_pipe_cap_is_not_overflow() {
    let result = shell("printf 1234", limits(4, 4, 500));
    assert_eq!(result.termination_reason, TerminationReason::Exited);
    assert_eq!(result.stdout, b"1234");
    assert!(!result.stdout_overflow);
    assert_process_ownership(&result);
}

#[test]
fn cap_plus_one_kills_or_records_the_exit_race_and_reaps() {
    let result = shell("printf 12345; while :; do :; done", limits(4, 64, 500));
    assert_eq!(result.termination_reason, TerminationReason::StdoutLimit);
    assert_eq!(result.stdout, b"1234");
    assert!(result.stdout_overflow);
    assert!(result.kill_attempted);
    assert_process_ownership(&result);
}

#[test]
fn stdout_and_stderr_are_drained_concurrently() {
    let script = "i=0; while [ $i -lt 256 ]; do printf x; printf y >&2; i=$((i+1)); done";
    let result = shell(script, limits(512, 512, 500));
    assert_eq!(result.termination_reason, TerminationReason::Exited);
    assert_eq!(result.stdout_bytes, 256);
    assert_eq!(result.stderr_bytes, 256);
    assert!(result.status.success());
    assert_process_ownership(&result);
}

#[test]
fn deadline_after_ready_kills_waits_and_joins() {
    let result = shell("printf READY; while :; do :; done", limits(64, 64, 100));
    assert_eq!(result.termination_reason, TerminationReason::Deadline);
    assert_eq!(result.stdout, b"READY");
    assert!(result.kill_attempted);
    assert_process_ownership(&result);
}

#[test]
fn nonzero_exit_is_retained_without_becoming_a_runner_error() {
    let result = shell("printf problem >&2; exit 7", limits(64, 64, 500));
    assert_eq!(result.termination_reason, TerminationReason::Exited);
    assert_eq!(result.status.code(), Some(7));
    assert_eq!(result.stderr, b"problem");
    assert_process_ownership(&result);
}

#[test]
fn stderr_cap_plus_one_is_bounded_and_reaped() {
    let result = shell("printf 12345 >&2; while :; do :; done", limits(64, 4, 500));
    assert_eq!(result.termination_reason, TerminationReason::StderrLimit);
    assert_eq!(result.stderr, b"1234");
    assert!(result.stderr_overflow);
    assert_process_ownership(&result);
}

#[test]
fn simultaneous_pipe_overflow_is_owned() {
    let result = shell("printf 12345; printf 67890 >&2; while :; do :; done", limits(4, 4, 500));
    assert!(matches!(
        result.termination_reason,
        TerminationReason::StdoutLimit | TerminationReason::StderrLimit
    ));
    assert!(result.stdout_overflow || result.stderr_overflow);
    assert!(result.kill_attempted);
    assert_process_ownership(&result);
}

#[test]
fn already_exited_cap_race_is_explicit() {
    let result = shell("printf 12345", limits(4, 64, 500));
    assert_eq!(result.termination_reason, TerminationReason::StdoutLimit);
    assert!(result.stdout_overflow);
    assert!(result.kill_attempted || result.limit_observed_after_exit);
    assert_process_ownership(&result);
}

#[test]
fn spawn_failure_is_returned_without_a_child() {
    let error = run(
        &CommandSpec {
            program: PathBuf::from("/definitely/absent/reovim-helper"),
            arguments: Vec::new(),
            current_dir: PathBuf::from("/"),
            environment: BTreeMap::new(),
            clear_environment: true,
        },
        limits(4, 4, 100),
    )
    .unwrap_err();
    assert_eq!(error.kind(), std::io::ErrorKind::NotFound);
}

#[test]
fn command_argv_environment_and_directory_are_explicit() {
    let script = "printf '%s|%s|%s' \"$1\" \"$ONLY\" \"$PWD\"";
    let result = run(
        &CommandSpec {
            program: PathBuf::from("/bin/sh"),
            arguments: vec![
                OsString::from("-c"),
                OsString::from(script),
                OsString::from("reovim-sh"),
                OsString::from("argument"),
            ],
            current_dir: PathBuf::from("/"),
            environment: BTreeMap::from([(OsString::from("ONLY"), OsString::from("value"))]),
            clear_environment: true,
        },
        limits(128, 64, 500),
    )
    .unwrap();
    assert_eq!(result.stdout, b"argument|value|/");
    assert_eq!(
        result.argv,
        [
            OsString::from("/bin/sh"),
            OsString::from("-c"),
            OsString::from(script),
            OsString::from("reovim-sh"),
            OsString::from("argument"),
        ]
    );
    assert_process_ownership(&result);
}

#[test]
fn invalid_utf8_is_retained_as_bytes() {
    let result = shell("printf '\\377'", limits(4, 4, 500));
    assert_eq!(result.stdout, [0xff]);
    assert!(std::str::from_utf8(&result.stdout).is_err());
    assert_process_ownership(&result);
}

#[test]
fn fast_exit_process_group_identity_race_remains_owned() {
    for _ in 0..32 {
        let result = shell(":", tight_supervision_limits(8, 8, 100));
        assert_eq!(result.termination_reason, TerminationReason::Exited);
        assert!(result.status.success());
        assert_process_ownership(&result);
    }
}

#[test]
#[cfg(target_os = "linux")]
fn descendant_held_pipe_is_cleaned_as_one_bounded_group() {
    let start = std::time::Instant::now();
    let result = shell("/bin/sleep 5 &", tight_supervision_limits(64, 64, 100));
    assert_eq!(result.termination_reason, TerminationReason::Exited);
    assert!(result.status.success());
    assert!(result.kill_attempted);
    assert!(result.descendant_residue_observed);
    assert_process_ownership(&result);
    assert!(start.elapsed() < Duration::from_secs(2));
}

static NEXT_FIXTURE: AtomicU64 = AtomicU64::new(0);

struct RepositoryFixture {
    root: PathBuf,
}

struct PlainTemp(PathBuf);

impl PlainTemp {
    fn new(label: &str) -> Self {
        let sequence = NEXT_FIXTURE.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir()
            .join(format!("reovim-product-v2-{label}-{}-{sequence}", std::process::id()));
        fs::create_dir(&path).unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt as _;
            fs::set_permissions(&path, fs::Permissions::from_mode(0o700)).unwrap();
        }
        Self(path.canonicalize().unwrap())
    }
}

impl Drop for PlainTemp {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

impl RepositoryFixture {
    fn copy_current(label: &str) -> Self {
        let source = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..").canonicalize().unwrap();
        let sequence = NEXT_FIXTURE.fetch_add(1, Ordering::Relaxed);
        let root = std::env::temp_dir()
            .join(format!("reovim-product-v2-{label}-{}-{sequence}", std::process::id()));
        fs::create_dir(&root).unwrap();
        copy_repository_tree(&source, &root).unwrap();
        let fixture = Self { root };
        fixture.retain_default_profile();
        fixture
    }

    fn path(&self, relative: &str) -> PathBuf {
        self.root.join(relative)
    }

    fn bytes(&self, relative: &str) -> Vec<u8> {
        fs::read(self.path(relative)).unwrap()
    }

    fn restore(&self, relative: &str, bytes: &[u8]) {
        fs::write(self.path(relative), bytes).unwrap();
    }

    fn json(&self, relative: &str) -> serde_json::Value {
        serde_json::from_slice(&self.bytes(relative)).unwrap()
    }

    fn write_json(&self, relative: &str, value: &serde_json::Value) {
        let mut bytes = serde_json::to_vec_pretty(value).unwrap();
        bytes.push(b'\n');
        self.restore(relative, &bytes);
    }

    fn retain_default_profile(&self) {
        let mut topology = self.json("conformance/workspace-topology.json");
        topology["workspaces"][0]["metadata_profiles"].as_array_mut().unwrap().retain(|profile| {
            profile.get("id").and_then(serde_json::Value::as_str) == Some("hosted-default")
        });
        self.write_json("conformance/workspace-topology.json", &topology);

        let mut reviews = self.json("conformance/dependency-reviews.json");
        for admission in reviews["workspaces"][0]["admissions"].as_array_mut().unwrap() {
            admission["resolved_uses"].as_array_mut().unwrap().retain(|resolved_use| {
                resolved_use.get("profile").and_then(serde_json::Value::as_str)
                    == Some("hosted-default")
            });
        }
        self.write_json("conformance/dependency-reviews.json", &reviews);
    }

    fn check(&self) -> CheckerRun {
        let output = self.command("check").output().unwrap();
        CheckerRun { status: output.status, stdout: output.stdout, stderr: output.stderr }
    }

    fn exact_check(&self, revision: &str) -> CheckerRun {
        let output = self
            .command("check")
            .arg("--git")
            .arg("/usr/bin/git")
            .arg("--expected-revision")
            .arg(revision)
            .arg("--execution-environment")
            .arg("linux-x86-64-git-2-55-0")
            .output()
            .unwrap();
        CheckerRun { status: output.status, stdout: output.stdout, stderr: output.stderr }
    }

    fn generate(&self, revision: &str, evidence_root: &Path) -> CheckerRun {
        let output = self
            .command("generate")
            .arg("--git")
            .arg("/usr/bin/git")
            .arg("--expected-revision")
            .arg(revision)
            .arg("--execution-environment")
            .arg("linux-x86-64-git-2-55-0")
            .arg("--evidence-root")
            .arg(evidence_root)
            .output()
            .unwrap();
        CheckerRun { status: output.status, stdout: output.stdout, stderr: output.stderr }
    }

    fn command(&self, action: &str) -> std::process::Command {
        let cargo = PathBuf::from(std::env::var_os("CARGO").expect("Cargo sets CARGO"))
            .canonicalize()
            .unwrap();
        let rustc = cargo.parent().unwrap().join("rustc").canonicalize().unwrap();
        let cargo_home = std::env::var_os("CARGO_HOME")
            .map_or_else(
                || PathBuf::from(std::env::var_os("HOME").expect("HOME is set")).join(".cargo"),
                PathBuf::from,
            )
            .canonicalize()
            .unwrap();
        let mut command = std::process::Command::new(env!("CARGO_BIN_EXE_reovim-conformance"));
        command
            .arg(action)
            .arg("--root")
            .arg(&self.root)
            .arg("--policy")
            .arg(self.path("conformance/source-classification.json"))
            .arg("--topology")
            .arg(self.path("conformance/workspace-topology.json"))
            .arg("--entrypoint-reviews")
            .arg(self.path("conformance/entrypoint-reviews.json"))
            .arg("--dependency-reviews")
            .arg(self.path("conformance/dependency-reviews.json"))
            .arg("--product-closures")
            .arg(self.path("conformance/product-closures.json"))
            .arg("--execution-tool-reviews")
            .arg(self.path("conformance/execution-tool-reviews.json"))
            .arg("--cargo")
            .arg(cargo)
            .arg("--rustc")
            .arg(rustc)
            .arg("--cargo-home")
            .arg(cargo_home);
        command
    }

    fn git(&self, arguments: &[&str]) -> Vec<u8> {
        let output = std::process::Command::new("/usr/bin/git")
            .args(arguments)
            .current_dir(&self.root)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "git {arguments:?}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        output.stdout
    }

    fn initialize_git(&self) -> String {
        self.git(&["init", "-q"]);
        self.git(&["config", "user.name", "Reovim Fixture"]);
        self.git(&["config", "user.email", "fixture@reovim.invalid"]);
        self.git(&["config", "commit.gpgSign", "false"]);
        self.git(&["config", "core.hooksPath", "/dev/null"]);
        self.git(&["config", "core.fileMode", "true"]);
        self.git(&["config", "core.autocrlf", "false"]);
        self.git(&["add", "-f", "--all"]);
        self.git(&["commit", "-q", "-m", "fixture"]);
        String::from_utf8(self.git(&["rev-parse", "HEAD"])).unwrap().trim().to_owned()
    }
}

impl Drop for RepositoryFixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

struct CheckerRun {
    status: std::process::ExitStatus,
    stdout: Vec<u8>,
    stderr: Vec<u8>,
}

impl CheckerRun {
    fn report(&self) -> serde_json::Value {
        serde_json::from_slice(&self.stdout).unwrap_or_else(|error| {
            panic!(
                "checker stdout is not a report: {error}; stderr={}",
                String::from_utf8_lossy(&self.stderr)
            )
        })
    }

    fn finding_codes(&self) -> BTreeSet<String> {
        self.report()["findings"]
            .as_array()
            .unwrap()
            .iter()
            .map(|finding| finding["code"].as_str().unwrap().to_owned())
            .collect()
    }

    fn assert_rejected(&self) {
        assert!(!self.status.success(), "unexpected pass");
        assert!(!self.stdout.is_empty() || !self.stderr.is_empty());
    }

    fn fatal_code(&self) -> Option<String> {
        String::from_utf8_lossy(&self.stderr)
            .lines()
            .rev()
            .find_map(|line| serde_json::from_str::<serde_json::Value>(line).ok())
            .and_then(|value| value["code"].as_str().map(str::to_owned))
    }
}

fn copy_repository_tree(source: &Path, destination: &Path) -> io::Result<()> {
    for entry in fs::read_dir(source)? {
        let entry = entry?;
        let name = entry.file_name();
        if matches!(name.to_str(), Some(".git" | "target" | "tmp")) {
            continue;
        }
        let source_path = entry.path();
        let destination_path = destination.join(&name);
        let metadata = fs::symlink_metadata(&source_path)?;
        if metadata.file_type().is_symlink() {
            return Err(io::Error::new(io::ErrorKind::InvalidData, "source fixture has a symlink"));
        }
        if metadata.is_dir() {
            fs::create_dir(&destination_path)?;
            copy_repository_tree(&source_path, &destination_path)?;
        } else if metadata.is_file() {
            fs::copy(&source_path, &destination_path)?;
            fs::set_permissions(&destination_path, metadata.permissions())?;
        }
    }
    Ok(())
}

fn push_rule(policy: &mut serde_json::Value, rule: serde_json::Value) {
    policy["rules"].as_array_mut().unwrap().push(rule);
}

fn report_identity(run: &CheckerRun) -> String {
    run.report()["report_identity"].as_str().unwrap().to_owned()
}

fn verify_report_cli(path: &Path) -> CheckerRun {
    let output = std::process::Command::new(env!("CARGO_BIN_EXE_reovim-conformance"))
        .arg("verify")
        .arg("--report")
        .arg(path)
        .output()
        .unwrap();
    CheckerRun { status: output.status, stdout: output.stdout, stderr: output.stderr }
}

#[test]
#[allow(clippy::too_many_lines)]
fn product_empty_v2_full_cli_and_mutation_matrix() {
    let fixture = RepositoryFixture::copy_current("matrix");
    let policy_path = "conformance/source-classification.json";
    let policy_original = fixture.bytes(policy_path);
    let manifest_path = "tools/conformance/Cargo.toml";
    let manifest_original = fixture.bytes(manifest_path);
    let main_path = "tools/conformance/src/main.rs";
    let main_original = fixture.bytes(main_path);
    let check_script_path = "scripts/check.sh";
    let check_script_original = fixture.bytes(check_script_path);

    let current = fixture.check();
    assert!(
        current.status.success(),
        "current fixture failed: stdout={} stderr={}",
        String::from_utf8_lossy(&current.stdout),
        String::from_utf8_lossy(&current.stderr)
    );
    let current_report = current.report();
    assert_eq!(current_report["schema"], "reovim-source-role-map/v2");
    assert_eq!(current_report["subject"]["kind"], "active_repository");
    assert_eq!(current_report["mode"], "candidate");
    assert_eq!(current_report["revision"], serde_json::Value::Null);
    for field in [
        "closures",
        "compiler_units",
        "compiler_inputs",
        "extern_edges",
        "artifacts",
        "package_memberships",
        "public_consumers",
        "executions",
    ] {
        assert_eq!(current_report["product_proof"][field], serde_json::json!([]));
    }
    let baseline_identity = report_identity(&current);

    // Passing vendored Tool input: its classified-content digest binds
    // `vendor/input.dat\nvendored\n\n` exactly.
    fs::create_dir(fixture.path("vendor")).unwrap();
    fs::write(fixture.path("vendor/input.dat"), b"vendored\n").unwrap();
    let mut policy = fixture.json(policy_path);
    push_rule(
        &mut policy,
        serde_json::json!({
            "prefix": "vendor/input.dat",
            "precedence": 20,
            "source_owner": {"kind": "repository_tooling"},
            "source_provenance": "vendored",
            "source_use": "tool",
            "upstream": {
                "identity": "product-v2-fixture/v1",
                "digest": "sha256:160cc4537148c27cb3f269c9126739f4a99b1fad7f2edad4143c5b1729124e33",
                "digest_subject": {"kind": "classified_content"}
            }
        }),
    );
    fixture.write_json(policy_path, &policy);
    let vendored = fixture.check();
    assert!(
        vendored.status.success(),
        "vendored fixture failed: stdout={} stderr={}",
        String::from_utf8_lossy(&vendored.stdout),
        String::from_utf8_lossy(&vendored.stderr)
    );
    fs::remove_dir_all(fixture.path("vendor")).unwrap();
    fixture.restore(policy_path, &policy_original);

    for invocation in [
        b"\n\"$ROOT/tmp/hidden-helper\"\n".as_slice(),
        b"\nHIDDEN=\"$ROOT/tmp/hidden-helper\"; \"$HIDDEN\"\n".as_slice(),
        b"\nHIDDEN=\"$ROOT/tmp/hidden-helper\"; sh \"$HIDDEN\"\n".as_slice(),
        b"\nHIDDEN=\"$ROOT/tmp/hidden-helper\"; exec \\\n  \"$HIDDEN\"\n".as_slice(),
        b"\nOUT=\"$(\"$ROOT/tmp/hidden-helper\")\"\n".as_slice(),
        b"\nOUT=`\"$ROOT/tmp/hidden-helper\"`\n".as_slice(),
        b"\nVALUES=(\n  \"$(\"$ROOT/tmp/hidden-helper\")\"\n)\n".as_slice(),
        b"\ncat <<EOF\n$(\"$ROOT/tmp/hidden-helper\")\nEOF\n".as_slice(),
    ] {
        let mut variable_entrypoint = check_script_original.clone();
        variable_entrypoint.extend_from_slice(invocation);
        fixture.restore(check_script_path, &variable_entrypoint);
        let variable_command = fixture.check();
        variable_command.assert_rejected();
        assert!(
            variable_command.finding_codes().contains("ENTRYPOINT_LOCAL_INPUT_PROOF_UNAVAILABLE")
        );
        fixture.restore(check_script_path, &check_script_original);
    }

    // M1-M3: schema/parser rejects unknown axes, invalid owner/use, and the
    // tooling owner encoded as a Product role before Cargo is consulted.
    for mutation in [
        serde_json::json!({"kind": "unknown"}),
        serde_json::json!({"kind": "repository_tooling"}),
        serde_json::json!({"kind": "product", "role": "RepositoryTooling"}),
    ] {
        let mut policy = fixture.json(policy_path);
        policy["rules"][0]["source_owner"] = mutation;
        if matches!(
            policy["rules"][0]["source_owner"]["kind"].as_str(),
            Some("repository_tooling" | "product")
        ) {
            policy["rules"][0]["source_use"] = serde_json::json!("production");
        }
        fixture.write_json(policy_path, &policy);
        let rejected = fixture.check();
        rejected.assert_rejected();
        assert_eq!(rejected.fatal_code().as_deref(), Some("POLICY_SCHEMA"));
        fixture.restore(policy_path, &policy_original);
    }
    for field in ["source_provenance", "source_use"] {
        let mut policy = fixture.json(policy_path);
        policy["rules"][0][field] = serde_json::json!("unknown");
        fixture.write_json(policy_path, &policy);
        let rejected = fixture.check();
        rejected.assert_rejected();
        assert_eq!(rejected.fatal_code().as_deref(), Some("POLICY_SCHEMA"));
        fixture.restore(policy_path, &policy_original);
    }

    let mut policy = fixture.json(policy_path);
    push_rule(
        &mut policy,
        serde_json::json!({
            "prefix": "tools",
            "precedence": 10,
            "source_owner": {"kind": "repository_tooling"},
            "source_provenance": "authored",
            "source_use": "tool"
        }),
    );
    fixture.write_json(policy_path, &policy);
    let ambiguous = fixture.check();
    ambiguous.assert_rejected();
    assert!(ambiguous.finding_codes().contains("SOURCE_CLASSIFICATION"));
    fixture.restore(policy_path, &policy_original);

    // M4-M6: unmatched path, undeclared helper, omitted literal include input,
    // orphan module, and an unregistered Cargo target all remain visible.
    fs::write(fixture.path("unmatched.bin"), b"unmatched").unwrap();
    fs::write(fixture.path("scripts/undeclared-helper.py"), b"print('fixture')\n").unwrap();
    fs::create_dir(fixture.path("tmp")).unwrap();
    fs::write(fixture.path("tmp/hidden"), b"excluded").unwrap();
    fs::write(fixture.path("tmp/second-hidden"), b"excluded second input").unwrap();
    fs::write(fixture.path("tools/conformance/src/orphan.rs"), b"pub fn orphan() {}\n").unwrap();
    fs::write(fixture.path("tools/conformance/src/unused.rs"), b"pub fn unused() {}\n").unwrap();
    fs::write(
        fixture.path("tools/conformance/build-helper.rs"),
        b"#![forbid(unsafe_code)]\nfn main() {}\n",
    )
    .unwrap();
    fs::write(fixture.path("tools/conformance/src/duplicate.rs"), b"pub fn one() {}\n").unwrap();
    fs::create_dir(fixture.path("tools/conformance/src/duplicate")).unwrap();
    fs::write(fixture.path("tools/conformance/src/duplicate/mod.rs"), b"pub fn two() {}\n")
        .unwrap();
    let mut main = main_original.clone();
    main.extend_from_slice(
        b"\nconst _: &str = include_str!(\"../../../tmp/hidden\");\nconst _: &str = arbitrary::include_str!(\"../../../Cargo.toml\");\nmod fixture_missing;\nmod duplicate;\n#[path = \"../../../tmp/hidden.rs\"] mod fixture_path;\n",
    );
    fixture.restore(main_path, &main);
    let mut check_script = check_script_original.clone();
    check_script.extend_from_slice(
        b"\nsource tmp/hidden\nsource /tmp/reovim-escape\nbash tmp/hidden\npython ../tmp/reovim-escape.py\n./scripts/check.sh ./tmp/second-hidden\n",
    );
    fixture.restore(check_script_path, &check_script);
    let mut manifest = String::from_utf8(manifest_original.clone()).unwrap();
    manifest = manifest.replace("publish = false", "publish = false\nbuild = \"build-helper.rs\"");
    manifest.push_str("\n[[bin]]\nname = \"fixture-extra\"\npath = \"src/orphan.rs\"\n");
    fixture.restore(manifest_path, manifest.as_bytes());
    let omitted = fixture.check();
    omitted.assert_rejected();
    let codes = omitted.finding_codes();
    for code in [
        "SOURCE_CLASSIFICATION",
        "TOPOLOGY_SCRIPT_ENTRYPOINT",
        "INCLUDE_TARGET_UNCLASSIFIED",
        "INCLUDE_QUALIFIED_PROOF_UNAVAILABLE",
        "ENTRYPOINT_LOCAL_INPUT_UNCLASSIFIED",
        "ENTRYPOINT_LOCAL_INPUT_PROOF_UNAVAILABLE",
        "TOPOLOGY_TARGET_EXTRA",
        "TOPOLOGY_BUILD_SCRIPT_EXTRA",
        "UNREGISTERED_RUST_SOURCE",
        "MODULE_SOURCE_MISSING",
        "MODULE_SOURCE_AMBIGUOUS",
        "UNSUPPORTED_PATH_MODULE",
    ] {
        assert!(codes.contains(code), "missing {code}: {codes:?}");
    }
    assert!(omitted.report()["findings"].as_array().unwrap().iter().any(|finding| {
        finding["code"] == "ENTRYPOINT_LOCAL_INPUT_UNCLASSIFIED"
            && finding["path"] == "tmp/second-hidden"
    }));
    fs::remove_file(fixture.path("unmatched.bin")).unwrap();
    fs::remove_file(fixture.path("scripts/undeclared-helper.py")).unwrap();
    fs::remove_dir_all(fixture.path("tmp")).unwrap();
    fs::remove_file(fixture.path("tools/conformance/src/orphan.rs")).unwrap();
    fs::remove_file(fixture.path("tools/conformance/src/unused.rs")).unwrap();
    fs::remove_file(fixture.path("tools/conformance/build-helper.rs")).unwrap();
    fs::remove_dir_all(fixture.path("tools/conformance/src/duplicate")).unwrap();
    fs::remove_file(fixture.path("tools/conformance/src/duplicate.rs")).unwrap();
    fixture.restore(main_path, &main_original);
    fixture.restore(check_script_path, &check_script_original);
    fixture.restore(manifest_path, &manifest_original);

    // Active v2 generation remains Product-empty until the bounded compiler,
    // package, and public-consumer proof gate is complete. Product source
    // without a matching closure is a typed fatal configuration mismatch.
    fs::create_dir(fixture.path("product")).unwrap();
    fs::write(fixture.path("product/lib.rs"), b"pub use tooling::*; include!(\"generated.rs\");\n")
        .unwrap();
    let mut policy = fixture.json(policy_path);
    push_rule(
        &mut policy,
        serde_json::json!({
            "prefix": "product",
            "precedence": 20,
            "source_owner": {"kind": "product", "role": "Foundation"},
            "source_provenance": "authored",
            "source_use": "production"
        }),
    );
    fixture.write_json(policy_path, &policy);
    let product = fixture.check();
    product.assert_rejected();
    assert_eq!(product.fatal_code().as_deref(), Some("PRODUCT_CLOSURE_MISSING"));
    fs::remove_dir_all(fixture.path("product")).unwrap();
    fixture.restore(policy_path, &policy_original);

    // The retired v1 shipping-root field cannot be interpreted as a v2
    // Product closure or silently upgraded.
    let mut policy = fixture.json(policy_path);
    policy["shipping_closure_roots"] = serde_json::json!([]);
    fixture.write_json(policy_path, &policy);
    let shipping = fixture.check();
    shipping.assert_rejected();
    assert_eq!(shipping.fatal_code().as_deref(), Some("POLICY_SCHEMA"));
    fixture.restore(policy_path, &policy_original);

    // M12: a newly declared concrete cycle cannot be hidden by policy.
    let mut manifest = String::from_utf8(fixture.bytes("lib/depgraph/Cargo.toml")).unwrap();
    let depgraph_original = manifest.as_bytes().to_vec();
    manifest.push_str("\n[dependencies.reovim-conformance]\npath = \"../../tools/conformance\"\n");
    fixture.restore("lib/depgraph/Cargo.toml", manifest.as_bytes());
    let cycle = fixture.check();
    cycle.assert_rejected();
    assert_eq!(cycle.fatal_code().as_deref(), Some("CARGO_NONZERO"));
    fixture.restore("lib/depgraph/Cargo.toml", &depgraph_original);

    // M13: an in-scope symlink escape is not hidden by classification.
    #[cfg(unix)]
    {
        std::os::unix::fs::symlink("/", fixture.path("scripts/escape-link")).unwrap();
        let escape = fixture.check();
        escape.assert_rejected();
        assert!(escape.finding_codes().contains("SYMLINK_IN_SCOPE"));
        fs::remove_file(fixture.path("scripts/escape-link")).unwrap();
    }

    // M14: generated Rust cannot pass as BuildOutput.
    fs::write(fixture.path("tools/conformance/src/generated.rs"), b"pub fn generated() {}\n")
        .unwrap();
    let mut policy = fixture.json(policy_path);
    push_rule(
        &mut policy,
        serde_json::json!({
            "prefix": "tools/conformance/src/generated.rs",
            "precedence": 30,
            "source_owner": {"kind": "repository_tooling"},
            "source_provenance": "generated",
            "source_use": "build_output",
            "upstream": {
                "identity": "fixture-generator/v1",
                "digest": "sha256:0000000000000000000000000000000000000000000000000000000000000000",
                "digest_subject": {"kind": "paths", "paths": ["Cargo.toml"]}
            }
        }),
    );
    fixture.write_json(policy_path, &policy);
    let generated = fixture.check();
    generated.assert_rejected();
    assert!(generated.finding_codes().contains("RUST_SOURCE_BUILD_OUTPUT"));
    fs::remove_file(fixture.path("tools/conformance/src/generated.rs")).unwrap();
    fixture.restore(policy_path, &policy_original);

    // M15-M17: upstream omission, policy self-coverage omission, and generated
    // result-as-policy are rejected.
    let mut policy = fixture.json(policy_path);
    policy["rules"][0]["source_provenance"] = serde_json::json!("vendored");
    fixture.write_json(policy_path, &policy);
    let missing_upstream = fixture.check();
    missing_upstream.assert_rejected();
    assert_eq!(missing_upstream.fatal_code().as_deref(), Some("POLICY_SCHEMA"));
    fixture.restore(policy_path, &policy_original);

    let mut policy = fixture.json(policy_path);
    policy["rules"]
        .as_array_mut()
        .unwrap()
        .retain(|rule| rule["prefix"].as_str() != Some("conformance/source-classification.json"));
    fixture.write_json(policy_path, &policy);
    let self_omitted = fixture.check();
    self_omitted.assert_rejected();
    assert!(self_omitted.finding_codes().contains("SOURCE_CLASSIFICATION"));
    fixture.restore(policy_path, &policy_original);

    let mut policy = fixture.json(policy_path);
    push_rule(
        &mut policy,
        serde_json::json!({
            "prefix": "conformance/source-role-map.json",
            "precedence": 30,
            "source_owner": {"kind": "repository_tooling"},
            "source_provenance": "authored",
            "source_use": "tool"
        }),
    );
    fixture.write_json(policy_path, &policy);
    let self_reference = fixture.check();
    self_reference.assert_rejected();
    assert_eq!(self_reference.fatal_code().as_deref(), Some("POLICY_RESULT_SELF_REFERENCE"));
    fixture.restore(policy_path, &policy_original);

    let entrypoint_reviews_path = "conformance/entrypoint-reviews.json";
    let entrypoint_reviews_original = fixture.bytes(entrypoint_reviews_path);
    let mut entrypoint_reviews = fixture.json(entrypoint_reviews_path);
    entrypoint_reviews["reviewed_at"] = serde_json::json!("2026-09-01T00:00:01Z");
    fixture.write_json(entrypoint_reviews_path, &entrypoint_reviews);
    let changed_entrypoint_review = fixture.check();
    assert!(changed_entrypoint_review.status.success());
    assert_ne!(baseline_identity, report_identity(&changed_entrypoint_review));
    fixture.restore(entrypoint_reviews_path, &entrypoint_reviews_original);

    // M19: changing a committed feature profile changes report identity even
    // when this dependency graph happens to resolve to the same packages.
    let topology_path = "conformance/workspace-topology.json";
    let topology_original = fixture.bytes(topology_path);
    let mut topology = fixture.json(topology_path);
    topology["workspaces"][0]["metadata_profiles"][0]["all_features"] = serde_json::json!(true);
    fixture.write_json(topology_path, &topology);
    let changed_profile = fixture.check();
    assert!(changed_profile.status.success());
    assert_ne!(baseline_identity, report_identity(&changed_profile));
    fixture.restore(topology_path, &topology_original);
}

#[test]
#[allow(clippy::too_many_lines)]
fn exact_git_generate_verify_and_raw_identity_mutants() {
    let fixture = RepositoryFixture::copy_current("exact");
    let mut revision = fixture.initialize_git();
    let evidence = PlainTemp::new("evidence");

    let generated = fixture.generate(&revision, &evidence.0);
    assert!(
        generated.status.success(),
        "generate failed: stdout={} stderr={}",
        String::from_utf8_lossy(&generated.stdout),
        String::from_utf8_lossy(&generated.stderr)
    );
    let response = generated.report();
    assert_eq!(response["effect_certainty"], "applied");
    assert_eq!(response["cleanup_state"], "complete");
    assert_eq!(response["durability_state"], "directory_synced");
    let report_path = PathBuf::from(response["path"].as_str().unwrap());
    assert!(report_path.starts_with(&evidence.0));
    let retained_bytes = fs::read(&report_path).unwrap();
    let verified = verify_report_cli(&report_path);
    assert!(verified.status.success(), "verify: {}", String::from_utf8_lossy(&verified.stderr));

    let no_clobber = fixture.generate(&revision, &evidence.0);
    no_clobber.assert_rejected();
    assert_eq!(no_clobber.fatal_code().as_deref(), Some("EVIDENCE_EXISTS"));
    assert!(String::from_utf8_lossy(&no_clobber.stderr).contains("not_applied"));
    assert_eq!(fs::read(&report_path).unwrap(), retained_bytes);
    assert!(fs::read_dir(report_path.parent().unwrap()).unwrap().all(|entry| {
        !entry.unwrap().file_name().to_string_lossy().starts_with(".source-role-map")
    }));

    let mut tampered: serde_json::Value =
        serde_json::from_slice(&fs::read(&report_path).unwrap()).unwrap();
    tampered["result"]["status"] = serde_json::json!("fail");
    fs::write(&report_path, serde_json::to_vec(&tampered).unwrap()).unwrap();
    let verify_tampered = verify_report_cli(&report_path);
    verify_tampered.assert_rejected();
    assert_eq!(verify_tampered.fatal_code().as_deref(), Some("VERIFY_REPORT_V2"));

    let wrong_revision = fixture.exact_check(&"0".repeat(40));
    wrong_revision.assert_rejected();
    assert_eq!(wrong_revision.fatal_code().as_deref(), Some("GIT_REVISION_MISMATCH"));

    fs::write(fixture.path(".gitmodules"), b"[submodule \"x\"]\n").unwrap();
    let modules = fixture.exact_check(&revision);
    modules.assert_rejected();
    assert_eq!(modules.fatal_code().as_deref(), Some("GIT_UNMODELED_SEMANTICS"));
    fs::remove_file(fixture.path(".gitmodules")).unwrap();

    fs::create_dir(fixture.path("nested")).unwrap();
    fs::write(fixture.path("nested/.gitattributes"), b"* -text\n").unwrap();
    let untracked_attributes = fixture.exact_check(&revision);
    untracked_attributes.assert_rejected();
    assert_eq!(untracked_attributes.fatal_code().as_deref(), Some("GIT_UNMODELED_SEMANTICS"));
    fixture.git(&["add", "-f", "nested/.gitattributes"]);
    fixture.git(&["commit", "-q", "-m", "tracked nested attributes"]);
    revision = String::from_utf8(fixture.git(&["rev-parse", "HEAD"])).unwrap().trim().to_owned();
    let tracked_attributes = fixture.exact_check(&revision);
    tracked_attributes.assert_rejected();
    assert_eq!(tracked_attributes.fatal_code().as_deref(), Some("GIT_UNMODELED_SEMANTICS"));
    fixture.git(&["rm", "-q", "nested/.gitattributes"]);
    fixture.git(&["commit", "-q", "-m", "remove attributes"]);
    match fs::remove_dir(fixture.path("nested")) {
        Ok(()) => {}
        Err(error) if error.kind() == io::ErrorKind::NotFound => {}
        Err(error) => panic!("remove nested fixture: {error}"),
    }
    revision = String::from_utf8(fixture.git(&["rev-parse", "HEAD"])).unwrap().trim().to_owned();

    fs::write(fixture.path(".git/info/attributes"), b"* -text\n").unwrap();
    let info_attributes = fixture.exact_check(&revision);
    info_attributes.assert_rejected();
    assert_eq!(info_attributes.fatal_code().as_deref(), Some("GIT_INFO_ATTRIBUTES"));
    fs::remove_file(fixture.path(".git/info/attributes")).unwrap();

    fixture.git(&["update-index", "--assume-unchanged", "README.md"]);
    let hidden_index = fixture.exact_check(&revision);
    hidden_index.assert_rejected();
    assert_eq!(hidden_index.fatal_code().as_deref(), Some("GIT_INDEX_FLAGS"));
    fixture.git(&["update-index", "--no-assume-unchanged", "README.md"]);
    fixture.git(&["update-index", "--skip-worktree", "README.md"]);
    let skipped_index = fixture.exact_check(&revision);
    skipped_index.assert_rejected();
    assert_eq!(skipped_index.fatal_code().as_deref(), Some("GIT_INDEX_FLAGS"));
    fixture.git(&["update-index", "--no-skip-worktree", "README.md"]);

    fs::write(fixture.path("scripts/ignored.pyc"), b"ignored").unwrap();
    let ignored_extra = fixture.exact_check(&revision);
    ignored_extra.assert_rejected();
    assert_eq!(ignored_extra.fatal_code().as_deref(), Some("REVISION_INVENTORY_MISMATCH"));
    fs::remove_file(fixture.path("scripts/ignored.pyc")).unwrap();

    #[cfg(unix)]
    {
        std::os::unix::fs::symlink("/", fixture.path("scripts/exact-link")).unwrap();
        let symlink = fixture.exact_check(&revision);
        symlink.assert_rejected();
        assert!(symlink.finding_codes().contains("SYMLINK_IN_SCOPE"));
        fs::remove_file(fixture.path("scripts/exact-link")).unwrap();
    }

    let readme = fixture.bytes("README.md");
    let mut staged = readme.clone();
    staged.extend_from_slice(b"\nstaged fixture\n");
    fixture.restore("README.md", &staged);
    fixture.git(&["add", "README.md"]);
    let staged_bytes = fixture.exact_check(&revision);
    staged_bytes.assert_rejected();
    assert_eq!(staged_bytes.fatal_code().as_deref(), Some("GIT_INDEX_TREE_MISMATCH"));
    fixture.git(&["reset", "-q", "HEAD", "--", "README.md"]);
    fixture.restore("README.md", &readme);

    let mut dirty = readme.clone();
    dirty.extend_from_slice(b"\nworktree fixture\n");
    fixture.restore("README.md", &dirty);
    let worktree_bytes = fixture.exact_check(&revision);
    worktree_bytes.assert_rejected();
    assert_eq!(worktree_bytes.fatal_code().as_deref(), Some("GIT_BLOB_MISMATCH"));
    fixture.restore("README.md", &readme);

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        let path = fixture.path("README.md");
        let original_mode = fs::metadata(&path).unwrap().permissions().mode();
        let mut permissions = fs::metadata(&path).unwrap().permissions();
        permissions.set_mode(original_mode ^ 0o100);
        fs::set_permissions(&path, permissions).unwrap();
        fixture.git(&["add", "README.md"]);
        let staged_mode = fixture.exact_check(&revision);
        staged_mode.assert_rejected();
        assert_eq!(staged_mode.fatal_code().as_deref(), Some("GIT_INDEX_TREE_MISMATCH"));
        fixture.git(&["reset", "-q", "HEAD", "--", "README.md"]);

        let mut permissions = fs::metadata(&path).unwrap().permissions();
        permissions.set_mode(original_mode);
        fs::set_permissions(&path, permissions).unwrap();
        let mut permissions = fs::metadata(&path).unwrap().permissions();
        permissions.set_mode(original_mode ^ 0o100);
        fs::set_permissions(&path, permissions).unwrap();
        let worktree_mode = fixture.exact_check(&revision);
        worktree_mode.assert_rejected();
        assert_eq!(worktree_mode.fatal_code().as_deref(), Some("GIT_MODE_MISMATCH"));
        let mut permissions = fs::metadata(&path).unwrap().permissions();
        permissions.set_mode(original_mode);
        fs::set_permissions(path, permissions).unwrap();
    }
}
