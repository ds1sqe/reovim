//! Recursive descent snippet parser (#136).
//!
//! Handles the full TextMate/LSP snippet grammar:
//! - Tab stops: `$N`, `${N}`, `${N/regex/replace/flags}`
//! - Placeholders: `${N:default}`, `${N:${M:nested}}`
//! - Variables: `$VAR`, `${VAR}`, `${VAR:default}`, `${VAR/regex/replace/flags}`
//! - Choices: `${N|a,b,c|}`
//! - Transforms with captures, case modifiers, and conditionals
//! - Escape sequences: `\$`, `\}`, `\\`
//!
//! Unrecognized `$` sequences are treated as literal text (graceful fallback).
//!
//! # Error Strategy
//!
//! Following Helix: unparseable syntax gracefully becomes `Text` nodes
//! rather than hard errors. This ensures forward compatibility with
//! LSP snippets using features not yet implemented.

use std::fmt;

use crate::ast::{CaseModifier, FormatItem, SnippetBody, SnippetElement, TabStopId, Transform};

/// Parse error for snippet body text.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ParseError {
    /// Unexpected end of input while parsing a construct.
    UnexpectedEof {
        /// What was being parsed when EOF occurred.
        context: &'static str,
    },
    /// Tab stop ID could not be parsed as a number.
    InvalidTabStopId {
        /// The text that could not be parsed.
        found: String,
    },
}

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnexpectedEof { context } => {
                write!(f, "unexpected end of input while parsing {context}")
            }
            Self::InvalidTabStopId { found } => {
                write!(f, "invalid tab stop id: {found:?}")
            }
        }
    }
}

/// Parse a snippet body string into a `SnippetBody`.
///
/// # Errors
///
/// Returns `ParseError` if the input contains a malformed construct
/// that cannot be recovered from (e.g., `${` without closing `}`).
pub fn parse(input: &str) -> Result<SnippetBody, ParseError> {
    let mut parser = Parser::new(input);
    let elements = parser.parse_elements(None)?;
    Ok(SnippetBody::new(merge_adjacent_text(elements)))
}

/// Internal parser state.
struct Parser<'a> {
    /// Input bytes.
    input: &'a [u8],
    /// Current position.
    pos: usize,
}

impl<'a> Parser<'a> {
    const fn new(input: &'a str) -> Self {
        Self {
            input: input.as_bytes(),
            pos: 0,
        }
    }

    /// Peek at the current byte without advancing.
    fn peek(&self) -> Option<u8> {
        self.input.get(self.pos).copied()
    }

    /// Advance and return the current byte.
    fn advance(&mut self) -> Option<u8> {
        let ch = self.input.get(self.pos).copied()?;
        self.pos += 1;
        Some(ch)
    }

    /// Parse elements until `stop_char` is encountered or EOF.
    /// If `stop_char` is `None`, parse until EOF.
    fn parse_elements(&mut self, stop_char: Option<u8>) -> Result<Vec<SnippetElement>, ParseError> {
        let mut elements = Vec::new();
        let mut text_buf = String::new();

        while let Some(ch) = self.peek() {
            if stop_char == Some(ch) {
                // Don't consume the stop character; caller handles it
                break;
            }

            match ch {
                b'\\' => {
                    // Escape sequence
                    self.advance(); // consume backslash
                    match self.advance() {
                        Some(b'$') => text_buf.push('$'),
                        Some(b'}') => text_buf.push('}'),
                        Some(b'\\') => text_buf.push('\\'),
                        Some(other) => {
                            // Unknown escape: emit both characters literally
                            text_buf.push('\\');
                            text_buf.push(char::from(other));
                        }
                        None => {
                            // Trailing backslash: emit literally
                            text_buf.push('\\');
                        }
                    }
                }
                b'$' => {
                    // Flush text buffer before processing dollar
                    if !text_buf.is_empty() {
                        elements.push(SnippetElement::Text(std::mem::take(&mut text_buf)));
                    }

                    match self.parse_dollar()? {
                        Some(elem) => elements.push(elem),
                        None => {
                            // Lone `$` at end: treat as literal
                            text_buf.push('$');
                        }
                    }
                }
                _ => {
                    self.advance();
                    text_buf.push(char::from(ch));
                }
            }
        }

        // Flush remaining text
        if !text_buf.is_empty() {
            elements.push(SnippetElement::Text(text_buf));
        }

        Ok(elements)
    }

