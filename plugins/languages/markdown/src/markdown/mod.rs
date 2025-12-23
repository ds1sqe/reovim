//! Markdown language renderer
//!
//! Implements `LanguageRenderer` to provide visual decorations for markdown files.

mod config;

use std::any::Any;

use tree_sitter::{Language, Parser, Query, StreamingIterator, Tree};

pub use config::MarkdownConfig;

use reovim_core::{
    decoration::{Decoration, DecorationContext, LanguageId, LanguageRenderer},
    highlight::Span,
    modd::EditMode,
};

/// Markdown renderer implementing the `LanguageRenderer` trait
///
/// Note: Currently unused - replaced by `MarkdownDecorator` which implements
/// the buffer-centric `DecorationProvider` trait. Kept for reference and tests.
#[allow(dead_code)]
pub struct MarkdownRenderer {
    config: MarkdownConfig,
    /// Cached inline decoration query (compiled once)
    inline_query: Option<Query>,
    /// Inline language for creating parsers
    inline_language: Language,
}

impl Default for MarkdownRenderer {
    fn default() -> Self {
        Self::new()
    }
}

/// Inline decorations query source
#[allow(dead_code)]
const INLINE_DECORATIONS_QUERY: &str = r"
; Markdown inline decoration captures
; Used for concealing markers while preserving styled content

; Emphasis (italic) - conceal the * markers
(emphasis) @decoration.emphasis

; Strong emphasis (bold) - conceal the ** markers
(strong_emphasis) @decoration.strong

; Inline links - conceal brackets and URL
(inline_link) @decoration.link

; Code spans - for potential styling
(code_span) @decoration.code_span

; Strikethrough - conceal ~~ markers
(strikethrough) @decoration.strikethrough
";

/// Block decorations query source (for tests)
#[cfg(test)]
const BLOCK_DECORATIONS_QUERY: &str = r"
; Markdown decoration queries for visual rendering
; Used by MarkdownRenderer for concealment and visual enhancements

