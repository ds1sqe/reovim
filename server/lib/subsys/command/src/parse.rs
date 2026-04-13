//! Command-line parsing for ex-commands.
//!
//! Pure functions that split a command-line string into its components
//! and bind arguments to specs. No side effects, no execution -- mechanism only.

use std::{collections::HashMap, fmt};

use reovim_subsys_command_types::{ArgKind, ArgSpec, ArgValue};

/// Parsed command-line input.
///
/// Result of parsing a string like `"write! filename.txt"` into
/// structured components: name (`"write"`), bang (`true`),
/// args (`["filename.txt"]`), `raw_args` (`"filename.txt"`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedCmdline {
    /// The command name (e.g., `"write"`, `"q"`).
    pub name: String,
    /// Whether the command was invoked with `!` (e.g., `:q!`).
    pub bang: bool,
    /// Positional arguments after the command name (whitespace-split).
    pub args: Vec<String>,
    /// Raw argument text after the command name (preserves quoting and spacing).
    pub raw_args: String,
}

/// Parse a command-line string into name, bang, and args.
///
/// Grammar: `[name][!] [arg1 arg2 ...]`
///
/// Command names start alphabetic and may continue with ASCII alphanumeric
/// characters or `-`. Arguments start at the first character that is not a
/// valid command-name character (or at whitespace). This preserves vim-style
/// commands like `:s/pat/rep/` where there is no space between the command
/// name and arguments while also supporting kebab-case ex-commands such as
/// `:write-quit`.
///
/// Returns `None` for empty or whitespace-only input.
///
/// # Examples
///
/// ```
/// use reovim_subsys_command::parse_cmdline;
///
/// let parsed = parse_cmdline("w filename.txt").unwrap();
/// assert_eq!(parsed.name, "w");
/// assert!(!parsed.bang);
/// assert_eq!(parsed.args, vec!["filename.txt"]);
/// assert_eq!(parsed.raw_args, "filename.txt");
///
/// let parsed = parse_cmdline("q!").unwrap();
/// assert_eq!(parsed.name, "q");
/// assert!(parsed.bang);
/// assert!(parsed.args.is_empty());
/// assert!(parsed.raw_args.is_empty());
///
/// let parsed = parse_cmdline("s/foo/bar/g").unwrap();
/// assert_eq!(parsed.name, "s");
/// assert_eq!(parsed.raw_args, "/foo/bar/g");
///
/// assert!(parse_cmdline("").is_none());
/// ```
#[must_use]
pub fn parse_cmdline(input: &str) -> Option<ParsedCmdline> {
    let input = input.trim();
    if input.is_empty() {
        return None;
    }

    // Find where the command name ends. Ex-command names start alphabetic and
    // may continue with ASCII alphanumeric characters or '-'. This keeps
    // `:s/pat/rep/` parsing as name="s", args="/pat/rep/" while allowing
    // kebab-case commands such as `:write-quit`.
    let name_end = input
        .find(|c: char| !c.is_ascii_alphanumeric() && c != '-')
        .unwrap_or(input.len());

    let cmd_part = &input[..name_end];
    let rest = &input[name_end..];

    // Extract bang if the rest starts with '!'
    let (bang, args_part) = rest
        .strip_prefix('!')
        .map_or_else(|| (false, rest.trim_start()), |after| (true, after.trim_start()));

    let name = cmd_part;

    let args = if args_part.is_empty() {
        vec![]
    } else {
        args_part.split_whitespace().map(String::from).collect()
    };

    Some(ParsedCmdline {
        name: name.to_string(),
        bang,
        args,
        raw_args: args_part.to_string(),
    })
}

