use super::*;

// ============================================================================
// Helpers
// ============================================================================

fn parse(lines: &[&str]) -> Vec<Vec<StyledSpan>> {
    let owned: Vec<String> = lines.iter().map(|s| String::from(*s)).collect();
    parse_markdown(&owned)
}

fn assert_single_span(spans: &[StyledSpan], text: &str, style: &Style) {
    assert_eq!(spans.len(), 1, "Expected 1 span, got {}: {spans:?}", spans.len());
    assert_eq!(spans[0].text, text);
    assert_eq!(spans[0].style, *style);
}

// ============================================================================
// StyledSpan unit tests
// ============================================================================

#[test]
fn styled_span_new_and_display_width() {
    let span = StyledSpan::new("hello", plain_style());
    assert_eq!(span.display_width(), 5);
    assert_eq!(span.text, "hello");
    assert_eq!(span.style, plain_style());
}

#[test]
fn styled_span_clone_eq() {
    let a = StyledSpan::new("test", bold_style());
    #[allow(clippy::redundant_clone)]
    let b = a.clone();
    assert_eq!(a, b);
}

#[test]
fn styled_span_debug() {
    let span = StyledSpan::new("x", plain_style());
    assert!(format!("{span:?}").contains("StyledSpan"));
}

// ============================================================================
// Heading tests
// ============================================================================

#[test]
fn heading_h1() {
    let result = parse(&["# Title"]);
    assert_single_span(&result[0], "Title", &heading_style());
}

#[test]
fn heading_h2() {
    let result = parse(&["## Subtitle"]);
    assert_single_span(&result[0], "Subtitle", &heading_style());
}

#[test]
fn heading_h6() {
    let result = parse(&["###### Deep"]);
    assert_single_span(&result[0], "Deep", &heading_style());
}

#[test]
fn heading_no_space_is_not_heading() {
    let result = parse(&["#notaheading"]);
    // Should be treated as plain text
    assert_single_span(&result[0], "#notaheading", &plain_style());
}

#[test]
fn heading_7_hashes_is_not_heading() {
    let result = parse(&["####### too many"]);
    assert_single_span(&result[0], "####### too many", &plain_style());
}

#[test]
fn heading_empty_content() {
    let result = parse(&["#"]);
    // # alone is a valid heading with empty content
    assert_single_span(&result[0], "", &heading_style());
}

#[test]
fn heading_just_hashes_with_space() {
    let result = parse(&["## "]);
    assert_single_span(&result[0], "", &heading_style());
}

// ============================================================================
// Bold tests
// ============================================================================

#[test]
fn bold_asterisks() {
    let result = parse(&["some **bold** text"]);
    assert_eq!(result[0].len(), 3);
    assert_eq!(result[0][0], StyledSpan::new("some ", plain_style()));
    assert_eq!(result[0][1], StyledSpan::new("bold", bold_style()));
    assert_eq!(result[0][2], StyledSpan::new(" text", plain_style()));
}

#[test]
fn bold_underscores() {
    let result = parse(&["__bold__"]);
    assert_single_span(&result[0], "bold", &bold_style());
}

#[test]
fn bold_unclosed() {
    let result = parse(&["**unclosed"]);
    // Treated as literal
    assert_single_span(&result[0], "**unclosed", &plain_style());
}

// ============================================================================
// Italic tests
// ============================================================================

#[test]
fn italic_asterisk() {
    let result = parse(&["*italic* word"]);
    assert_eq!(result[0].len(), 2);
    assert_eq!(result[0][0], StyledSpan::new("italic", italic_style()));
    assert_eq!(result[0][1], StyledSpan::new(" word", plain_style()));
}

#[test]
fn italic_underscore() {
    let result = parse(&["_italic_"]);
    assert_single_span(&result[0], "italic", &italic_style());
}

#[test]
fn italic_unclosed() {
    let result = parse(&["*unclosed"]);
    assert_single_span(&result[0], "*unclosed", &plain_style());
}

// ============================================================================
// Code span tests
// ============================================================================

#[test]
fn code_span() {
    let result = parse(&["use `Option<T>` here"]);
    assert_eq!(result[0].len(), 3);
    assert_eq!(result[0][0], StyledSpan::new("use ", plain_style()));
    assert_eq!(result[0][1], StyledSpan::new("Option<T>", code_span_style()));
    assert_eq!(result[0][2], StyledSpan::new(" here", plain_style()));
}

#[test]
fn code_span_unclosed() {
    let result = parse(&["`unclosed"]);
    assert_single_span(&result[0], "`unclosed", &plain_style());
}

// ============================================================================
// Code block tests
// ============================================================================

#[test]
fn code_block_fenced() {
    let result = parse(&["```rust", "fn main() {}", "```"]);
    assert_eq!(result.len(), 3);
    assert!(result[0].is_empty(), "Opening fence should be hidden");
    assert_single_span(&result[1], "fn main() {}", &code_block_style());
    assert!(result[2].is_empty(), "Closing fence should be hidden");
}

#[test]
fn code_block_tilde() {
    let result = parse(&["~~~", "code", "~~~"]);
    assert!(result[0].is_empty());
    assert_single_span(&result[1], "code", &code_block_style());
    assert!(result[2].is_empty());
}

