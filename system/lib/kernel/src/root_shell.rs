//! Root daemon parser and scheduled program dispatch frontend.
//!
//! The root shell owns line tokenization, argv shaping, and scheduled `/bin`
//! program entry dispatch only. Executable lookup and admission belong to the
//! exec/syscall path.

use crate::{
    program::{
        self, LoadedProgram, ProgramArgvBuffer, ProgramEnvBuffer, ProgramStatus,
        program_env_name_is_valid,
    },
    rootd::RootDaemon,
    syscall::{ProgramStdoutCapture, ProgramSyscalls, SyscallContext},
    vfs::PathBuf,
};

const MAX_ARGS: usize = 8;
const MAX_TOKEN_BYTES: usize = 64;

/// Mutable root-shell state carried across program dispatch.
pub struct RootShellSession {
    cwd: PathBuf,
    shell_target_requested: Option<&'static str>,
    shell_start_requested: bool,
    line_discipline_requested: bool,
    line_discipline: &'static str,
    pipe_mode: &'static str,
    owner_path: &'static str,
    owner_loader: &'static str,
    owner_entry_name: &'static str,
}

impl RootShellSession {
    /// Starts a shell session at `/`.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            cwd: PathBuf::root(),
            shell_target_requested: None,
            shell_start_requested: false,
            line_discipline_requested: false,
            line_discipline: "none",
            pipe_mode: "none",
            owner_path: "root-shell",
            owner_loader: "kernel",
            owner_entry_name: "root_shell",
        }
    }

    /// Current working directory.
    #[must_use]
    pub fn cwd(&self) -> &str {
        self.cwd.as_str()
    }

    pub(crate) fn set_cwd(&mut self, cwd: PathBuf) {
        self.cwd = cwd;
    }

    pub(crate) fn request_shell_start(&mut self) {
        self.shell_start_requested = true;
    }

    pub(crate) fn request_line_discipline(
        &mut self,
        line_discipline: &'static str,
        pipe_mode: &'static str,
    ) {
        self.line_discipline_requested = true;
        self.line_discipline = line_discipline;
        self.pipe_mode = pipe_mode;
    }

    pub(crate) fn request_shell_target(&mut self, program: &'static str) {
        self.shell_target_requested = Some(program);
    }

    pub(crate) fn install_shell_owner(
        &mut self,
        program_path: &'static str,
        loader: &'static str,
        entry_name: &'static str,
    ) {
        self.owner_path = program_path;
        self.owner_loader = loader;
        self.owner_entry_name = entry_name;
    }

    /// `/bin` program requested by `/bin/init` as the shell/session target.
    #[must_use]
    pub const fn shell_target_requested(&self) -> Option<&'static str> {
        self.shell_target_requested
    }

    /// Whether the requested shell target entered interactive shell startup.
    #[must_use]
    pub const fn shell_start_requested(&self) -> bool {
        self.shell_start_requested
    }

    /// Whether the shell target selected the interactive command-line discipline.
    #[must_use]
    pub const fn line_discipline_requested(&self) -> bool {
        self.line_discipline_requested
    }

    /// Command-line parser/discipline requested by the shell target.
    #[must_use]
    pub const fn line_discipline(&self) -> &'static str {
        self.line_discipline
    }

    /// Pipeline mode requested by the shell target.
    #[must_use]
    pub const fn pipe_mode(&self) -> &'static str {
        self.pipe_mode
    }

    /// Executable path currently owning the interactive shell session.
    #[must_use]
    pub const fn owner_path(&self) -> &'static str {
        self.owner_path
    }

    /// Loader/source kind for the executable that owns the shell session.
    #[must_use]
    pub const fn owner_loader(&self) -> &'static str {
        self.owner_loader
    }

    /// Stable entry name for the executable that owns the shell session.
    #[must_use]
    pub const fn owner_entry_name(&self) -> &'static str {
        self.owner_entry_name
    }
}