    /// Parse a `$` construct. The `$` has NOT been consumed yet.
    fn parse_dollar(&mut self) -> Result<Option<SnippetElement>, ParseError> {
        self.advance(); // consume `$`

        match self.peek() {
            Some(b'{') => {
                self.advance(); // consume `{`
                self.parse_braced()
            }
            Some(ch) if ch.is_ascii_digit() => {
                // Simple tab stop: $0, $1, ..., $9
                self.advance();
                let id = TabStopId::from(ch - b'0');
                Ok(Some(SnippetElement::TabStop {
                    id,
                    transform: None,
                }))
            }
            Some(ch) if is_var_start(ch) => {
                // Simple variable: $VAR_NAME
                let name = self.read_identifier();
                Ok(Some(SnippetElement::Variable {
                    name,
                    default: None,
                    transform: None,
                }))
            }
            _ => {
                // Unknown $ sequence: treat $ as literal text
                Ok(None)
            }
        }
    }

    /// Parse the content inside `${...}`. The `${` has been consumed.
    fn parse_braced(&mut self) -> Result<Option<SnippetElement>, ParseError> {
        // Check first character to decide: digit → tab stop, letter/_ → variable.
        match self.peek() {
            Some(ch) if ch.is_ascii_digit() => self.parse_braced_tabstop(),
            Some(ch) if is_var_start(ch) => self.parse_braced_variable(),
            _ => Ok(Some(self.fallback_braced(""))),
        }
    }

    /// Parse braced tab stop: `${N}`, `${N:default}`, `${N/re/rep/flags}`,
    /// `${N|a,b,c|}`. The `${` has been consumed.
    fn parse_braced_tabstop(&mut self) -> Result<Option<SnippetElement>, ParseError> {
        let id_str = self.read_digits();
        let id: TabStopId = id_str
            .parse()
            .map_err(|_| ParseError::InvalidTabStopId { found: id_str })?;

        match self.peek() {
            Some(b'}') => {
                self.advance(); // consume `}`
                Ok(Some(SnippetElement::TabStop {
                    id,
                    transform: None,
                }))
            }
            Some(b':') => {
                self.advance(); // consume `:`
                self.parse_placeholder(id)
            }
            Some(b'/') => {
                // Transform: ${N/regex/replacement/options}
                let transform = self.parse_transform()?;
                Ok(Some(SnippetElement::TabStop {
                    id,
                    transform: Some(transform),
                }))
            }
            Some(b'|') => {
                // Choice: ${N|a,b,c|}
                self.parse_choice(id)
            }
            Some(_) | None => {
                // Unrecognized syntax after number,
                // consume rest to `}` and treat as text
                let id_text = id.to_string();
                Ok(Some(self.fallback_braced(&id_text)))
            }
        }
    }

    /// Parse braced variable: `${VAR}`, `${VAR:default}`, `${VAR/re/rep/flags}`.
    /// The `${` has been consumed.
    fn parse_braced_variable(&mut self) -> Result<Option<SnippetElement>, ParseError> {
        let name = self.read_identifier();

        match self.peek() {
            Some(b'}') => {
                self.advance(); // consume `}`
                Ok(Some(SnippetElement::Variable {
                    name,
                    default: None,
                    transform: None,
                }))
            }
            Some(b':') => {
                self.advance(); // consume `:`
                let body = self.parse_elements(Some(b'}'))?;
                match self.peek() {
                    Some(b'}') => {
                        self.advance(); // consume `}`
                        let default = if body.is_empty() { None } else { Some(body) };
                        Ok(Some(SnippetElement::Variable {
                            name,
                            default,
                            transform: None,
                        }))
                    }
                    _ => Err(ParseError::UnexpectedEof {
                        context: "variable default",
                    }),
                }
            }
            Some(b'/') => {
                // Transform: ${VAR/regex/replacement/options}
                let transform = self.parse_transform()?;
                Ok(Some(SnippetElement::Variable {
                    name,
                    default: None,
                    transform: Some(transform),
                }))
            }
            Some(_) | None => {
                // Unrecognized syntax after variable name.
                // Fallback to literal text.
                Ok(Some(self.fallback_braced(&name)))
            }
        }
    }

