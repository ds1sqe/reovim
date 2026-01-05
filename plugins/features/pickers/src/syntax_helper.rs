//! Syntax highlighting helper for microscope previews

use reovim_core::{highlight::Highlight, syntax::SyntaxFactory};
use reovim_plugin_microscope::microscope::StyledSpan;

/// Compute styled lines for preview content
///
/// Creates a transient syntax provider, parses content, saturates injections,
/// and converts highlights to StyledSpan format.
pub fn compute_styled_lines(
    factory: &dyn SyntaxFactory,
    file_path: &str,
    content: &str,
    lines: &[String],
) -> Option<Vec<Vec<StyledSpan>>> {
    // Create syntax provider for this file
    let mut syntax = factory.create_syntax(file_path, content)?;

    // Parse and saturate language injections (markdown code blocks, etc.)
    syntax.parse(content);
    syntax.saturate_injections(content);

    // Highlight the viewport (all lines in preview)
    let highlights = syntax.highlight_range(content, 0, lines.len() as u32);

    // Convert to StyledSpan format
    Some(convert_highlights_to_spans(highlights, lines))
}

/// Convert Highlight spans to StyledSpan format grouped by line
fn convert_highlights_to_spans(
    highlights: Vec<Highlight>,
    lines: &[String],
) -> Vec<Vec<StyledSpan>> {
    let mut line_spans: Vec<Vec<StyledSpan>> = vec![Vec::new(); lines.len()];

    for hl in highlights {
        let start_line = hl.span.start_line as usize;
        let end_line = hl.span.end_line as usize;

        if start_line == end_line {
            // Single-line highlight
            if start_line < line_spans.len() {
                line_spans[start_line].push(StyledSpan {
                    start: hl.span.start_col as usize,
                    end: hl.span.end_col as usize,
                    style: hl.style,
                });
            }
        } else {
            // Multi-line highlight: split across lines

            // First line: from start_col to end of line
            if start_line < line_spans.len() {
                let line_len = lines.get(start_line).map_or(0, |l| l.len());
                line_spans[start_line].push(StyledSpan {
                    start: hl.span.start_col as usize,
                    end: line_len,
                    style: hl.style.clone(),
                });
            }

            // Middle lines: entire line
            for mid_line in (start_line + 1)..end_line {
                if mid_line < line_spans.len() {
                    let line_len = lines.get(mid_line).map_or(0, |l| l.len());
                    line_spans[mid_line].push(StyledSpan {
                        start: 0,
                        end: line_len,
                        style: hl.style.clone(),
                    });
                }
            }

            // End line: from start to end_col
            if end_line < line_spans.len() {
                line_spans[end_line].push(StyledSpan {
                    start: 0,
                    end: hl.span.end_col as usize,
                    style: hl.style,
                });
            }
        }
    }

    // Sort each line's spans by start position for rendering
    for spans in &mut line_spans {
        spans.sort_by_key(|s| s.start);
    }

    line_spans
}

#[cfg(test)]
mod tests {
    use super::*;
    use reovim_core::highlight::{Span, Style};

    #[test]
    fn test_single_line_highlight() {
        let lines = vec!["fn main() {}".to_string()];
        let highlights = vec![Highlight {
            span: Span {
                start_line: 0,
                start_col: 0,
                end_line: 0,
                end_col: 2,
            },
            style: Style::default(),
        }];

        let spans = convert_highlights_to_spans(highlights, &lines);
        assert_eq!(spans.len(), 1);
        assert_eq!(spans[0].len(), 1);
        assert_eq!(spans[0][0].start, 0);
        assert_eq!(spans[0][0].end, 2);
    }

    #[test]
    fn test_multi_line_highlight() {
        let lines = vec![
            "/* comment".to_string(),
            "   continues".to_string(),
            "   ends */".to_string(),
        ];
        let highlights = vec![Highlight {
            span: Span {
                start_line: 0,
                start_col: 0,
                end_line: 2,
                end_col: 10,
            },
            style: Style::default(),
        }];

        let spans = convert_highlights_to_spans(highlights, &lines);
        assert_eq!(spans.len(), 3);
        assert_eq!(spans[0][0].start, 0);
        assert_eq!(spans[0][0].end, 10); // Full first line
        assert_eq!(spans[1][0].start, 0);
        assert_eq!(spans[1][0].end, 12); // Full middle line
        assert_eq!(spans[2][0].start, 0);
        assert_eq!(spans[2][0].end, 10); // Up to end_col
    }
}