impl Default for RootShellSession {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum ParseStatus {
    Empty,
    Ok,
    TooLong,
    TooMany,
    UnclosedQuote,
}

#[derive(Clone, Copy, Debug)]
enum QuoteState {
    None,
    Single,
    Double,
}

/// Result from executing one root-shell line.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct ProgramLineResult {
    status: ProgramStatus,
}

/// Owned `/bin` argv/env entry payload.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct ProgramInvocation {
    argv: ProgramArgvBuffer,
    env: ProgramEnvBuffer,
}

impl ProgramInvocation {
    pub(crate) const fn new(argv: ProgramArgvBuffer, env: ProgramEnvBuffer) -> Self {
        Self { argv, env }
    }

    /// Returns admitted argv.
    #[must_use]
    pub const fn argv(&self) -> &ProgramArgvBuffer {
        &self.argv
    }

    /// Returns admitted env.
    #[must_use]
    pub const fn env(&self) -> &ProgramEnvBuffer {
        &self.env
    }
}

impl ProgramLineResult {
    const fn new(status: ProgramStatus) -> Self {
        Self { status }
    }

    /// Returns the program status.
    #[must_use]
    pub const fn status(self) -> ProgramStatus {
        self.status
    }
}

#[derive(Clone, Copy)]
struct Arg {
    bytes: [u8; MAX_TOKEN_BYTES],
    len: usize,
    present: bool,
}

impl Arg {
    const fn new() -> Self {
        Self {
            bytes: [0u8; MAX_TOKEN_BYTES],
            len: 0,
            present: false,
        }
    }

    fn clear(&mut self) {
        self.len = 0;
        self.present = false;
    }

    fn push_byte(&mut self, byte: u8, status: &mut ParseStatus) {
        if self.len >= MAX_TOKEN_BYTES {
            *status = ParseStatus::TooLong;
            return;
        }
        self.present = true;
        self.bytes[self.len] = byte;
        self.len += 1;
    }

    fn mark_present(&mut self) {
        self.present = true;
    }

    fn as_str(&self) -> &str {
        // Tokens are ASCII-only in shell fixtures and in current CLI transcripts.
        // SAFETY: this boundary intentionally accepts kernel-shell bytes and avoids
        // UTF-8 validation in hot-path parsing. If shell input includes
        // non-ASCII by mistake, output will still be byte-preserving.
        unsafe { core::str::from_utf8_unchecked(&self.bytes[..self.len]) }
    }
}

#[derive(Clone, Copy)]
struct ParsedLine {
    args: [Arg; MAX_ARGS],
    argc: usize,
}

impl ParsedLine {
    const fn empty() -> Self {
        Self {
            args: [Arg::new(); MAX_ARGS],
            argc: 0,
        }
    }
}

fn push_arg(args: &mut ParsedLine, arg: &mut Arg, status: &mut ParseStatus) {
    if !arg.present {
        return;
    }
    if args.argc >= MAX_ARGS {
        *status = ParseStatus::TooMany;
        return;
    }
    args.args[args.argc] = *arg;
    args.argc += 1;
    arg.clear();
}

