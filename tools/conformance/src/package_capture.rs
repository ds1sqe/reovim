//! Exact, bounded Cargo source-package membership capture.

use std::{
    collections::{BTreeMap, BTreeSet},
    ffi::{OsStr, OsString},
    fmt,
    path::{Component, Path, PathBuf},
};

use crate::{
    compiler_capture::{
        RUSTC_LAUNCHER_LIBRARY, RUSTC_LAUNCHER_MODE, RUSTC_LAUNCHER_RUSTC,
    },
    digest,
    process::{
        self, CommandSpec, ProcessLimits, ProcessResult, TerminationReason,
    },
};

const SEMANTIC_DOMAIN: &[u8] = b"reovim-package-member-set/v1\0";
const COMPILER_LIBRARY_PATH: &str = "/compiler/lib";

/// Position of one package-list capture around compiler execution.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Phase {
    BeforeCompiler,
    AfterCompiler,
}

impl Phase {
    /// Stable report spelling.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::BeforeCompiler => "before_compiler",
            Self::AfterCompiler => "after_compiler",
        }
    }
}

/// Whether Cargo may inspect a dirty candidate tree.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum DirtyMode {
    Reject,
}

/// Committed limits for one package-list execution and parse.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Limits {
    pub process: ProcessLimits,
    pub output_bytes: usize,
    pub member_count: usize,
    pub path_bytes: usize,
}

/// Exact prepared-root Cargo package-list selection.
#[derive(Debug, Clone, Copy)]
pub struct Request<'a> {
    pub cargo: &'a Path,
    pub rustc: &'a Path,
    pub rustc_launcher: &'a Path,
    pub cargo_home: &'a Path,
    pub cargo_target_dir: &'a Path,
    pub temp_dir: &'a Path,
    pub manifest: &'a Path,
    pub package: &'a str,
    pub phase: Phase,
    pub dirty_mode: DirtyMode,
}

/// Semantic and raw identity retained from one package-list capture.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Observation {
    pub phase: Phase,
    pub dirty_mode: DirtyMode,
    pub command_argv: Vec<String>,
    pub raw_output_digest: String,
    pub semantic_digest: String,
    pub stdout_bytes: u64,
    pub member_path_bytes: u64,
    pub members: Vec<PathBuf>,
}

/// Package-list observation accepted after full process supervision.
#[derive(Debug)]
pub struct Capture {
    pub observation: Observation,
}

/// Stable comparison result retaining both raw identities.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Comparison {
    pub semantic_digest: String,
    pub before_raw_output_digest: String,
    pub after_raw_output_digest: String,
    pub dirty_mode: DirtyMode,
}

/// Typed package-list construction, execution, parse, or comparison failure.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Error {
    pub code: &'static str,
    pub message: String,
}

impl Error {
    fn new(code: &'static str, message: impl Into<String>) -> Self {
        Self { code, message: message.into() }
    }
}

impl fmt::Display for Error {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}: {}", self.code, self.message)
    }
}

impl std::error::Error for Error {}