#[test]
fn code_block_multiline() {
    let result = parse(&["```", "line 1", "line 2", "```"]);
    assert_eq!(result.len(), 4);
    assert_single_span(&result[1], "line 1", &code_block_style());
    assert_single_span(&result[2], "line 2", &code_block_style());
}

// ============================================================================
// Horizontal rule tests
// ============================================================================

#[test]
fn horizontal_rule_dashes() {
    let result = parse(&["---"]);
    assert_eq!(result[0].len(), 1);
    assert_eq!(result[0][0].style, rule_style());
    assert!(result[0][0].text.contains('\u{2500}'));
}

#[test]
fn horizontal_rule_asterisks() {
    let result = parse(&["***"]);
    assert_eq!(result[0][0].style, rule_style());
}

#[test]
fn horizontal_rule_underscores() {
    let result = parse(&["___"]);
    assert_eq!(result[0][0].style, rule_style());
}

#[test]
fn horizontal_rule_with_spaces() {
    let result = parse(&["- - -"]);
    assert_eq!(result[0][0].style, rule_style());
}

#[test]
fn not_horizontal_rule_too_short() {
    let result = parse(&["--"]);
    // Two dashes is not a horizontal rule, treated as plain text
    assert_single_span(&result[0], "--", &plain_style());
}

// ============================================================================
// Plain text tests
// ============================================================================

#[test]
fn plain_text() {
    let result = parse(&["just plain text"]);
    assert_single_span(&result[0], "just plain text", &plain_style());
}

#[test]
fn empty_line() {
    let result = parse(&[""]);
    assert_single_span(&result[0], "", &plain_style());
}

#[test]
fn multiline_mixed() {
    let result = parse(&["# Title", "plain", "**bold**"]);
    assert_eq!(result.len(), 3);
    assert_single_span(&result[0], "Title", &heading_style());
    assert_single_span(&result[1], "plain", &plain_style());
    assert_single_span(&result[2], "bold", &bold_style());
}

// ============================================================================
// Style function coverage
// ============================================================================

#[test]
fn style_functions_return_distinct_styles() {
    let styles = [
        plain_style(),
        heading_style(),
        bold_style(),
        italic_style(),
        code_span_style(),
        code_block_style(),
        rule_style(),
    ];
    // At minimum, heading and code_span should differ from plain
    assert_ne!(styles[0], styles[1]); // plain != heading
    assert_ne!(styles[0], styles[4]); // plain != code_span
    assert_ne!(styles[0], styles[5]); // plain != code_block
    assert_ne!(styles[0], styles[6]); // plain != rule
}

// ============================================================================
// is_code_fence tests
// ============================================================================

#[test]
fn is_code_fence_backticks() {
    assert!(is_code_fence("```"));
    assert!(is_code_fence("```rust"));
    assert!(is_code_fence("  ```"));
}

#[test]
fn is_code_fence_tildes() {
    assert!(is_code_fence("~~~"));
    assert!(is_code_fence("  ~~~py"));
}

#[test]
fn is_not_code_fence() {
    assert!(!is_code_fence("``"));
    assert!(!is_code_fence("~~"));
    assert!(!is_code_fence("normal text"));
}

// ============================================================================
// is_horizontal_rule tests
// ============================================================================

#[test]
fn hr_various() {
    assert!(is_horizontal_rule("---"));
    assert!(is_horizontal_rule("***"));
    assert!(is_horizontal_rule("___"));
    assert!(is_horizontal_rule("- - - -"));
    assert!(is_horizontal_rule("  ---  "));
}

#[test]
fn not_hr() {
    assert!(!is_horizontal_rule("--"));
    assert!(!is_horizontal_rule("abc"));
    assert!(!is_horizontal_rule("-*-"));
}

// ============================================================================
// strip_heading tests
// ============================================================================

#[test]
fn strip_heading_levels() {
    assert_eq!(strip_heading("# A"), Some("A"));
    assert_eq!(strip_heading("## B"), Some("B"));
    assert_eq!(strip_heading("### C"), Some("C"));
    assert_eq!(strip_heading("###### F"), Some("F"));
}

#[test]
fn strip_heading_no_space() {
    assert_eq!(strip_heading("#nope"), None);
}

#[test]
fn strip_heading_too_many() {
    assert_eq!(strip_heading("####### G"), None);
}

#[test]
fn strip_heading_not_heading() {
    assert_eq!(strip_heading("plain"), None);
}

// ============================================================================
// find_double_marker / find_single_marker coverage
// ============================================================================

#[test]
fn find_double_marker_found() {
    assert_eq!(find_double_marker("hello**world", '*'), Some(5));
}

#[test]
fn find_double_marker_not_found() {
    assert_eq!(find_double_marker("hello*world", '*'), None);
}

#[test]
fn find_single_marker_found() {
    assert_eq!(find_single_marker("hello*world", '*'), Some(5));
}

#[test]
fn find_single_marker_skips_double() {
    // **word*end — first two ** are a double marker, skipped; single * at index 6
    assert_eq!(find_single_marker("**word*end", '*'), Some(6));
}

#[test]
fn find_single_marker_not_found() {
    assert_eq!(find_single_marker("hello world", '*'), None);
}
