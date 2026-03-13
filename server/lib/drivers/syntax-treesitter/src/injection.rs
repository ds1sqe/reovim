//! Injection management for recursive embedded language highlighting.
//!
//! This module provides `InjectionManager` for creating and managing child
//! `SyntaxDriver` instances that handle embedded languages (e.g., Rust code
//! blocks in Markdown, Markdown in Rust doc comments).
//!
//! # Architecture
//!
//! ```text
//! TreeSitterDriver("rust")                    <- parent (depth 0)
//!   +-- InjectionManager (depth 0)
//!        +-- Box<dyn SyntaxDriver>("markdown")      <- child (depth 1)
//!             +-- InjectionManager (depth 1)
//!                  +-- Box<dyn SyntaxDriver>("python")  <- grandchild (depth 2)
//! ```
//!
//! # Recursive Injection
//!
//! Each child driver is a full `SyntaxDriver` (typically `TreeSitterDriver`) that
//! can detect and highlight its own injections. This enables arbitrary nesting:
//! Rust doc comments contain Markdown, which can contain fenced code blocks in
//! Python, and so on.
//!
//! Depth is capped at [`MAX_INJECTION_DEPTH`] to prevent infinite recursion
//! (e.g., language A injects B, B injects A).
//!
//! # Lock Ordering Invariant
//!
//! Each child driver owns its own independent locks (parser, tree, content,
//! cursor, `injection_manager`). A child's `highlights()` never acquires any
//! lock belonging to an ancestor. This is guaranteed by construction -- child
//! drivers are separate struct instances. The factory's internal `RwLock` (if
//! any) is only acquired as a read lock during child creation.

use std::{collections::HashMap, ops::Range, sync::Arc};

use reovim_driver_syntax::{Annotation, Injection, SyntaxDriver, SyntaxDriverFactory};

/// Maximum injection nesting depth.
///
/// Prevents infinite recursion in pathological cases (e.g., a language that
/// injects itself). Depth 0 = direct child of root driver, depth 4 = maximum.
pub const MAX_INJECTION_DEPTH: u8 = 4;

/// Manages injection detection and recursive highlighting.
///
/// The injection manager coordinates highlighting of embedded languages by
/// creating child `SyntaxDriver` instances via a shared `SyntaxDriverFactory`.
/// Each child driver is a full `TreeSitterDriver` that can recursively detect
/// and highlight its own injections.
pub struct InjectionManager {
    /// Cached child drivers by language ID.
    children: HashMap<String, Box<dyn SyntaxDriver>>,
    /// Factory for creating child drivers on demand.
    factory: Option<Arc<dyn SyntaxDriverFactory>>,
    /// Current nesting depth (0 = direct child of root driver).
    depth: u8,
}

impl Default for InjectionManager {
    fn default() -> Self {
        Self::new()
    }
}

impl InjectionManager {
    /// Create a new injection manager without a factory.
    ///
    /// Without a factory, no child drivers can be created dynamically.
    /// This is used for drivers at `MAX_INJECTION_DEPTH` (leaf nodes).
    #[must_use]
    pub fn new() -> Self {
        Self {
            children: HashMap::new(),
            factory: None,
            depth: 0,
        }
    }

    /// Create a new injection manager with a factory and depth.
    ///
    /// The factory is used to create child drivers on demand. The depth
    /// controls recursion: when `depth >= MAX_INJECTION_DEPTH`, children
    /// are created without their own injection managers (leaf nodes).
    #[must_use]
    pub fn with_factory(factory: Arc<dyn SyntaxDriverFactory>, depth: u8) -> Self {
        Self {
            children: HashMap::new(),
            factory: Some(factory),
            depth,
        }
    }

    /// Set or replace the factory for dynamic child driver creation.
    pub fn set_factory(&mut self, factory: Arc<dyn SyntaxDriverFactory>) {
        self.factory = Some(factory);
    }

