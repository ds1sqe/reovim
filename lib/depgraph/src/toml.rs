//! Minimal std-only TOML reader for the depgraph probe (1.2 §5).
//!
//! # Supported subset
//!
//! This reader handles exactly the TOML shapes the probe needs — no more.
//! It is not a general-purpose TOML parser. Using an out-of-subset construct
//! returns [`crate::ProbeError::Parse`] with the offending line number.
//!
//! Supported constructs:
//! - Top-level and one-level table headers: `[table]`, `[a.b]`.
//!   (Two-level depth: `[a.b]` is treated as table `a.b`; three or more
//!   dotted components are a parse error.)
//! - Array-of-tables headers: `[[entry]]` with string fields.
//! - Bare-key `= "string"` assignments.
//! - Bare-key `= { inline-table }` where every value is a `"string"` or a
//!   `true`/`false` boolean.
//! - Bare-key `= true` / `= false` booleans.
//! - Bare-key `= ["a", "b"]` single-line arrays of quoted strings
//!   (the shape of workspace `members` lists); trailing comma
//!   tolerated.
//! - Comments (`#` to end of line) and blank lines.
//!
//! Unsupported (all → [`crate::ProbeError::Parse`]):
//! - Quoted keys.
//! - Multi-line arrays, and arrays with non-string elements.
//! - Multi-line strings.
//! - Integer, float, or datetime literal values (where string required).
//! - Nested inline tables beyond one level.
//! - Table headers deeper than two dotted components.
//! - Duplicate table headers (same `[section]` appearing more than once).
//!
//! # Error invariants
//!
//! Every error path returns `ProbeError::Parse` with a message that names the
//! offending line number and a human-readable reason. This is required by the
//! 100% MC/DC policy: every error arm must be reachable from a unit test.

use std::{
    collections::{BTreeMap, BTreeSet},
    path::Path,
};

use crate::ProbeError;

/// The parsed shape of a single `Cargo.toml` or probe catalog TOML file.
///
/// All values are retained as strings; the caller interprets them.
// `TomlDoc` repeats the module name `toml`; the prefix is intentional here
// because callers import this as `toml::TomlDoc`, not `toml::Doc`.
#[allow(clippy::module_name_repetitions)]
#[derive(Debug, Default)]
pub struct TomlDoc {
    /// Named sections: `[table]` → key-value pairs.
    pub sections: BTreeMap<String, BTreeMap<String, TomlValue>>,
    /// Array-of-tables: `[[entry]]` → list of key-value records.
    pub arrays: BTreeMap<String, Vec<BTreeMap<String, String>>>,
}

impl TomlDoc {
    /// Returns the string value at `[section].key`, if present.
    #[must_use]
    pub fn get_str(&self, section: &str, key: &str) -> Option<&str> {
        self.sections
            .get(section)
            .and_then(|s| s.get(key))
            .and_then(TomlValue::as_str)
    }

    /// Returns the entries of an array-of-tables `[[name]]`.
    #[must_use]
    pub fn array(&self, name: &str) -> &[BTreeMap<String, String>] {
        self.arrays.get(name).map_or(&[], Vec::as_slice)
    }
}

/// A TOML value in the supported subset.
// `TomlValue` repeats the module name `toml`; the prefix is intentional
// because callers import this as `toml::TomlValue`, not `toml::Value`.
#[allow(clippy::module_name_repetitions)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TomlValue {
    /// A quoted string.
    String(String),
    /// A boolean literal.
    Bool(bool),
    /// An inline table (one level deep; all values are strings or booleans).
    InlineTable(BTreeMap<String, Self>),
    /// A single-line array of quoted strings (e.g. workspace `members`).
    Array(Vec<String>),
}

impl TomlValue {
    /// Returns the inner string if this is a `String` variant.
    #[must_use]
    pub const fn as_str(&self) -> Option<&str> {
        if let Self::String(s) = self {
            Some(s.as_str())
        } else {
            None
        }
    }

