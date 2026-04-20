#![allow(clippy::literal_string_with_formatting_args)]

use super::*;

#[cfg_attr(coverage_nightly, coverage(off))]
fn as_placeholder(elem: &SnippetElement) -> (TabStopId, &Vec<SnippetElement>) {
    match elem {
        SnippetElement::Placeholder { id, body } => (*id, body),
        other => panic!("expected Placeholder, got {other:?}"),
    }
}

// =========================================================================
// ParseError
// =========================================================================

#[test]
fn test_parse_error_display_eof() {
    let err = ParseError::UnexpectedEof {
        context: "placeholder",
    };
    assert_eq!(err.to_string(), "unexpected end of input while parsing placeholder");
}

#[test]
fn test_parse_error_display_invalid_id() {
    let err = ParseError::InvalidTabStopId {
        found: "abc".to_string(),
    };
    assert_eq!(err.to_string(), "invalid tab stop id: \"abc\"");
}

#[test]
fn test_parse_error_clone() {
    let err = ParseError::UnexpectedEof { context: "test" };
    let cloned = err.clone();
    assert_eq!(err, cloned);
}

#[test]
fn test_parse_error_debug() {
    let err = ParseError::InvalidTabStopId {
        found: "x".to_string(),
    };
    let debug = format!("{err:?}");
    assert!(debug.contains("InvalidTabStopId"));
}

// =========================================================================
// Empty / plain text
// =========================================================================

#[test]
fn test_parse_empty() {
    let body = parse("").unwrap();
    assert!(body.is_empty());
}

#[test]
fn test_parse_plain_text() {
    let body = parse("hello world").unwrap();
    assert_eq!(body.elements(), &[SnippetElement::Text("hello world".to_string())]);
}

// =========================================================================
// Simple tab stops: $N
// =========================================================================

#[test]
fn test_parse_dollar_zero() {
    let body = parse("$0").unwrap();
    assert_eq!(
        body.elements(),
        &[SnippetElement::TabStop {
            id: 0,
            transform: None,
        }]
    );
}

#[test]
fn test_parse_dollar_1() {
    let body = parse("$1").unwrap();
    assert_eq!(
        body.elements(),
        &[SnippetElement::TabStop {
            id: 1,
            transform: None,
        }]
    );
}

#[test]
fn test_parse_dollar_9() {
    let body = parse("$9").unwrap();
    assert_eq!(
        body.elements(),
        &[SnippetElement::TabStop {
            id: 9,
            transform: None,
        }]
    );
}

#[test]
fn test_parse_text_with_tabstop() {
    let body = parse("fn $1()").unwrap();
    assert_eq!(body.len(), 3);
    assert_eq!(body.elements()[0], SnippetElement::Text("fn ".to_string()));
    assert_eq!(
        body.elements()[1],
        SnippetElement::TabStop {
            id: 1,
            transform: None,
        }
    );
    assert_eq!(body.elements()[2], SnippetElement::Text("()".to_string()));
}

#[test]
fn test_parse_multiple_tabstops() {
    let body = parse("$1 $2 $0").unwrap();
    assert_eq!(body.len(), 5);
}

// =========================================================================
// Braced tab stops: ${N}
// =========================================================================

#[test]
fn test_parse_braced_tabstop() {
    let body = parse("${1}").unwrap();
    assert_eq!(
        body.elements(),
        &[SnippetElement::TabStop {
            id: 1,
            transform: None,
        }]
    );
}

#[test]
fn test_parse_multi_digit_braced() {
    let body = parse("${12}").unwrap();
    assert_eq!(
        body.elements(),
        &[SnippetElement::TabStop {
            id: 12,
            transform: None,
        }]
    );
}

#[test]
fn test_parse_braced_zero() {
    let body = parse("${0}").unwrap();
    assert_eq!(
        body.elements(),
        &[SnippetElement::TabStop {
            id: 0,
            transform: None,
        }]
    );
}

// =========================================================================
// Placeholders: ${N:default}
// =========================================================================

#[test]
fn test_parse_placeholder_simple() {
    let body = parse("${1:name}").unwrap();
    assert_eq!(
        body.elements(),
        &[SnippetElement::Placeholder {
            id: 1,
            body: vec![SnippetElement::Text("name".to_string())],
        }]
    );
}

#[test]
fn test_parse_placeholder_empty_default() {
    let body = parse("${1:}").unwrap();
    assert_eq!(
        body.elements(),
        &[SnippetElement::Placeholder {
            id: 1,
            body: vec![],
        }]
    );
}

#[test]
fn test_parse_placeholder_with_tabstop() {
    let body = parse("${1:hello $2 world}").unwrap();
    let (id, inner) = as_placeholder(&body.elements()[0]);
    assert_eq!(id, 1);
    assert_eq!(inner.len(), 3);
    assert_eq!(inner[0], SnippetElement::Text("hello ".to_string()));
    assert_eq!(
        inner[1],
        SnippetElement::TabStop {
            id: 2,
            transform: None,
        }
    );
    assert_eq!(inner[2], SnippetElement::Text(" world".to_string()));
}

