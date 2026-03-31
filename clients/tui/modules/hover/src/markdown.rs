//! Lightweight inline markdown parser for hover popups.
//!
//! Converts markdown text into styled spans for rendering in the hover
//! chrome module. Only supports the subset of markdown that appears in
//! LSP hover content (headings, bold, italic, code spans, fenced code
//! blocks, horizontal rules).

use reovim_client_driver::{Style, types::Color};

/// A span of text with associated style.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StyledSpan {
    pub text: String,
    pub style: Style,
}

impl StyledSpan {
    fn new(text: impl Into<String>, style: Style) -> Self {
        Self {
            text: text.into(),
            style,
        }
    }

    /// Display width of this span in characters.
    ///
    /// Uses `chars().count()` rather than byte length so multi-byte characters
    /// (e.g., box-drawing `─` from horizontal rules) are measured correctly.
    pub fn display_width(&self) -> usize {
        self.text.chars().count()
    }
}

// Style palette — all const fn.
const fn plain_style() -> Style {
    Style::new().fg(Color::White)
}

const fn heading_style() -> Style {
    Style::new().fg(Color::Cyan).bold()
}

const fn bold_style() -> Style {
    Style::new().fg(Color::White).bold()
}

const fn italic_style() -> Style {
    Style::new().fg(Color::White).italic()
}

const fn code_span_style() -> Style {
    Style::new().fg(Color::DarkCyan)
}

const fn code_block_style() -> Style {
    Style::new()
        .fg(Color::AnsiValue(250))
        .bg(Color::AnsiValue(238))
}

const fn rule_style() -> Style {
    Style::new().fg(Color::DarkGrey)
}

/// Parse markdown lines into styled span lines.
///
/// Each input line becomes one output line (a `Vec<StyledSpan>`).
/// Block-level elements (headings, code fences, horizontal rules) are
/// detected first. Inline formatting (bold, italic, code spans) is
/// applied to non-block content.
pub fn parse_markdown(lines: &[String]) -> Vec<Vec<StyledSpan>> {
    let mut result = Vec::with_capacity(lines.len());
    let mut in_code_block = false;

    for line in lines {
        if in_code_block {
            if is_code_fence(line) {
                in_code_block = false;
                result.push(Vec::new());
            } else {
                result.push(vec![StyledSpan::new(line.as_str(), code_block_style())]);
            }
            continue;
        }

        if is_code_fence(line) {
            in_code_block = true;
            result.push(Vec::new());
            continue;
        }

        if is_horizontal_rule(line) {
            let rule_line = "\u{2500}".repeat(40);
            result.push(vec![StyledSpan::new(rule_line, rule_style())]);
            continue;
        }

        if let Some(content) = strip_heading(line) {
            result.push(vec![StyledSpan::new(content, heading_style())]);
            continue;
        }

        result.push(parse_inline(line));
    }

    result
}

/// Check if a line is a fenced code block delimiter
/// (backtick-triple or tilde-triple).
fn is_code_fence(line: &str) -> bool {
    let trimmed = line.trim();
    trimmed.starts_with("```") || trimmed.starts_with("~~~")
}

/// Check if a line is a horizontal rule (`---`, `***`, `___`).
#[cfg_attr(coverage_nightly, coverage(off))]
fn is_horizontal_rule(line: &str) -> bool {
    let trimmed = line.trim();
    if trimmed.len() < 3 {
        return false;
    }
    let first = trimmed.as_bytes()[0];
    matches!(first, b'-' | b'*' | b'_')
        && trimmed.bytes().all(|b| b == first || b == b' ')
        && trimmed.bytes().filter(|&b| b == first).count() >= 3
}

