//! Regex transform execution for snippet tab stops and variables.
//!
//! Implements the `/${regex}/${replacement}/${options}` transform syntax
//! from the TextMate/LSP snippet specification.
//!
//! Supports capture group references (`$1`, `${1}`), case modifiers
//! (`${1:/upcase}`), and conditional insertions (`${1:+if}`, `${1:-else}`,
//! `${1:?if:else}`).

use regex::Regex;

use crate::ast::{CaseModifier, FormatItem, Transform};

/// Apply a transform to the given input text.
///
/// The transform's regex is matched against the input, and for each match
/// the replacement items are evaluated to produce the output text.
/// If the `g` option is set, all matches are replaced; otherwise only the first.
///
/// Returns the transformed string, or the original input if the regex is invalid.
#[must_use]
pub fn apply_transform(input: &str, transform: &Transform) -> String {
    let Ok(re) = Regex::new(&transform.regex) else {
        return input.to_owned();
    };

    let global = transform.options.contains('g');

    if global {
        apply_global(&re, input, &transform.replacement)
    } else {
        apply_first(&re, input, &transform.replacement)
    }
}

/// Replace the first match.
fn apply_first(re: &Regex, input: &str, replacement: &[FormatItem]) -> String {
    let Some(m) = re.captures(input) else {
        return input.to_owned();
    };

    let full = m.get(0).expect("match group 0 always exists");
    let mut result = String::with_capacity(input.len());
    result.push_str(&input[..full.start()]);
    expand_captures(&m, replacement, &mut result);
    result.push_str(&input[full.end()..]);
    result
}

/// Replace all non-overlapping matches.
fn apply_global(re: &Regex, input: &str, replacement: &[FormatItem]) -> String {
    let mut result = String::with_capacity(input.len());
    let mut last_end = 0;

    for m in re.captures_iter(input) {
        let full = m.get(0).expect("match group 0 always exists");
        result.push_str(&input[last_end..full.start()]);
        expand_captures(&m, replacement, &mut result);
        last_end = full.end();
    }

    result.push_str(&input[last_end..]);
    result
}

/// Expand replacement items using capture groups from a match.
fn expand_captures(caps: &regex::Captures<'_>, items: &[FormatItem], out: &mut String) {
    for item in items {
        match item {
            FormatItem::Text(t) => out.push_str(t),
            FormatItem::Capture(n) => {
                if let Some(m) = caps.get(*n) {
                    out.push_str(m.as_str());
                }
            }
            FormatItem::CaseChange(n, modifier) => {
                if let Some(m) = caps.get(*n) {
                    out.push_str(&apply_case_modifier(m.as_str(), *modifier));
                }
            }
            FormatItem::Conditional {
                capture,
                if_text,
                else_text,
            } => {
                if caps.get(*capture).is_some_and(|m| !m.as_str().is_empty()) {
                    out.push_str(if_text);
                } else {
                    out.push_str(else_text);
                }
            }
        }
    }
}

/// Apply a case modifier to text.
#[must_use]
pub fn apply_case_modifier(input: &str, modifier: CaseModifier) -> String {
    match modifier {
        CaseModifier::Upcase => input.to_uppercase(),
        CaseModifier::Downcase => input.to_lowercase(),
        CaseModifier::Capitalize => capitalize(input),
        CaseModifier::CamelCase => to_camel_case(input),
        CaseModifier::PascalCase => to_pascal_case(input),
        CaseModifier::SnakeCase => to_snake_case(input),
        CaseModifier::KebabCase => to_kebab_case(input),
    }
}

/// Capitalize the first character, lowercase the rest.
fn capitalize(input: &str) -> String {
    let mut chars = input.chars();
    chars.next().map_or_else(String::new, |first| {
        let mut result = first.to_uppercase().to_string();
        for ch in chars {
            result.extend(ch.to_lowercase());
        }
        result
    })
}

/// Split input into words by non-alphanumeric boundaries.
fn split_words(input: &str) -> Vec<String> {
    let mut words = Vec::new();
    let mut current = String::new();

    for ch in input.chars() {
        if ch.is_alphanumeric() {
            current.push(ch);
        } else if !current.is_empty() {
            words.push(std::mem::take(&mut current));
        }
    }
    if !current.is_empty() {
        words.push(current);
    }
    words
}

/// Convert to `camelCase`.
fn to_camel_case(input: &str) -> String {
    let words = split_words(input);
    let mut result = String::new();
    for (i, word) in words.iter().enumerate() {
        if i == 0 {
            result.push_str(&word.to_lowercase());
        } else {
            result.push_str(&capitalize(word));
        }
    }
    result
}

/// Convert to `PascalCase`.
fn to_pascal_case(input: &str) -> String {
    split_words(input).iter().map(|w| capitalize(w)).collect()
}

/// Convert to `snake_case`.
fn to_snake_case(input: &str) -> String {
    split_words(input)
        .iter()
        .map(|w| w.to_lowercase())
        .collect::<Vec<_>>()
        .join("_")
}

/// Convert to `kebab-case`.
fn to_kebab_case(input: &str) -> String {
    split_words(input)
        .iter()
        .map(|w| w.to_lowercase())
        .collect::<Vec<_>>()
        .join("-")
}

#[cfg(test)]
mod tests {
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
}
