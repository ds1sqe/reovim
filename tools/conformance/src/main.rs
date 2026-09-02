#![forbid(unsafe_code)]

use std::{
    collections::BTreeMap,
    env,
    io::{self, Write as _},
    path::{Path, PathBuf},
    process::ExitCode,
};

use reovim_conformance::{CheckOptions, Fatal, LimitOverrides, Mode};
use serde_json::json;

const USAGE: &str = "\
Usage:
  reovim-conformance check --root ABS --policy ABS --topology ABS \\
    --entrypoint-reviews ABS --dependency-reviews ABS --product-closures ABS \\
    --execution-tool-reviews ABS --cargo ABS --rustc ABS \\
    --cargo-home ABS \\
    [--git ABS --expected-revision HEX --execution-environment ID] \\
    [--stdout-limit N] [--stderr-limit N] [--deadline-ms N] \\
    [--supervision-deadline-ms N] [--report-limit N]
  reovim-conformance generate --root ABS --policy ABS --topology ABS \\
    --entrypoint-reviews ABS --dependency-reviews ABS --product-closures ABS \\
    --execution-tool-reviews ABS --cargo ABS --rustc ABS \\
    --cargo-home ABS \\
    --git ABS --expected-revision HEX --execution-environment ID \\
    --evidence-root ABS \\
    [--stdout-limit N] [--stderr-limit N] [--deadline-ms N] \\
    [--supervision-deadline-ms N] [--report-limit N]
  reovim-conformance verify --report ABS

`check` runs every committed topology profile. Supplying both --git and
--expected-revision selects exact clean-tree mode; omitting both is candidate
mode. CLI limits may only tighten committed maxima. Exit: 0 pass, 2 findings,
64 usage, 70 fatal runner/configuration/internal failure.
";

fn main() -> ExitCode {
    if reovim_conformance::private_prepared_root_requested() {
        return prepared_root_main();
    }
    if reovim_conformance::private_rustc_launcher_requested() {
        return rustc_launcher_main();
    }
    if reovim_conformance::private_rustc_wrapper_requested() {
        return rustc_wrapper_main();
    }
    match run() {
        Ok(code) => code,
        Err(CliError::Usage(message)) => {
            let value = json!({"code": "CLI_USAGE", "message": message});
            write_stderr(&value);
            ExitCode::from(64)
        }
        Err(CliError::Fatal(error)) => {
            let value = json!({"code": error.code, "message": error.message});
            write_stderr(&value);
            ExitCode::from(70)
        }
        Err(CliError::Publication(error)) => {
            let value = json!({
                "code": error.fatal.code,
                "message": error.fatal.message,
                "effect_certainty": error.effect_certainty.as_str(),
                "path": error
                    .intended_path
                    .map(|path| path.display().to_string()),
                "cleanup_state": error.cleanup_state.as_str(),
                "durability_state": error.durability_state.as_str(),
            });
            write_stderr(&value);
            ExitCode::from(70)
        }
    }
}

fn rustc_launcher_main() -> ExitCode {
    match reovim_conformance::private_rustc_launcher_entry() {
        Ok(status) => u8::try_from(status).map_or_else(
            |_| {
                write_stderr(&json!({
                    "code": "COMPILER_LAUNCHER_STATUS",
                    "message": "rustc launcher status is outside 0..=255",
                }));
                ExitCode::from(70)
            },
            ExitCode::from,
        ),
        Err(error) => {
            write_stderr(&json!({
                "code": error.code,
                "message": error.message,
            }));
            ExitCode::from(70)
        }
    }
}

fn prepared_root_main() -> ExitCode {
    match reovim_conformance::private_prepared_root_entry() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            write_stderr(&json!({
                "code": error.code,
                "message": error.message,
            }));
            ExitCode::from(70)
        }
    }
}

fn rustc_wrapper_main() -> ExitCode {
    match reovim_conformance::private_rustc_wrapper_entry() {
        Ok(status) => u8::try_from(status).map_or_else(
            |_| {
                write_stderr(&json!({
                    "code": "COMPILER_WRAPPER_STATUS",
                    "message": "rustc wrapper status is outside 0..=255",
                }));
                ExitCode::from(70)
            },
            ExitCode::from,
        ),
        Err(error) => {
            write_stderr(&json!({
                "code": error.code,
                "message": error.message,
            }));
            ExitCode::from(70)
        }
    }
}