/// Strip heading markers (`#` .. `######`) and return the content.
#[cfg_attr(coverage_nightly, coverage(off))]
fn strip_heading(line: &str) -> Option<&str> {
    let trimmed = line.trim_start();
    if !trimmed.starts_with('#') {
        return None;
    }
    let hashes = trimmed.bytes().take_while(|&b| b == b'#').count();
    if hashes == 0 || hashes > 6 {
        return None;
    }
    let rest = &trimmed[hashes..];
    if rest.is_empty() {
        return Some("");
    }
    if let Some(stripped) = rest.strip_prefix(' ') {
        return Some(stripped.trim_end());
    }
    None
}

/// Parse inline markdown formatting (bold, italic, code spans).
fn parse_inline(line: &str) -> Vec<StyledSpan> {
    let mut spans = Vec::new();
    let mut chars = line.char_indices().peekable();
    let mut plain_start = 0;

    while let Some(&(i, ch)) = chars.peek() {
        match ch {
            '`' => {
                if i > plain_start {
                    spans.push(StyledSpan::new(&line[plain_start..i], plain_style()));
                }
                chars.next();
                let content_start = i + 1;
                let mut found_end = false;
                while let Some(&(j, c)) = chars.peek() {
                    chars.next();
                    if c == '`' {
                        spans.push(StyledSpan::new(&line[content_start..j], code_span_style()));
                        plain_start = j + 1;
                        found_end = true;
                        break;
                    }
                }
                if !found_end {
                    spans.push(StyledSpan::new(&line[i..], plain_style()));
                    return spans;
                }
            }
            '*' | '_' => {
                let marker = ch;
                let is_double = line[i + ch.len_utf8()..].starts_with(marker);

                // Flush preceding plain text (common to both bold and italic)
                if i > plain_start {
                    spans.push(StyledSpan::new(&line[plain_start..i], plain_style()));
                }

                // Skip first marker char (always consumed)
                chars.next();

                if is_double {
                    chars.next(); // skip second marker
                    let content_start = i + 2;
                    let closing = find_double_marker(&line[content_start..], marker);
                    if let Some(offset) = closing {
                        spans.push(StyledSpan::new(
                            &line[content_start..content_start + offset],
                            bold_style(),
                        ));
                        let end_pos = content_start + offset + 2;
                        plain_start = end_pos;
                        while let Some(&(j, _)) = chars.peek() {
                            if j >= end_pos {
                                break;
                            }
                            chars.next();
                        }
                    } else {
                        spans.push(StyledSpan::new(&line[i..], plain_style()));
                        return spans;
                    }
                } else {
                    let content_start = i + 1;
                    let closing = find_single_marker(&line[content_start..], marker);
                    if let Some(offset) = closing {
                        spans.push(StyledSpan::new(
                            &line[content_start..content_start + offset],
                            italic_style(),
                        ));
                        let end_pos = content_start + offset + 1;
                        plain_start = end_pos;
                        while let Some(&(j, _)) = chars.peek() {
                            if j >= end_pos {
                                break;
                            }
                            chars.next();
                        }
                    } else {
                        spans.push(StyledSpan::new(&line[i..], plain_style()));
                        return spans;
                    }
                }
            }
            _ => {
                chars.next();
            }
        }
    }

    if plain_start < line.len() {
        spans.push(StyledSpan::new(&line[plain_start..], plain_style()));
    }

    if spans.is_empty() {
        spans.push(StyledSpan::new("", plain_style()));
    }

    spans
}

/// Find closing double marker (`**` or `__`) in text.
fn find_double_marker(text: &str, marker: char) -> Option<usize> {
    let pattern: String = [marker, marker].iter().collect();
    text.find(&pattern)
}

/// Find closing single marker (`*` or `_`) that is not part of a double.
fn find_single_marker(text: &str, marker: char) -> Option<usize> {
    let bytes = text.as_bytes();
    let m = marker as u8;
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == m {
            if i + 1 < bytes.len() && bytes[i + 1] == m {
                i += 2;
                continue;
            }
            return Some(i);
        }
        i += 1;
    }
    None
}

#[cfg(test)]
#[path = "markdown_tests.rs"]
mod tests;