// =========================================================================
// Nested placeholders: ${1:outer ${2:inner}}
// =========================================================================

#[test]
fn test_parse_nested_placeholder() {
    let body = parse("${1:outer ${2:inner}}").unwrap();
    let (id, inner) = as_placeholder(&body.elements()[0]);
    assert_eq!(id, 1);
    assert_eq!(inner.len(), 2);
    assert_eq!(inner[0], SnippetElement::Text("outer ".to_string()));
    let (inner_id, inner_body) = as_placeholder(&inner[1]);
    assert_eq!(inner_id, 2);
    assert_eq!(inner_body, &[SnippetElement::Text("inner".to_string())]);
}

#[test]
fn test_parse_deeply_nested() {
    let body = parse("${1:${2:${3:deep}}}").unwrap();
    let (_, depth1) = as_placeholder(&body.elements()[0]);
    let (_, depth2) = as_placeholder(&depth1[0]);
    let (id, depth3) = as_placeholder(&depth2[0]);
    assert_eq!(id, 3);
    assert_eq!(depth3, &[SnippetElement::Text("deep".to_string())]);
}

// =========================================================================
// Escape sequences
// =========================================================================

#[test]
fn test_parse_escape_dollar() {
    let body = parse("cost: \\$10").unwrap();
    assert_eq!(body.elements(), &[SnippetElement::Text("cost: $10".to_string())]);
}

#[test]
fn test_parse_escape_brace() {
    let body = parse("\\}").unwrap();
    assert_eq!(body.elements(), &[SnippetElement::Text("}".to_string())]);
}

#[test]
fn test_parse_escape_backslash() {
    let body = parse("\\\\").unwrap();
    assert_eq!(body.elements(), &[SnippetElement::Text("\\".to_string())]);
}

#[test]
fn test_parse_unknown_escape() {
    let body = parse("\\n").unwrap();
    assert_eq!(body.elements(), &[SnippetElement::Text("\\n".to_string())]);
}

#[test]
fn test_parse_trailing_backslash() {
    let body = parse("end\\").unwrap();
    assert_eq!(body.elements(), &[SnippetElement::Text("end\\".to_string())]);
}

#[test]
fn test_parse_escape_in_placeholder() {
    let body = parse("${1:price \\$5}").unwrap();
    let (_, inner) = as_placeholder(&body.elements()[0]);
    assert_eq!(inner, &[SnippetElement::Text("price $5".to_string())]);
}

// =========================================================================
// Graceful fallback
// =========================================================================

#[test]
fn test_parse_lone_dollar_at_end() {
    let body = parse("end$").unwrap();
    assert_eq!(body.elements(), &[SnippetElement::Text("end$".to_string())]);
}

#[test]
fn test_parse_dollar_followed_by_letter() {
    // $x is a simple variable reference
    let body = parse("$x").unwrap();
    assert_eq!(
        body.elements(),
        &[SnippetElement::Variable {
            name: "x".to_string(),
            default: None,
            transform: None,
        }]
    );
}

#[test]
fn test_parse_braced_variable() {
    // ${foo} is a variable reference
    let body = parse("${foo}").unwrap();
    assert_eq!(
        body.elements(),
        &[SnippetElement::Variable {
            name: "foo".to_string(),
            default: None,
            transform: None,
        }]
    );
}

// =========================================================================
// Complex / realistic snippets
// =========================================================================

#[test]
fn test_parse_function_snippet() {
    let body = parse("fn ${1:name}(${2:params}) -> ${3:Type} {\n\t$0\n}").unwrap();
    // "fn ", placeholder1, "(", placeholder2, ") -> ", placeholder3,
    // " {\n\t", tabstop0, "\n}"
    assert_eq!(body.len(), 9);
    assert_eq!(body.elements()[0], SnippetElement::Text("fn ".to_string()));
    assert!(matches!(body.elements()[1], SnippetElement::Placeholder { id: 1, .. }));
    assert_eq!(body.elements()[2], SnippetElement::Text("(".to_string()));
    assert!(matches!(body.elements()[3], SnippetElement::Placeholder { id: 2, .. }));
    assert_eq!(body.elements()[4], SnippetElement::Text(") -> ".to_string()));
    assert!(matches!(body.elements()[5], SnippetElement::Placeholder { id: 3, .. }));
    assert_eq!(body.elements()[6], SnippetElement::Text(" {\n\t".to_string()));
    // Wait - $0 and \n} should be separate elements
}

#[test]
fn test_parse_function_snippet_elements_count() {
    let body = parse("fn ${1:name}(${2:params}) -> ${3:Type} {\n\t$0\n}").unwrap();
    // Elements: "fn ", placeholder1, "(", placeholder2, ") -> ", placeholder3,
    //           " {\n\t", tabstop0, "\n}"
    assert_eq!(body.len(), 9);
}

#[test]
fn test_parse_for_loop() {
    let body = parse("for ${1:i} in ${2:iter} {\n\t$0\n}").unwrap();
    assert_eq!(body.len(), 7);
}

