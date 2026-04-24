//! Parser for the `REOVIM_VIEW_HINT` environment variable.
//!
//! Exposed as a `&str`-taking function (not `std::env::var`) so the
//! parsing logic is testable without touching the process environment.
//! The caller reads the env var, trims surrounding whitespace if any,
//! and passes the raw value here.

use reovim_ext_client_tui_cap_cell_view::ViewHint;

/// Parse a `REOVIM_VIEW_HINT` value into a [`ViewHint`], or `None` if
/// the value is empty or unrecognized.
///
/// Accepted forms (case-insensitive): `full`, `fullblock`, `full_block`,
/// `full-block`, `half`, `halfblock`, `half_block`, `half-block`,
/// `braille`.
#[must_use]
pub fn parse_view_hint(raw: &str) -> Option<ViewHint> {
    let normalized = raw.trim().to_ascii_lowercase().replace(['_', '-'], "");
    match normalized.as_str() {
        "full" | "fullblock" => Some(ViewHint::FullBlock),
        "half" | "halfblock" => Some(ViewHint::HalfBlock),
        "braille" => Some(ViewHint::Braille),
        _ => None,
    }
}

#[cfg(test)]
#[path = "view_hint_env_tests.rs"]
mod tests;
