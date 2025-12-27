//! Markdown decoration provider
//!
//! Implements `DecorationProvider` trait from core to provide decorations
//! for markdown files (heading icons, bullets, checkboxes, etc.).

use std::sync::RwLock;

use {
    reovim_core::{
        decoration::{Decoration, DecorationProvider},
        highlight::Span,
    },
    tree_sitter::{Language, Parser, Query, StreamingIterator, Tree},
};

use crate::markdown::MarkdownConfig;

/// Map language name (from code fence) to file extension for icon lookup
///
/// The icon registry uses file extensions, so we need to convert
/// language names like "rust" to extensions like "rs".
fn lang_to_extension(lang: &str) -> String {
    let lower = lang.to_lowercase();
    match lower.as_str() {
        "rust" => "rs".to_string(),
        "python" => "py".to_string(),
        "javascript" => "js".to_string(),
        "typescript" => "ts".to_string(),
        "golang" => "go".to_string(),
        "c++" => "cpp".to_string(),
        "shell" | "zsh" => "sh".to_string(),
        "dockerfile" => "docker".to_string(),
        "makefile" | "make" => "mk".to_string(),
        "csharp" => "cs".to_string(),
        "fsharp" => "fs".to_string(),
        _ => lower,
    }
}

/// Get icon for a language using the global icon registry
fn language_icon(lang: &str) -> String {
    use reovim_core::style::icons::registry;

    let ext = lang_to_extension(lang);
    let registry = registry().read().unwrap();

    // file_icon() returns &'static str with built-in fallback
    registry.file_icon(&ext).to_string()
}

/// Block decorations query for markdown
const BLOCK_DECORATIONS_QUERY: &str = r#"
; Markdown decoration queries for visual rendering

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

; Fenced code block language icon - capture opening fence and language
(fenced_code_block
  (fenced_code_block_delimiter) @decoration.code_fence_open
  (info_string
    (language) @decoration.code_lang))

; Tables (pipe tables)
(pipe_table) @decoration.table
(pipe_table_header) @decoration.table.header
(pipe_table_delimiter_row) @decoration.table.delimiter
"#;

/// Inline decorations query for markdown (emphasis, links, etc.)
const INLINE_DECORATIONS_QUERY: &str = r#"
; Markdown inline decoration captures

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
"#;

/// Markdown decoration provider implementing `DecorationProvider` trait
///
/// This is owned by the buffer and provides decorations on demand.
pub struct MarkdownDecorator {
    /// Configuration for rendering
    config: MarkdownConfig,
    /// Block-level parser (markdown grammar)
    parser: RwLock<Parser>,
    /// Block-level decoration query
    block_query: Query,
    /// Inline parser (markdown_inline grammar)
    inline_parser: RwLock<Parser>,
    /// Inline decoration query
    inline_query: Query,
    /// Cached parse tree (block level)
    tree: RwLock<Option<Tree>>,
    /// Whether the cached tree is valid
    valid: RwLock<bool>,
}

impl MarkdownDecorator {
    /// Create a new markdown decorator with default configuration
    #[must_use]
    pub fn new() -> Option<Self> {
        Self::with_config(MarkdownConfig::default())
    }

    /// Create a new markdown decorator with custom configuration
    #[must_use]
    pub fn with_config(config: MarkdownConfig) -> Option<Self> {
        let block_lang: Language = tree_sitter_md::LANGUAGE.into();
        let inline_lang: Language = tree_sitter_md::INLINE_LANGUAGE.into();

        // Create block parser
        let mut parser = Parser::new();
        parser.set_language(&block_lang).ok()?;

        // Create inline parser
        let mut inline_parser = Parser::new();
        inline_parser.set_language(&inline_lang).ok()?;

        // Compile queries
        let block_query = Query::new(&block_lang, BLOCK_DECORATIONS_QUERY).ok()?;
        let inline_query = Query::new(&inline_lang, INLINE_DECORATIONS_QUERY).ok()?;

        Some(Self {
            config,
            parser: RwLock::new(parser),
            block_query,
            inline_parser: RwLock::new(inline_parser),
            inline_query,
            tree: RwLock::new(None),
            valid: RwLock::new(false),
        })
    }

