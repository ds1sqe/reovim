use {
    super::*,
    crate::ast::{CaseModifier, FormatItem, Transform},
};

// =========================================================================
// apply_transform: basic matching
// =========================================================================

#[test]
fn test_no_match_returns_input() {
    let t = Transform {
        regex: "xyz".to_string(),
        replacement: vec![FormatItem::Text("replaced".to_string())],
        options: String::new(),
    };
    assert_eq!(apply_transform("hello", &t), "hello");
}

#[test]
fn test_first_match_only() {
    let t = Transform {
        regex: "o".to_string(),
        replacement: vec![FormatItem::Text("0".to_string())],
        options: String::new(),
    };
    assert_eq!(apply_transform("foobar", &t), "f0obar");
}

#[test]
fn test_global_match() {
    let t = Transform {
        regex: "o".to_string(),
        replacement: vec![FormatItem::Text("0".to_string())],
        options: "g".to_string(),
    };
    assert_eq!(apply_transform("foobar", &t), "f00bar");
}

#[test]
fn test_invalid_regex_returns_input() {
    let t = Transform {
        regex: "[invalid".to_string(),
        replacement: vec![],
        options: String::new(),
    };
    assert_eq!(apply_transform("hello", &t), "hello");
}

// =========================================================================
// Capture groups
// =========================================================================

#[test]
fn test_capture_group() {
    let t = Transform {
        regex: "(\\w+)\\s+(\\w+)".to_string(),
        replacement: vec![
            FormatItem::Capture(2),
            FormatItem::Text(" ".to_string()),
            FormatItem::Capture(1),
        ],
        options: String::new(),
    };
    assert_eq!(apply_transform("hello world", &t), "world hello");
}

#[test]
fn test_capture_group_missing() {
    let t = Transform {
        regex: "(a)(b)?".to_string(),
        replacement: vec![
            FormatItem::Capture(1),
            FormatItem::Text("-".to_string()),
            FormatItem::Capture(2), // group 2 didn't match
        ],
        options: String::new(),
    };
    assert_eq!(apply_transform("ac", &t), "a-c");
}

// =========================================================================
// Case modifiers
// =========================================================================

#[test]
fn test_case_change_upcase() {
    let t = Transform {
        regex: "(\\w+)".to_string(),
        replacement: vec![FormatItem::CaseChange(1, CaseModifier::Upcase)],
        options: String::new(),
    };
    assert_eq!(apply_transform("hello world", &t), "HELLO world");
}

#[test]
fn test_case_change_downcase() {
    let t = Transform {
        regex: "(\\w+)".to_string(),
        replacement: vec![FormatItem::CaseChange(1, CaseModifier::Downcase)],
        options: "g".to_string(),
    };
    assert_eq!(apply_transform("HELLO WORLD", &t), "hello world");
}

#[test]
fn test_case_change_capitalize() {
    let t = Transform {
        regex: "(\\w+)".to_string(),
        replacement: vec![FormatItem::CaseChange(1, CaseModifier::Capitalize)],
        options: "g".to_string(),
    };
    assert_eq!(apply_transform("hello world", &t), "Hello World");
}

#[test]
fn test_case_change_missing_group() {
    let t = Transform {
        regex: "(a)".to_string(),
        replacement: vec![FormatItem::CaseChange(5, CaseModifier::Upcase)],
        options: String::new(),
    };
    // Group 5 doesn't exist — emits nothing for that item
    assert_eq!(apply_transform("abc", &t), "bc");
}

// =========================================================================
// Conditional
// =========================================================================

#[test]
fn test_conditional_matched() {
    let t = Transform {
        regex: "(\\w+)?".to_string(),
        replacement: vec![FormatItem::Conditional {
            capture: 1,
            if_text: "yes".to_string(),
            else_text: "no".to_string(),
        }],
        options: String::new(),
    };
    assert_eq!(apply_transform("hello", &t), "yes");
}