/// Tokenize argument text with quote and escape awareness.
///
/// Handles:
/// - Double quotes: `"foo bar"` -> single token `foo bar`
/// - Single quotes: `'foo bar'` -> single token `foo bar`
/// - Backslash escapes: `foo\ bar` -> single token `foo bar`
/// - Mixed: `foo "bar baz" qux` -> `["foo", "bar baz", "qux"]`
///
/// Unclosed quotes are treated as extending to end of input.
#[must_use]
pub fn tokenize_args(input: &str) -> Vec<String> {
    let mut tokens = Vec::new();
    let mut current = String::new();
    let mut chars = input.chars().peekable();
    let mut has_content = false; // tracks if we've seen quotes or chars for this token

    while let Some(&ch) = chars.peek() {
        match ch {
            ' ' | '\t' => {
                if has_content {
                    tokens.push(std::mem::take(&mut current));
                    has_content = false;
                }
                chars.next();
            }
            '"' | '\'' => {
                has_content = true;
                let quote = ch;
                chars.next(); // consume opening quote
                loop {
                    match chars.next() {
                        Some(c) if c == quote => break,
                        Some('\\') if quote == '"' => {
                            // Inside double quotes, backslash escapes next char
                            if let Some(escaped) = chars.next() {
                                current.push(escaped);
                            }
                        }
                        Some(c) => current.push(c),
                        None => break, // unclosed quote
                    }
                }
            }
            '\\' => {
                has_content = true;
                chars.next(); // consume backslash
                if let Some(escaped) = chars.next() {
                    current.push(escaped);
                }
            }
            _ => {
                has_content = true;
                current.push(ch);
                chars.next();
            }
        }
    }

    if has_content {
        tokens.push(current);
    }

    tokens
}

/// Error type for argument binding failures.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ArgError {
    /// A required argument is missing.
    MissingRequired {
        /// The argument name.
        name: &'static str,
        /// The expected argument kind.
        kind: ArgKind,
    },
    /// Too many arguments provided.
    TooManyArgs {
        /// Expected argument count.
        expected: usize,
        /// Actual argument count.
        got: usize,
    },
    /// Argument value could not be parsed as expected type.
    InvalidValue {
        /// The argument name.
        name: &'static str,
        /// The expected argument kind.
        kind: ArgKind,
        /// The actual value that failed to parse.
        value: String,
    },
}

impl fmt::Display for ArgError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingRequired { name, kind } => {
                write!(f, "E471: Missing required argument: {name} ({kind:?})")
            }
            Self::TooManyArgs { expected, got } => {
                write!(f, "E488: Too many arguments (expected {expected}, got {got})")
            }
            Self::InvalidValue { name, kind, value } => {
                write!(f, "E474: Invalid value for {name} ({kind:?}): \"{value}\"")
            }
        }
    }
}

/// Bind arguments to specs, producing a map of name -> value.
///
/// Tokenizes `raw_args` and matches tokens to specs in order.
/// Bang is handled separately via `parsed.bang` and not consumed from tokens.
///
/// # Errors
///
/// Returns `ArgError` if a required argument is missing, too many arguments
/// are provided, or a value cannot be parsed as the expected type.
pub fn bind_args(
    specs: &[ArgSpec],
    raw_args: &str,
    bang: bool,
) -> Result<HashMap<String, ArgValue>, ArgError> {
    let tokens = tokenize_args(raw_args);
    let mut result = HashMap::new();
    let mut token_idx = 0;
    // Track how many positional (non-bang) specs there are
    let positional_count = specs.iter().filter(|s| s.kind != ArgKind::Bang).count();

    // Track whether the last spec was Rest (which consumes everything)
    let mut consumed_rest = false;

    for spec in specs {
        match spec.kind {
            ArgKind::Bang => {
                // Bang is from the parsed command, not from tokens
                if bang {
                    result.insert(spec.name.to_string(), ArgValue::Bang(true));
                }
            }
            ArgKind::Rest => {
                // Rest consumes all remaining raw text after already-consumed tokens
                let remaining = remaining_raw(raw_args, token_idx);
                if remaining.is_empty() {
                    if spec.required {
                        return Err(ArgError::MissingRequired {
                            name: spec.name,
                            kind: spec.kind,
                        });
                    }
                } else {
                    result.insert(spec.name.to_string(), ArgValue::String(remaining));
                    token_idx = tokens.len(); // consume all
                }
                consumed_rest = true;
            }
            _ => {
                if token_idx >= tokens.len() {
                    if spec.required {
                        return Err(ArgError::MissingRequired {
                            name: spec.name,
                            kind: spec.kind,
                        });
                    }
                    continue;
                }
                let token = &tokens[token_idx];
                token_idx += 1;
                let value = parse_token(spec.name, spec.kind, token)?;
                result.insert(spec.name.to_string(), value);
            }
        }
    }

    // Check for leftover tokens (unless last spec was Rest)
    if !consumed_rest && token_idx < tokens.len() {
        return Err(ArgError::TooManyArgs {
            expected: positional_count,
            got: positional_count + (tokens.len() - token_idx),
        });
    }

    Ok(result)
}