fn run() -> Result<ExitCode, CliError> {
    let mut arguments = env::args_os();
    let _program = arguments.next();
    let Some(command) = arguments.next() else {
        return Err(CliError::Usage(USAGE.to_owned()));
    };
    if command == "--help" || command == "-h" {
        let mut stdout = io::stdout().lock();
        stdout
            .write_all(USAGE.as_bytes())
            .and_then(|()| stdout.flush())
            .map_err(|error| {
                CliError::Fatal(Fatal {
                    code: "STDOUT_WRITE",
                    message: error.to_string(),
                })
            })?;
        return Ok(ExitCode::SUCCESS);
    }
    if command == "--version" || command == "-V" {
        let version =
            concat!("reovim-conformance ", env!("CARGO_PKG_VERSION"), "\n");
        let mut stdout = io::stdout().lock();
        stdout
            .write_all(version.as_bytes())
            .and_then(|()| stdout.flush())
            .map_err(|error| {
                CliError::Fatal(Fatal {
                    code: "STDOUT_WRITE",
                    message: error.to_string(),
                })
            })?;
        return Ok(ExitCode::SUCCESS);
    }
    let command = command
        .into_string()
        .map_err(|_| CliError::Usage("command must be UTF-8".to_owned()))?;
    let raw = arguments.collect::<Vec<_>>();
    let parsed = Arguments::parse(&raw)?;
    match command.as_str() {
        "check" => check_command(&parsed),
        "generate" => generate_command(&parsed),
        "verify" => verify_command(&parsed),
        _ => Err(CliError::Usage(format!(
            "unknown command {command:?}\n{USAGE}"
        ))),
    }
}

fn check_command(arguments: &Arguments) -> Result<ExitCode, CliError> {
    arguments.reject_unknown(&common_names())?;
    let outcome =
        reovim_conformance::check(&common_options(arguments, false)?)?;
    let mut stdout = io::stdout().lock();
    stdout
        .write_all(outcome.report_bytes())
        .and_then(|()| stdout.flush())
        .map_err(|error| {
            CliError::Fatal(Fatal {
                code: "STDOUT_WRITE",
                message: error.to_string(),
            })
        })?;
    Ok(if outcome.is_clean() { ExitCode::SUCCESS } else { ExitCode::from(2) })
}

fn generate_command(arguments: &Arguments) -> Result<ExitCode, CliError> {
    let mut names = common_names();
    names.push("evidence-root");
    arguments.reject_unknown(&names)?;
    let evidence_root = arguments.path("evidence-root")?;
    let options = common_options(arguments, true)?;
    let outcome = reovim_conformance::check(&options)?;
    if !outcome.is_clean() {
        let mut stdout = io::stdout().lock();
        stdout
            .write_all(outcome.report_bytes())
            .and_then(|()| stdout.flush())
            .map_err(|error| {
                CliError::Fatal(Fatal {
                    code: "STDOUT_WRITE",
                    message: error.to_string(),
                })
            })?;
        return Ok(ExitCode::from(2));
    }
    let expected_path = evidence_root
        .join(outcome.report_identity())
        .join("conformance/source-role-map.json");
    let mut response = serde_json::to_vec(&json!({
        "schema": "reovim-conformance-generate/v1",
        "report_identity": outcome.report_identity(),
        "path": expected_path.display().to_string(),
        "effect_certainty": "applied",
        "cleanup_state": "complete",
        "durability_state": "directory_synced",
    }))
    .map_err(|error| {
        CliError::Fatal(Fatal {
            code: "GENERATE_RESPONSE",
            message: error.to_string(),
        })
    })?;
    response.push(b'\n');
    let publication =
        reovim_conformance::write_evidence(&outcome, &evidence_root)
            .map_err(CliError::Publication)?;
    if publication.path() != expected_path {
        return Err(CliError::Publication(
            reovim_conformance::PublicationError {
                fatal: Fatal {
                    code: "EVIDENCE_PATH_IDENTITY",
                    message: concat!(
                        "published evidence path differs from the ",
                        "pre-serialized response",
                    )
                    .to_owned(),
                },
                effect_certainty: reovim_conformance::EffectCertainty::Applied,
                intended_path: Some(publication.path().to_path_buf()),
                cleanup_state: publication.cleanup_state(),
                durability_state: publication.durability_state(),
            },
        ));
    }
    deliver_applied_response(
        &mut io::stdout().lock(),
        &response,
        publication.path(),
        publication.cleanup_state(),
        publication.durability_state(),
    )?;
    Ok(ExitCode::SUCCESS)
}