#[test]
fn test_parse_if_else() {
    let body = parse("if ${1:cond} {\n\t${2:body}\n} else {\n\t${3:alt}\n}").unwrap();
    assert_eq!(body.len(), 7);
}

// =========================================================================
// Error cases
// =========================================================================

#[test]
fn test_parse_unclosed_placeholder() {
    let result = parse("${1:hello");
    assert!(result.is_err());
    assert_eq!(
        result.unwrap_err(),
        ParseError::UnexpectedEof {
            context: "placeholder",
        }
    );
}

#[test]
fn test_parse_unclosed_braced_no_colon() {
    // ${1 without closing brace: falls through to fallback
    let body = parse("${1").unwrap();
    assert_eq!(body.elements(), &[SnippetElement::Text("${1".to_string())]);
}

// =========================================================================
// Adjacent text merging
// =========================================================================

#[test]
fn test_merge_adjacent_text() {
    let elements = vec![
        SnippetElement::Text("a".to_string()),
        SnippetElement::Text("b".to_string()),
        SnippetElement::TabStop {
            id: 1,
            transform: None,
        },
        SnippetElement::Text("c".to_string()),
        SnippetElement::Text("d".to_string()),
    ];
    let merged = merge_adjacent_text(elements);
    assert_eq!(merged.len(), 3);
    assert_eq!(merged[0], SnippetElement::Text("ab".to_string()));
    assert_eq!(
        merged[1],
        SnippetElement::TabStop {
            id: 1,
            transform: None,
        }
    );
    assert_eq!(merged[2], SnippetElement::Text("cd".to_string()));
}

#[test]
fn test_merge_no_adjacent_text() {
    let elements = vec![
        SnippetElement::TabStop {
            id: 1,
            transform: None,
        },
        SnippetElement::TabStop {
            id: 2,
            transform: None,
        },
    ];
    let merged = merge_adjacent_text(elements);
    assert_eq!(merged.len(), 2);
}

#[test]
fn test_merge_empty() {
    let merged = merge_adjacent_text(vec![]);
    assert!(merged.is_empty());
}

#[test]
fn test_merge_single_text() {
    let elements = vec![SnippetElement::Text("only".to_string())];
    let merged = merge_adjacent_text(elements);
    assert_eq!(merged.len(), 1);
}

// =========================================================================
// Edge cases
// =========================================================================

#[test]
fn test_parse_only_tabstops() {
    let body = parse("$1$2$0").unwrap();
    assert_eq!(body.len(), 3);
}

#[test]
fn test_parse_consecutive_braced() {
    let body = parse("${1}${2}").unwrap();
    assert_eq!(body.len(), 2);
}

#[test]
fn test_parse_placeholder_followed_by_tabstop() {
    let body = parse("${1:hello}$0").unwrap();
    assert_eq!(body.len(), 2);
    assert!(matches!(body.elements()[0], SnippetElement::Placeholder { .. }));
    assert!(matches!(body.elements()[1], SnippetElement::TabStop { id: 0, .. }));
}

#[test]
fn test_parse_newlines_and_tabs() {
    let body = parse("line1\n\tline2\n\t\tline3").unwrap();
    assert_eq!(
        body.elements(),
        &[SnippetElement::Text(
            "line1\n\tline2\n\t\tline3".to_string()
        )]
    );
}

#[test]
fn test_parse_escaped_in_text_context() {
    let body = parse("a\\$b\\}c\\\\d").unwrap();
    assert_eq!(body.elements(), &[SnippetElement::Text("a$b}c\\d".to_string())]);
}

// =========================================================================
// Braced fallback: unknown syntax after number
// =========================================================================

#[test]
fn test_parse_choice_syntax() {
    let body = parse("${1|one,two|}").unwrap();
    assert_eq!(
        body.elements(),
        &[SnippetElement::Choice {
            id: 1,
            choices: vec!["one".to_string(), "two".to_string()],
        }]
    );
}

#[test]
fn test_parse_braced_unknown_char_after_number() {
    // ${1x} — 'x' after number triggers Some(_) in parse_braced
    let body = parse("${1x}").unwrap();
    assert_eq!(body.elements(), &[SnippetElement::Text("${1x}".to_string())]);
}

#[test]
fn test_parse_braced_variable_no_closing_brace() {
    // ${foo: without closing brace → error
    let result = parse("${foo:");
    assert!(result.is_err());
}

#[test]
fn test_parse_variable_transform() {
    let body = parse("${TM_FILENAME/(.*)\\..+/$1/}").unwrap();
    assert_eq!(body.len(), 1);
    match &body.elements()[0] {
        SnippetElement::Variable {
            name,
            default,
            transform,
        } => {
            assert_eq!(name, "TM_FILENAME");
            assert!(default.is_none());
            let t = transform.as_ref().unwrap();
            assert_eq!(t.regex, "(.*)\\..+");
            assert_eq!(t.replacement.len(), 1);
            assert_eq!(t.replacement[0], FormatItem::Capture(1));
            assert!(t.options.is_empty());
        }
        other => panic!("expected Variable, got {other:?}"),
    }
}

// =========================================================================
// Variable parsing
// =========================================================================

