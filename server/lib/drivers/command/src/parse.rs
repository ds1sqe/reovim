//! Command-line parsing for ex-commands.
//!
//! Pure functions that split a command-line string into its components
//! and bind arguments to specs. No side effects, no execution -- mechanism only.

use std::{collections::HashMap, fmt};

use reovim_driver_command_types::{ArgKind, ArgSpec, ArgValue};

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
/// Returns `None` for empty or whitespace-only input.
///
/// # Examples
///
/// ```
/// use reovim_driver_command::parse_cmdline;
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
/// assert!(parse_cmdline("").is_none());
/// ```
#[must_use]
pub fn parse_cmdline(input: &str) -> Option<ParsedCmdline> {
    let input = input.trim();
    if input.is_empty() {
        return None;
    }

    // Split at first whitespace: command part vs args part
    let (cmd_part, args_part) =
        input
            .find(|c: char| c.is_whitespace())
            .map_or((input, ""), |space_idx| {
                let cmd = &input[..space_idx];
                let args = input[space_idx..].trim();
                (cmd, args)
            });

    // Extract bang if present (e.g., "q!" -> ("q", true))
    let (name, bang) = cmd_part
        .strip_suffix('!')
        .map_or((cmd_part, false), |stripped| (stripped, true));

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
fn remaining_raw(raw_args: &str, consumed: usize) -> String {
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
mod tests {
    use super::*;

    // === parse_cmdline tests ===

    #[test]
    fn test_parse_empty() {
        assert!(parse_cmdline("").is_none());
    }

    #[test]
    fn test_parse_whitespace_only() {
        assert!(parse_cmdline("   ").is_none());
    }

    #[test]
    fn test_parse_simple_command() {
        let parsed = parse_cmdline("w").unwrap();
        assert_eq!(parsed.name, "w");
        assert!(!parsed.bang);
        assert!(parsed.args.is_empty());
        assert!(parsed.raw_args.is_empty());
    }

    #[test]
    fn test_parse_command_with_bang() {
        let parsed = parse_cmdline("q!").unwrap();
        assert_eq!(parsed.name, "q");
        assert!(parsed.bang);
        assert!(parsed.args.is_empty());
        assert!(parsed.raw_args.is_empty());
    }

    #[test]
    fn test_parse_command_with_single_arg() {
        let parsed = parse_cmdline("w filename.txt").unwrap();
        assert_eq!(parsed.name, "w");
        assert!(!parsed.bang);
        assert_eq!(parsed.args, vec!["filename.txt"]);
        assert_eq!(parsed.raw_args, "filename.txt");
    }

    #[test]
    fn test_parse_command_with_multiple_args() {
        let parsed = parse_cmdline("e file1.txt file2.txt").unwrap();
        assert_eq!(parsed.name, "e");
        assert!(!parsed.bang);
        assert_eq!(parsed.args, vec!["file1.txt", "file2.txt"]);
        assert_eq!(parsed.raw_args, "file1.txt file2.txt");
    }

    #[test]
    fn test_parse_command_with_bang_and_args() {
        let parsed = parse_cmdline("w! filename.txt").unwrap();
        assert_eq!(parsed.name, "w");
        assert!(parsed.bang);
        assert_eq!(parsed.args, vec!["filename.txt"]);
        assert_eq!(parsed.raw_args, "filename.txt");
    }

    #[test]
    fn test_parse_leading_trailing_whitespace() {
        let parsed = parse_cmdline("  write filename.txt  ").unwrap();
        assert_eq!(parsed.name, "write");
        assert!(!parsed.bang);
        assert_eq!(parsed.args, vec!["filename.txt"]);
        assert_eq!(parsed.raw_args, "filename.txt");
    }

    #[test]
    fn test_parse_multiple_spaces_between_args() {
        let parsed = parse_cmdline("e   file1    file2").unwrap();
        assert_eq!(parsed.name, "e");
        assert_eq!(parsed.args, vec!["file1", "file2"]);
        assert_eq!(parsed.raw_args, "file1    file2");
    }

    #[test]
    fn test_parse_long_command_name() {
        let parsed = parse_cmdline("colorscheme gruvbox").unwrap();
        assert_eq!(parsed.name, "colorscheme");
        assert!(!parsed.bang);
        assert_eq!(parsed.args, vec!["gruvbox"]);
        assert_eq!(parsed.raw_args, "gruvbox");
    }

    #[test]
    fn test_parsed_cmdline_clone() {
        let parsed = parse_cmdline("w! file.txt").unwrap();
        #[allow(clippy::redundant_clone)]
        let cloned = parsed.clone();
        assert_eq!(parsed, cloned);
    }

    #[test]
    fn test_parsed_cmdline_debug() {
        let parsed = parse_cmdline("q").unwrap();
        let debug_str = format!("{parsed:?}");
        assert!(debug_str.contains("ParsedCmdline"));
        assert!(debug_str.contains('q'));
    }

    #[test]
    fn test_parsed_cmdline_equality() {
        let a = parse_cmdline("w file.txt").unwrap();
        let b = parse_cmdline("w file.txt").unwrap();
        assert_eq!(a, b);
    }

    #[test]
    fn test_parsed_cmdline_inequality_name() {
        let a = parse_cmdline("w").unwrap();
        let b = parse_cmdline("q").unwrap();
        assert_ne!(a, b);
    }

    #[test]
    fn test_parsed_cmdline_inequality_bang() {
        let a = parse_cmdline("q").unwrap();
        let b = parse_cmdline("q!").unwrap();
        assert_ne!(a, b);
    }

    #[test]
    fn test_parsed_cmdline_inequality_args() {
        let a = parse_cmdline("w file1.txt").unwrap();
        let b = parse_cmdline("w file2.txt").unwrap();
        assert_ne!(a, b);
    }

    #[test]
    fn test_parse_bang_only() {
        // "!" alone -- name is empty string, bang is true
        let parsed = parse_cmdline("!").unwrap();
        assert_eq!(parsed.name, "");
        assert!(parsed.bang);
        assert!(parsed.args.is_empty());
        assert!(parsed.raw_args.is_empty());
    }

    // === tokenize_args tests ===

    #[test]
    fn test_tokenize_empty() {
        assert!(tokenize_args("").is_empty());
    }

    #[test]
    fn test_tokenize_whitespace_only() {
        assert!(tokenize_args("   ").is_empty());
    }

    #[test]
    fn test_tokenize_single_word() {
        assert_eq!(tokenize_args("hello"), vec!["hello"]);
    }

    #[test]
    fn test_tokenize_multiple_words() {
        assert_eq!(tokenize_args("foo bar baz"), vec!["foo", "bar", "baz"]);
    }

    #[test]
    fn test_tokenize_double_quotes() {
        assert_eq!(tokenize_args(r#""foo bar""#), vec!["foo bar"]);
    }

    #[test]
    fn test_tokenize_single_quotes() {
        assert_eq!(tokenize_args("'foo bar'"), vec!["foo bar"]);
    }

    #[test]
    fn test_tokenize_mixed_quotes_and_words() {
        assert_eq!(tokenize_args(r#"foo "bar baz" qux"#), vec!["foo", "bar baz", "qux"]);
    }

    #[test]
    fn test_tokenize_backslash_escape() {
        assert_eq!(tokenize_args(r"foo\ bar"), vec!["foo bar"]);
    }

    #[test]
    fn test_tokenize_escape_in_double_quotes() {
        assert_eq!(tokenize_args(r#""foo\"bar""#), vec![r#"foo"bar"#]);
    }

    #[test]
    fn test_tokenize_no_escape_in_single_quotes() {
        // Single quotes don't process backslash escapes
        assert_eq!(tokenize_args(r"'foo\bar'"), vec![r"foo\bar"]);
    }

    #[test]
    fn test_tokenize_unclosed_double_quote() {
        assert_eq!(tokenize_args(r#""foo bar"#), vec!["foo bar"]);
    }

    #[test]
    fn test_tokenize_unclosed_single_quote() {
        assert_eq!(tokenize_args("'foo bar"), vec!["foo bar"]);
    }

    #[test]
    fn test_tokenize_empty_quotes() {
        assert_eq!(tokenize_args(r#"foo "" bar"#), vec!["foo", "", "bar"]);
    }

    #[test]
    fn test_tokenize_adjacent_quoted_and_unquoted() {
        // "foo"bar becomes foobar (quoted and unquoted concatenate)
        assert_eq!(tokenize_args(r#""foo"bar"#), vec!["foobar"]);
    }

    #[test]
    fn test_tokenize_trailing_backslash() {
        // Trailing backslash with nothing to escape
        assert_eq!(tokenize_args(r"foo\"), vec!["foo"]);
    }

    #[test]
    fn test_tokenize_tabs() {
        assert_eq!(tokenize_args("foo\tbar"), vec!["foo", "bar"]);
    }

    // === ArgError tests ===

    #[test]
    fn test_arg_error_missing_required_display() {
        let err = ArgError::MissingRequired {
            name: "file",
            kind: ArgKind::FilePath,
        };
        assert_eq!(err.to_string(), "E471: Missing required argument: file (FilePath)");
    }

    #[test]
    fn test_arg_error_too_many_args_display() {
        let err = ArgError::TooManyArgs {
            expected: 1,
            got: 3,
        };
        assert_eq!(err.to_string(), "E488: Too many arguments (expected 1, got 3)");
    }

    #[test]
    fn test_arg_error_invalid_value_display() {
        let err = ArgError::InvalidValue {
            name: "count",
            kind: ArgKind::Count,
            value: "abc".to_string(),
        };
        assert_eq!(err.to_string(), r#"E474: Invalid value for count (Count): "abc""#);
    }

    #[test]
    fn test_arg_error_clone_eq() {
        let err = ArgError::MissingRequired {
            name: "file",
            kind: ArgKind::FilePath,
        };
        #[allow(clippy::redundant_clone)]
        let cloned = err.clone();
        assert_eq!(err, cloned);
    }

    #[test]
    fn test_arg_error_debug() {
        let err = ArgError::TooManyArgs {
            expected: 1,
            got: 2,
        };
        let debug = format!("{err:?}");
        assert!(debug.contains("TooManyArgs"));
    }

    // === bind_args tests ===

    #[test]
    fn test_bind_empty_specs_empty_input() {
        let result = bind_args(&[], "", false).unwrap();
        assert!(result.is_empty());
    }

    #[test]
    fn test_bind_single_filepath() {
        let specs = [ArgSpec::required("file", ArgKind::FilePath, "File")];
        let result = bind_args(&specs, "test.txt", false).unwrap();
        assert_eq!(result.get("file"), Some(&ArgValue::FilePath("test.txt".to_string())));
    }

    #[test]
    fn test_bind_optional_filepath_missing() {
        let specs = [ArgSpec::optional("file", ArgKind::FilePath, "File")];
        let result = bind_args(&specs, "", false).unwrap();
        assert!(result.is_empty());
    }

    #[test]
    fn test_bind_required_filepath_missing() {
        let specs = [ArgSpec::required("file", ArgKind::FilePath, "File")];
        let result = bind_args(&specs, "", false);
        assert_eq!(
            result,
            Err(ArgError::MissingRequired {
                name: "file",
                kind: ArgKind::FilePath,
            })
        );
    }

    #[test]
    fn test_bind_string_arg() {
        let specs = [ArgSpec::required("name", ArgKind::String, "Name")];
        let result = bind_args(&specs, "hello", false).unwrap();
        assert_eq!(result.get("name"), Some(&ArgValue::String("hello".to_string())));
    }

    #[test]
    fn test_bind_count_arg() {
        let specs = [ArgSpec::required("count", ArgKind::Count, "Count")];
        let result = bind_args(&specs, "42", false).unwrap();
        assert_eq!(result.get("count"), Some(&ArgValue::Count(42)));
    }

    #[test]
    fn test_bind_count_invalid() {
        let specs = [ArgSpec::required("count", ArgKind::Count, "Count")];
        let result = bind_args(&specs, "abc", false);
        assert_eq!(
            result,
            Err(ArgError::InvalidValue {
                name: "count",
                kind: ArgKind::Count,
                value: "abc".to_string(),
            })
        );
    }

    #[test]
    fn test_bind_bool_true() {
        let specs = [ArgSpec::required("flag", ArgKind::Bool, "Flag")];
        let result = bind_args(&specs, "true", false).unwrap();
        assert_eq!(result.get("flag"), Some(&ArgValue::Bool(true)));
    }

    #[test]
    fn test_bind_bool_false() {
        let specs = [ArgSpec::required("flag", ArgKind::Bool, "Flag")];
        let result = bind_args(&specs, "false", false).unwrap();
        assert_eq!(result.get("flag"), Some(&ArgValue::Bool(false)));
    }

    #[test]
    fn test_bind_bool_invalid() {
        let specs = [ArgSpec::required("flag", ArgKind::Bool, "Flag")];
        let result = bind_args(&specs, "yes", false);
        assert_eq!(
            result,
            Err(ArgError::InvalidValue {
                name: "flag",
                kind: ArgKind::Bool,
                value: "yes".to_string(),
            })
        );
    }

    #[test]
    fn test_bind_char_arg() {
        let specs = [ArgSpec::required("ch", ArgKind::Char, "Character")];
        let result = bind_args(&specs, "x", false).unwrap();
        assert_eq!(result.get("ch"), Some(&ArgValue::Char('x')));
    }

    #[test]
    fn test_bind_char_invalid_multi() {
        let specs = [ArgSpec::required("ch", ArgKind::Char, "Character")];
        let result = bind_args(&specs, "abc", false);
        assert_eq!(
            result,
            Err(ArgError::InvalidValue {
                name: "ch",
                kind: ArgKind::Char,
                value: "abc".to_string(),
            })
        );
    }

    #[test]
    fn test_bind_register_arg() {
        let specs = [ArgSpec::required("reg", ArgKind::Register, "Register")];
        let result = bind_args(&specs, "a", false).unwrap();
        assert_eq!(result.get("reg"), Some(&ArgValue::Register('a')));
    }

    #[test]
    fn test_bind_register_invalid() {
        let specs = [ArgSpec::required("reg", ArgKind::Register, "Register")];
        let result = bind_args(&specs, "ab", false);
        assert_eq!(
            result,
            Err(ArgError::InvalidValue {
                name: "reg",
                kind: ArgKind::Register,
                value: "ab".to_string(),
            })
        );
    }

    #[test]
    fn test_bind_bang_present() {
        let specs = [ArgSpec::optional("bang", ArgKind::Bang, "Force")];
        let result = bind_args(&specs, "", true).unwrap();
        assert_eq!(result.get("bang"), Some(&ArgValue::Bang(true)));
    }

    #[test]
    fn test_bind_bang_absent() {
        let specs = [ArgSpec::optional("bang", ArgKind::Bang, "Force")];
        let result = bind_args(&specs, "", false).unwrap();
        assert!(!result.contains_key("bang"));
    }

    #[test]
    fn test_bind_rest_consumes_all() {
        let specs = [ArgSpec::optional("text", ArgKind::Rest, "Text")];
        let result = bind_args(&specs, "hello world foo", false).unwrap();
        assert_eq!(result.get("text"), Some(&ArgValue::String("hello world foo".to_string())));
    }

    #[test]
    fn test_bind_rest_empty() {
        let specs = [ArgSpec::optional("text", ArgKind::Rest, "Text")];
        let result = bind_args(&specs, "", false).unwrap();
        assert!(!result.contains_key("text"));
    }

    #[test]
    fn test_bind_rest_required_empty() {
        let specs = [ArgSpec::required("text", ArgKind::Rest, "Text")];
        let result = bind_args(&specs, "", false);
        assert_eq!(
            result,
            Err(ArgError::MissingRequired {
                name: "text",
                kind: ArgKind::Rest,
            })
        );
    }

    #[test]
    fn test_bind_arg_then_rest() {
        let specs = [
            ArgSpec::required("mode", ArgKind::String, "Mode"),
            ArgSpec::optional("text", ArgKind::Rest, "Remaining"),
        ];
        let result = bind_args(&specs, "insert hello world", false).unwrap();
        assert_eq!(result.get("mode"), Some(&ArgValue::String("insert".to_string())));
        assert_eq!(result.get("text"), Some(&ArgValue::String("hello world".to_string())));
    }

    #[test]
    fn test_bind_too_many_args() {
        let specs = [ArgSpec::required("file", ArgKind::FilePath, "File")];
        let result = bind_args(&specs, "a.txt b.txt", false);
        assert_eq!(
            result,
            Err(ArgError::TooManyArgs {
                expected: 1,
                got: 2,
            })
        );
    }

    #[test]
    fn test_bind_bang_does_not_consume_tokens() {
        // Bang spec + filepath spec: bang comes from flag, file from tokens
        let specs = [
            ArgSpec::optional("bang", ArgKind::Bang, "Force"),
            ArgSpec::optional("file", ArgKind::FilePath, "File"),
        ];
        let result = bind_args(&specs, "test.txt", true).unwrap();
        assert_eq!(result.get("bang"), Some(&ArgValue::Bang(true)));
        assert_eq!(result.get("file"), Some(&ArgValue::FilePath("test.txt".to_string())));
    }

    #[test]
    fn test_bind_multiple_positional_args() {
        let specs = [
            ArgSpec::required("src", ArgKind::FilePath, "Source"),
            ArgSpec::required("dst", ArgKind::FilePath, "Dest"),
        ];
        let result = bind_args(&specs, "a.txt b.txt", false).unwrap();
        assert_eq!(result.get("src"), Some(&ArgValue::FilePath("a.txt".to_string())));
        assert_eq!(result.get("dst"), Some(&ArgValue::FilePath("b.txt".to_string())));
    }

    #[test]
    fn test_bind_quoted_filepath() {
        let specs = [ArgSpec::required("file", ArgKind::FilePath, "File")];
        let result = bind_args(&specs, r#""my file.txt""#, false).unwrap();
        assert_eq!(result.get("file"), Some(&ArgValue::FilePath("my file.txt".to_string())));
    }

    #[test]
    fn test_bind_no_specs_with_args_errors() {
        let result = bind_args(&[], "extra", false);
        assert_eq!(
            result,
            Err(ArgError::TooManyArgs {
                expected: 0,
                got: 1,
            })
        );
    }

    #[test]
    fn test_bind_motion_kind_errors() {
        // Motion is not parseable from text input
        let specs = [ArgSpec::required("m", ArgKind::Motion, "Motion")];
        let result = bind_args(&specs, "w", false);
        assert!(result.is_err());
    }

    #[test]
    fn test_bind_range_kind_errors() {
        let specs = [ArgSpec::required("r", ArgKind::Range, "Range")];
        let result = bind_args(&specs, "1,5", false);
        assert!(result.is_err());
    }

    #[test]
    fn test_bind_buffer_id_kind_errors() {
        let specs = [ArgSpec::required("b", ArgKind::BufferId, "Buffer")];
        let result = bind_args(&specs, "1", false);
        assert!(result.is_err());
    }
}