/// Constructs the exact shell-free Cargo package-list invocation.
///
/// Exact mode omits `--allow-dirty`; the candidate-only mode adds it and the
/// resulting observation retains that fact.
///
/// # Errors
///
/// Returns a typed error for a non-normalized prepared path or package name.
pub fn command_spec(
    request: Request<'_>,
    path_byte_limit: usize,
) -> Result<CommandSpec, Error> {
    if path_byte_limit == 0 {
        return Err(Error::new(
            "PACKAGE_CAPTURE_ZERO_LIMIT",
            "package command path limit must be positive",
        ));
    }
    for (name, path) in [
        ("Cargo", request.cargo),
        ("rustc", request.rustc),
        ("rustc launcher", request.rustc_launcher),
        ("Cargo home", request.cargo_home),
        ("Cargo target directory", request.cargo_target_dir),
        ("temporary directory", request.temp_dir),
        ("workspace manifest", request.manifest),
    ] {
        validate_absolute(path, name, path_byte_limit)?;
    }
    validate_manifest(request.manifest)?;
    validate_package_name(request.package, path_byte_limit)?;

    let arguments = command_arguments(request);
    let environment = BTreeMap::from([
        (
            OsString::from("CARGO_HOME"),
            request.cargo_home.as_os_str().to_owned(),
        ),
        (OsString::from("CARGO_NET_OFFLINE"), OsString::from("true")),
        (
            OsString::from("LD_LIBRARY_PATH"),
            OsString::from(COMPILER_LIBRARY_PATH),
        ),
        (
            OsString::from("CARGO_TARGET_DIR"),
            request.cargo_target_dir.as_os_str().to_owned(),
        ),
        (
            OsString::from("RUSTC"),
            request.rustc_launcher.as_os_str().to_owned(),
        ),
        (OsString::from(RUSTC_LAUNCHER_MODE), OsString::from("1")),
        (
            OsString::from(RUSTC_LAUNCHER_RUSTC),
            request.rustc.as_os_str().to_owned(),
        ),
        (
            OsString::from(RUSTC_LAUNCHER_LIBRARY),
            OsString::from(COMPILER_LIBRARY_PATH),
        ),
        (OsString::from("TEMP"), request.temp_dir.as_os_str().to_owned()),
        (OsString::from("TERM"), OsString::from("dumb")),
        (OsString::from("TMP"), request.temp_dir.as_os_str().to_owned()),
        (OsString::from("TMPDIR"), request.temp_dir.as_os_str().to_owned()),
    ]);
    Ok(CommandSpec {
        program: request.cargo.to_path_buf(),
        arguments,
        current_dir: PathBuf::from("/"),
        environment,
        clear_environment: true,
    })
}

/// Runs and parses one exact package-list capture.
///
/// # Errors
///
/// Returns a typed error for invalid limits, execution uncertainty, nonzero or
/// noisy Cargo output, ownership uncertainty, or malformed package paths.
pub fn run(request: Request<'_>, limits: Limits) -> Result<Capture, Error> {
    validate_limits(limits)?;
    let spec = command_spec(request, limits.path_bytes)?;
    let process = process::run(&spec, limits.process).map_err(|failure| {
        Error::new("PACKAGE_LIST_RUN", failure.to_string())
    })?;
    validate_process(&process)?;
    require_argv(&spec, &process)?;
    let command_argv = utf8_argv(&process.argv)?;
    let observation = parse_output(
        request.phase,
        request.dirty_mode,
        command_argv,
        &process.stdout,
        limits,
    )?;
    Ok(Capture { observation })
}

/// Parses retained raw package-list output under the committed bounds.
///
/// # Errors
///
/// Returns a typed error for invalid limits, encoding, framing, path form,
/// duplicates, or exhausted output/member/path bounds.
pub fn parse_output(
    phase: Phase,
    dirty_mode: DirtyMode,
    command_argv: Vec<String>,
    output: &[u8],
    limits: Limits,
) -> Result<Observation, Error> {
    validate_limits(limits)?;
    validate_observed_argv(&command_argv, dirty_mode)?;
    validate_output_size(output, limits.output_bytes)?;
    let text = std::str::from_utf8(output).map_err(|failure| {
        Error::new("PACKAGE_LIST_UTF8", failure.to_string())
    })?;
    validate_framing(text)?;

    let mut members = BTreeSet::new();
    let mut member_path_bytes = 0_u64;
    for line in text[..text.len() - 1].split('\n') {
        let path = parse_member(line, limits.path_bytes)?;
        charge_member(&mut member_path_bytes, line, limits.output_bytes)?;
        if !members.insert(path) {
            return Err(Error::new(
                "PACKAGE_LIST_DUPLICATE",
                format!("duplicate package path {line:?}"),
            ));
        }
        if members.len() > limits.member_count {
            return Err(Error::new(
                "PACKAGE_LIST_MEMBER_COUNT_LIMIT",
                format!("package members exceed {}", limits.member_count),
            ));
        }
    }
    let members = members.into_iter().collect::<Vec<_>>();
    let semantic_digest = semantic_digest(&members)?;
    let stdout_bytes = u64::try_from(output.len()).map_err(|_| {
        Error::new(
            "PACKAGE_LIST_BYTE_COUNT",
            "package-list byte count does not fit u64",
        )
    })?;
    Ok(Observation {
        phase,
        dirty_mode,
        command_argv,
        raw_output_digest: digest::bytes(output),
        semantic_digest,
        stdout_bytes,
        member_path_bytes,
        members,
    })
}