#[test]
fn test_conditional_not_matched() {
    let t = Transform {
        regex: "(z)?x".to_string(),
        replacement: vec![FormatItem::Conditional {
            capture: 1,
            if_text: "found".to_string(),
            else_text: "missing".to_string(),
        }],
        options: String::new(),
    };
    assert_eq!(apply_transform("x", &t), "missing");
}

// =========================================================================
// apply_case_modifier standalone
// =========================================================================

#[test]
fn test_apply_upcase() {
    assert_eq!(apply_case_modifier("hello", CaseModifier::Upcase), "HELLO");
}

#[test]
fn test_apply_downcase() {
    assert_eq!(apply_case_modifier("HELLO", CaseModifier::Downcase), "hello");
}

#[test]
fn test_apply_capitalize() {
    assert_eq!(apply_case_modifier("hello", CaseModifier::Capitalize), "Hello");
}

#[test]
fn test_apply_capitalize_empty() {
    assert_eq!(apply_case_modifier("", CaseModifier::Capitalize), "");
}

#[test]
fn test_apply_camel_case() {
    assert_eq!(apply_case_modifier("hello_world", CaseModifier::CamelCase), "helloWorld");
}

#[test]
fn test_apply_pascal_case() {
    assert_eq!(apply_case_modifier("hello_world", CaseModifier::PascalCase), "HelloWorld");
}

#[test]
fn test_apply_snake_case() {
    assert_eq!(apply_case_modifier("HelloWorld", CaseModifier::SnakeCase), "helloworld");
}

#[test]
fn test_apply_snake_case_with_spaces() {
    assert_eq!(apply_case_modifier("hello world", CaseModifier::SnakeCase), "hello_world");
}

#[test]
fn test_apply_kebab_case() {
    assert_eq!(apply_case_modifier("hello world", CaseModifier::KebabCase), "hello-world");
}

// =========================================================================
// split_words
// =========================================================================

#[test]
fn test_split_words_empty() {
    assert!(split_words("").is_empty());
}

#[test]
fn test_split_words_single() {
    assert_eq!(split_words("hello"), vec!["hello"]);
}

#[test]
fn test_split_words_underscore() {
    assert_eq!(split_words("hello_world"), vec!["hello", "world"]);
}

#[test]
fn test_split_words_spaces() {
    assert_eq!(split_words("hello world"), vec!["hello", "world"]);
}

#[test]
fn test_split_words_mixed_delimiters() {
    assert_eq!(split_words("hello-world_foo bar"), vec!["hello", "world", "foo", "bar"]);
}

#[test]
fn test_split_words_only_delimiters() {
    assert!(split_words("---___   ").is_empty());
}

// =========================================================================
// Complex transforms
// =========================================================================

#[test]
fn test_complex_replacement() {
    // Replace "class Foo" with "Foo_CLASS"
    let t = Transform {
        regex: "class (\\w+)".to_string(),
        replacement: vec![
            FormatItem::Capture(1),
            FormatItem::Text("_".to_string()),
            FormatItem::CaseChange(1, CaseModifier::Upcase),
        ],
        options: String::new(),
    };
    assert_eq!(apply_transform("class Foo extends Bar", &t), "Foo_FOO extends Bar");
}

#[test]
fn test_global_with_captures() {
    let t = Transform {
        regex: "(\\w)".to_string(),
        replacement: vec![FormatItem::CaseChange(1, CaseModifier::Upcase)],
        options: "g".to_string(),
    };
    assert_eq!(apply_transform("abc", &t), "ABC");
}

#[test]
fn test_empty_replacement() {
    let t = Transform {
        regex: "world".to_string(),
        replacement: vec![],
        options: String::new(),
    };
    assert_eq!(apply_transform("hello world", &t), "hello ");
}

#[test]
fn test_global_no_matches() {
    let t = Transform {
        regex: "xyz".to_string(),
        replacement: vec![FormatItem::Text("!".to_string())],
        options: "g".to_string(),
    };
    assert_eq!(apply_transform("hello", &t), "hello");
}
