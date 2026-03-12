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
fn test_tokenize_unclosed_double_quote_trailing_backslash() {
    // Backslash at end of unclosed double-quoted string: chars.next() returns None
    assert_eq!(tokenize_args(r#""test\"#), vec!["test"]);
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
fn test_bind_rest_after_leading_whitespace() {
    // Exercises the whitespace-skip loop in remaining_raw
    let specs = [
        ArgSpec::required("mode", ArgKind::String, "Mode"),
        ArgSpec::optional("text", ArgKind::Rest, "Remaining"),
    ];
    let result = bind_args(&specs, "  insert  hello world", false).unwrap();
    assert_eq!(result.get("mode"), Some(&ArgValue::String("insert".to_string())));
    assert_eq!(result.get("text"), Some(&ArgValue::String("hello world".to_string())));
}

#[test]
fn test_bind_rest_after_quoted_arg() {
    // Exercises the quoted-token branch in remaining_raw
    let specs = [
        ArgSpec::required("file", ArgKind::String, "File"),
        ArgSpec::optional("text", ArgKind::Rest, "Remaining"),
    ];
    let result = bind_args(&specs, r#""my file" rest of text"#, false).unwrap();
    assert_eq!(result.get("file"), Some(&ArgValue::String("my file".to_string())));
    assert_eq!(result.get("text"), Some(&ArgValue::String("rest of text".to_string())));
}

#[test]
fn test_bind_rest_after_single_quoted_arg() {
    let specs = [
        ArgSpec::required("file", ArgKind::String, "File"),
        ArgSpec::optional("text", ArgKind::Rest, "Remaining"),
    ];
    let result = bind_args(&specs, "'my file' rest of text", false).unwrap();
    assert_eq!(result.get("file"), Some(&ArgValue::String("my file".to_string())));
    assert_eq!(result.get("text"), Some(&ArgValue::String("rest of text".to_string())));
}

#[test]
fn test_bind_rest_after_escaped_arg() {
    // Exercises the backslash-escape branch in remaining_raw's unquoted scan
    let specs = [
        ArgSpec::required("file", ArgKind::String, "File"),
        ArgSpec::optional("text", ArgKind::Rest, "Remaining"),
    ];
    // Tokenizer: "my\ file" -> ["my file"], "rest" -> rest
    let result = bind_args(&specs, r"my\ file rest", false).unwrap();
    assert_eq!(result.get("file"), Some(&ArgValue::String("my file".to_string())));
    assert_eq!(result.get("text"), Some(&ArgValue::String("rest".to_string())));
}

#[test]
fn test_bind_rest_after_quoted_with_escape() {
    // Exercises the backslash-escape inside double-quoted token
    let specs = [
        ArgSpec::required("file", ArgKind::String, "File"),
        ArgSpec::optional("text", ArgKind::Rest, "Remaining"),
    ];
    let result = bind_args(&specs, r#""my \"file\"" rest"#, false).unwrap();
    assert_eq!(result.get("file"), Some(&ArgValue::String(r#"my "file""#.to_string())));
    assert_eq!(result.get("text"), Some(&ArgValue::String("rest".to_string())));
}

#[test]
fn test_bind_rest_exhausted_input() {
    // When all tokens are consumed and Rest is optional, no key is set
    let specs = [
        ArgSpec::required("a", ArgKind::String, "A"),
        ArgSpec::required("b", ArgKind::String, "B"),
        ArgSpec::optional("text", ArgKind::Rest, "Remaining"),
    ];
    let result = bind_args(&specs, "one two", false).unwrap();
    assert_eq!(result.get("a"), Some(&ArgValue::String("one".to_string())));
    assert_eq!(result.get("b"), Some(&ArgValue::String("two".to_string())));
    assert!(!result.contains_key("text"));
}

#[test]
fn test_remaining_raw_consumed_past_end() {
    // Directly test remaining_raw with consumed > actual tokens
    assert_eq!(remaining_raw("x", 2), String::new());
}

#[test]
fn test_remaining_raw_unclosed_quote() {
    // Exercises the unclosed-quote path: pos scans to end without finding
    // closing quote, and remaining is empty since whole string consumed
    assert_eq!(remaining_raw(r#""unclosed"#, 1), String::new());
}

#[test]
fn test_remaining_raw_tab_separator() {
    // Two consumed tokens separated by tab: the whitespace-skip loop at line 344
    // encounters tab on the second iteration (between first and second token)
    let specs = [
        ArgSpec::required("a", ArgKind::String, "A"),
        ArgSpec::required("b", ArgKind::String, "B"),
        ArgSpec::optional("rest", ArgKind::Rest, "Rest"),
    ];
    let result = bind_args(&specs, "first\tsecond rest", false).unwrap();
    assert_eq!(result["a"], ArgValue::String("first".to_string()));
    assert_eq!(result["b"], ArgValue::String("second".to_string()));
    assert_eq!(result["rest"], ArgValue::String("rest".to_string()));
}

#[test]
fn test_remaining_raw_single_quote_with_backslash() {
    // Single-quoted arg containing backslash: bytes[pos] == b'\\' && quote == b'"' is false
    let specs = [
        ArgSpec::required("a", ArgKind::String, "A"),
        ArgSpec::optional("rest", ArgKind::Rest, "Rest"),
    ];
    let result = bind_args(&specs, r"'foo\bar' rest", false).unwrap();
    assert_eq!(result["a"], ArgValue::String(r"foo\bar".to_string()));
    assert_eq!(result["rest"], ArgValue::String("rest".to_string()));
}

#[test]
fn test_remaining_raw_unquoted_token_before_tab() {
    // Unquoted token followed by tab: bytes[pos] != b'\t' is false (exits loop)
    let specs = [
        ArgSpec::required("a", ArgKind::String, "A"),
        ArgSpec::optional("rest", ArgKind::Rest, "Rest"),
    ];
    let result = bind_args(&specs, "first\trest", false).unwrap();
    assert_eq!(result["a"], ArgValue::String("first".to_string()));
    assert_eq!(result["rest"], ArgValue::String("rest".to_string()));
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