; Heading markers for concealment (# ## ### etc.)
(atx_heading (atx_h1_marker) @decoration.heading.1.marker)
(atx_heading (atx_h2_marker) @decoration.heading.2.marker)
(atx_heading (atx_h3_marker) @decoration.heading.3.marker)
(atx_heading (atx_h4_marker) @decoration.heading.4.marker)
(atx_heading (atx_h5_marker) @decoration.heading.5.marker)
(atx_heading (atx_h6_marker) @decoration.heading.6.marker)

; Full heading lines for background styling
(atx_heading) @decoration.heading.line

; List bullet markers for replacement (- + *)
(list_marker_minus) @decoration.list.bullet
(list_marker_plus) @decoration.list.bullet
(list_marker_star) @decoration.list.bullet

; Checkboxes for replacement
(task_list_marker_unchecked) @decoration.checkbox.unchecked
(task_list_marker_checked) @decoration.checkbox.checked

; Code blocks for background styling
(fenced_code_block) @decoration.code_block
(indented_code_block) @decoration.code_block

; Tables (pipe tables)
(pipe_table) @decoration.table
(pipe_table_header) @decoration.table.header
(pipe_table_delimiter_row) @decoration.table.delimiter
";

#[allow(dead_code)]
impl MarkdownRenderer {
    /// Create a new markdown renderer with default configuration
    #[must_use]
    pub fn new() -> Self {
        let inline_language: Language = tree_sitter_md::INLINE_LANGUAGE.into();

        // Compile the inline decorations query once
        let inline_query = Query::new(&inline_language, INLINE_DECORATIONS_QUERY).ok();

        Self {
            config: MarkdownConfig::default(),
            inline_query,
            inline_language,
        }
    }

    /// Create a markdown renderer with custom configuration
    #[must_use]
    pub fn with_config(config: MarkdownConfig) -> Self {
        let inline_language: Language = tree_sitter_md::INLINE_LANGUAGE.into();
        let inline_query = Query::new(&inline_language, INLINE_DECORATIONS_QUERY).ok();

        Self {
            config,
            inline_query,
            inline_language,
        }
    }

    /// Parse content with inline grammar (creates temporary parser)
    fn parse_inline_content(&self, content: &str) -> Option<Tree> {
        let mut parser = Parser::new();
        parser.set_language(&self.inline_language).ok()?;
        parser.parse(content, None)
    }

    /// Render heading decorations
    #[allow(clippy::cast_possible_truncation)]
    fn render_headings(&self, ctx: &DecorationContext, query: &Query) -> Vec<Decoration> {
        let mut decorations = Vec::new();
        let mut cursor = tree_sitter::QueryCursor::new();

        let mut matches = cursor.matches(query, ctx.tree.root_node(), ctx.content.as_bytes());

        while let Some(match_) = matches.next() {
            for capture in match_.captures {
                let capture_name = &query.capture_names()[capture.index as usize];
                let node = capture.node;

                // Handle heading markers (# ## ### etc.)
                if capture_name.starts_with("decoration.heading.")
                    && capture_name.ends_with(".marker")
                {
                    // Extract heading level from capture name
                    let level: usize = capture_name
                        .strip_prefix("decoration.heading.")
                        .and_then(|s| s.strip_suffix(".marker"))
                        .and_then(|s| s.parse().ok())
                        .unwrap_or(1);

                    if (1..=6).contains(&level) {
                        let idx = level - 1;
                        let icon = self.config.headings.icons[idx];
                        let style = Some(self.config.headings.styles[idx].clone());

                        let start = node.start_position();
                        let end = node.end_position();

                        // Conceal the marker (e.g., "## ") with the icon
                        decorations.push(Decoration::conceal(
                            Span::new(
                                start.row as u32,
                                start.column as u32,
                                end.row as u32,
                                end.column as u32 + 1, // +1 for the space after marker
                            ),
                            icon,
                            style,
                        ));
                    }
                }

                // Handle heading line backgrounds
                if *capture_name == "decoration.heading.line" {
                    // Determine heading level from the node
                    let level = Self::get_heading_level(node);
                    if (1..=6).contains(&level) {
                        let idx = level - 1;
                        if let Some(bg_style) = &self.config.headings.backgrounds[idx] {
                            decorations.push(Decoration::single_line_background(
                                node.start_position().row as u32,
                                bg_style.clone(),
                            ));
                        }
                    }
                }
            }
        }

        decorations
    }

    /// Render list decorations (bullets and checkboxes)
    #[allow(clippy::cast_possible_truncation)]
    fn render_lists(&self, ctx: &DecorationContext, query: &Query) -> Vec<Decoration> {
        let mut decorations = Vec::new();
        let mut cursor = tree_sitter::QueryCursor::new();

        let mut matches = cursor.matches(query, ctx.tree.root_node(), ctx.content.as_bytes());

        while let Some(match_) = matches.next() {
            for capture in match_.captures {
                let capture_name = &query.capture_names()[capture.index as usize];
                let node = capture.node;
                let start = node.start_position();
                let end = node.end_position();

                match *capture_name {
                    "decoration.list.bullet" => {
                        // Determine nesting level (simplified: based on column position)
                        let indent_level = (start.column / 2).min(3);
                        let bullet = self.config.lists.bullets[indent_level];

                        decorations.push(Decoration::conceal(
                            Span::new(
                                start.row as u32,
                                start.column as u32,
                                end.row as u32,
                                end.column as u32,
                            ),
                            bullet.to_string(),
                            Some(self.config.lists.bullet_style.clone()),
                        ));
                    }
                    "decoration.checkbox.checked" => {
                        decorations.push(Decoration::conceal(
                            Span::new(
                                start.row as u32,
                                start.column as u32,
                                end.row as u32,
                                end.column as u32,
                            ),
                            self.config.lists.checkbox_checked.to_string(),
                            Some(self.config.lists.checkbox_checked_style.clone()),
                        ));
                    }
                    "decoration.checkbox.unchecked" => {
                        decorations.push(Decoration::conceal(
                            Span::new(
                                start.row as u32,
                                start.column as u32,
                                end.row as u32,
                                end.column as u32,
                            ),
                            self.config.lists.checkbox_unchecked.to_string(),
                            Some(self.config.lists.checkbox_unchecked_style.clone()),
                        ));
                    }
                    _ => {}
                }
            }
        }

        decorations
    }

    /// Render code block decorations
    #[allow(clippy::cast_possible_truncation)]
    fn render_code_blocks(&self, ctx: &DecorationContext, query: &Query) -> Vec<Decoration> {
        let mut decorations = Vec::new();

        if self.config.code_blocks.background.is_none() {
            return decorations;
        }

        let mut cursor = tree_sitter::QueryCursor::new();
        let mut matches = cursor.matches(query, ctx.tree.root_node(), ctx.content.as_bytes());

        while let Some(match_) = matches.next() {
            for capture in match_.captures {
                let capture_name = &query.capture_names()[capture.index as usize];

                if *capture_name == "decoration.code_block" {
                    let node = capture.node;
                    let start_line = node.start_position().row as u32;
                    let end_line = node.end_position().row as u32;

                    if let Some(bg_style) = &self.config.code_blocks.background {
                        decorations.push(Decoration::line_background(
                            start_line,
                            end_line,
                            bg_style.clone(),
                        ));
                    }
                }
            }
        }

        decorations
    }

    /// Render inline formatting decorations (emphasis, links)
    ///
    /// Uses the inline grammar (`INLINE_LANGUAGE`) to parse and detect emphasis, strong, links.
    #[allow(clippy::cast_possible_truncation)]
    #[allow(clippy::too_many_lines)]
    fn render_inline(&self, ctx: &DecorationContext) -> Vec<Decoration> {
        if !self.config.inline.conceal_emphasis && !self.config.inline.conceal_links {
            return Vec::new();
        }

        // Get the inline query (compiled at renderer creation)
        let Some(query) = &self.inline_query else {
            return Vec::new();
        };

        // Parse content with inline grammar
        let Some(tree) = self.parse_inline_content(ctx.content) else {
            return Vec::new();
        };

        let mut decorations = Vec::new();
        let mut cursor = tree_sitter::QueryCursor::new();
        let mut matches = cursor.matches(query, tree.root_node(), ctx.content.as_bytes());

        while let Some(match_) = matches.next() {
            for capture in match_.captures {
                let capture_name = &query.capture_names()[capture.index as usize];
                let node = capture.node;
                let start = node.start_position();
                let end = node.end_position();

                match *capture_name {
                    "decoration.emphasis" if self.config.inline.conceal_emphasis => {
                        // Hide the opening * marker
                        decorations.push(Decoration::Hide {
                            span: Span::new(
                                start.row as u32,
                                start.column as u32,
                                start.row as u32,
                                start.column as u32 + 1,
                            ),
                        });
                        // Hide the closing * marker
                        decorations.push(Decoration::Hide {
                            span: Span::new(
                                end.row as u32,
                                end.column as u32 - 1,
                                end.row as u32,
                                end.column as u32,
                            ),
                        });
                        // Apply italic style to the content (between markers)
                        decorations.push(Decoration::inline_style(
                            Span::new(
                                start.row as u32,
                                start.column as u32 + 1,
                                end.row as u32,
                                end.column as u32 - 1,
                            ),
                            self.config.inline.italic_style.clone(),
                        ));
                    }
                    "decoration.strong" if self.config.inline.conceal_emphasis => {
                        // Hide the opening ** marker
                        decorations.push(Decoration::Hide {
                            span: Span::new(
                                start.row as u32,
                                start.column as u32,
                                start.row as u32,
                                start.column as u32 + 2,
                            ),
                        });
                        // Hide the closing ** marker
                        decorations.push(Decoration::Hide {
                            span: Span::new(
                                end.row as u32,
                                end.column as u32 - 2,
                                end.row as u32,
                                end.column as u32,
                            ),
                        });
                        // Apply bold style to the content (between markers)
                        decorations.push(Decoration::inline_style(
                            Span::new(
                                start.row as u32,
                                start.column as u32 + 2,
                                end.row as u32,
                                end.column as u32 - 2,
                            ),
                            self.config.inline.bold_style.clone(),
                        ));
                    }
                    "decoration.link" if self.config.inline.conceal_links => {
                        // For inline_link: [text](url) or [text](url "title")
                        // Hide [ at start
                        decorations.push(Decoration::Hide {
                            span: Span::new(
                                start.row as u32,
                                start.column as u32,
                                start.row as u32,
                                start.column as u32 + 1,
                            ),
                        });
                        // Find and hide ](...) part by looking for the closing bracket
                        // The inline_link structure is: [ link_text ] ( link_destination )
                        // We need to find where link_text ends to hide ](...) to the end
                        let mut walker = node.walk();
                        for child in node.children(&mut walker) {
                            if child.kind() == "link_text" {
                                let text_start = child.start_position();
                                let text_end = child.end_position();
                                // Hide from after link_text (the ]) to end of node (includes url)
                                decorations.push(Decoration::Hide {
                                    span: Span::new(
                                        text_end.row as u32,
                                        text_end.column as u32,
                                        end.row as u32,
                                        end.column as u32,
                                    ),
                                });
                                // Apply link style (underline, color) to the link text
                                decorations.push(Decoration::inline_style(
                                    Span::new(
                                        text_start.row as u32,
                                        text_start.column as u32,
                                        text_end.row as u32,
                                        text_end.column as u32,
                                    ),
                                    self.config.inline.link_style.clone(),
                                ));
                            }
                        }
                    }
                    _ => {}
                }
            }
        }

        decorations
    }

    /// Render table decorations
    #[allow(clippy::cast_possible_truncation)]
    fn render_tables(&self, ctx: &DecorationContext, query: &Query) -> Vec<Decoration> {
        if !self.config.tables.enabled {
            return Vec::new();
        }

        let mut decorations = Vec::new();
        let mut cursor = tree_sitter::QueryCursor::new();
        let mut matches = cursor.matches(query, ctx.tree.root_node(), ctx.content.as_bytes());

        while let Some(match_) = matches.next() {
            for capture in match_.captures {
                let capture_name = &query.capture_names()[capture.index as usize];
                let node = capture.node;

                match *capture_name {
                    "decoration.table.header" => {
                        // Apply header row background
                        if let Some(bg_style) = &self.config.tables.header_background {
                            let start_line = node.start_position().row as u32;
                            decorations.push(Decoration::single_line_background(
                                start_line,
                                bg_style.clone(),
                            ));
                        }
                    }
                    "decoration.table.delimiter" => {
                        // Apply dim styling to delimiter row via line background
                        let start_line = node.start_position().row as u32;
                        decorations.push(Decoration::single_line_background(
                            start_line,
                            self.config.tables.delimiter_style.clone(),
                        ));
                    }
                    _ => {}
                }
            }
        }

        decorations
    }

    /// Get heading level from an `atx_heading` node
    fn get_heading_level(node: tree_sitter::Node) -> usize {
        // Walk children to find the marker
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            let kind = child.kind();
            if kind.starts_with("atx_h") && kind.ends_with("_marker") {
                // atx_h1_marker, atx_h2_marker, etc.
                if let Some(level_str) = kind
                    .strip_prefix("atx_h")
                    .and_then(|s| s.strip_suffix("_marker"))
                    && let Ok(level) = level_str.parse::<usize>()
                {
                    return level;
                }
            }
        }
        1 // Default to H1
    }
}