fn validate_observed_argv(
    argv: &[String],
    dirty_mode: DirtyMode,
) -> Result<(), Error> {
    if argv.is_empty()
        || argv
            .iter()
            .any(|item| item.is_empty() || item.chars().any(char::is_control))
    {
        return Err(Error::new(
            "PACKAGE_LIST_ARGV_FORM",
            "package-list argv contains empty or control text",
        ));
    }
    let dirty_flags =
        argv.iter().filter(|item| item.as_str() == "--allow-dirty").count();
    let matches = dirty_mode == DirtyMode::Reject && dirty_flags == 0;
    if matches {
        return Ok(());
    }
    Err(Error::new(
        "PACKAGE_LIST_DIRTY_MODE",
        "retained dirty mode differs from package-list argv",
    ))
}

/// Reconciles the required before/after semantic package-member sets.
///
/// Raw output order remains visible through both retained digests. Different
/// raw order is accepted only when the canonical semantic set is unchanged.
///
/// # Errors
///
/// Returns a typed error for wrong phases, changed invocation/dirty policy, or
/// different semantic member sets.
pub fn compare(
    before: &Observation,
    after: &Observation,
) -> Result<Comparison, Error> {
    validate_phases(before, after)?;
    validate_semantic_identity(before)?;
    validate_semantic_identity(after)?;
    if before.dirty_mode != after.dirty_mode {
        return Err(Error::new(
            "PACKAGE_LIST_DIRTY_MODE_DRIFT",
            "before/after dirty policy differs",
        ));
    }
    if before.command_argv != after.command_argv {
        return Err(Error::new(
            "PACKAGE_LIST_COMMAND_DRIFT",
            "before/after Cargo package-list argv differs",
        ));
    }
    if before.members != after.members
        || before.semantic_digest != after.semantic_digest
    {
        return Err(Error::new(
            "PACKAGE_LIST_DRIFT",
            difference_message(&before.members, &after.members),
        ));
    }
    Ok(Comparison {
        semantic_digest: before.semantic_digest.clone(),
        before_raw_output_digest: before.raw_output_digest.clone(),
        after_raw_output_digest: after.raw_output_digest.clone(),
        dirty_mode: before.dirty_mode,
    })
}

fn validate_semantic_identity(observation: &Observation) -> Result<(), Error> {
    validate_observed_argv(&observation.command_argv, observation.dirty_mode)?;
    let ordered = observation.members.windows(2).all(|pair| pair[0] < pair[1]);
    let expected = semantic_digest(&observation.members)?;
    if !observation.members.is_empty()
        && ordered
        && expected == observation.semantic_digest
    {
        return Ok(());
    }
    Err(Error::new(
        "PACKAGE_LIST_SEMANTIC_IDENTITY",
        "member order or semantic digest is invalid",
    ))
}

fn command_arguments(request: Request<'_>) -> Vec<OsString> {
    vec![
        OsString::from("package"),
        OsString::from("--list"),
        OsString::from("--quiet"),
        OsString::from("--locked"),
        OsString::from("--offline"),
        OsString::from("--no-metadata"),
        OsString::from("--manifest-path"),
        request.manifest.as_os_str().to_owned(),
        OsString::from("--package"),
        OsString::from(request.package),
    ]
}

fn validate_limits(limits: Limits) -> Result<(), Error> {
    if limits.output_bytes == 0
        || limits.member_count == 0
        || limits.path_bytes == 0
    {
        return Err(Error::new(
            "PACKAGE_CAPTURE_ZERO_LIMIT",
            "package capture limits must be positive",
        ));
    }
    if limits.process.stdout_bytes != limits.output_bytes {
        return Err(Error::new(
            "PACKAGE_CAPTURE_PROCESS_LIMIT",
            "stdout process bound must equal the package-list byte bound",
        ));
    }
    Ok(())
}