    /// Returns the inner bool if this is a `Bool` variant.
    #[must_use]
    pub const fn as_bool(&self) -> Option<bool> {
        if let Self::Bool(b) = self {
            Some(*b)
        } else {
            None
        }
    }

    /// Returns the inner inline-table if this is an `InlineTable` variant.
    #[must_use]
    pub const fn as_table(&self) -> Option<&BTreeMap<String, Self>> {
        if let Self::InlineTable(t) = self {
            Some(t)
        } else {
            None
        }
    }
}

/// Parses a TOML file using the supported subset.
///
/// # Errors
///
/// Returns `ProbeError::Io` when the file cannot be read, or
/// `ProbeError::Parse` when the content uses an unsupported construct.
pub fn parse_file(path: &Path) -> Result<TomlDoc, ProbeError> {
    let text = std::fs::read_to_string(path).map_err(|source| ProbeError::Io {
        path: path.to_path_buf(),
        source,
    })?;
    parse_text(&text, path)
}

/// Parses TOML text using the supported subset.
///
/// `file_path` is used only for error messages.
///
/// # Errors
///
/// Returns `ProbeError::Parse` when the content uses an unsupported construct.
pub fn parse_text(text: &str, file_path: &Path) -> Result<TomlDoc, ProbeError> {
    let mut doc = TomlDoc::default();
    // Single parse scope: where a `key = value` line lands. Carrying the
    // active [[array]] record inside the variant makes "in an array scope
    // without a record" unrepresentable.
    let mut scope = Scope::Root;
    // Seen section names (for duplicate detection).
    let mut seen_sections: BTreeSet<String> = BTreeSet::new();

    for (line_idx, raw_line) in text.lines().enumerate() {
        let line_no = line_idx + 1;
        let line = strip_comment(raw_line).trim();
        if line.is_empty() {
            continue;
        }
        if line.starts_with("[[") {
            // Array-of-tables header: `[[name]]`
            flush_scope(&mut scope, &mut doc);
            let name = parse_array_header(line, file_path, line_no)?;
            scope = Scope::Array(name, BTreeMap::new());
        } else if line.starts_with('[') {
            // Section header: `[section]` or `[a.b]`
            flush_scope(&mut scope, &mut doc);
            let name = parse_section_header(line, file_path, line_no)?;
            if !seen_sections.insert(name.clone()) {
                return Err(err(file_path, line_no, &format!("duplicate table header `[{name}]`")));
            }
            doc.sections.entry(name.clone()).or_default();
            scope = Scope::Table(name);
        } else if let Some((raw_key, raw_val)) = line.split_once('=') {
            let (key, value) = parse_kv(raw_key, raw_val, file_path, line_no)?;
            match &mut scope {
                Scope::Root => {
                    return Err(err(
                        file_path,
                        line_no,
                        "key-value pair outside any [table] is not supported",
                    ));
                }
                Scope::Array(_, record) => {
                    // Inside an [[array]] record: only string values are allowed.
                    let s = value.as_str().ok_or_else(|| {
                        err(file_path, line_no, "array-of-tables field must be a string value")
                    })?;
                    record.insert(key, s.to_owned());
                }
                Scope::Table(name) => {
                    doc.sections
                        .entry(name.clone())
                        .or_default()
                        .insert(key, value);
                }
            }
        } else {
            return Err(err(file_path, line_no, "unrecognised line shape"));
        }
    }
    flush_scope(&mut scope, &mut doc);
    Ok(doc)
}

/// Where a `key = value` line lands during [`parse_text`].
enum Scope {
    /// Before any header: top-level key-value pairs.
    Root,
    /// Inside a `[table]` header.
    Table(String),
    /// Inside a `[[array]]` header, accumulating the current record.
    Array(String, BTreeMap<String, String>),
}

// ── internal helpers ─────────────────────────────────────────────────────────

