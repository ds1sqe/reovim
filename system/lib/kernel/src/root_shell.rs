//! Root daemon parser and scheduled program dispatch frontend.
//!
//! The root shell owns line tokenization, argv shaping, and scheduled `/bin`
//! program entry dispatch only. Executable lookup and admission belong to the
//! exec/syscall path.

use crate::{
    program::{self, LoadedProgram, ProgramArgvBuffer, ProgramStatus},
    rootd::RootDaemon,
    syscall::{ProgramStdoutCapture, ProgramSyscalls, SyscallContext},
    vfs::PathBuf,
};

const MAX_ARGS: usize = 8;
const MAX_TOKEN_BYTES: usize = 64;

/// Mutable root-shell state carried across program dispatch.
pub struct RootShellSession {
    cwd: PathBuf,
}

impl RootShellSession {
    /// Starts a shell session at `/`.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            cwd: PathBuf::root(),
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
    stdin: &[u8],
    stdout_capture: Option<&ProgramStdoutCapture>,
    current: Option<SyscallContext>,
) -> ProgramStatus {
    let argv = argv.borrowed();
    let mut syscalls = ProgramSyscalls::new_with_stdin_and_stdout_capture(
        daemon,
        session,
        current,
        stdin,
        stdout_capture,
    );
    program::run_loaded_program(program, &argv, &mut syscalls)
}

fn program_argv_buffer(parsed: &ParsedLine) -> Result<ProgramArgvBuffer, ShellLineError> {
    let mut argv = ProgramArgvBuffer::empty();
    let mut index = 0usize;
    while index < parsed.argc {
        argv.push(parsed.args[index].as_str())
            .map_err(|_| ShellLineError::TooLong)?;
        index += 1;
    }
    Ok(argv)
}

/// Parses a root-shell input line into an owned program argv buffer.
pub(crate) fn parse_program_argv(line: &[u8]) -> Result<Option<ProgramArgvBuffer>, ShellLineError> {
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

    program_argv_buffer(&parsed).map(Some)
}

fn execute_loaded_program_from_argv(
    daemon: &RootDaemon<'_>,
    session: &mut RootShellSession,
    program: LoadedProgram,
    argv: &ProgramArgvBuffer,
    stdin: &[u8],
    stdout_capture: Option<&ProgramStdoutCapture>,
    current: Option<SyscallContext>,
) -> ProgramLineResult {
    let status = run_program(daemon, session, program, argv, stdin, stdout_capture, current);
    ProgramLineResult::new(status)
}

/// Execute a scheduler-selected loaded program using its admitted argv.
pub(crate) fn execute_loaded_program_argv(
    daemon: &RootDaemon<'_>,
    session: &mut RootShellSession,
    program: LoadedProgram,
    argv: &ProgramArgvBuffer,
    stdin: &[u8],
    stdout_capture: Option<&ProgramStdoutCapture>,
    current: Option<SyscallContext>,
) -> ProgramLineResult {
    if argv.argc() == 0 {
        return ProgramLineResult::new(ProgramStatus::Empty);
    }

    execute_loaded_program_from_argv(daemon, session, program, argv, stdin, stdout_capture, current)
}

#[cfg(feature = "selftest")]
#[path = "root_shell_tests.rs"]
mod tests;
