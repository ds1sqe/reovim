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
#[path = "transform_tests.rs"]
mod tests;