    /// Set the nesting depth.
    pub const fn set_depth(&mut self, depth: u8) {
        self.depth = depth;
    }

    /// Get the current nesting depth.
    #[must_use]
    pub const fn depth(&self) -> u8 {
        self.depth
    }

    /// Check if a child driver exists for a language.
    #[must_use]
    pub fn has_child(&self, language_id: &str) -> bool {
        self.children.contains_key(language_id)
    }

    /// Get the number of cached child drivers.
    #[must_use]
    pub fn child_count(&self) -> usize {
        self.children.len()
    }

    /// Highlight all injections that overlap the given byte range.
    ///
    /// For each injection:
    /// 1. Creates a child driver if one doesn't exist (via factory)
    /// 2. Configures the child's injection manager for recursive highlighting
    /// 3. Parses the embedded content
    /// 4. Calls the child's `highlights()` for recursive highlighting
    /// 5. Collects all child highlights (offset-adjusted to parent coordinates)
    pub fn highlight_injections(
        &mut self,
        injections: &[Injection],
        full_content: &str,
        byte_range: Range<usize>,
    ) -> Vec<Annotation> {
        // Phase 1: Ensure child drivers exist for all injected languages
        if let Some(factory) = &self.factory {
            for injection in injections {
                if !self.children.contains_key(&injection.language_id)
                    && let Some(mut driver) = factory.create(&injection.language_id)
                {
                    // Configure child for recursive injection if not at max depth
                    let child_depth = self.depth + 1;
                    if child_depth < MAX_INJECTION_DEPTH {
                        driver.set_injection_factory(factory.clone());
                    }
                    driver.set_injection_depth(child_depth);
                    self.children.insert(injection.language_id.clone(), driver);
                }
            }
        }

        // Phase 2: Highlight with all available child drivers
        let mut all_highlights = Vec::new();

        for injection in injections {
            let inj_range = injection.byte_range();
            // Check if injection overlaps the requested range
            if inj_range.end <= byte_range.start || inj_range.start >= byte_range.end {
                continue;
            }

            if let Some(driver) = self.children.get_mut(&injection.language_id) {
                let highlights =
                    Self::highlight_single_injection(&mut **driver, injection, full_content);
                all_highlights.extend(highlights);
            }
        }

        all_highlights
    }

    /// Highlight a single injection using the child driver.
    ///
    /// For single-range injections: extracts the substring, parses, highlights,
    /// then offsets annotations to parent document coordinates.
    ///
    /// For multi-range (combined) injections: concatenates content from all
    /// ranges (stripping doc comment prefixes), parses as one document, then
    /// translates annotations back to parent coordinates.
    fn highlight_single_injection(
        driver: &mut dyn SyntaxDriver,
        injection: &Injection,
        full_content: &str,
    ) -> Vec<Annotation> {
        if injection.ranges.is_empty() {
            return Vec::new();
        }

        let overall = injection.byte_range();
        if overall.start >= full_content.len() || overall.end > full_content.len() {
            return Vec::new();
        }

        if injection.ranges.len() == 1 {
            Self::highlight_single_range(driver, &injection.ranges[0], full_content)
        } else {
            Self::highlight_combined_ranges(driver, &injection.ranges, full_content)
        }
    }

    /// Highlight a single-range injection (e.g., a fenced code block).
    fn highlight_single_range(
        driver: &mut dyn SyntaxDriver,
        range: &Range<usize>,
        full_content: &str,
    ) -> Vec<Annotation> {
        let content = &full_content[range.clone()];
        if content.is_empty() {
            return Vec::new();
        }

        driver.parse(content);
        let mut highlights = driver.highlights(0..content.len());

        // Offset highlights to parent document coordinates
        for h in &mut highlights {
            h.start_byte += range.start;
            h.end_byte += range.start;
        }

        highlights
    }

