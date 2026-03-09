//! Command-line parsing for ex-commands.
//!
//! Pure function that splits a command-line string into its components.
//! No side effects, no execution -- mechanism only.

/// Parsed command-line input.
///
/// Result of parsing a string like `"write! filename.txt"` into
/// structured components: name (`"write"`), bang (`true`),
/// args (`["filename.txt"]`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedCmdline {
    /// The command name (e.g., `"write"`, `"q"`).
    pub name: String,
    /// Whether the command was invoked with `!` (e.g., `:q!`).
    pub bang: bool,
    /// Positional arguments after the command name.
    pub args: Vec<String>,
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
///
/// let parsed = parse_cmdline("q!").unwrap();
/// assert_eq!(parsed.name, "q");
/// assert!(parsed.bang);
/// assert!(parsed.args.is_empty());
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
    })
}

#[cfg(test)]
mod tests {
    use super::*;

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
    }

    #[test]
    fn test_parse_command_with_bang() {
        let parsed = parse_cmdline("q!").unwrap();
        assert_eq!(parsed.name, "q");
        assert!(parsed.bang);
        assert!(parsed.args.is_empty());
    }

    #[test]
    fn test_parse_command_with_single_arg() {
        let parsed = parse_cmdline("w filename.txt").unwrap();
        assert_eq!(parsed.name, "w");
        assert!(!parsed.bang);
        assert_eq!(parsed.args, vec!["filename.txt"]);
    }

    #[test]
    fn test_parse_command_with_multiple_args() {
        let parsed = parse_cmdline("e file1.txt file2.txt").unwrap();
        assert_eq!(parsed.name, "e");
        assert!(!parsed.bang);
        assert_eq!(parsed.args, vec!["file1.txt", "file2.txt"]);
    }

    #[test]
    fn test_parse_command_with_bang_and_args() {
        let parsed = parse_cmdline("w! filename.txt").unwrap();
        assert_eq!(parsed.name, "w");
        assert!(parsed.bang);
        assert_eq!(parsed.args, vec!["filename.txt"]);
    }

    #[test]
    fn test_parse_leading_trailing_whitespace() {
        let parsed = parse_cmdline("  write filename.txt  ").unwrap();
        assert_eq!(parsed.name, "write");
        assert!(!parsed.bang);
        assert_eq!(parsed.args, vec!["filename.txt"]);
    }

    #[test]
    fn test_parse_multiple_spaces_between_args() {
        let parsed = parse_cmdline("e   file1    file2").unwrap();
        assert_eq!(parsed.name, "e");
        assert_eq!(parsed.args, vec!["file1", "file2"]);
    }

    #[test]
    fn test_parse_long_command_name() {
        let parsed = parse_cmdline("colorscheme gruvbox").unwrap();
        assert_eq!(parsed.name, "colorscheme");
        assert!(!parsed.bang);
        assert_eq!(parsed.args, vec!["gruvbox"]);
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
    }
}