fn validate_absolute(
    path: &Path,
    name: &str,
    path_byte_limit: usize,
) -> Result<(), Error> {
    let text = path.to_str().ok_or_else(|| {
        Error::new("PACKAGE_COMMAND_PATH_UTF8", format!("{name} is not UTF-8"))
    })?;
    if text.len() > path_byte_limit {
        return Err(Error::new(
            "PACKAGE_COMMAND_PATH_LIMIT",
            format!("{name} exceeds {path_byte_limit} bytes"),
        ));
    }
    if normalized_absolute(text) {
        return Ok(());
    }
    Err(Error::new(
        "PACKAGE_COMMAND_PATH",
        format!("{name} must be normalized absolute UTF-8"),
    ))
}

fn normalized_absolute(text: &str) -> bool {
    text.starts_with('/')
        && (text == "/" || !text.ends_with('/'))
        && !text.contains(['\0', '\r', '\n', '\\'])
        && !text.contains("//")
        && !text.split('/').any(|part| matches!(part, "." | ".."))
}

fn validate_manifest(path: &Path) -> Result<(), Error> {
    if path.file_name() == Some(OsStr::new("Cargo.toml")) {
        return Ok(());
    }
    Err(Error::new(
        "PACKAGE_COMMAND_MANIFEST",
        "workspace manifest must name Cargo.toml",
    ))
}

fn validate_package_name(
    name: &str,
    path_byte_limit: usize,
) -> Result<(), Error> {
    let valid = !name.is_empty()
        && name.len() <= path_byte_limit
        && name.bytes().all(|byte| {
            byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_')
        })
        && name.as_bytes()[0].is_ascii_alphanumeric();
    if valid {
        return Ok(());
    }
    Err(Error::new(
        "PACKAGE_COMMAND_PACKAGE",
        "package name is outside the exact Cargo-name grammar",
    ))
}

fn validate_process(process: &ProcessResult) -> Result<(), Error> {
    if process.stdout_read_error.is_some()
        || process.stderr_read_error.is_some()
    {
        return Err(Error::new(
            "PACKAGE_LIST_PIPE_READ",
            format!(
                "stdout={:?}, stderr={:?}",
                process.stdout_read_error, process.stderr_read_error
            ),
        ));
    }
    if process.termination_reason != TerminationReason::Exited
        || process.stdout_overflow
        || process.stderr_overflow
    {
        return Err(Error::new(
            "PACKAGE_LIST_PROCESS_BOUND",
            format!("Cargo stopped by {}", process.termination_reason.as_str()),
        ));
    }
    if !process.status.success() {
        return Err(Error::new(
            "PACKAGE_LIST_NONZERO",
            format!("Cargo exited with {}", process.status),
        ));
    }
    if !process.stderr.is_empty() {
        return Err(Error::new(
            "PACKAGE_LIST_STDERR",
            "quiet Cargo package-list wrote stderr",
        ));
    }
    validate_capture_counts(process)?;
    validate_process_ownership(process)
}

fn validate_capture_counts(process: &ProcessResult) -> Result<(), Error> {
    let stdout = u64::try_from(process.stdout.len()).map_err(|_| {
        Error::new(
            "PACKAGE_LIST_BYTE_COUNT",
            "retained stdout length does not fit u64",
        )
    })?;
    let stderr = u64::try_from(process.stderr.len()).map_err(|_| {
        Error::new(
            "PACKAGE_LIST_BYTE_COUNT",
            "retained stderr length does not fit u64",
        )
    })?;
    if process.stdout_bytes == stdout && process.stderr_bytes == stderr {
        return Ok(());
    }
    Err(Error::new(
        "PACKAGE_LIST_BYTE_COUNT",
        "process byte counts differ from retained pipe bytes",
    ))
}

#[cfg(target_os = "linux")]
fn validate_process_ownership(process: &ProcessResult) -> Result<(), Error> {
    let complete = process.pgid.is_some()
        && process.group_identity_validated
        && process.group_absent_after_reap
        && process.descendants_owned
        && process.descendants_reaped
        && !process.descendant_residue_observed
        && process.reaped
        && process.readers_joined;
    if complete {
        return Ok(());
    }
    Err(Error::new(
        "PACKAGE_LIST_PROCESS_OWNERSHIP",
        "Cargo process ownership or cleanup evidence is incomplete",
    ))
}