    /// Highlight a combined (multi-range) injection (e.g., doc comment lines).
    ///
    /// Concatenates content from all ranges (stripping doc comment prefixes),
    /// parses as one document, then translates annotations back to parent
    /// coordinates.
    fn highlight_combined_ranges(
        driver: &mut dyn SyntaxDriver,
        ranges: &[Range<usize>],
        full_content: &str,
    ) -> Vec<Annotation> {
        let mut combined_content = String::new();
        // (src_start, dst_start, prefix_len) for offset translation
        let mut range_offsets: Vec<(usize, usize, usize)> = Vec::new();

        for range in ranges {
            if range.start >= full_content.len() || range.end > full_content.len() {
                continue;
            }
            let line = &full_content[range.clone()];
            let (stripped, prefix_len) = strip_doc_comment_prefix(line);
            let dst_start = combined_content.len();
            combined_content.push_str(stripped);
            combined_content.push('\n');
            range_offsets.push((range.start, dst_start, prefix_len));
        }

        if combined_content.is_empty() {
            return Vec::new();
        }

        driver.parse(&combined_content);
        let highlights = driver.highlights(0..combined_content.len());

        // Translate highlights from concatenated coordinates back to parent
        let mut result = Vec::new();
        for h in highlights {
            if let Some(translated) = translate_combined_highlight(&h, &range_offsets, ranges) {
                result.push(translated);
            }
        }

        result
    }

    /// Invalidate all cached child drivers.
    ///
    /// Forces re-creation on next highlight. Call when the parent document
    /// content changes significantly.
    pub fn invalidate(&mut self) {
        self.children.clear();
    }
}

impl std::fmt::Debug for InjectionManager {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("InjectionManager")
            .field("child_count", &self.children.len())
            .field("languages", &self.children.keys().collect::<Vec<_>>())
            .field("has_factory", &self.factory.is_some())
            .field("depth", &self.depth)
            .finish_non_exhaustive()
    }
}

/// Strip doc comment prefix from a line.
///
/// Handles `/// `, `///`, `//! `, `//!` prefixes.
/// Returns (`stripped_content`, `prefix_byte_length`).
fn strip_doc_comment_prefix(line: &str) -> (&str, usize) {
    // Try "/// " (with space)
    if let Some(rest) = line.strip_prefix("/// ") {
        return (rest, 4);
    }
    // Try "///" (no space, but not "////")
    if let Some(rest) = line.strip_prefix("///")
        && !rest.starts_with('/')
    {
        return (rest, 3);
    }
    // Try "//! " (with space)
    if let Some(rest) = line.strip_prefix("//! ") {
        return (rest, 4);
    }
    // Try "//!" (no space)
    if let Some(rest) = line.strip_prefix("//!") {
        return (rest, 3);
    }
    // No prefix to strip
    (line, 0)
}

/// Translate a highlight from concatenated content coordinates to parent
/// document coordinates.
fn translate_combined_highlight(
    highlight: &Annotation,
    range_offsets: &[(usize, usize, usize)], // (src_start, dst_start, prefix_len)
    source_ranges: &[Range<usize>],
) -> Option<Annotation> {
    for (i, &(src_start, dst_start, prefix_len)) in range_offsets.iter().enumerate() {
        let src_range = &source_ranges[i];
        let stripped_len = (src_range.end - src_range.start).saturating_sub(prefix_len);
        let dst_end = dst_start + stripped_len;

        if highlight.start_byte >= dst_start && highlight.start_byte < dst_end {
            let offset_in_chunk = highlight.start_byte - dst_start;
            let start = src_start + prefix_len + offset_in_chunk;

            let end_offset = highlight
                .end_byte
                .saturating_sub(dst_start)
                .min(stripped_len);
            let end = src_start + prefix_len + end_offset;

            return Some(Annotation::new(start, end, highlight.category.clone()));
        }
    }
    None
}

#[cfg(test)]
#[path = "injection_tests.rs"]
mod tests;