fn verify_command(arguments: &Arguments) -> Result<ExitCode, CliError> {
    arguments.reject_unknown(&["report"])?;
    let report = arguments.path("report")?;
    let verified = reovim_conformance::verify_report(&report)?;
    let mut response = serde_json::to_vec(&json!({
        "schema": "reovim-conformance-verify/v1",
        "report_identity": verified.identity,
        "mode": verified.mode,
        "status": verified.status,
    }))
    .map_err(|error| {
        CliError::Fatal(Fatal {
            code: "VERIFY_RESPONSE",
            message: error.to_string(),
        })
    })?;
    response.push(b'\n');
    let mut stdout = io::stdout().lock();
    stdout.write_all(&response).and_then(|()| stdout.flush()).map_err(
        |error| {
            CliError::Fatal(Fatal {
                code: "STDOUT_WRITE",
                message: error.to_string(),
            })
        },
    )?;
    Ok(ExitCode::SUCCESS)
}

fn deliver_applied_response(
    writer: &mut impl io::Write,
    response: &[u8],
    path: &Path,
    cleanup_state: reovim_conformance::CleanupState,
    durability_state: reovim_conformance::DurabilityState,
) -> Result<(), CliError> {
    writer.write_all(response).and_then(|()| writer.flush()).map_err(|error| {
        CliError::Publication(reovim_conformance::PublicationError {
            fatal: Fatal {
                code: "EVIDENCE_RESPONSE_WRITE",
                message: error.to_string(),
            },
            effect_certainty: reovim_conformance::EffectCertainty::Applied,
            intended_path: Some(path.to_path_buf()),
            cleanup_state,
            durability_state,
        })
    })
}

fn write_stderr(value: &serde_json::Value) {
    let mut stderr = io::stderr().lock();
    let _ = serde_json::to_writer(&mut stderr, value)
        .and_then(|()| stderr.write_all(b"\n").map_err(serde_json::Error::io));
    let _ = stderr.flush();
}

fn common_options(
    arguments: &Arguments,
    require_exact: bool,
) -> Result<CheckOptions, CliError> {
    let git = arguments.optional_path("git");
    let expected_revision =
        arguments.optional("expected-revision").map(str::to_owned);
    let execution_environment =
        arguments.optional("execution-environment").map(str::to_owned);
    let mode = match (git, expected_revision, execution_environment) {
        (None, None, None) if !require_exact => Mode::Candidate,
        (Some(git), Some(expected_revision), Some(execution_environment)) => {
            Mode::Exact { git, expected_revision, execution_environment }
        }
        (None, None, None) => {
            return Err(CliError::Usage(
                concat!(
                    "generate requires --git, --expected-revision, and ",
                    "--execution-environment",
                )
                .to_owned(),
            ));
        }
        _ => {
            return Err(CliError::Usage(
                concat!(
                    "--git, --expected-revision, and ",
                    "--execution-environment must be supplied together",
                )
                .to_owned(),
            ));
        }
    };
    Ok(CheckOptions {
        root: arguments.path("root")?,
        policy: arguments.path("policy")?,
        topology: arguments.path("topology")?,
        entrypoint_reviews: arguments.path("entrypoint-reviews")?,
        dependency_reviews: arguments.path("dependency-reviews")?,
        product_closures: arguments.path("product-closures")?,
        execution_tool_reviews: arguments.path("execution-tool-reviews")?,
        cargo: arguments.path("cargo")?,
        rustc: arguments.path("rustc")?,
        cargo_home: arguments.path("cargo-home")?,
        mode,
        limits: LimitOverrides {
            stdout_bytes: arguments.number("stdout-limit")?,
            stderr_bytes: arguments.number("stderr-limit")?,
            deadline_ms: arguments.number("deadline-ms")?,
            supervision_deadline_ms: arguments
                .number("supervision-deadline-ms")?,
            report_bytes: arguments.number("report-limit")?,
        },
    })
}

fn common_names() -> Vec<&'static str> {
    vec![
        "root",
        "policy",
        "topology",
        "entrypoint-reviews",
        "dependency-reviews",
        "product-closures",
        "execution-tool-reviews",
        "cargo",
        "rustc",
        "cargo-home",
        "git",
        "expected-revision",
        "execution-environment",
        "stdout-limit",
        "stderr-limit",
        "deadline-ms",
        "supervision-deadline-ms",
        "report-limit",
    ]
}

#[derive(Debug)]
enum CliError {
    Usage(String),
    Fatal(Fatal),
    Publication(reovim_conformance::PublicationError),
}

impl From<Fatal> for CliError {
    fn from(error: Fatal) -> Self {
        Self::Fatal(error)
    }
}

#[derive(Debug)]
struct Arguments {
    values: BTreeMap<String, Option<String>>,
}