#[cfg(not(target_os = "linux"))]
fn validate_process_ownership(_process: &ProcessResult) -> Result<(), Error> {
    Err(Error::new(
        "PACKAGE_LIST_PLATFORM_UNAVAILABLE",
        "Product package capture requires Linux process ownership",
    ))
}

fn require_argv(
    spec: &CommandSpec,
    process: &ProcessResult,
) -> Result<(), Error> {
    let mut expected = Vec::with_capacity(spec.arguments.len() + 1);
    expected.push(spec.program.as_os_str().to_owned());
    expected.extend(spec.arguments.iter().cloned());
    if process.argv == expected {
        return Ok(());
    }
    Err(Error::new(
        "PACKAGE_LIST_ARGV_DRIFT",
        "executed argv differs from the exact package-list command",
    ))
}

fn utf8_argv(argv: &[OsString]) -> Result<Vec<String>, Error> {
    argv.iter()
        .map(|item| {
            item.to_str().map(str::to_owned).ok_or_else(|| {
                Error::new(
                    "PACKAGE_LIST_ARGV_UTF8",
                    "package-list argv contains non-UTF-8 text",
                )
            })
        })
        .collect()
}

fn validate_output_size(output: &[u8], limit: usize) -> Result<(), Error> {
    if output.len() <= limit {
        return Ok(());
    }
    Err(Error::new(
        "PACKAGE_LIST_BYTE_LIMIT",
        format!("package-list output exceeds {limit} bytes"),
    ))
}

fn validate_framing(text: &str) -> Result<(), Error> {
    if text.is_empty() {
        return Err(Error::new(
            "PACKAGE_LIST_EMPTY",
            "package-list output is empty",
        ));
    }
    if !text.ends_with('\n') {
        return Err(Error::new(
            "PACKAGE_LIST_FINAL_LF",
            "package-list output lacks its final LF",
        ));
    }
    if text.contains('\0') {
        return Err(Error::new(
            "PACKAGE_LIST_NUL",
            "package-list output contains NUL",
        ));
    }
    if text.contains('\r') {
        return Err(Error::new(
            "PACKAGE_LIST_CR",
            "package-list output contains CR",
        ));
    }
    if text == "\n" {
        return Err(Error::new(
            "PACKAGE_LIST_EMPTY_PATH",
            "package-list output contains no member path",
        ));
    }
    Ok(())
}

fn parse_member(text: &str, path_limit: usize) -> Result<PathBuf, Error> {
    validate_member_text(text, path_limit)?;
    let path = PathBuf::from(text);
    if path.is_absolute() {
        return Err(member_error("PACKAGE_LIST_PATH_ABSOLUTE", text));
    }
    if path.components().all(|part| matches!(part, Component::Normal(_))) {
        return Ok(path);
    }
    Err(member_error("PACKAGE_LIST_PATH_NOT_NORMALIZED", text))
}

fn validate_member_text(text: &str, path_limit: usize) -> Result<(), Error> {
    if text.is_empty() {
        return Err(member_error("PACKAGE_LIST_EMPTY_PATH", text));
    }
    if text.len() > path_limit {
        return Err(member_error("PACKAGE_LIST_PATH_BYTE_LIMIT", text));
    }
    if text.starts_with('/') {
        return Err(member_error("PACKAGE_LIST_PATH_ABSOLUTE", text));
    }
    if windows_absolute(text) {
        return Err(member_error("PACKAGE_LIST_PATH_ABSOLUTE", text));
    }
    if text.contains('\\') {
        return Err(member_error("PACKAGE_LIST_PATH_BACKSLASH", text));
    }
    if text.ends_with('/') || text.contains("//") {
        return Err(member_error("PACKAGE_LIST_PATH_NOT_NORMALIZED", text));
    }
    if text.split('/').any(|part| part == "..") {
        return Err(member_error("PACKAGE_LIST_PATH_PARENT", text));
    }
    if text.split('/').any(|part| part == ".") {
        return Err(member_error("PACKAGE_LIST_PATH_NOT_NORMALIZED", text));
    }
    if text.chars().any(char::is_control) {
        return Err(member_error("PACKAGE_LIST_PATH_CONTROL", text));
    }
    Ok(())
}