impl LanguageRenderer for MarkdownRenderer {
    fn language_id(&self) -> LanguageId {
        LanguageId::Markdown
    }

    fn file_extensions(&self) -> &[&str] {
        &["md", "markdown", "mkd"]
    }

    fn render(&self, ctx: &DecorationContext, query: &Query) -> Vec<Decoration> {
        if !self.config.enabled {
            return Vec::new();
        }

        let mut decorations = Vec::new();

        // Render each decoration type (block-level uses passed query)
        decorations.extend(self.render_headings(ctx, query));
        decorations.extend(self.render_lists(ctx, query));
        decorations.extend(self.render_code_blocks(ctx, query));
        decorations.extend(self.render_tables(ctx, query));

        // Inline elements use internal inline grammar and query
        decorations.extend(self.render_inline(ctx));

        decorations
    }

    fn should_show_raw(&self, line: u32, ctx: &DecorationContext) -> bool {
        // Always show raw in insert mode
        if matches!(ctx.edit_mode, EditMode::Insert(_)) {
            return true;
        }

        // Optionally show raw on cursor line in normal mode
        if self.config.raw_on_cursor_line && line == ctx.cursor_line {
            return true;
        }

        false
    }

    fn config(&self) -> &dyn Any {
        &self.config
    }

    fn config_mut(&mut self) -> &mut dyn Any {
        &mut self.config
    }