/// Strips a trailing `# comment` from a line, respecting quoted strings
/// (a `#` inside a quoted string is not a comment).
fn strip_comment(line: &str) -> &str {
    // Walk the raw bytes; track whether we are inside a double-quoted string.
    let bytes = line.as_bytes();
    let mut in_string = false;
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'"' => in_string = !in_string,
            b'\\' if in_string => i += 1, // skip escaped char
            b'#' if !in_string => return &line[..i],
            _ => {}
        }
        i += 1;
    }
    line
}

/// Constructs a `ProbeError::Parse` with a `line N:` prefix.
fn err(path: &Path, line_no: usize, reason: &str) -> ProbeError {
    ProbeError::Parse {
        path: path.to_path_buf(),
        message: format!("line {line_no}: {reason}"),
    }
}

/// Flushes an active `Scope::Array` record into `doc.arrays`; other scopes
/// flush to nothing. The scope resets to `Root` either way (a header always
/// follows, or parsing ends).
fn flush_scope(scope: &mut Scope, doc: &mut TomlDoc) {
    if let Scope::Array(name, fields) = std::mem::replace(scope, Scope::Root) {
        doc.arrays.entry(name).or_default().push(fields);
    }
}

/// Parses `[section]` or `[a.b]` → returns the normalized name.
///
/// # Errors
///
/// Returns `ProbeError::Parse` when:
/// - The closing `]` is missing.
/// - The header contains more than two dotted components.
fn parse_section_header(line: &str, path: &Path, line_no: usize) -> Result<String, ProbeError> {
    let inner = line
        .strip_prefix('[')
        .and_then(|s| s.strip_suffix(']'))
        .ok_or_else(|| err(path, line_no, "unclosed `[` in section header"))?
        .trim();
    let parts: Vec<&str> = inner.split('.').map(str::trim).collect();
    if parts.len() > 2 {
        return Err(err(
            path,
            line_no,
            "table header depth > 2 dotted components is not supported",
        ));
    }
    Ok(parts.join("."))
}

/// Parses `[[name]]` → returns the name.
///
/// # Errors
///
/// Returns `ProbeError::Parse` when the closing `]]` is missing.
fn parse_array_header(line: &str, path: &Path, line_no: usize) -> Result<String, ProbeError> {
    let inner = line
        .strip_prefix("[[")
        .and_then(|s| s.strip_suffix("]]"))
        .ok_or_else(|| err(path, line_no, "unclosed `[[` in array-of-tables header"))?
        .trim();
    Ok(inner.to_owned())
}

/// Parses `key = value` → `(key, TomlValue)`.
///
/// # Errors
///
/// Returns `ProbeError::Parse` when:
/// - The key is empty.
/// - The value is a non-string, non-bool, non-inline-table literal where a
///   string is syntactically expected.
/// - An inline table is unclosed.
/// - A string value has an unclosed `"`.
fn parse_kv(
    raw_key: &str,
    raw_val: &str,
    path: &Path,
    line_no: usize,
) -> Result<(String, TomlValue), ProbeError> {
    let key = raw_key.trim();
    if key.is_empty() {
        return Err(err(path, line_no, "empty key"));
    }
    let val_str = raw_val.trim();
    let value = parse_value(val_str, path, line_no)?;
    Ok((key.to_owned(), value))
}

/// Parses a TOML value in the supported subset.
///
/// # Errors
///
/// Returns `ProbeError::Parse` for unsupported or malformed values.
fn parse_value(s: &str, path: &Path, line_no: usize) -> Result<TomlValue, ProbeError> {
    if s.starts_with('"') {
        parse_string(s, path, line_no).map(TomlValue::String)
    } else if s.starts_with('{') {
        parse_inline_table(s, path, line_no).map(TomlValue::InlineTable)
    } else if s == "true" {
        Ok(TomlValue::Bool(true))
    } else if s == "false" {
        Ok(TomlValue::Bool(false))
    } else if s.starts_with('[') {
        parse_array(s, path, line_no).map(TomlValue::Array)
    } else {
        Err(err(
            path,
            line_no,
            &format!("non-string, non-bool, non-inline-table value `{s}` is not supported"),
        ))
    }
}