fn tokenize(input: &[u8]) -> (ParsedLine, ParseStatus) {
    let mut line = ParsedLine::empty();
    let mut status = ParseStatus::Empty;
    let mut current = Arg::new();
    let mut quote = QuoteState::None;
    let mut i = 0usize;

    while i < input.len() {
        let byte = input[i];

        match quote {
            QuoteState::None => match byte {
                b'\n' | b'\r' => {
                    push_arg(&mut line, &mut current, &mut status);
                    break;
                }
                b' ' | b'\t' => {
                    push_arg(&mut line, &mut current, &mut status);
                }
                b'\'' => {
                    current.mark_present();
                    quote = QuoteState::Single;
                    if status == ParseStatus::Empty {
                        status = ParseStatus::Ok;
                    }
                }
                b'"' => {
                    current.mark_present();
                    quote = QuoteState::Double;
                    if status == ParseStatus::Empty {
                        status = ParseStatus::Ok;
                    }
                }
                b'#' if line.argc == 0 && current.len == 0 => {
                    status = ParseStatus::Ok;
                    break;
                }
                _ => {
                    if status == ParseStatus::Empty {
                        status = ParseStatus::Ok;
                    }
                    current.push_byte(byte, &mut status);
                }
            },
            QuoteState::Single => {
                if byte == b'\'' {
                    quote = QuoteState::None;
                } else {
                    current.push_byte(byte, &mut status);
                }
            }
            QuoteState::Double => {
                if byte == b'"' {
                    quote = QuoteState::None;
                } else {
                    current.push_byte(byte, &mut status);
                }
            }
        }

        i += 1;
    }

    match quote {
        QuoteState::None => {}
        QuoteState::Single | QuoteState::Double => status = ParseStatus::UnclosedQuote,
    }

    if status == ParseStatus::Empty && current.len > 0 {
        status = ParseStatus::Ok;
    }
    push_arg(&mut line, &mut current, &mut status);

    (line, status)
}

/// Parse error for a root-shell input line before exec admission.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ShellLineError {
    /// One token exceeded the bounded shell token capacity.
    TooLong,
    /// The line supplied more argv entries than the bounded parser accepts.
    TooMany,
    /// A quote was opened without a matching close.
    UnclosedQuote,
    /// Pipe syntax was malformed.
    InvalidPipe,
}

impl ShellLineError {
    /// Operator-facing parse error text.
    #[must_use]
    pub const fn message(self) -> &'static str {
        match self {
            Self::TooLong => "error: shell token too long",
            Self::TooMany => "error: too many arguments",
            Self::UnclosedQuote => "error: unterminated quote",
            Self::InvalidPipe => "error: invalid pipe",
        }
    }
}

fn trim_shell_line(mut line: &[u8]) -> &[u8] {
    while matches!(line.first(), Some(b' ' | b'\t' | b'\r' | b'\n')) {
        line = &line[1..];
    }
    while matches!(line.last(), Some(b' ' | b'\t' | b'\r' | b'\n')) {
        line = &line[..line.len() - 1];
    }
    line
}

/// Splits one shell line at a single unquoted pipe.
pub(crate) fn split_pipeline(line: &[u8]) -> Result<Option<(&[u8], &[u8])>, ShellLineError> {
    let mut quote = QuoteState::None;
    let mut pipe_at = None;
    let mut index = 0usize;

    while index < line.len() {
        let byte = line[index];
        match quote {
            QuoteState::None => match byte {
                b'\'' => quote = QuoteState::Single,
                b'"' => quote = QuoteState::Double,
                b'|' => {
                    if pipe_at.is_some() {
                        return Err(ShellLineError::InvalidPipe);
                    }
                    pipe_at = Some(index);
                }
                b'\n' | b'\r' => break,
                _ => {}
            },
            QuoteState::Single => {
                if byte == b'\'' {
                    quote = QuoteState::None;
                }
            }
            QuoteState::Double => {
                if byte == b'"' {
                    quote = QuoteState::None;
                }
            }
        }
        index += 1;
    }

    match quote {
        QuoteState::None => {}
        QuoteState::Single | QuoteState::Double => return Err(ShellLineError::UnclosedQuote),
    }

    let Some(pipe_at) = pipe_at else {
        return Ok(None);
    };
    let left = trim_shell_line(&line[..pipe_at]);
    let right = trim_shell_line(&line[pipe_at + 1..index]);
    if left.is_empty() || right.is_empty() {
        return Err(ShellLineError::InvalidPipe);
    }
    Ok(Some((left, right)))
}

