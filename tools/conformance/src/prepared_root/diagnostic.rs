//! Bounded, redacted same-binary child diagnostic decoding.

use crate::digest;

use super::{linux::one_final_line, model::DispatchBounds};

pub(super) fn child_error_summary(
    stderr: &[u8],
    bounds: DispatchBounds,
) -> String {
    typed_child_error(stderr, bounds).unwrap_or_else(|| {
        format!(
            "malformed_child_diagnostic bytes={} digest={}",
            stderr.len(),
            digest::bytes(stderr)
        )
    })
}

fn typed_child_error(stderr: &[u8], bounds: DispatchBounds) -> Option<String> {
    let line = one_final_line(stderr, "PREPARED_CHILD_DIAGNOSTIC").ok()?;
    let value =
        crate::json::parse(line, bounds.json_depth, bounds.json_items).ok()?;
    let object = value.as_object()?;
    if object.len() != 2 || !object.contains_key("code") {
        return None;
    }
    let code = normalized_child_code(object.get("code")?.as_str()?)?;
    let message = object.get("message")?.as_str()?;
    Some(format!(
        "child_code={code}; child_message={}",
        redact_diagnostic(message)
    ))
}

fn normalized_child_code(value: &str) -> Option<&str> {
    let valid = !value.is_empty()
        && value.len() <= 96
        && value.bytes().all(|byte| {
            byte.is_ascii_uppercase() || byte.is_ascii_digit() || byte == b'_'
        });
    valid.then_some(value)
}

pub(super) fn redact_diagnostic(value: &str) -> String {
    const LIMIT: usize = 512;

    let mut output = String::new();
    let mut path = false;
    for character in value.chars() {
        if !push_redacted(&mut output, character, &mut path, LIMIT) {
            break;
        }
    }
    output
}

fn push_redacted(
    output: &mut String,
    character: char,
    path: &mut bool,
    limit: usize,
) -> bool {
    if *path && character.is_whitespace() {
        *path = false;
    }
    if *path {
        return true;
    }
    if character == '/' {
        return push_path_marker(output, path, limit);
    }
    if output.len().saturating_add(character.len_utf8()) > limit {
        return false;
    }
    output.push(if character.is_control() { '?' } else { character });
    true
}

fn push_path_marker(
    output: &mut String,
    path: &mut bool,
    limit: usize,
) -> bool {
    if output.len().saturating_add(6) > limit {
        return false;
    }
    output.push_str("<path>");
    *path = true;
    true
}