#[test]
fn test_parse_simple_variable() {
    let body = parse("$TM_FILENAME").unwrap();
    assert_eq!(
        body.elements(),
        &[SnippetElement::Variable {
            name: "TM_FILENAME".to_string(),
            default: None,
            transform: None,
        }]
    );
}

#[test]
fn test_parse_variable_with_underscore() {
    let body = parse("$CURRENT_YEAR").unwrap();
    assert_eq!(
        body.elements(),
        &[SnippetElement::Variable {
            name: "CURRENT_YEAR".to_string(),
            default: None,
            transform: None,
        }]
    );
}

#[test]
fn test_parse_braced_variable_simple() {
    let body = parse("${TM_FILENAME}").unwrap();
    assert_eq!(
        body.elements(),
        &[SnippetElement::Variable {
            name: "TM_FILENAME".to_string(),
            default: None,
            transform: None,
        }]
    );
}

#[test]
fn test_parse_variable_with_default() {
    let body = parse("${TM_FILENAME:untitled}").unwrap();
    assert_eq!(
        body.elements(),
        &[SnippetElement::Variable {
            name: "TM_FILENAME".to_string(),
            default: Some(vec![SnippetElement::Text("untitled".to_string())]),
            transform: None,
        }]
    );
}

#[test]
fn test_parse_variable_with_empty_default() {
    let body = parse("${VAR:}").unwrap();
    assert_eq!(
        body.elements(),
        &[SnippetElement::Variable {
            name: "VAR".to_string(),
            default: None,
            transform: None,
        }]
    );
}

#[test]
fn test_parse_variable_default_with_tabstop() {
    let body = parse("${VAR:default $1 text}").unwrap();
    let elems = body.elements();
    assert_eq!(elems.len(), 1);
    match &elems[0] {
        SnippetElement::Variable { name, default, .. } => {
            assert_eq!(name, "VAR");
            let inner = default.as_ref().unwrap();
            assert_eq!(inner.len(), 3);
            assert_eq!(inner[0], SnippetElement::Text("default ".to_string()));
            assert!(matches!(inner[1], SnippetElement::TabStop { id: 1, .. }));
            assert_eq!(inner[2], SnippetElement::Text(" text".to_string()));
        }
        other => panic!("expected Variable, got {other:?}"),
    }
}

#[test]
fn test_parse_variable_in_text() {
    let body = parse("Hello $NAME!").unwrap();
    assert_eq!(body.len(), 3);
    assert_eq!(body.elements()[0], SnippetElement::Text("Hello ".to_string()));
    assert_eq!(
        body.elements()[1],
        SnippetElement::Variable {
            name: "NAME".to_string(),
            default: None,
            transform: None,
        }
    );
    assert_eq!(body.elements()[2], SnippetElement::Text("!".to_string()));
}

#[test]
fn test_parse_variable_followed_by_digits() {
    // $ABC123 — reads the entire identifier including digits
    let body = parse("$ABC123").unwrap();
    assert_eq!(
        body.elements(),
        &[SnippetElement::Variable {
            name: "ABC123".to_string(),
            default: None,
            transform: None,
        }]
    );
}

#[test]
fn test_parse_underscore_variable() {
    let body = parse("$_private").unwrap();
    assert_eq!(
        body.elements(),
        &[SnippetElement::Variable {
            name: "_private".to_string(),
            default: None,
            transform: None,
        }]
    );
}

#[test]
fn test_parse_mixed_tabstops_and_variables() {
    let body = parse("$TM_FILENAME:$1:${CURRENT_YEAR}").unwrap();
    assert_eq!(body.len(), 5);
    assert!(matches!(body.elements()[0], SnippetElement::Variable { .. }));
    assert_eq!(body.elements()[1], SnippetElement::Text(":".to_string()));
    assert!(matches!(body.elements()[2], SnippetElement::TabStop { id: 1, .. }));
    assert_eq!(body.elements()[3], SnippetElement::Text(":".to_string()));
    assert!(matches!(body.elements()[4], SnippetElement::Variable { .. }));
}

#[test]
fn test_is_var_start() {
    assert!(is_var_start(b'a'));
    assert!(is_var_start(b'Z'));
    assert!(is_var_start(b'_'));
    assert!(!is_var_start(b'0'));
    assert!(!is_var_start(b' '));
    assert!(!is_var_start(b'$'));
}

// =========================================================================
// Transform syntax: ${N/regex/replacement/options}
// =========================================================================

#[test]
fn test_parse_transform_simple() {
    let body = parse("${1/foo/bar/}").unwrap();
    assert_eq!(body.len(), 1);
    match &body.elements()[0] {
        SnippetElement::TabStop { id, transform } => {
            assert_eq!(*id, 1);
            let t = transform.as_ref().unwrap();
            assert_eq!(t.regex, "foo");
            assert_eq!(t.replacement, vec![FormatItem::Text("bar".to_string())]);
            assert!(t.options.is_empty());
        }
        other => panic!("expected TabStop, got {other:?}"),
    }
}