/// Parse a single token into an `ArgValue` based on the expected kind.
fn parse_token(name: &'static str, kind: ArgKind, token: &str) -> Result<ArgValue, ArgError> {
    match kind {
        ArgKind::FilePath => Ok(ArgValue::FilePath(token.to_string())),
        ArgKind::String => Ok(ArgValue::String(token.to_string())),
        ArgKind::Count => {
            token
                .parse::<usize>()
                .map(ArgValue::Count)
                .map_err(|_| ArgError::InvalidValue {
                    name,
                    kind,
                    value: token.to_string(),
                })
        }
        ArgKind::Bool => match token {
            "true" => Ok(ArgValue::Bool(true)),
            "false" => Ok(ArgValue::Bool(false)),
            _ => Err(ArgError::InvalidValue {
                name,
                kind,
                value: token.to_string(),
            }),
        },
        ArgKind::Char => {
            let mut chars = token.chars();
            match (chars.next(), chars.next()) {
                (Some(c), None) => Ok(ArgValue::Char(c)),
                _ => Err(ArgError::InvalidValue {
                    name,
                    kind,
                    value: token.to_string(),
                }),
            }
        }
        ArgKind::Register => {
            let mut chars = token.chars();
            match (chars.next(), chars.next()) {
                (Some(c), None) => Ok(ArgValue::Register(c)),
                _ => Err(ArgError::InvalidValue {
                    name,
                    kind,
                    value: token.to_string(),
                }),
            }
        }
        // These kinds are not expected from ex-command text input
        ArgKind::Bang | ArgKind::Rest | ArgKind::Motion | ArgKind::Range | ArgKind::BufferId => {
            Err(ArgError::InvalidValue {
                name,
                kind,
                value: token.to_string(),
            })
        }
    }
}

/// Compute the remaining raw text after consuming `consumed` tokens.
///
/// Finds where the consumed tokens end in `raw_args` and returns the rest,
/// trimmed of leading whitespace.
pub fn remaining_raw(raw_args: &str, consumed: usize) -> String {
    if consumed == 0 {
        return raw_args.trim().to_string();
    }

    // Re-scan raw_args to find where token N starts
    let mut pos = 0;
    let bytes = raw_args.as_bytes();
    for _ in 0..consumed {
        // Skip whitespace
        while pos < bytes.len() && (bytes[pos] == b' ' || bytes[pos] == b'\t') {
            pos += 1;
        }
        if pos >= bytes.len() {
            return String::new();
        }
        // Skip token (respect quotes)
        match bytes[pos] {
            b'"' | b'\'' => {
                let quote = bytes[pos];
                pos += 1;
                while pos < bytes.len() && bytes[pos] != quote {
                    if bytes[pos] == b'\\' && quote == b'"' {
                        pos += 1; // skip escaped char
                    }
                    pos += 1;
                }
                if pos < bytes.len() {
                    pos += 1; // skip closing quote
                }
            }
            _ => {
                while pos < bytes.len() && bytes[pos] != b' ' && bytes[pos] != b'\t' {
                    if bytes[pos] == b'\\' {
                        pos += 1; // skip escaped char
                    }
                    pos += 1;
                }
            }
        }
    }

    raw_args[pos..].trim().to_string()
}

#[cfg(test)]
#[path = "parse_tests.rs"]
mod tests;