impl Arguments {
    fn parse(raw: &[std::ffi::OsString]) -> Result<Self, CliError> {
        Ok(Self { values: parse_argument_values(raw)? })
    }

    fn reject_unknown(&self, admitted: &[&str]) -> Result<(), CliError> {
        reject_unknown_arguments(&self.values, admitted)
    }

    fn path(&self, name: &str) -> Result<PathBuf, CliError> {
        self.optional_path(name)
            .ok_or_else(|| CliError::Usage(format!("missing --{name}")))
    }

    fn optional_path(&self, name: &str) -> Option<PathBuf> {
        self.optional(name).map(PathBuf::from)
    }

    fn optional(&self, name: &str) -> Option<&str> {
        self.values.get(name).and_then(Option::as_deref)
    }

    fn number<T>(&self, name: &str) -> Result<Option<T>, CliError>
    where
        T: std::str::FromStr,
        T::Err: std::fmt::Display,
    {
        let Some(value) = self.optional(name) else {
            return Ok(None);
        };
        parse_number(name, value).map(Some)
    }
}

fn parse_argument_values(
    raw: &[std::ffi::OsString],
) -> Result<BTreeMap<String, Option<String>>, CliError> {
    let mut values = BTreeMap::new();
    let mut index = 0;
    while index < raw.len() {
        let (name, value) = parse_argument(raw, index)?;
        insert_argument(&mut values, name, value)?;
        index += 2;
    }
    Ok(values)
}

fn parse_argument(
    raw: &[std::ffi::OsString],
    index: usize,
) -> Result<(String, String), CliError> {
    let raw_name = raw[index].to_str().ok_or_else(|| {
        CliError::Usage("argument names must be UTF-8".to_owned())
    })?;
    let Some(name) = raw_name.strip_prefix("--") else {
        return Err(CliError::Usage(format!(
            "unexpected positional argument {raw_name:?}"
        )));
    };
    let value = raw
        .get(index + 1)
        .ok_or_else(|| CliError::Usage(format!("--{name} requires a value")))?;
    let value = value.to_str().ok_or_else(|| {
        CliError::Usage(format!("--{name} value must be UTF-8"))
    })?;
    if value.starts_with("--") {
        return Err(CliError::Usage(format!("--{name} requires a value")));
    }
    Ok((name.to_owned(), value.to_owned()))
}

fn insert_argument(
    values: &mut BTreeMap<String, Option<String>>,
    name: String,
    value: String,
) -> Result<(), CliError> {
    if values.contains_key(&name) {
        return Err(CliError::Usage(format!("duplicate --{name}")));
    }
    values.insert(name, Some(value));
    Ok(())
}

fn reject_unknown_arguments(
    values: &BTreeMap<String, Option<String>>,
    admitted: &[&str],
) -> Result<(), CliError> {
    for name in values.keys() {
        if !admitted.contains(&name.as_str()) {
            return Err(CliError::Usage(format!("unknown option --{name}")));
        }
    }
    Ok(())
}

fn parse_number<T>(name: &str, value: &str) -> Result<T, CliError>
where
    T: std::str::FromStr,
    T::Err: std::fmt::Display,
{
    value
        .parse()
        .map_err(|error| CliError::Usage(format!("invalid --{name}: {error}")))
}

#[cfg(test)]
mod tests {
    use std::io;

    struct ClosedOutput;

    impl io::Write for ClosedOutput {
        fn write(&mut self, _buffer: &[u8]) -> io::Result<usize> {
            Err(io::Error::new(io::ErrorKind::BrokenPipe, "closed output"))
        }

        fn flush(&mut self) -> io::Result<()> {
            Err(io::Error::new(io::ErrorKind::BrokenPipe, "closed output"))
        }
    }

    #[test]
    fn post_commit_response_failure_retains_applied_effect_and_path() {
        let path = std::path::Path::new(
            "/evidence/identity/conformance/source-role-map.json",
        );
        let error = super::deliver_applied_response(
            &mut ClosedOutput,
            b"{}\n",
            path,
            reovim_conformance::CleanupState::Complete,
            reovim_conformance::DurabilityState::DirectorySynced,
        )
        .unwrap_err();
        let super::CliError::Publication(error) = error else {
            panic!("expected publication delivery error")
        };
        assert_eq!(
            error.effect_certainty,
            reovim_conformance::EffectCertainty::Applied
        );
        assert_eq!(error.intended_path.as_deref(), Some(path));
        assert_eq!(
            error.cleanup_state,
            reovim_conformance::CleanupState::Complete
        );
        assert_eq!(
            error.durability_state,
            reovim_conformance::DurabilityState::DirectorySynced
        );
    }
}