#[test]
fn test_parse_transform_with_options() {
    let body = parse("${1/\\w+/X/g}").unwrap();
    match &body.elements()[0] {
        SnippetElement::TabStop { transform, .. } => {
            let t = transform.as_ref().unwrap();
            assert_eq!(t.regex, "\\w+");
            assert_eq!(t.options, "g");
        }
        other => panic!("expected TabStop, got {other:?}"),
    }
}

#[test]
fn test_parse_transform_capture_reference() {
    let body = parse("${1/(\\w+)/$1/}").unwrap();
    match &body.elements()[0] {
        SnippetElement::TabStop { transform, .. } => {
            let t = transform.as_ref().unwrap();
            assert_eq!(t.regex, "(\\w+)");
            assert_eq!(t.replacement, vec![FormatItem::Capture(1)]);
        }
        other => panic!("expected TabStop, got {other:?}"),
    }
}

#[test]
fn test_parse_transform_braced_capture() {
    let body = parse("${1/(\\w+)/${1}/}").unwrap();
    match &body.elements()[0] {
        SnippetElement::TabStop { transform, .. } => {
            let t = transform.as_ref().unwrap();
            assert_eq!(t.replacement, vec![FormatItem::Capture(1)]);
        }
        other => panic!("expected TabStop, got {other:?}"),
    }
}

#[test]
fn test_parse_transform_case_change() {
    let body = parse("${1/(\\w+)/${1:/upcase}/}").unwrap();
    match &body.elements()[0] {
        SnippetElement::TabStop { transform, .. } => {
            let t = transform.as_ref().unwrap();
            assert_eq!(t.replacement, vec![FormatItem::CaseChange(1, CaseModifier::Upcase)]);
        }
        other => panic!("expected TabStop, got {other:?}"),
    }
}

#[test]
fn test_parse_transform_conditional_if() {
    let body = parse("${1/(x)?/${1:+yes}/}").unwrap();
    match &body.elements()[0] {
        SnippetElement::TabStop { transform, .. } => {
            let t = transform.as_ref().unwrap();
            assert_eq!(
                t.replacement,
                vec![FormatItem::Conditional {
                    capture: 1,
                    if_text: "yes".to_string(),
                    else_text: String::new(),
                }]
            );
        }
        other => panic!("expected TabStop, got {other:?}"),
    }
}

#[test]
fn test_parse_transform_conditional_else() {
    let body = parse("${1/(x)?/${1:-no}/}").unwrap();
    match &body.elements()[0] {
        SnippetElement::TabStop { transform, .. } => {
            let t = transform.as_ref().unwrap();
            assert_eq!(
                t.replacement,
                vec![FormatItem::Conditional {
                    capture: 1,
                    if_text: String::new(),
                    else_text: "no".to_string(),
                }]
            );
        }
        other => panic!("expected TabStop, got {other:?}"),
    }
}

#[test]
fn test_parse_transform_conditional_if_else() {
    let body = parse("${1/(x)?/${1:?yes:no}/}").unwrap();
    match &body.elements()[0] {
        SnippetElement::TabStop { transform, .. } => {
            let t = transform.as_ref().unwrap();
            assert_eq!(
                t.replacement,
                vec![FormatItem::Conditional {
                    capture: 1,
                    if_text: "yes".to_string(),
                    else_text: "no".to_string(),
                }]
            );
        }
        other => panic!("expected TabStop, got {other:?}"),
    }
}

#[test]
fn test_parse_transform_escaped_slash_in_regex() {
    let body = parse("${1/a\\/b/x/}").unwrap();
    match &body.elements()[0] {
        SnippetElement::TabStop { transform, .. } => {
            let t = transform.as_ref().unwrap();
            assert_eq!(t.regex, "a/b");
        }
        other => panic!("expected TabStop, got {other:?}"),
    }
}

#[test]
fn test_parse_transform_mixed_replacement() {
    let body = parse("${1/(\\w+) (\\w+)/$2 $1/}").unwrap();
    match &body.elements()[0] {
        SnippetElement::TabStop { transform, .. } => {
            let t = transform.as_ref().unwrap();
            assert_eq!(t.replacement.len(), 3);
            assert_eq!(t.replacement[0], FormatItem::Capture(2));
            assert_eq!(t.replacement[1], FormatItem::Text(" ".to_string()));
            assert_eq!(t.replacement[2], FormatItem::Capture(1));
        }
        other => panic!("expected TabStop, got {other:?}"),
    }
}

#[test]
fn test_parse_transform_empty_replacement() {
    let body = parse("${1/foo//}").unwrap();
    match &body.elements()[0] {
        SnippetElement::TabStop { transform, .. } => {
            let t = transform.as_ref().unwrap();
            assert_eq!(t.regex, "foo");
            assert!(t.replacement.is_empty());
        }
        other => panic!("expected TabStop, got {other:?}"),
    }
}

// =========================================================================
// Choice syntax: ${N|a,b,c|}
// =========================================================================

#[test]
fn test_parse_choice_single() {
    let body = parse("${1|only|}").unwrap();
    assert_eq!(
        body.elements(),
        &[SnippetElement::Choice {
            id: 1,
            choices: vec!["only".to_string()],
        }]
    );
}

