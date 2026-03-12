//! Profile name validation.

/// Validate a profile name.
///
/// Valid names: non-empty, starts with alphanumeric, only `[a-zA-Z0-9_-]`,
/// max 64 characters. This prevents path traversal and filesystem issues.
///
/// # Errors
///
/// Returns a descriptive error string if the name is invalid.
pub fn validate_profile_name(name: &str) -> Result<(), String> {
    if name.is_empty() {
        return Err("profile name must not be empty".to_string());
    }

    if name.len() > 64 {
        return Err(format!("profile name too long ({} chars, max 64)", name.len()));
    }

    let first = name.as_bytes()[0];
    if !first.is_ascii_alphanumeric() {
        return Err(format!(
            "profile name must start with a letter or digit, got '{}'",
            char::from(first)
        ));
    }

    if let Some(bad) = name
        .chars()
        .find(|c| !matches!(c, 'a'..='z' | 'A'..='Z' | '0'..='9' | '_' | '-'))
    {
        return Err(format!(
            "profile name contains invalid character '{bad}' (only alphanumeric, '-', '_' allowed)"
        ));
    }

    Ok(())
}

#[cfg(test)]
#[path = "validate_tests.rs"]
mod tests;