    /// Generate heading decorations
    #[allow(clippy::cast_possible_truncation)]
    fn render_headings(&self, tree: &Tree, content: &str) -> Vec<Decoration> {
        let mut decorations = Vec::new();
        let mut cursor = tree_sitter::QueryCursor::new();
        let mut matches = cursor.matches(&self.block_query, tree.root_node(), content.as_bytes());

        while let Some(match_) = matches.next() {
            for capture in match_.captures {
                let capture_name = &self.block_query.capture_names()[capture.index as usize];
                let node = capture.node;

                // Handle heading markers (# ## ### etc.)
                if capture_name.starts_with("decoration.heading.")
                    && capture_name.ends_with(".marker")
                {
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

                        decorations.push(Decoration::conceal(
                            Span::new(
                                start.row as u32,
                                start.column as u32,
                                end.row as u32,
                                end.column as u32 + 1, // +1 for space after marker
                            ),
                            icon,
                            style,
                        ));
                    }
                }

                // Handle heading line backgrounds
                if *capture_name == "decoration.heading.line" {
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

    /// Generate list decorations (bullets and checkboxes)
    #[allow(clippy::cast_possible_truncation)]
    fn render_lists(&self, tree: &Tree, content: &str) -> Vec<Decoration> {
        let mut decorations = Vec::new();
        let mut cursor = tree_sitter::QueryCursor::new();
        let mut matches = cursor.matches(&self.block_query, tree.root_node(), content.as_bytes());

        while let Some(match_) = matches.next() {
            for capture in match_.captures {
                let capture_name = &self.block_query.capture_names()[capture.index as usize];
                let node = capture.node;
                let start = node.start_position();
                let end = node.end_position();

                match *capture_name {
                    "decoration.list.bullet" => {
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

    /// Generate code block decorations
    #[allow(clippy::cast_possible_truncation)]
    fn render_code_blocks(&self, tree: &Tree, content: &str) -> Vec<Decoration> {
        let mut decorations = Vec::new();
        let mut cursor = tree_sitter::QueryCursor::new();
        let mut matches = cursor.matches(&self.block_query, tree.root_node(), content.as_bytes());

        // Track fence/lang pairs to combine them
        let mut pending_fence: Option<(u32, u32, u32, u32)> = None; // (row, start_col, end_col, row)

        while let Some(match_) = matches.next() {
            // Process captures in order for this match
            let mut fence_span: Option<(u32, u32, u32, u32)> = None;
            let mut lang_info: Option<(u32, u32, u32, u32, String)> = None;

            for capture in match_.captures {
                let capture_name = &self.block_query.capture_names()[capture.index as usize];
                let node = capture.node;

                match *capture_name {
                    "decoration.code_block" => {
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
                    "decoration.code_fence_open" => {
                        let start = node.start_position();
                        let end = node.end_position();
                        fence_span = Some((
                            start.row as u32,
                            start.column as u32,
                            end.column as u32,
                            end.row as u32,
                        ));
                        pending_fence = Some((
                            start.row as u32,
                            start.column as u32,
                            end.column as u32,
                            end.row as u32,
                        ));
                    }
                    "decoration.code_lang" => {
                        let start = node.start_position();
                        let end = node.end_position();
                        let lang_text =
                            node.utf8_text(content.as_bytes()).unwrap_or("").to_string();
                        lang_info = Some((
                            start.row as u32,
                            start.column as u32,
                            end.column as u32,
                            end.row as u32,
                            lang_text,
                        ));
                    }
                    _ => {}
                }
            }

            // If we have both fence and lang in this match, generate icon decoration
            if self.config.code_blocks.show_language_icon
                && let Some((lang_row, lang_start, lang_end, _, lang_text)) = lang_info
            {
                // Use fence from same match, or pending fence from previous match
                let fence_to_use = fence_span.or_else(|| pending_fence.take());

                if let Some(fence) = fence_to_use {
                    let icon = language_icon(&lang_text);

                    // Hide the opening fence (```)
                    decorations.push(Decoration::Hide {
                        span: Span::new(fence.0, fence.1, fence.3, fence.2),
                    });

                    // Replace language name with icon + language
                    decorations.push(Decoration::Conceal {
                        span: Span::new(lang_row, lang_start, lang_row, lang_end),
                        replacement: format!("{icon}{lang_text}"),
                        style: Some(self.config.code_blocks.lang_style.clone()),
                    });
                }
            }
        }

        decorations
    }

    /// Generate table decorations
    #[allow(clippy::cast_possible_truncation)]
    fn render_tables(&self, tree: &Tree, content: &str) -> Vec<Decoration> {
        if !self.config.tables.enabled {
            return Vec::new();
        }

        let mut decorations = Vec::new();
        let mut cursor = tree_sitter::QueryCursor::new();
        let mut matches = cursor.matches(&self.block_query, tree.root_node(), content.as_bytes());

        while let Some(match_) = matches.next() {
            for capture in match_.captures {
                let capture_name = &self.block_query.capture_names()[capture.index as usize];
                let node = capture.node;

                match *capture_name {
                    "decoration.table.header" => {
                        if let Some(bg_style) = &self.config.tables.header_background {
                            let start_line = node.start_position().row as u32;
                            decorations.push(Decoration::single_line_background(
                                start_line,
                                bg_style.clone(),
                            ));
                        }
                    }
                    "decoration.table.delimiter" => {
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

    /// Generate inline decorations (emphasis, links)
    #[allow(clippy::cast_possible_truncation)]
    #[allow(clippy::too_many_lines)]
    fn render_inline(&self, content: &str) -> Vec<Decoration> {
        if !self.config.inline.conceal_emphasis && !self.config.inline.conceal_links {
            return Vec::new();
        }

        // Parse with inline grammar
        let tree = {
            let mut parser = self.inline_parser.write().unwrap();
            parser.parse(content, None)
        };

        let Some(tree) = tree else {
            return Vec::new();
        };

        let mut decorations = Vec::new();
        let mut cursor = tree_sitter::QueryCursor::new();
        let mut matches = cursor.matches(&self.inline_query, tree.root_node(), content.as_bytes());

        while let Some(match_) = matches.next() {
            for capture in match_.captures {
                let capture_name = &self.inline_query.capture_names()[capture.index as usize];
                let node = capture.node;
                let start = node.start_position();
                let end = node.end_position();

                match *capture_name {
                    "decoration.emphasis" if self.config.inline.conceal_emphasis => {
                        // Hide opening * marker
                        decorations.push(Decoration::Hide {
                            span: Span::new(
                                start.row as u32,
                                start.column as u32,
                                start.row as u32,
                                start.column as u32 + 1,
                            ),
                        });
                        // Hide closing * marker
                        decorations.push(Decoration::Hide {
                            span: Span::new(
                                end.row as u32,
                                end.column as u32 - 1,
                                end.row as u32,
                                end.column as u32,
                            ),
                        });
                        // Apply italic style
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
                        // Hide opening ** marker
                        decorations.push(Decoration::Hide {
                            span: Span::new(
                                start.row as u32,
                                start.column as u32,
                                start.row as u32,
                                start.column as u32 + 2,
                            ),
                        });
                        // Hide closing ** marker
                        decorations.push(Decoration::Hide {
                            span: Span::new(
                                end.row as u32,
                                end.column as u32 - 2,
                                end.row as u32,
                                end.column as u32,
                            ),
                        });
                        // Apply bold style
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
                        // Hide opening [
                        decorations.push(Decoration::Hide {
                            span: Span::new(
                                start.row as u32,
                                start.column as u32,
                                start.row as u32,
                                start.column as u32 + 1,
                            ),
                        });
                        // Find link_text child and hide ](...) part
                        let mut walker = node.walk();
                        for child in node.children(&mut walker) {
                            if child.kind() == "link_text" {
                                let text_start = child.start_position();
                                let text_end = child.end_position();
                                // Hide from after link_text to end
                                decorations.push(Decoration::Hide {
                                    span: Span::new(
                                        text_end.row as u32,
                                        text_end.column as u32,
                                        end.row as u32,
                                        end.column as u32,
                                    ),
                                });
                                // Apply link style
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

    /// Get heading level from an atx_heading node
    fn get_heading_level(node: tree_sitter::Node) -> usize {
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            let kind = child.kind();
            if kind.starts_with("atx_h")
                && kind.ends_with("_marker")
                && let Some(level_str) = kind
                    .strip_prefix("atx_h")
                    .and_then(|s| s.strip_suffix("_marker"))
                && let Ok(level) = level_str.parse::<usize>()
            {
                return level;
            }
        }
        1
    }
}

impl Default for MarkdownDecorator {
    fn default() -> Self {
        Self::new().expect("Failed to create MarkdownDecorator")
    }
}

impl MarkdownDecorator {
    /// Parse content and return decorations in one call (stateless).
    ///
    /// Convenience method for one-shot markdown parsing, useful for
    /// hover popups and other contexts where caching isn't needed.
    #[must_use]
    pub fn parse_decorations(content: &str) -> Vec<Decoration> {
        let Some(mut decorator) = Self::new() else {
            return Vec::new();
        };
        decorator.refresh(content);
        #[allow(clippy::cast_possible_truncation)]
        decorator.decoration_range(content, 0, content.lines().count() as u32)
    }
}

impl DecorationProvider for MarkdownDecorator {
    fn language_id(&self) -> &str {
        "markdown"
    }

    fn decoration_range(&self, content: &str, _start_line: u32, _end_line: u32) -> Vec<Decoration> {
        if !self.config.enabled {
            return Vec::new();
        }

        // Get cached tree
        let tree_guard = self.tree.read().unwrap();
        let Some(tree) = tree_guard.as_ref() else {
            return Vec::new();
        };

        let mut decorations = Vec::new();

        // Generate all decoration types
        decorations.extend(self.render_headings(tree, content));
        decorations.extend(self.render_lists(tree, content));
        decorations.extend(self.render_code_blocks(tree, content));
        decorations.extend(self.render_tables(tree, content));
        decorations.extend(self.render_inline(content));

        decorations
    }

    fn refresh(&mut self, content: &str) {
        // Re-parse and cache tree
        let tree = {
            let mut parser = self.parser.write().unwrap();
            parser.parse(content, None)
        };

        *self.tree.write().unwrap() = tree;
        *self.valid.write().unwrap() = true;
    }

    fn is_valid(&self) -> bool {
        *self.valid.read().unwrap()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_decorator_generates_heading_decorations() {
        let mut decorator = MarkdownDecorator::new().expect("Failed to create decorator");
        let content = "# Heading 1\n\nSome text.\n";

        decorator.refresh(content);

        let decorations = decorator.decoration_range(content, 0, 3);

        println!("Generated {} decorations:", decorations.len());
        for d in &decorations {
            println!("  {d:?}");
        }

        assert!(!decorations.is_empty(), "Should generate decorations");

        // Check for heading marker conceal
        let has_heading_conceal = decorations.iter().any(
            |d| matches!(d, Decoration::Conceal { replacement, .. } if replacement.contains("󰉫")),
        );
        assert!(has_heading_conceal, "Should have heading marker conceal");
    }

    #[test]
    fn test_decorator_generates_list_decorations() {
        let mut decorator = MarkdownDecorator::new().expect("Failed to create decorator");
        let content = "- Item 1\n- Item 2\n  - Nested\n";

        decorator.refresh(content);

        let decorations = decorator.decoration_range(content, 0, 3);

        println!("Generated {} decorations:", decorations.len());
        for d in &decorations {
            println!("  {d:?}");
        }

        // Should have bullet replacements
        let has_bullet = decorations.iter().any(|d| {
            matches!(d, Decoration::Conceal { replacement, .. } if replacement == "•" || replacement == "◦")
        });
        assert!(has_bullet, "Should have bullet decorations");
    }

    #[test]
    fn test_decorator_generates_inline_decorations() {
        let mut decorator = MarkdownDecorator::new().expect("Failed to create decorator");
        let content = "This is *italic* and **bold** text.\n";

        decorator.refresh(content);

        let decorations = decorator.decoration_range(content, 0, 1);

        println!("Generated {} decorations:", decorations.len());
        for d in &decorations {
            println!("  {d:?}");
        }

        // Should have Hide decorations for markers
        let hide_count = decorations
            .iter()
            .filter(|d| matches!(d, Decoration::Hide { .. }))
            .count();

        assert!(hide_count >= 4, "Should have at least 4 Hide decorations, got {hide_count}");
    }
}