#[test]
fn test_parse_choice_three_items() {
    let body = parse("${1|one,two,three|}").unwrap();
    assert_eq!(
        body.elements(),
        &[SnippetElement::Choice {
            id: 1,
            choices: vec!["one".to_string(), "two".to_string(), "three".to_string()],
        }]
    );
}

#[test]
fn test_parse_choice_escaped_comma() {
    let body = parse("${1|a\\,b,c|}").unwrap();
    match &body.elements()[0] {
        SnippetElement::Choice { choices, .. } => {
            assert_eq!(choices, &["a,b".to_string(), "c".to_string()]);
        }
        other => panic!("expected Choice, got {other:?}"),
    }
}

#[test]
fn test_parse_choice_escaped_pipe() {
    let body = parse("${1|a\\|b,c|}").unwrap();
    match &body.elements()[0] {
        SnippetElement::Choice { choices, .. } => {
            assert_eq!(choices, &["a|b".to_string(), "c".to_string()]);
        }
        other => panic!("expected Choice, got {other:?}"),
    }
}

#[test]
fn test_parse_choice_in_context() {
    let body = parse("type: ${1|public,private,protected|} $2").unwrap();
    assert_eq!(body.len(), 4);
    assert_eq!(body.elements()[0], SnippetElement::Text("type: ".to_string()));
    assert!(matches!(body.elements()[1], SnippetElement::Choice { id: 1, .. }));
    assert_eq!(body.elements()[2], SnippetElement::Text(" ".to_string()));
    assert!(matches!(body.elements()[3], SnippetElement::TabStop { id: 2, .. }));
}

#[test]
fn test_parse_choice_unclosed() {
    let result = parse("${1|a,b");
    assert!(result.is_err());
}

// =========================================================================
// parse_case_modifier
// =========================================================================

#[test]
fn test_parse_case_modifier_all() {
    assert_eq!(parse_case_modifier("upcase"), Some(CaseModifier::Upcase));
    assert_eq!(parse_case_modifier("downcase"), Some(CaseModifier::Downcase));
    assert_eq!(parse_case_modifier("capitalize"), Some(CaseModifier::Capitalize));
    assert_eq!(parse_case_modifier("camelcase"), Some(CaseModifier::CamelCase));
    assert_eq!(parse_case_modifier("pascalcase"), Some(CaseModifier::PascalCase));
    assert_eq!(parse_case_modifier("snakecase"), Some(CaseModifier::SnakeCase));
    assert_eq!(parse_case_modifier("kebabcase"), Some(CaseModifier::KebabCase));
    assert_eq!(parse_case_modifier("unknown"), None);
}

// =========================================================================
// Transform with dollar sign in replacement
// =========================================================================

#[test]
fn test_parse_transform_escaped_dollar_in_replacement() {
    let body = parse("${1/foo/\\$/}").unwrap();
    match &body.elements()[0] {
        SnippetElement::TabStop { transform, .. } => {
            let t = transform.as_ref().unwrap();
            assert_eq!(t.replacement, vec![FormatItem::Text("$".to_string())]);
        }
        other => panic!("expected TabStop, got {other:?}"),
    }
}

#[test]
fn test_parse_transform_lone_dollar_in_replacement() {
    let body = parse("${1/foo/$/}").unwrap();
    match &body.elements()[0] {
        SnippetElement::TabStop { transform, .. } => {
            let t = transform.as_ref().unwrap();
            assert_eq!(t.replacement, vec![FormatItem::Text("$".to_string())]);
        }
        other => panic!("expected TabStop, got {other:?}"),
    }
}

// =========================================================================
// Transform edge cases: regex segment
// =========================================================================

#[test]
fn test_parse_transform_escaped_backslash_in_regex() {
    // Regex contains literal backslash: a\\b in input
    let body = parse("${1/a\\\\b/x/}").unwrap();
    match &body.elements()[0] {
        SnippetElement::TabStop { transform, .. } => {
            let t = transform.as_ref().unwrap();
            assert_eq!(t.regex, "a\\b");
        }
        other => panic!("expected TabStop, got {other:?}"),
    }
}

#[test]
fn test_parse_transform_unknown_escape_in_regex() {
    // \n in regex is unknown escape — preserved literally
    let body = parse("${1/a\\nb/x/}").unwrap();
    match &body.elements()[0] {
        SnippetElement::TabStop { transform, .. } => {
            let t = transform.as_ref().unwrap();
            assert_eq!(t.regex, "a\\nb");
        }
        other => panic!("expected TabStop, got {other:?}"),
    }
}

#[test]
fn test_parse_transform_eof_in_regex_trailing_backslash() {
    // Trailing backslash in regex with no closing /
    let result = parse("${1/abc\\");
    assert!(result.is_err());
}

#[test]
fn test_parse_transform_eof_in_regex_no_slash() {
    // EOF before regex delimiter /
    let result = parse("${1/abc");
    assert!(result.is_err());
}

// =========================================================================
// Transform edge cases: replacement segment
// =========================================================================