const fn windows_absolute(text: &str) -> bool {
    let bytes = text.as_bytes();
    bytes.len() >= 2 && bytes[0].is_ascii_alphabetic() && bytes[1] == b':'
}

fn member_error(code: &'static str, text: &str) -> Error {
    Error::new(code, format!("invalid package path {text:?}"))
}

fn charge_member(
    total: &mut u64,
    text: &str,
    limit: usize,
) -> Result<(), Error> {
    let bytes = u64::try_from(text.len()).map_err(|_| {
        Error::new(
            "PACKAGE_LIST_BYTE_COUNT",
            "member path byte count does not fit u64",
        )
    })?;
    *total = total.checked_add(bytes).ok_or_else(|| {
        Error::new(
            "PACKAGE_LIST_BYTE_COUNT",
            "aggregate member path byte count overflow",
        )
    })?;
    let limit = u64::try_from(limit).map_err(|_| {
        Error::new(
            "PACKAGE_LIST_BYTE_COUNT",
            "package-list limit does not fit u64",
        )
    })?;
    if *total <= limit {
        return Ok(());
    }
    Err(Error::new(
        "PACKAGE_LIST_BYTE_LIMIT",
        "aggregate member path bytes exceed the output bound",
    ))
}

pub fn semantic_digest(members: &[PathBuf]) -> Result<String, Error> {
    let mut input = Vec::new();
    input.extend_from_slice(SEMANTIC_DOMAIN);
    append_length(&mut input, members.len())?;
    for member in members {
        let text = member.to_str().ok_or_else(|| {
            Error::new(
                "PACKAGE_LIST_PATH_UTF8",
                "semantic member path is not UTF-8",
            )
        })?;
        append_length(&mut input, text.len())?;
        input.extend_from_slice(text.as_bytes());
    }
    Ok(digest::bytes(&input))
}

fn append_length(input: &mut Vec<u8>, length: usize) -> Result<(), Error> {
    let length = u64::try_from(length).map_err(|_| {
        Error::new(
            "PACKAGE_LIST_BYTE_COUNT",
            "semantic digest field length does not fit u64",
        )
    })?;
    input.extend_from_slice(&length.to_be_bytes());
    Ok(())
}

fn validate_phases(
    before: &Observation,
    after: &Observation,
) -> Result<(), Error> {
    if before.phase == Phase::BeforeCompiler
        && after.phase == Phase::AfterCompiler
    {
        return Ok(());
    }
    Err(Error::new(
        "PACKAGE_LIST_PHASE",
        "comparison requires before_compiler then after_compiler",
    ))
}

fn difference_message(before: &[PathBuf], after: &[PathBuf]) -> String {
    let before = before.iter().collect::<BTreeSet<_>>();
    let after = after.iter().collect::<BTreeSet<_>>();
    let missing = before
        .difference(&after)
        .take(4)
        .map(|path| path.display().to_string())
        .collect::<Vec<_>>();
    let added = after
        .difference(&before)
        .take(4)
        .map(|path| path.display().to_string())
        .collect::<Vec<_>>();
    format!("package members changed; missing={missing:?}, added={added:?}")
}

#[cfg(test)]
mod tests {
    use std::{
        ffi::{OsStr, OsString},
        path::{Path, PathBuf},
        time::Duration,
    };

    use super::{DirtyMode, Limits, Phase, Request};
    use crate::process::{ProcessLimits, ProcessResult, TerminationReason};

    fn limits() -> Limits {
        Limits {
            process: ProcessLimits {
                stdout_bytes: 1024,
                stderr_bytes: 1024,
                deadline: Duration::from_secs(1),
                supervision_deadline: Duration::from_secs(2),
            },
            output_bytes: 1024,
            member_count: 16,
            path_bytes: 128,
        }
    }