    /// Parse a placeholder body: `${N:...}`. The `${N:` has been consumed.
    fn parse_placeholder(&mut self, id: TabStopId) -> Result<Option<SnippetElement>, ParseError> {
        let body = self.parse_elements(Some(b'}'))?;

        match self.peek() {
            Some(b'}') => {
                self.advance(); // consume `}`
                Ok(Some(SnippetElement::Placeholder { id, body }))
            }
            _ => Err(ParseError::UnexpectedEof {
                context: "placeholder",
            }),
        }
    }

    /// Parse a transform: `/regex/replacement/options}`.
    /// The current position is at the `/` before the regex.
    fn parse_transform(&mut self) -> Result<Transform, ParseError> {
        self.advance(); // consume first `/`

        // Read regex (until unescaped `/`)
        let regex = self.read_transform_segment()?;

        // Read replacement items (until unescaped `/`)
        let replacement = self.parse_replacement()?;

        // Read options (until `}`)
        let mut options = String::new();
        while let Some(ch) = self.peek() {
            if ch == b'}' {
                self.advance(); // consume `}`
                break;
            }
            self.advance();
            options.push(char::from(ch));
        }

        Ok(Transform {
            regex,
            replacement,
            options,
        })
    }

    /// Read a transform segment (regex or options) until unescaped `/`.
    fn read_transform_segment(&mut self) -> Result<String, ParseError> {
        let mut segment = String::new();
        loop {
            match self.peek() {
                Some(b'/') => {
                    self.advance(); // consume `/`
                    return Ok(segment);
                }
                Some(b'\\') => {
                    self.advance(); // consume `\`
                    match self.advance() {
                        Some(b'/') => segment.push('/'),
                        Some(b'\\') => segment.push('\\'),
                        Some(other) => {
                            segment.push('\\');
                            segment.push(char::from(other));
                        }
                        None => {
                            segment.push('\\');
                            return Err(ParseError::UnexpectedEof {
                                context: "transform regex",
                            });
                        }
                    }
                }
                Some(ch) => {
                    self.advance();
                    segment.push(char::from(ch));
                }
                None => {
                    return Err(ParseError::UnexpectedEof {
                        context: "transform regex",
                    });
                }
            }
        }
    }

    /// Parse replacement items until unescaped `/`.
    /// Handles `$N`, `${N}`, `${N:/modifier}`, `${N:+if}`, `${N:-else}`,
    /// `${N:?if:else}`.
    fn parse_replacement(&mut self) -> Result<Vec<FormatItem>, ParseError> {
        let mut items = Vec::new();
        let mut text_buf = String::new();

        loop {
            match self.peek() {
                Some(b'/') => {
                    self.advance(); // consume `/`
                    if !text_buf.is_empty() {
                        items.push(FormatItem::Text(text_buf));
                    }
                    return Ok(items);
                }
                Some(b'\\') => {
                    self.advance(); // consume `\`
                    match self.advance() {
                        Some(b'/') => text_buf.push('/'),
                        Some(b'\\') => text_buf.push('\\'),
                        Some(b'$') => text_buf.push('$'),
                        Some(other) => {
                            text_buf.push('\\');
                            text_buf.push(char::from(other));
                        }
                        None => {
                            text_buf.push('\\');
                            return Err(ParseError::UnexpectedEof {
                                context: "transform replacement",
                            });
                        }
                    }
                }
                Some(b'$') => {
                    if !text_buf.is_empty() {
                        items.push(FormatItem::Text(std::mem::take(&mut text_buf)));
                    }
                    self.advance(); // consume `$`
                    match self.peek() {
                        Some(ch) if ch.is_ascii_digit() => {
                            let digits = self.read_digits();
                            if let Ok(n) = digits.parse::<usize>() {
                                items.push(FormatItem::Capture(n));
                            }
                        }
                        Some(b'{') => {
                            self.advance(); // consume `{`
                            if let Some(item) = self.parse_format_item() {
                                items.push(item);
                            }
                        }
                        _ => {
                            text_buf.push('$');
                        }
                    }
                }
                Some(ch) => {
                    self.advance();
                    text_buf.push(char::from(ch));
                }
                None => {
                    return Err(ParseError::UnexpectedEof {
                        context: "transform replacement",
                    });
                }
            }
        }
    }