fn run_program(
    daemon: &RootDaemon<'_>,
    session: &mut RootShellSession,
    program: LoadedProgram,
    argv: &ProgramArgvBuffer,
    env: &ProgramEnvBuffer,
    stdin: &[u8],
    stdout_capture: Option<&ProgramStdoutCapture>,
    current: Option<SyscallContext>,
) -> ProgramStatus {
    let mut syscalls = ProgramSyscalls::new_with_stdin_and_stdout_capture(
        daemon,
        session,
        current,
        stdin,
        stdout_capture,
    );
    program::run_loaded_program(program, argv, env, stdin, &mut syscalls)
}

fn split_env_assignment(token: &str) -> Option<(&str, &str)> {
    let bytes = token.as_bytes();
    let mut split = 0usize;
    while split < bytes.len() && bytes[split] != b'=' {
        split += 1;
    }
    if split == 0 || split >= bytes.len() {
        return None;
    }
    let name = &bytes[..split];
    if !program_env_name_is_valid(name) {
        return None;
    }
    Some((&token[..split], &token[split + 1..]))
}

fn program_invocation(parsed: &ParsedLine) -> Result<Option<ProgramInvocation>, ShellLineError> {
    let mut env = ProgramEnvBuffer::empty();
    let mut first_argv = 0usize;
    while first_argv < parsed.argc {
        let token = parsed.args[first_argv].as_str();
        let Some((name, value)) = split_env_assignment(token) else {
            break;
        };
        env.push(name, value).map_err(|_| ShellLineError::TooLong)?;
        first_argv += 1;
    }
    if first_argv >= parsed.argc {
        return Ok(None);
    }

    let mut argv = ProgramArgvBuffer::empty();
    let mut index = first_argv;
    while index < parsed.argc {
        argv.push(parsed.args[index].as_str())
            .map_err(|_| ShellLineError::TooLong)?;
        index += 1;
    }
    Ok(Some(ProgramInvocation::new(argv, env)))
}

/// Parses a root-shell input line into owned argv plus leading env assignments.
pub(crate) fn parse_program_invocation(
    line: &[u8],
) -> Result<Option<ProgramInvocation>, ShellLineError> {
    let (parsed, status) = tokenize(line);

    match status {
        ParseStatus::Empty => return Ok(None),
        ParseStatus::TooLong => return Err(ShellLineError::TooLong),
        ParseStatus::TooMany => return Err(ShellLineError::TooMany),
        ParseStatus::UnclosedQuote => return Err(ShellLineError::UnclosedQuote),
        ParseStatus::Ok => {}
    }

    if parsed.argc == 0 {
        return Ok(None);
    }

    program_invocation(&parsed)
}

fn execute_loaded_program_from_argv(
    daemon: &RootDaemon<'_>,
    session: &mut RootShellSession,
    program: LoadedProgram,
    argv: &ProgramArgvBuffer,
    env: &ProgramEnvBuffer,
    stdin: &[u8],
    stdout_capture: Option<&ProgramStdoutCapture>,
    current: Option<SyscallContext>,
) -> ProgramLineResult {
    let status = run_program(daemon, session, program, argv, env, stdin, stdout_capture, current);
    ProgramLineResult::new(status)
}

/// Execute a scheduler-selected loaded program using its admitted argv.
pub(crate) fn execute_loaded_program_argv(
    daemon: &RootDaemon<'_>,
    session: &mut RootShellSession,
    program: LoadedProgram,
    argv: &ProgramArgvBuffer,
    env: &ProgramEnvBuffer,
    stdin: &[u8],
    stdout_capture: Option<&ProgramStdoutCapture>,
    current: Option<SyscallContext>,
) -> ProgramLineResult {
    if argv.argc() == 0 {
        return ProgramLineResult::new(ProgramStatus::Empty);
    }

    execute_loaded_program_from_argv(
        daemon,
        session,
        program,
        argv,
        env,
        stdin,
        stdout_capture,
        current,
    )
}

#[cfg(feature = "selftest")]
#[path = "root_shell_tests.rs"]
mod tests;