#[test]
fn test_parse_transform_escaped_backslash_in_replacement() {
    let body = parse("${1/x/a\\\\b/}").unwrap();
    match &body.elements()[0] {
        SnippetElement::TabStop { transform, .. } => {
            let t = transform.as_ref().unwrap();
            assert_eq!(t.replacement, vec![FormatItem::Text("a\\b".to_string())]);
        }
        other => panic!("expected TabStop, got {other:?}"),
    }
}

#[test]
fn test_parse_transform_unknown_escape_in_replacement() {
    let body = parse("${1/x/a\\nb/}").unwrap();
    match &body.elements()[0] {
        SnippetElement::TabStop { transform, .. } => {
            let t = transform.as_ref().unwrap();
            assert_eq!(t.replacement, vec![FormatItem::Text("a\\nb".to_string())]);
        }
        other => panic!("expected TabStop, got {other:?}"),
    }
}

#[test]
fn test_parse_transform_eof_in_replacement_trailing_backslash() {
    let result = parse("${1/x/abc\\");
    assert!(result.is_err());
}

#[test]
fn test_parse_transform_eof_in_replacement_no_slash() {
    let result = parse("${1/x/abc");
    assert!(result.is_err());
}

#[test]
fn test_parse_transform_escaped_slash_in_replacement() {
    let body = parse("${1/x/a\\/b/}").unwrap();
    match &body.elements()[0] {
        SnippetElement::TabStop { transform, .. } => {
            let t = transform.as_ref().unwrap();
            assert_eq!(t.replacement, vec![FormatItem::Text("a/b".to_string())]);
        }
        other => panic!("expected TabStop, got {other:?}"),
    }
}

// =========================================================================
// Format item edge cases in replacement
// =========================================================================

#[test]
fn test_parse_transform_format_item_invalid_number() {
    // ${abc} in replacement — not a valid capture number, skipped
    let body = parse("${1/x/${abc}/}").unwrap();
    match &body.elements()[0] {
        SnippetElement::TabStop { transform, .. } => {
            let t = transform.as_ref().unwrap();
            // Invalid format items are silently ignored
            assert!(t.replacement.is_empty());
        }
        other => panic!("expected TabStop, got {other:?}"),
    }
}

#[test]
fn test_parse_transform_format_item_unknown_char_after_digit() {
    // ${1!} in replacement — unknown char after digit, skip to }
    let body = parse("${1/x/${1!}/}").unwrap();
    match &body.elements()[0] {
        SnippetElement::TabStop { transform, .. } => {
            let t = transform.as_ref().unwrap();
            assert_eq!(t.replacement, vec![FormatItem::Capture(1)]);
        }
        other => panic!("expected TabStop, got {other:?}"),
    }
}

#[test]
fn test_parse_transform_format_modifier_unknown() {
    // ${1:xyz} in replacement — unknown modifier after :
    let body = parse("${1/x/${1:xyz}/}").unwrap();
    match &body.elements()[0] {
        SnippetElement::TabStop { transform, .. } => {
            let t = transform.as_ref().unwrap();
            assert_eq!(t.replacement, vec![FormatItem::Capture(1)]);
        }
        other => panic!("expected TabStop, got {other:?}"),
    }
}

#[test]
fn test_parse_transform_case_modifier_unknown_returns_none() {
    // ${1:/badmod} — unknown case modifier name, returns None so nothing pushed
    let body = parse("${1/x/${1:/badmod}/}").unwrap();
    match &body.elements()[0] {
        SnippetElement::TabStop { transform, .. } => {
            let t = transform.as_ref().unwrap();
            assert!(t.replacement.is_empty());
        }
        other => panic!("expected TabStop, got {other:?}"),
    }
}

// =========================================================================
// Choice edge cases
// =========================================================================

#[test]
fn test_parse_choice_stray_pipe() {
    // ${1|a|b|} — stray | not followed by } becomes literal |
    let body = parse("${1|a|b|}").unwrap();
    match &body.elements()[0] {
        SnippetElement::Choice { choices, .. } => {
            // "a|b" is one choice (stray | becomes literal)
            assert_eq!(choices, &["a|b".to_string()]);
        }
        other => panic!("expected Choice, got {other:?}"),
    }
}

#[test]
fn test_parse_choice_escaped_backslash() {
    let body = parse("${1|a\\\\b|}").unwrap();
    match &body.elements()[0] {
        SnippetElement::Choice { choices, .. } => {
            assert_eq!(choices, &["a\\b".to_string()]);
        }
        other => panic!("expected Choice, got {other:?}"),
    }
}

#[test]
fn test_parse_choice_unknown_escape() {
    let body = parse("${1|a\\xb|}").unwrap();
    match &body.elements()[0] {
        SnippetElement::Choice { choices, .. } => {
            assert_eq!(choices, &["a\\xb".to_string()]);
        }
        other => panic!("expected Choice, got {other:?}"),
    }
}

#[test]
fn test_parse_choice_escaped_backslash_eof() {
    let result = parse("${1|a\\");
    assert!(result.is_err());
}

// =========================================================================
// Braced fallback edge cases
// =========================================================================