    /// Parse a format item inside `${...}` in a replacement string.
    /// Handles `${N}`, `${N:/modifier}`, `${N:+if}`, `${N:-else}`, `${N:?if:else}`.
    fn parse_format_item(&mut self) -> Option<FormatItem> {
        let digits = self.read_digits();
        let Ok(n) = digits.parse::<usize>() else {
            // Not a valid number — skip to `}` and ignore
            while let Some(ch) = self.advance() {
                if ch == b'}' {
                    break;
                }
            }
            return None;
        };

        match self.peek() {
            Some(b'}') => {
                self.advance();
                Some(FormatItem::Capture(n))
            }
            Some(b':') => {
                self.advance(); // consume `:`
                self.parse_format_modifier(n)
            }
            _ => {
                // Skip to `}` and treat as simple capture
                while let Some(ch) = self.advance() {
                    if ch == b'}' {
                        break;
                    }
                }
                Some(FormatItem::Capture(n))
            }
        }
    }

    /// Parse a format modifier after `${N:`.
    /// Handles `${N:/upcase}`, `${N:+if}`, `${N:-else}`, `${N:?if:else}`.
    fn parse_format_modifier(&mut self, capture: usize) -> Option<FormatItem> {
        match self.peek() {
            Some(b'/') => {
                self.advance(); // consume `/`
                let modifier_name = self.read_until(b'}');
                self.advance(); // consume `}`
                let modifier = parse_case_modifier(&modifier_name);
                modifier.map(|m| FormatItem::CaseChange(capture, m))
            }
            Some(b'+') => {
                self.advance(); // consume `+`
                let if_text = self.read_until(b'}');
                self.advance(); // consume `}`
                Some(FormatItem::Conditional {
                    capture,
                    if_text,
                    else_text: String::new(),
                })
            }
            Some(b'-') => {
                self.advance(); // consume `-`
                let else_text = self.read_until(b'}');
                self.advance(); // consume `}`
                Some(FormatItem::Conditional {
                    capture,
                    if_text: String::new(),
                    else_text,
                })
            }
            Some(b'?') => {
                self.advance(); // consume `?`
                let if_text = self.read_until(b':');
                self.advance(); // consume `:`
                let else_text = self.read_until(b'}');
                self.advance(); // consume `}`
                Some(FormatItem::Conditional {
                    capture,
                    if_text,
                    else_text,
                })
            }
            _ => {
                // Unknown modifier — skip to `}` and treat as capture
                while let Some(ch) = self.advance() {
                    if ch == b'}' {
                        break;
                    }
                }
                Some(FormatItem::Capture(capture))
            }
        }
    }

    /// Read characters until `stop` byte, without consuming the stop byte.
    fn read_until(&mut self, stop: u8) -> String {
        let mut result = String::new();
        while let Some(ch) = self.peek() {
            if ch == stop {
                return result;
            }
            self.advance();
            result.push(char::from(ch));
        }
        result
    }