    fn request(dirty_mode: DirtyMode) -> Request<'static> {
        Request {
            cargo: Path::new("/tools/cargo"),
            rustc: Path::new("/tools/rustc"),
            rustc_launcher: Path::new("/checker/reovim-conformance"),
            cargo_home: Path::new("/scratch/cargo-home"),
            cargo_target_dir: Path::new("/scratch/package-target"),
            temp_dir: Path::new("/scratch/tmp"),
            manifest: Path::new("/repo/product/Cargo.toml"),
            package: "reovim-foundation",
            phase: Phase::BeforeCompiler,
            dirty_mode,
        }
    }

    fn argv() -> Vec<String> {
        let spec = super::command_spec(
            request(DirtyMode::Reject),
            limits().path_bytes,
        )
        .unwrap();
        std::iter::once(spec.program.into_os_string())
            .chain(spec.arguments)
            .map(|item| item.into_string().unwrap())
            .collect()
    }

    #[test]
    fn command_is_exact_and_rejects_dirty_override() {
        let exact = super::command_spec(
            request(DirtyMode::Reject),
            limits().path_bytes,
        )
        .unwrap();
        let expected = [
            "package",
            "--list",
            "--quiet",
            "--locked",
            "--offline",
            "--no-metadata",
            "--manifest-path",
            "/repo/product/Cargo.toml",
            "--package",
            "reovim-foundation",
        ];

        assert_eq!(exact.program, Path::new("/tools/cargo"));
        assert_eq!(exact.current_dir, Path::new("/"));
        assert!(exact.clear_environment);
        assert_eq!(exact.arguments, os_strings(&expected));
        assert_eq!(
            exact.environment.get(OsStr::new("CARGO_NET_OFFLINE")),
            Some(&OsString::from("true"))
        );
    }

    fn os_strings(values: &[&str]) -> Vec<OsString> {
        values.iter().map(OsString::from).collect()
    }

    #[cfg(unix)]
    fn successful_process() -> ProcessResult {
        use std::os::unix::process::ExitStatusExt as _;

        ProcessResult {
            argv: vec![OsString::from("/tools/cargo")],
            status: std::process::ExitStatus::from_raw(0),
            stdout: b"Cargo.toml\n".to_vec(),
            stderr: Vec::new(),
            stdout_bytes: 11,
            stderr_bytes: 0,
            stdout_overflow: false,
            stderr_overflow: false,
            stdout_read_error: None,
            stderr_read_error: None,
            termination_reason: TerminationReason::Exited,
            kill_attempted: false,
            kill_raced_with_exit: false,
            limit_observed_after_exit: false,
            leader_pid: 10,
            pgid: Some(10),
            group_identity_validated: true,
            group_absent_after_reap: true,
            descendants_owned: true,
            descendants_reaped: true,
            descendant_residue_observed: false,
            reaped: true,
            readers_joined: true,
        }
    }

    #[cfg(unix)]
    #[test]
    fn process_capture_counts_and_cleanup_are_mandatory() {
        let process = successful_process();
        super::validate_process(&process).unwrap();

        let mut wrong_count = successful_process();
        wrong_count.stdout_bytes = 10;
        let count = super::validate_process(&wrong_count).unwrap_err();
        assert_eq!(count.code, "PACKAGE_LIST_BYTE_COUNT");

        let mut residue = successful_process();
        residue.descendant_residue_observed = true;
        let ownership = super::validate_process(&residue).unwrap_err();
        assert_eq!(ownership.code, "PACKAGE_LIST_PROCESS_OWNERSHIP");
    }

    #[test]
    fn parser_retains_raw_identity_and_canonical_semantic_set() {
        let output = b"src/lib.rs\nCargo.toml\n.cargo_vcs_info.json\n";
        let parsed = super::parse_output(
            Phase::BeforeCompiler,
            DirtyMode::Reject,
            argv(),
            output,
            limits(),
        )
        .unwrap();

        assert_eq!(parsed.phase.as_str(), "before_compiler");
        assert_eq!(parsed.dirty_mode, DirtyMode::Reject);
        assert_eq!(parsed.stdout_bytes, 43);
        assert_eq!(parsed.member_path_bytes, 40);
        assert_eq!(
            parsed.members,
            [".cargo_vcs_info.json", "Cargo.toml", "src/lib.rs",]
                .map(PathBuf::from)
        );
    }

    #[test]
    fn parser_rejects_encoding_framing_and_path_mutants() {
        let cases: &[(&[u8], &str)] = &[
            (b"", "PACKAGE_LIST_EMPTY"),
            (b"Cargo.toml", "PACKAGE_LIST_FINAL_LF"),
            (b"Cargo.toml\r\n", "PACKAGE_LIST_CR"),
            (b"Cargo.toml\0\n", "PACKAGE_LIST_NUL"),
            (b"Cargo.toml\nCargo.toml\n", "PACKAGE_LIST_DUPLICATE"),
            (b"/Cargo.toml\n", "PACKAGE_LIST_PATH_ABSOLUTE"),
            (b"C:/Cargo.toml\n", "PACKAGE_LIST_PATH_ABSOLUTE"),
            (b"../Cargo.toml\n", "PACKAGE_LIST_PATH_PARENT"),
            (b"a\\b\n", "PACKAGE_LIST_PATH_BACKSLASH"),
            (b"a//b\n", "PACKAGE_LIST_PATH_NOT_NORMALIZED"),
            (b"a/./b\n", "PACKAGE_LIST_PATH_NOT_NORMALIZED"),
            (b"a\tb\n", "PACKAGE_LIST_PATH_CONTROL"),
            (&[0xff, b'\n'], "PACKAGE_LIST_UTF8"),
        ];
        for (output, code) in cases {
            let error = super::parse_output(
                Phase::BeforeCompiler,
                DirtyMode::Reject,
                argv(),
                output,
                limits(),
            )
            .unwrap_err();
            assert_eq!(error.code, *code, "output={output:?}");
        }
    }

    #[test]
    fn parser_enforces_output_member_and_path_bounds() {
        let mut bounded = limits();
        bounded.output_bytes = 4;
        bounded.process.stdout_bytes = 4;
        let output = super::parse_output(
            Phase::BeforeCompiler,
            DirtyMode::Reject,
            argv(),
            b"Cargo.toml\n",
            bounded,
        )
        .unwrap_err();
        assert_eq!(output.code, "PACKAGE_LIST_BYTE_LIMIT");

        bounded = limits();
        bounded.member_count = 1;
        let count = super::parse_output(
            Phase::BeforeCompiler,
            DirtyMode::Reject,
            argv(),
            b"a\nb\n",
            bounded,
        )
        .unwrap_err();
        assert_eq!(count.code, "PACKAGE_LIST_MEMBER_COUNT_LIMIT");

        bounded = limits();
        bounded.path_bytes = 1;
        let path = super::parse_output(
            Phase::BeforeCompiler,
            DirtyMode::Reject,
            argv(),
            b"ab\n",
            bounded,
        )
        .unwrap_err();
        assert_eq!(path.code, "PACKAGE_LIST_PATH_BYTE_LIMIT");
    }

    #[test]
    fn comparison_uses_semantics_without_erasing_raw_order() {
        let before = super::parse_output(
            Phase::BeforeCompiler,
            DirtyMode::Reject,
            argv(),
            b"a\nb\n",
            limits(),
        )
        .unwrap();
        let after = super::parse_output(
            Phase::AfterCompiler,
            DirtyMode::Reject,
            argv(),
            b"b\na\n",
            limits(),
        )
        .unwrap();
        let comparison = super::compare(&before, &after).unwrap();
        assert_eq!(comparison.semantic_digest, before.semantic_digest);
        assert_ne!(
            comparison.before_raw_output_digest,
            comparison.after_raw_output_digest
        );

        let changed = super::parse_output(
            Phase::AfterCompiler,
            DirtyMode::Reject,
            argv(),
            b"a\nc\n",
            limits(),
        )
        .unwrap();
        let drift = super::compare(&before, &changed).unwrap_err();
        assert_eq!(drift.code, "PACKAGE_LIST_DRIFT");
    }
}