#[test]
fn test_parse_braced_unknown_start_char() {
    // ${ followed by non-digit, non-letter → fallback
    let body = parse("${!foo}").unwrap();
    assert_eq!(body.elements(), &[SnippetElement::Text("${!foo}".to_string())]);
}

#[test]
fn test_parse_braced_variable_unknown_suffix() {
    // ${VAR!} — unknown char after variable name
    let body = parse("${VAR!rest}").unwrap();
    assert_eq!(body.elements(), &[SnippetElement::Text("${VAR!rest}".to_string())]);
}

#[test]
fn test_parse_fallback_braced_no_closing_brace() {
    // ${ followed by unknown char and no closing brace
    let body = parse("${!abc").unwrap();
    assert_eq!(body.elements(), &[SnippetElement::Text("${!abc".to_string())]);
}

#[test]
fn test_parse_braced_tabstop_eof_after_number() {
    // ${1 — no closing brace or suffix, already tested but ensure None path
    let body = parse("${1").unwrap();
    assert_eq!(body.elements(), &[SnippetElement::Text("${1".to_string())]);
}

#[test]
fn test_parse_braced_variable_eof_after_name() {
    // ${VAR — no closing brace, hits Some(_)|None fallback
    let body = parse("${VAR").unwrap();
    assert_eq!(body.elements(), &[SnippetElement::Text("${VAR".to_string())]);
}

// =========================================================================
// Dollar followed by non-identifier/non-digit/non-brace
// =========================================================================

#[test]
fn test_parse_dollar_followed_by_special_char() {
    // $ followed by a char that is not {, digit, or var_start
    let body = parse("$ rest").unwrap();
    assert_eq!(body.elements(), &[SnippetElement::Text("$ rest".to_string())]);
}

#[test]
fn test_parse_dollar_followed_by_exclamation() {
    let body = parse("$!").unwrap();
    assert_eq!(body.elements(), &[SnippetElement::Text("$!".to_string())]);
}

// =========================================================================
// Transform: EOF while reading options
// =========================================================================

#[test]
fn test_parse_transform_eof_in_options() {
    // Options section has no closing } — EOF while reading options
    let body = parse("${1/foo/bar/gi").unwrap();
    match &body.elements()[0] {
        SnippetElement::TabStop { transform, .. } => {
            let t = transform.as_ref().unwrap();
            assert_eq!(t.options, "gi");
        }
        other => panic!("expected TabStop, got {other:?}"),
    }
}

// =========================================================================
// Replacement: $ capture number overflow
// =========================================================================

#[test]
fn test_parse_transform_capture_overflow_in_replacement() {
    // Huge capture number in replacement that overflows usize
    let body = parse("${1/x/$99999999999999999999999/}").unwrap();
    match &body.elements()[0] {
        SnippetElement::TabStop { transform, .. } => {
            let t = transform.as_ref().unwrap();
            // Overflow: digits don't parse, nothing pushed for that capture
            assert!(t.replacement.is_empty());
        }
        other => panic!("expected TabStop, got {other:?}"),
    }
}

// =========================================================================
// Format item EOF paths in replacement
// =========================================================================

#[test]
fn test_parse_transform_format_item_eof_invalid_number() {
    // ${abc without closing } — invalid number then EOF in skip loop
    let result = parse("${1/x/${abc");
    assert!(result.is_err());
}

#[test]
fn test_parse_transform_format_item_eof_after_digit() {
    // ${1! without closing } — unknown char after digit, then EOF in skip loop
    let result = parse("${1/x/${1!");
    assert!(result.is_err());
}

#[test]
fn test_parse_transform_format_modifier_eof() {
    // ${1:xyz without closing } — unknown modifier, then EOF in skip loop
    let result = parse("${1/x/${1:xyz");
    assert!(result.is_err());
}

// =========================================================================
// read_until EOF path
// =========================================================================

#[test]
fn test_parse_transform_read_until_eof() {
    // ${1:+if without closing } — read_until hits EOF
    let result = parse("${1/x/${1:+something");
    assert!(result.is_err());
}

// =========================================================================
// Choice: empty choice body (|}  )
// =========================================================================

#[test]
fn test_parse_choice_empty_body() {
    // ${1||} — empty current when reaching |}
    let body = parse("${1||}").unwrap();
    assert_eq!(
        body.elements(),
        &[SnippetElement::Choice {
            id: 1,
            choices: vec![],
        }]
    );
}

#[test]
fn test_parse_all_case_modifiers_in_transform() {
    for modifier in &[
        "upcase",
        "downcase",
        "capitalize",
        "camelcase",
        "pascalcase",
        "snakecase",
        "kebabcase",
    ] {
        let input = format!("${{1/(\\w+)/${{1:/{modifier}}}/}}");
        let body = parse(&input).unwrap();
        match &body.elements()[0] {
            SnippetElement::TabStop { transform, .. } => {
                let t = transform.as_ref().unwrap();
                assert_eq!(t.replacement.len(), 1);
                assert!(
                    matches!(&t.replacement[0], FormatItem::CaseChange(1, _)),
                    "expected CaseChange for {modifier}"
                );
            }
            other => panic!("expected TabStop, got {other:?}"),
        }
    }
}