    /// Parse a choice: `${N|a,b,c|}`. The `${N` has been consumed, position is at `|`.
    fn parse_choice(&mut self, id: TabStopId) -> Result<Option<SnippetElement>, ParseError> {
        self.advance(); // consume `|`

        let mut choices = Vec::new();
        let mut current = String::new();

        loop {
            match self.peek() {
                Some(b'|') => {
                    self.advance(); // consume `|`
                    // Must be followed by `}`
                    match self.peek() {
                        Some(b'}') => {
                            self.advance(); // consume `}`
                            if !current.is_empty() {
                                choices.push(current);
                            }
                            return Ok(Some(SnippetElement::Choice { id, choices }));
                        }
                        _ => {
                            // Stray `|` — treat as part of text
                            current.push('|');
                        }
                    }
                }
                Some(b',') => {
                    self.advance(); // consume `,`
                    choices.push(std::mem::take(&mut current));
                }
                Some(b'\\') => {
                    self.advance(); // consume `\`
                    match self.advance() {
                        Some(b',') => current.push(','),
                        Some(b'|') => current.push('|'),
                        Some(b'\\') => current.push('\\'),
                        Some(other) => {
                            current.push('\\');
                            current.push(char::from(other));
                        }
                        None => {
                            current.push('\\');
                            return Err(ParseError::UnexpectedEof { context: "choice" });
                        }
                    }
                }
                Some(ch) => {
                    self.advance();
                    current.push(char::from(ch));
                }
                None => {
                    return Err(ParseError::UnexpectedEof { context: "choice" });
                }
            }
        }
    }

    /// Read consecutive ASCII digits from current position.
    fn read_digits(&mut self) -> String {
        let mut digits = String::new();
        while let Some(ch) = self.peek() {
            if ch.is_ascii_digit() {
                digits.push(char::from(ch));
                self.advance();
            } else {
                break;
            }
        }
        digits
    }

    /// Read an identifier: `[a-zA-Z_][a-zA-Z0-9_]*`.
    fn read_identifier(&mut self) -> String {
        let mut name = String::new();
        while let Some(ch) = self.peek() {
            if ch.is_ascii_alphanumeric() || ch == b'_' {
                name.push(char::from(ch));
                self.advance();
            } else {
                break;
            }
        }
        name
    }

    /// Consume characters until `}` and emit the whole `${...}` as literal text.
    /// Used for unrecognized braced constructs (graceful fallback).
    fn fallback_braced(&mut self, prefix: &str) -> SnippetElement {
        let mut content = format!("${{{prefix}");
        while let Some(ch) = self.peek() {
            self.advance();
            content.push(char::from(ch));
            if ch == b'}' {
                return SnippetElement::Text(content);
            }
        }
        // No closing brace found: emit what we have as text
        SnippetElement::Text(content)
    }
}

/// Parse a case modifier name to a `CaseModifier`.
fn parse_case_modifier(name: &str) -> Option<CaseModifier> {
    match name {
        "upcase" => Some(CaseModifier::Upcase),
        "downcase" => Some(CaseModifier::Downcase),
        "capitalize" => Some(CaseModifier::Capitalize),
        "camelcase" => Some(CaseModifier::CamelCase),
        "pascalcase" => Some(CaseModifier::PascalCase),
        "snakecase" => Some(CaseModifier::SnakeCase),
        "kebabcase" => Some(CaseModifier::KebabCase),
        _ => None,
    }
}

/// Check if a byte is a valid variable name start character (letter or underscore).
const fn is_var_start(ch: u8) -> bool {
    ch.is_ascii_alphabetic() || ch == b'_'
}

/// Merge adjacent `Text` elements into single elements.
fn merge_adjacent_text(elements: Vec<SnippetElement>) -> Vec<SnippetElement> {
    let mut result: Vec<SnippetElement> = Vec::with_capacity(elements.len());

    for elem in elements {
        match (&mut result.last_mut(), &elem) {
            (Some(SnippetElement::Text(existing)), SnippetElement::Text(new)) => {
                existing.push_str(new);
            }
            _ => result.push(elem),
        }
    }

    result
}

#[cfg(test)]
#[path = "parser_tests.rs"]
mod tests;
