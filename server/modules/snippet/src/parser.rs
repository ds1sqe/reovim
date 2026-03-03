//! Recursive descent snippet parser (#136).
//!
//! Phase 1 handles: `$N`, `${N}`, `${N:default}`, `${N:${M:nested}}`,
//! escape sequences (`\$`, `\}`, `\\`), and graceful fallback for
//! unrecognized `$` sequences (treated as literal text).
//!
//! # Error Strategy
//!
//! Following Helix: unparseable syntax gracefully becomes `Text` nodes
//! rather than hard errors. This ensures forward compatibility with
//! LSP snippets using features not yet implemented.

use std::fmt;

use crate::ast::{SnippetBody, SnippetElement, TabStopId};

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
            _ => {
                // Unknown $ sequence: treat $ as literal text
                Ok(None)
            }
        }
    }

    /// Parse the content inside `${...}`. The `${` has been consumed.
    fn parse_braced(&mut self) -> Result<Option<SnippetElement>, ParseError> {
        // Read the number
        let id_str = self.read_digits();
        if id_str.is_empty() {
            // Not a number - could be a variable name. For now, treat as literal.
            // Consume up to closing `}` and emit as text.
            return Ok(Some(self.fallback_braced(&id_str)));
        }

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
            Some(_) | None => {
                // Unrecognized syntax after number (e.g., `${1|...}` choice),
                // consume rest to `}` and treat as text
                let id_text = id.to_string();
                Ok(Some(self.fallback_braced(&id_text)))
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
#[allow(clippy::literal_string_with_formatting_args)]
mod tests {
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
        // Unknown: $x → literal "$" then "x"
        let body = parse("$x").unwrap();
        assert_eq!(body.elements(), &[SnippetElement::Text("$x".to_string())]);
    }

    #[test]
    fn test_parse_braced_non_numeric() {
        // ${foo} is a variable name in the full grammar;
        // Phase 1 treats it as literal text (fallback)
        let body = parse("${foo}").unwrap();
        assert_eq!(body.elements(), &[SnippetElement::Text("${foo}".to_string())]);
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
    fn test_parse_braced_choice_syntax_fallback() {
        // ${1|one,two|} — choice syntax triggers Some(_) fallback in parse_braced
        let body = parse("${1|one,two|}").unwrap();
        // Phase 1 treats this as literal text (fallback)
        assert_eq!(
            body.elements(),
            &[SnippetElement::Text("${1|one,two|}".to_string())]
        );
    }

    #[test]
    fn test_parse_braced_unknown_char_after_number() {
        // ${1x} — 'x' after number triggers Some(_) in parse_braced
        let body = parse("${1x}").unwrap();
        assert_eq!(
            body.elements(),
            &[SnippetElement::Text("${1x}".to_string())]
        );
    }

    #[test]
    fn test_parse_braced_no_closing_brace() {
        // ${foo — non-numeric, no closing brace
        let body = parse("${foo").unwrap();
        assert_eq!(
            body.elements(),
            &[SnippetElement::Text("${foo".to_string())]
        );
    }
}