    fn is_enabled(&self) -> bool {
        self.config.enabled
    }

    fn set_enabled(&mut self, enabled: bool) {
        self.config.enabled = enabled;
    }
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        reovim_core::{decoration::DecorationContext, modd::EditMode},
    };

    #[test]
    fn test_markdown_renderer_generates_heading_decorations() {
        let renderer = MarkdownRenderer::new();
        let md_content = "# Heading 1\n\nSome text.\n";

        // Parse the content
        let mut parser = tree_sitter::Parser::new();
        let ts_lang: tree_sitter::Language = tree_sitter_md::LANGUAGE.into();
        parser
            .set_language(&ts_lang)
            .expect("Failed to set language");
        let tree = parser.parse(md_content, None).expect("Failed to parse");

        // Compile the decoration query directly
        let query = tree_sitter::Query::new(&ts_lang, BLOCK_DECORATIONS_QUERY)
            .expect("Failed to compile decoration query");

        // Create context
        let ctx = DecorationContext::new(md_content, &tree, &EditMode::Normal, 0, 0);

        // Generate decorations
        let decorations = renderer.render(&ctx, &query);

        println!("Generated {} decorations:", decorations.len());
        for d in &decorations {
            println!("  {d:?}");
        }

        // Should have at least one conceal for the heading marker
        assert!(!decorations.is_empty(), "Should generate decorations for markdown heading");

        // Check for heading marker conceal
        let has_heading_conceal = decorations.iter().any(
            |d| matches!(d, Decoration::Conceal { replacement, .. } if replacement.contains("󰉫")),
        );
        assert!(has_heading_conceal, "Should have heading marker conceal, got: {decorations:?}");
    }

    #[test]
    fn test_markdown_renderer_generates_inline_decorations() {
        let renderer = MarkdownRenderer::new();
        let md_content = "This is *italic* and **bold** text with a [link](http://example.com).\n";

        // Parse the content with block grammar
        let mut parser = tree_sitter::Parser::new();
        let ts_lang: tree_sitter::Language = tree_sitter_md::LANGUAGE.into();
        parser
            .set_language(&ts_lang)
            .expect("Failed to set language");
        let tree = parser.parse(md_content, None).expect("Failed to parse");

        // Compile the decoration query directly
        let query = tree_sitter::Query::new(&ts_lang, BLOCK_DECORATIONS_QUERY)
            .expect("Failed to compile decoration query");

        // Create context
        let ctx = DecorationContext::new(md_content, &tree, &EditMode::Normal, 0, 0);

        // Generate decorations
        let decorations = renderer.render(&ctx, &query);

        println!("Generated {} inline decorations:", decorations.len());
        for d in &decorations {
            println!("  {d:?}");
        }

        // Should have Hide decorations for emphasis markers
        let hide_count = decorations
            .iter()
            .filter(|d| matches!(d, Decoration::Hide { .. }))
            .count();

        // Expected: 2 for *italic* + 2 for **bold** + 2 for [link] = 6 Hide decorations
        assert!(
            hide_count >= 4,
            "Should have at least 4 Hide decorations for inline markers, got: {hide_count}"
        );
    }

    #[test]
    #[allow(clippy::items_after_statements)]
    fn test_link_with_title_tree_structure() {
        let content = r#"Click [Link](http://example.com "Title") for more."#;

        let inline_lang: tree_sitter::Language = tree_sitter_md::INLINE_LANGUAGE.into();
        let mut parser = tree_sitter::Parser::new();
        parser.set_language(&inline_lang).unwrap();

        let tree = parser.parse(content, None).unwrap();

        fn print_tree(node: tree_sitter::Node, indent: usize, content: &str) {
            let indent_str = "  ".repeat(indent);
            let start = node.start_position();
            let end = node.end_position();
            let text: String = content[node.byte_range()].chars().take(50).collect();
            println!(
                "{}{}  [{},{}]-[{},{}]  \"{}\"",
                indent_str,
                node.kind(),
                start.row,
                start.column,
                end.row,
                end.column,
                text.replace('\n', "\\n")
            );

            let mut cursor = node.walk();
            for child in node.children(&mut cursor) {
                print_tree(child, indent + 1, content);
            }
        }

        println!("\n=== Tree structure for link with title ===");
        print_tree(tree.root_node(), 0, content);
        println!("==========================================\n");
    }
}
