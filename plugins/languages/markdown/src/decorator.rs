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

use crate::config::MarkdownConfig;

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
const BLOCK_DECORATIONS_QUERY: &str = include_str!("queries/decorations.scm");

/// Inline decorations query for markdown (emphasis, links, etc.)
const INLINE_DECORATIONS_QUERY: &str = include_str!("queries_inline/decorations.scm");

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
                        // Add indentation based on level (H1=0, H2=1 space, H3=2 spaces, etc.)
                        let indent = if level > 1 {
                            " ".repeat((level - 1) * self.config.headings.indent_per_level as usize)
                        } else {
                            String::new()
                        };
                        let icon = self.config.headings.icons[idx];
                        let replacement = format!("{indent}{icon}");
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
                            replacement,
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

        // Track delimiter rows for box-drawing
        let mut delimiter_rows: Vec<u32> = Vec::new();
        let mut table_ranges: Vec<(u32, u32)> = Vec::new(); // (start_line, end_line)

        while let Some(match_) = matches.next() {
            for capture in match_.captures {
                let capture_name = &self.block_query.capture_names()[capture.index as usize];
                let node = capture.node;

                match *capture_name {
                    "decoration.table" => {
                        let start_line = node.start_position().row as u32;
                        // end_position is exclusive, so subtract 1 to get the actual last row
                        // But check if the end column is 0 (meaning the node ends at start of next line)
                        let end_pos = node.end_position();
                        let end_line = if end_pos.column == 0 && end_pos.row > 0 {
                            (end_pos.row - 1) as u32
                        } else {
                            end_pos.row as u32
                        };
                        table_ranges.push((start_line, end_line));
                    }
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
                        delimiter_rows.push(start_line);
                        decorations.push(Decoration::single_line_background(
                            start_line,
                            self.config.tables.delimiter_style.clone(),
                        ));
                    }
                    _ => {}
                }
            }
        }

        // Add box-drawing decorations if enabled
        if self.config.tables.borders.use_box_drawing {
            decorations.extend(self.render_table_borders(content, &table_ranges, &delimiter_rows));
        }

        decorations
    }

    /// Analyze table structure to find column positions
    ///
    /// Returns the pipe positions from the delimiter row (most reliable reference)
    /// as these define the canonical column boundaries.
    fn analyze_table_columns(
        &self,
        lines: &[&str],
        start_line: u32,
        end_line: u32,
        delimiter_rows: &[u32],
    ) -> Vec<u32> {
        // Find the delimiter row for this table to get canonical column positions
        let delimiter_line = delimiter_rows
            .iter()
            .find(|&&row| row >= start_line && row <= end_line)
            .copied();

        let reference_line = if let Some(delim) = delimiter_line {
            delim
        } else {
            // Fallback to header row if no delimiter found
            start_line
        };

        let line_idx = reference_line as usize;
        if line_idx >= lines.len() {
            return Vec::new();
        }

        // Get pipe positions from reference line
        lines[line_idx]
            .char_indices()
            .filter_map(|(idx, c)| if c == '|' { Some(idx as u32) } else { None })
            .collect()
    }

    /// Generate box-drawing border decorations for tables
    ///
    /// Uses a multi-pass algorithm:
    /// 1. Analyze table structure to find column positions and widths
    /// 2. Generate decorations for each row type (header, delimiter, data)
    ///
    /// Note: Top/bottom borders (┌───┐ / └───┘) require virtual text support
    /// which is not yet implemented in the core decoration system.
    #[allow(clippy::cast_possible_truncation)]
    fn render_table_borders(
        &self,
        content: &str,
        table_ranges: &[(u32, u32)],
        delimiter_rows: &[u32],
    ) -> Vec<Decoration> {
        let mut decorations = Vec::new();
        let lines: Vec<&str> = content.lines().collect();
        let chars = &self.config.tables.borders.chars;
        let style = &self.config.tables.borders.style;

        for &(start_line, end_line) in table_ranges {
            // Pass 1: Analyze table structure
            let canonical_pipes =
                self.analyze_table_columns(&lines, start_line, end_line, delimiter_rows);

            if canonical_pipes.is_empty() {
                continue;
            }

            let is_delimiter = |line: u32| delimiter_rows.contains(&line);

            // Pass 2: Generate decorations for each row
            for line_num in start_line..=end_line {
                let line_idx = line_num as usize;
                if line_idx >= lines.len() {
                    continue;
                }
                let line = lines[line_idx];

                // Get actual pipe positions for this line
                let line_pipes: Vec<u32> = line
                    .char_indices()
                    .filter_map(|(idx, c)| if c == '|' { Some(idx as u32) } else { None })
                    .collect();

                // Replace each pipe with appropriate box-drawing character
                for (pipe_idx, &pipe_col) in line_pipes.iter().enumerate() {
                    let is_first_pipe = pipe_idx == 0;
                    let is_last_pipe = pipe_idx == line_pipes.len() - 1;

                    let replacement_char = if is_delimiter(line_num) {
                        // Delimiter row uses T-junctions and cross
                        if is_first_pipe {
                            chars.left_tee
                        } else if is_last_pipe {
                            chars.right_tee
                        } else {
                            chars.cross
                        }
                    } else {
                        // Header and data rows use vertical bars
                        chars.vertical
                    };

                    decorations.push(Decoration::conceal(
                        Span::single_line(line_num, pipe_col, pipe_col + 1),
                        replacement_char.to_string(),
                        Some(style.clone()),
                    ));
                }

                // Replace dashes and colons in delimiter row with horizontal lines
                if is_delimiter(line_num) {
                    for (char_idx, c) in line.char_indices() {
                        if c == '-' || c == ':' {
                            decorations.push(Decoration::conceal(
                                Span::single_line(line_num, char_idx as u32, char_idx as u32 + 1),
                                chars.horizontal.to_string(),
                                Some(style.clone()),
                            ));
                        }
                    }
                }
            }
        }

        decorations
    }

    /// Generate horizontal rule decorations
    #[allow(clippy::cast_possible_truncation)]
    fn render_horizontal_rules(&self, tree: &Tree, content: &str) -> Vec<Decoration> {
        if !self.config.horizontal_rules.enabled {
            return Vec::new();
        }

        let mut decorations = Vec::new();
        let mut cursor = tree_sitter::QueryCursor::new();
        let mut matches = cursor.matches(&self.block_query, tree.root_node(), content.as_bytes());

        while let Some(match_) = matches.next() {
            for capture in match_.captures {
                let capture_name = &self.block_query.capture_names()[capture.index as usize];

                if *capture_name == "decoration.horizontal_rule" {
                    let node = capture.node;
                    let start = node.start_position();
                    // Get the actual text length (--- or *** or ___)
                    let text_len = node
                        .utf8_text(content.as_bytes())
                        .map(|s| s.trim_end().len())
                        .unwrap_or(3);

                    // Create the replacement string
                    let rule_char = self.config.horizontal_rules.character;
                    let width = self.config.horizontal_rules.min_width as usize;
                    let replacement: String = std::iter::repeat_n(rule_char, width).collect();

                    // Use single-line span to avoid multi-line issues
                    decorations.push(Decoration::conceal(
                        Span::single_line(
                            start.row as u32,
                            start.column as u32,
                            start.column as u32 + text_len as u32,
                        ),
                        replacement,
                        Some(self.config.horizontal_rules.style.clone()),
                    ));
                }
            }
        }

        decorations
    }

    /// Generate blockquote decorations
    #[allow(clippy::cast_possible_truncation)]
    fn render_blockquotes(&self, tree: &Tree, content: &str) -> Vec<Decoration> {
        if !self.config.blockquotes.enabled {
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
                    "decoration.blockquote" => {
                        // Apply background to entire blockquote
                        if let Some(bg_style) = &self.config.blockquotes.background {
                            let start_line = node.start_position().row as u32;
                            let end_line = node.end_position().row as u32;
                            decorations.push(Decoration::line_background(
                                start_line,
                                end_line,
                                bg_style.clone(),
                            ));
                        }
                    }
                    "decoration.blockquote.marker" => {
                        let start = node.start_position();
                        let end = node.end_position();

                        decorations.push(Decoration::conceal(
                            Span::new(
                                start.row as u32,
                                start.column as u32,
                                end.row as u32,
                                end.column as u32,
                            ),
                            self.config.blockquotes.marker,
                            Some(self.config.blockquotes.marker_style.clone()),
                        ));
                    }
                    _ => {}
                }
            }
        }

        decorations
    }

    /// Generate inline decorations (emphasis, links, code spans, strikethrough)
    #[allow(clippy::cast_possible_truncation)]
    #[allow(clippy::too_many_lines)]
    fn render_inline(&self, content: &str) -> Vec<Decoration> {
        // Early return if no inline features enabled
        if !self.config.inline.conceal_emphasis
            && !self.config.inline.conceal_links
            && !self.config.inline.conceal_code_span
            && !self.config.inline.conceal_strikethrough
        {
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
                                // Apply link style with optional background
                                let mut link_style = self.config.inline.link_style.clone();
                                if let Some(bg) = &self.config.inline.link_background {
                                    link_style = link_style.merge(bg);
                                }
                                decorations.push(Decoration::inline_style(
                                    Span::new(
                                        text_start.row as u32,
                                        text_start.column as u32,
                                        text_end.row as u32,
                                        text_end.column as u32,
                                    ),
                                    link_style,
                                ));
                            }
                        }
                    }
                    "decoration.code_span" if self.config.inline.conceal_code_span => {
                        // Hide opening backtick
                        decorations.push(Decoration::Hide {
                            span: Span::new(
                                start.row as u32,
                                start.column as u32,
                                start.row as u32,
                                start.column as u32 + 1,
                            ),
                        });
                        // Hide closing backtick
                        decorations.push(Decoration::Hide {
                            span: Span::new(
                                end.row as u32,
                                end.column as u32 - 1,
                                end.row as u32,
                                end.column as u32,
                            ),
                        });
                        // Apply code span style to content
                        decorations.push(Decoration::inline_style(
                            Span::new(
                                start.row as u32,
                                start.column as u32 + 1,
                                end.row as u32,
                                end.column as u32 - 1,
                            ),
                            self.config.inline.code_span_style.clone(),
                        ));
                    }
                    "decoration.strikethrough" if self.config.inline.conceal_strikethrough => {
                        // Hide opening ~~ markers
                        decorations.push(Decoration::Hide {
                            span: Span::new(
                                start.row as u32,
                                start.column as u32,
                                start.row as u32,
                                start.column as u32 + 2,
                            ),
                        });
                        // Hide closing ~~ markers
                        decorations.push(Decoration::Hide {
                            span: Span::new(
                                end.row as u32,
                                end.column as u32 - 2,
                                end.row as u32,
                                end.column as u32,
                            ),
                        });
                        // Apply strikethrough style
                        decorations.push(Decoration::inline_style(
                            Span::new(
                                start.row as u32,
                                start.column as u32 + 2,
                                end.row as u32,
                                end.column as u32 - 2,
                            ),
                            self.config.inline.strikethrough_style.clone(),
                        ));
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
        decorations.extend(self.render_horizontal_rules(tree, content));
        decorations.extend(self.render_blockquotes(tree, content));
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

    #[test]
    fn test_decorator_generates_horizontal_rule_decorations() {
        let mut decorator = MarkdownDecorator::new().expect("Failed to create decorator");
        let content = "Some text.\n\n---\n\nMore text.\n";

        decorator.refresh(content);

        let decorations = decorator.decoration_range(content, 0, 5);

        println!("Generated {} decorations:", decorations.len());
        for d in &decorations {
            println!("  {d:?}");
        }

        // Should have horizontal rule conceal with ─ characters
        let has_hr = decorations.iter().any(
            |d| matches!(d, Decoration::Conceal { replacement, .. } if replacement.contains('─')),
        );
        assert!(has_hr, "Should have horizontal rule decoration");
    }

    #[test]
    fn test_decorator_generates_blockquote_decorations() {
        let mut decorator = MarkdownDecorator::new().expect("Failed to create decorator");
        let content = "> This is a quote.\n> Second line.\n";

        decorator.refresh(content);

        let decorations = decorator.decoration_range(content, 0, 2);

        println!("Generated {} decorations:", decorations.len());
        for d in &decorations {
            println!("  {d:?}");
        }

        // Should have blockquote marker replacements (one per line)
        let marker_count = decorations.iter().filter(|d| {
            matches!(d, Decoration::Conceal { replacement, .. } if replacement.contains('│'))
        }).count();
        assert!(
            marker_count >= 2,
            "Should have at least 2 blockquote markers, got {marker_count}"
        );
    }

    #[test]
    fn test_decorator_generates_table_border_decorations() {
        let mut decorator = MarkdownDecorator::new().expect("Failed to create decorator");
        let content = "| A | B |\n|---|---|\n| 1 | 2 |\n";

        decorator.refresh(content);

        let decorations = decorator.decoration_range(content, 0, 3);

        println!("Content:");
        for (i, line) in content.lines().enumerate() {
            println!("  row {}: {:?}", i, line);
        }

        println!("\nGenerated {} decorations:", decorations.len());
        for d in &decorations {
            println!("  {d:?}");
        }

        // Count vertical pipe replacements (│)
        let vertical_count = decorations
            .iter()
            .filter(|d| matches!(d, Decoration::Conceal { replacement, .. } if replacement == "│"))
            .count();

        // Count T-junction replacements (├, ┤, ┼)
        let junction_count = decorations
            .iter()
            .filter(|d| {
                matches!(d, Decoration::Conceal { replacement, .. } if
                    replacement == "├" || replacement == "┤" || replacement == "┼")
            })
            .count();

        // Count horizontal dash replacements (─)
        let horizontal_count = decorations
            .iter()
            .filter(|d| matches!(d, Decoration::Conceal { replacement, .. } if replacement == "─"))
            .count();

        println!(
            "\nCounts: vertical={}, junction={}, horizontal={}",
            vertical_count, junction_count, horizontal_count
        );

        // Row 0 (| A | B |): 3 pipes -> 3 │
        // Row 1 (|---|---|): 3 pipes -> ├, ┼, ┤ (3 junctions), plus 6 dashes -> 6 ─
        // Row 2 (| 1 | 2 |): 3 pipes -> 3 │
        // Total: 6 │, 3 junctions, 6 ─
        // Note: Top/bottom borders require virtual text support (not yet implemented)

        assert!(
            vertical_count >= 6,
            "Should have at least 6 vertical pipes (│), got {}",
            vertical_count
        );
        assert!(
            junction_count >= 3,
            "Should have at least 3 junctions (├/┼/┤), got {}",
            junction_count
        );
        assert!(
            horizontal_count >= 6,
            "Should have at least 6 horizontal dashes (─), got {}",
            horizontal_count
        );
    }
}