/// Parses a single-line array of quoted strings: `["a", "b"]`.
///
/// A trailing comma is tolerated; every element must be a quoted
/// string (this is the shape of workspace `members` lists).
///
/// # Errors
///
/// Returns `ProbeError::Parse` when the closing `]` is missing or an
/// element is not a quoted string.
fn parse_array(s: &str, path: &Path, line_no: usize) -> Result<Vec<String>, ProbeError> {
    let inner = s
        .strip_prefix('[')
        .and_then(|t| t.rfind(']').map(|i| &t[..i]))
        .ok_or_else(|| err(path, line_no, "unclosed `[` in array value"))?
        .trim();
    let mut items = Vec::new();
    if inner.is_empty() {
        return Ok(items);
    }
    let entries = split_inline_entries(inner);
    let last = entries.len() - 1;
    for (i, entry) in entries.into_iter().enumerate() {
        let entry = entry.trim();
        if entry.is_empty() {
            if i == last {
                // Trailing comma.
                continue;
            }
            return Err(err(path, line_no, "empty array element"));
        }
        items.push(parse_string(entry, path, line_no)?);
    }
    Ok(items)
}

/// Parses a `"quoted string"` value, returning the inner content.
///
/// # Errors
///
/// Returns `ProbeError::Parse` when the closing `"` is missing.
fn parse_string(s: &str, path: &Path, line_no: usize) -> Result<String, ProbeError> {
    let inner = s
        .strip_prefix('"')
        .and_then(|t| t.rfind('"').map(|i| &t[..i]))
        .ok_or_else(|| err(path, line_no, "unclosed `\"` in string value"))?;
    Ok(inner.to_owned())
}

/// Parses an inline table `{ key = "val", flag = true }`.
///
/// Only one level of depth is supported; nested inline tables are rejected.
///
/// # Errors
///
/// Returns `ProbeError::Parse` when:
/// - The closing `}` is missing.
/// - A nested inline table is found (unsupported depth).
/// - A value is neither a string nor a boolean.
fn parse_inline_table(
    s: &str,
    path: &Path,
    line_no: usize,
) -> Result<BTreeMap<String, TomlValue>, ProbeError> {
    let inner = s
        .strip_prefix('{')
        .and_then(|t| t.rfind('}').map(|i| &t[..i]))
        .ok_or_else(|| err(path, line_no, "unclosed `{` in inline table"))?
        .trim();
    let mut map = BTreeMap::new();
    if inner.is_empty() {
        return Ok(map);
    }
    // Split on `,` while respecting quoted strings.
    for entry in split_inline_entries(inner) {
        let entry = entry.trim();
        if entry.is_empty() {
            continue;
        }
        let (k, v_str) = entry
            .split_once('=')
            .ok_or_else(|| err(path, line_no, "expected `=` inside inline table"))?;
        let k = k.trim();
        let v_str = v_str.trim();
        if v_str.starts_with('{') {
            return Err(err(path, line_no, "nested inline table (depth > 1) is not supported"));
        }
        let value = parse_value(v_str, path, line_no)?;
        map.insert(k.to_owned(), value);
    }
    Ok(map)
}

/// Splits an inline-table or array body on `,`, skipping commas inside
/// `"..."` strings and inside nested `[...]` brackets (so a
/// `features = ["a", "b"]` value stays one inline-table entry).
fn split_inline_entries(s: &str) -> Vec<&str> {
    let bytes = s.as_bytes();
    let mut parts = Vec::new();
    let mut start = 0usize;
    let mut in_string = false;
    let mut bracket_depth = 0usize;
    let mut i = 0usize;
    while i < bytes.len() {
        match bytes[i] {
            b'"' => in_string = !in_string,
            b'\\' if in_string => i += 1,
            b'[' if !in_string => bracket_depth += 1,
            b']' if !in_string => bracket_depth = bracket_depth.saturating_sub(1),
            b',' if !in_string && bracket_depth == 0 => {
                parts.push(&s[start..i]);
                start = i + 1;
            }
            _ => {}
        }
        i += 1;
    }
    parts.push(&s[start..]);
    parts
}

#[cfg(test)]
mod toml_tests {
    use std::path::Path;

    use super::{TomlValue, parse_text};

    fn fake_path() -> &'static Path {
        Path::new("test.toml")
    }

    // ── happy-path ────────────────────────────────────────────────────────

    #[test]
    fn parses_package_name() {
        let doc = parse_text(
            r#"
[package]
name = "reovim-depgraph"
"#,
            fake_path(),
        )
        .unwrap();
        assert_eq!(doc.get_str("package", "name"), Some("reovim-depgraph"));
    }

    #[test]
    fn parses_dotted_section() {
        let doc = parse_text(
            r#"
[workspace.lints]
foo = "bar"
"#,
            fake_path(),
        )
        .unwrap();
        assert_eq!(doc.get_str("workspace.lints", "foo"), Some("bar"));
    }

    #[test]
    fn parses_bool_values() {
        let doc = parse_text(
            r"
[package]
publish = false
",
            fake_path(),
        )
        .unwrap();
        let val = doc
            .sections
            .get("package")
            .and_then(|s| s.get("publish"))
            .unwrap();
        assert_eq!(val.as_bool(), Some(false));
    }

    #[test]
    fn parses_inline_table() {
        let doc = parse_text(
            r#"
[deps]
serde = { version = "1.0", optional = false }
"#,
            fake_path(),
        )
        .unwrap();
        let tbl = doc
            .sections
            .get("deps")
            .and_then(|s| s.get("serde"))
            .and_then(TomlValue::as_table)
            .unwrap();
        assert_eq!(tbl.get("version").and_then(TomlValue::as_str), Some("1.0"));
        assert_eq!(tbl.get("optional").and_then(TomlValue::as_bool), Some(false));
    }

    #[test]
    fn parses_array_of_tables() {
        let doc = parse_text(
            r#"
[[edge]]
from = "reovim"
to = "reovim-tui"
reason = "embedded"

[[edge]]
from = "apps/cli"
to = "reovim-cli"
reason = "cli"
"#,
            fake_path(),
        )
        .unwrap();
        let edges = doc.array("edge");
        assert_eq!(edges.len(), 2);
        assert_eq!(edges[0].get("from").map(String::as_str), Some("reovim"));
        assert_eq!(edges[1].get("to").map(String::as_str), Some("reovim-cli"));
    }

    #[test]
    fn strips_comments_and_blank_lines() {
        let doc = parse_text(
            r#"
# top comment
[package]
# mid comment
name = "foo" # trailing comment
"#,
            fake_path(),
        )
        .unwrap();
        assert_eq!(doc.get_str("package", "name"), Some("foo"));
    }

    #[test]
    fn hash_inside_string_is_not_a_comment() {
        let doc = parse_text(
            r#"
[package]
name = "foo#bar"
"#,
            fake_path(),
        )
        .unwrap();
        assert_eq!(doc.get_str("package", "name"), Some("foo#bar"));
    }

    #[test]
    fn empty_inline_table_is_ok() {
        let doc = parse_text(
            r"
[s]
k = {}
",
            fake_path(),
        )
        .unwrap();
        let tbl = doc
            .sections
            .get("s")
            .and_then(|s| s.get("k"))
            .and_then(TomlValue::as_table)
            .unwrap();
        assert!(tbl.is_empty());
    }

    // ── error paths (all must produce ProbeError::Parse with line number) ─

    #[test]
    fn unclosed_quote_is_parse_error() {
        let err = parse_text(
            r#"
[package]
name = "unclosed
"#,
            fake_path(),
        )
        .unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("line 3"), "{msg}");
        assert!(msg.contains("unclosed"), "{msg}");
    }

    #[test]
    fn unclosed_bracket_section_is_parse_error() {
        let err = parse_text("[package\nname = \"x\"\n", fake_path()).unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("line 1"), "{msg}");
        assert!(msg.contains("unclosed"), "{msg}");
    }

    #[test]
    fn unclosed_array_header_is_parse_error() {
        let err = parse_text("[[edge\nfrom = \"x\"\n", fake_path()).unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("line 1"), "{msg}");
    }

    #[test]
    fn unclosed_inline_table_is_parse_error() {
        let err = parse_text(
            r#"
[s]
k = { version = "1.0"
"#,
            fake_path(),
        )
        .unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("line 3"), "{msg}");
        assert!(msg.contains("unclosed"), "{msg}");
    }

    #[test]
    fn nested_inline_table_is_parse_error() {
        let err = parse_text(
            r#"
[s]
k = { a = { b = "c" } }
"#,
            fake_path(),
        )
        .unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("line 3"), "{msg}");
        assert!(msg.contains("nested"), "{msg}");
    }

    #[test]
    fn table_depth_beyond_two_is_parse_error() {
        let err = parse_text("[a.b.c]\nk = \"v\"\n", fake_path()).unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("line 1"), "{msg}");
        assert!(msg.contains("depth"), "{msg}");
    }

    #[test]
    fn non_string_value_where_string_required_is_parse_error() {
        // An integer literal (non-string, non-bool, non-inline-table) is unsupported.
        let err = parse_text(
            r"
[s]
k = 42
",
            fake_path(),
        )
        .unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("line 3"), "{msg}");
        assert!(msg.contains("not supported"), "{msg}");
    }

    #[test]
    fn duplicate_table_header_is_parse_error() {
        let err = parse_text("[package]\nname = \"a\"\n[package]\nname = \"b\"\n", fake_path())
            .unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("line 3"), "{msg}");
        assert!(msg.contains("duplicate"), "{msg}");
    }

    #[test]
    fn array_table_field_non_string_is_parse_error() {
        // Inside [[array]], all field values must be strings.
        let err = parse_text("[[edge]]\nfrom = \"a\"\nreason = { x = \"y\" }\n", fake_path())
            .unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("line 3"), "{msg}");
        assert!(msg.contains("string"), "{msg}");
    }

    // ── as_bool / as_table None arms (lines 106, 116) ────────────────────────

    #[test]
    fn as_bool_returns_none_for_non_bool_variants() {
        assert_eq!(TomlValue::String("x".to_owned()).as_bool(), None);
        assert_eq!(TomlValue::InlineTable(std::collections::BTreeMap::new()).as_bool(), None);
    }

    #[test]
    fn as_table_returns_none_for_non_table_variants() {
        assert_eq!(TomlValue::String("x".to_owned()).as_table(), None);
        assert_eq!(TomlValue::Bool(true).as_table(), None);
    }

    // ── parse_file IO error arm (lines 129-131) ───────────────────────────────

    #[test]
    fn parse_file_on_missing_path_returns_io_error() {
        use {super::parse_file, crate::ProbeError};
        let missing = std::path::Path::new("/tmp/reovim-depgraph-toml-no-such-file.toml");
        let err = parse_file(missing).unwrap_err();
        assert!(
            matches!(err, ProbeError::Io { .. }),
            "expected Io error for missing file, got {err:?}"
        );
    }

    // ── root-scope kv insertion (lines 184-186) ───────────────────────────────

    #[test]
    fn root_scope_kv_before_any_section_is_parse_error() {
        let msg = parse_text("edition = \"2024\"\n", fake_path())
            .unwrap_err()
            .to_string();
        assert!(msg.contains("outside any [table]"), "{msg}");
    }

    // ── unrecognised line shape (line 201) ────────────────────────────────────

    #[test]
    fn unrecognised_line_shape_is_parse_error() {
        // A line that is not empty, not a header, and contains no `=`.
        let err = parse_text("invalid line without equals\n", fake_path()).unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("line 1"), "{msg}");
        assert!(msg.contains("unrecognised"), "{msg}");
    }

    // ── strip_comment escape inside string (line 221) ─────────────────────────

    #[test]
    fn strip_comment_does_not_strip_hash_after_escaped_char_in_string() {
        // A backslash-escaped character inside a string must not confuse the
        // comment stripper; the `#` that follows stays part of the string.
        let doc = parse_text(
            r##"
[package]
name = "foo\"#bar"
"##,
            fake_path(),
        )
        .unwrap();
        // The backslash is inside the string; the `#` after it is NOT a comment.
        assert_eq!(doc.get_str("package", "name"), Some(r##"foo\"#bar"##));
    }

    // ── empty key in parse_kv (line 299) ─────────────────────────────────────

    #[test]
    fn empty_key_is_parse_error() {
        // A line of the form `= "value"` has an empty key.
        let err = parse_text("[s]\n= \"val\"\n", fake_path()).unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("line 2"), "{msg}");
        assert!(msg.contains("empty key"), "{msg}");
    }

    // ── trailing comma in inline table (line 372) ─────────────────────────────

    #[test]
    fn inline_table_trailing_comma_produces_empty_entry_that_is_skipped() {
        // A trailing comma after the last key-value pair produces an empty
        // string after splitting; the `if entry.is_empty() { continue; }` guard
        // at line 372 must handle it without error.
        let doc = parse_text(
            r#"
[s]
k = { a = "x", }
"#,
            fake_path(),
        )
        .unwrap();
        let tbl = doc
            .sections
            .get("s")
            .and_then(|s| s.get("k"))
            .and_then(TomlValue::as_table)
            .unwrap();
        assert_eq!(tbl.get("a").and_then(TomlValue::as_str), Some("x"));
    }

    // ── split_inline_entries backslash inside string (line 399) ──────────────

    #[test]
    fn split_inline_entries_handles_backslash_inside_string() {
        // A backslash inside a quoted value in an inline table must not cause
        // the split to break the entry boundary (line 399 escape arm).
        let doc = parse_text(
            r#"
[s]
k = { path = "a\\b", flag = true }
"#,
            fake_path(),
        )
        .unwrap();
        let tbl = doc
            .sections
            .get("s")
            .and_then(|s| s.get("k"))
            .and_then(TomlValue::as_table)
            .unwrap();
        assert_eq!(tbl.get("path").and_then(TomlValue::as_str), Some(r"a\\b"));
        assert_eq!(tbl.get("flag").and_then(TomlValue::as_bool), Some(true));
    }

    // ── single-line string arrays ─────────────────────────────────────────────

    /// The workspace `members` shape parses as an `Array` of strings.
    #[test]
    fn array_of_strings_parses() {
        let doc =
            parse_text("[workspace]\nmembers = [\"lib/depgraph\", \"uapi/abi\"]\n", fake_path())
                .unwrap();
        let v = doc
            .sections
            .get("workspace")
            .and_then(|s| s.get("members"))
            .unwrap();
        assert_eq!(*v, TomlValue::Array(vec!["lib/depgraph".to_owned(), "uapi/abi".to_owned()]));
    }

    #[test]
    fn empty_array_parses() {
        let doc = parse_text("[w]\nm = []\n", fake_path()).unwrap();
        let v = doc.sections.get("w").and_then(|s| s.get("m")).unwrap();
        assert_eq!(*v, TomlValue::Array(Vec::new()));
    }

    #[test]
    fn array_trailing_comma_is_tolerated() {
        let doc = parse_text("[w]\nm = [\"a\",]\n", fake_path()).unwrap();
        let v = doc.sections.get("w").and_then(|s| s.get("m")).unwrap();
        assert_eq!(*v, TomlValue::Array(vec!["a".to_owned()]));
    }

    #[test]
    fn array_empty_interior_element_is_parse_error() {
        let err = parse_text("[w]\nm = [\"a\",, \"b\"]\n", fake_path()).unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("empty array element"), "{msg}");
    }

    #[test]
    fn array_non_string_element_is_parse_error() {
        let err = parse_text("[w]\nm = [42]\n", fake_path()).unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("unclosed `\"`"), "{msg}");
    }

    #[test]
    fn array_unclosed_bracket_is_parse_error() {
        let err = parse_text("[w]\nm = [\"a\"\n", fake_path()).unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("unclosed `[`"), "{msg}");
    }

    /// Arrays are accepted inside inline tables (the future
    /// `features = [...]` dep shape).
    #[test]
    fn array_inside_inline_table_parses() {
        let doc =
            parse_text("[deps]\nfoo = { path = \"../foo\", features = [\"x\"] }\n", fake_path())
                .unwrap();
        let tbl = doc
            .sections
            .get("deps")
            .and_then(|s| s.get("foo"))
            .and_then(TomlValue::as_table)
            .unwrap();
        assert_eq!(tbl.get("features"), Some(&TomlValue::Array(vec!["x".to_owned()])));
    }

    // ── parse_inline_table missing `=` (line 372) ────────────────────────────
    // This closure (`ok_or_else` for "expected `=` inside inline table") is
    // exercised only by the integration build when reached via `parse_text`
    // from an external test binary.  This unit test closes the unit-build gap.

    #[test]
    fn inline_table_entry_without_equals_is_parse_error() {
        // An inline table entry that contains no `=` sign triggers the
        // `split_once('=').ok_or_else(...)` path inside `parse_inline_table`.
        let err = parse_text("[s]\nk = { noequalssign }\n", fake_path()).unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("expected `=` inside inline table"), "{msg}");
    }

    /// A multi-element array inside an inline table must stay one entry:
    /// the splitter tracks bracket depth, not just quotes.
    #[test]
    fn multi_element_array_inside_inline_table_parses() {
        let doc = parse_text(
            "[deps]\nfoo = { path = \"../foo\", features = [\"x\", \"y\"] }\n",
            fake_path(),
        )
        .unwrap();
        let tbl = doc
            .sections
            .get("deps")
            .and_then(|s| s.get("foo"))
            .and_then(TomlValue::as_table)
            .unwrap();
        assert_eq!(
            tbl.get("features"),
            Some(&TomlValue::Array(vec!["x".to_owned(), "y".to_owned()]))
        );
        assert_eq!(tbl.get("path").and_then(TomlValue::as_str), Some("../foo"));
    }

    /// Byte-class matrix (unit-build twin of the integration test): bare
    /// backslash outside a string; brackets/backslash/comma/hash inside a
    /// quoted inline-table string.
    #[test]
    fn byte_class_matrix() {
        let doc = parse_text(
            "[s]\nk\\ = \"v\"\nt = { p = \"a[1]\\\\,b#c\", q\\w = \"z\" }\n",
            fake_path(),
        )
        .expect("byte-class matrix must parse");
        let sec = doc.sections.get("s").expect("section s");
        assert!(sec.contains_key("k\\"), "bare backslash stays part of the key");
        let t = sec
            .get("t")
            .and_then(TomlValue::as_table)
            .expect("t is a table");
        assert_eq!(t.get("p").and_then(TomlValue::as_str), Some("a[1]\\\\,b#c"));
        assert_eq!(t.get("q\\w").and_then(TomlValue::as_str), Some("z"));
    }
}
