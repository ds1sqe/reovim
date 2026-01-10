use crate::render::{Decoration, DecorationKind, LineHighlight};

use super::Buffer;

impl Buffer {
    /// Get hash for a line (for highlight cache validation)
    ///
    /// Used by saturator to detect if line content changed.
    #[must_use]
    pub fn line_hash(&self, line_idx: usize) -> u64 {
        use std::hash::{Hash, Hasher};
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        if let Some(line) = self.contents.get(line_idx) {
            line.inner.hash(&mut hasher);
        }
        hasher.finish()
    }

    /// Update highlight cache for viewport (SATURATOR - call BEFORE render)
    ///
    /// This is the SLOW path that computes highlights. Runs outside render loop.
    /// Render just reads from cache - never calls this.
    #[allow(clippy::cast_possible_truncation)]
    pub fn update_highlights(&mut self, viewport_start: u16, viewport_end: u16) {
        let Some(syntax) = self.syntax.as_ref() else {
            return;
        };

        let line_count = self.contents.len();
        let start = viewport_start as usize;
        let end = (viewport_end as usize).min(line_count);

        // Check if any lines need computation (cache miss)
        let has_cache_miss = (start..end).any(|line_idx| {
            let hash = self.line_hash(line_idx);
            !self.highlight_cache.has(line_idx, hash)
        });

        if !has_cache_miss {
            return; // All cached, nothing to do
        }

        // Build content string for syntax analysis
        let content: String = self
            .contents
            .iter()
            .map(|line| line.inner.as_str())
            .collect::<Vec<_>>()
            .join("\n");

        // Compute highlights (SLOW - ~46ms)
        let all_highlights =
            syntax.highlight_range(&content, u32::from(viewport_start), u32::from(viewport_end));

        // Group by line
        let mut line_highlights: Vec<Vec<LineHighlight>> = vec![Vec::new(); line_count];
        for hl in all_highlights {
            let line_idx = hl.span.start_line as usize;
            if line_idx < line_highlights.len() {
                // Single-line highlight
                if hl.span.start_line == hl.span.end_line {
                    line_highlights[line_idx].push(LineHighlight {
                        start_col: hl.span.start_col as usize,
                        end_col: hl.span.end_col as usize,
                        style: hl.style,
                    });
                } else {
                    // Multi-line highlight: split across lines
                    let line_len = self.contents.get(line_idx).map_or(0, |l| l.inner.len());
                    line_highlights[line_idx].push(LineHighlight {
                        start_col: hl.span.start_col as usize,
                        end_col: line_len,
                        style: hl.style.clone(),
                    });

                    // Middle lines
                    for mid_line in (hl.span.start_line + 1)..hl.span.end_line {
                        let mid_idx = mid_line as usize;
                        if mid_idx < line_highlights.len() {
                            let mid_len = self.contents.get(mid_idx).map_or(0, |l| l.inner.len());
                            line_highlights[mid_idx].push(LineHighlight {
                                start_col: 0,
                                end_col: mid_len,
                                style: hl.style.clone(),
                            });
                        }
                    }

                    // End line
                    let end_idx = hl.span.end_line as usize;
                    if end_idx < line_highlights.len() {
                        line_highlights[end_idx].push(LineHighlight {
                            start_col: 0,
                            end_col: hl.span.end_col as usize,
                            style: hl.style,
                        });
                    }
                }
            }
        }

        // Sort highlights by start column for each line
        for line_hl in &mut line_highlights {
            line_hl.sort_by_key(|h| h.start_col);
        }

        // Clone current cache entries to preserve cached lines outside viewport
        let mut new_entries = self.highlight_cache.clone_entries();

        // Insert new highlights for viewport lines
        for line_idx in start..end {
            let hash = self.line_hash(line_idx);
            new_entries.insert(
                line_idx,
                (hash, line_highlights.get(line_idx).cloned().unwrap_or_default()),
            );
        }

        // Atomic store (lock-free swap)
        self.highlight_cache.store(new_entries);
    }

    /// Update decoration cache for viewport (call BEFORE render)
    ///
    /// This is the SLOW path - runs outside render loop.
    /// Same pattern as `update_highlights()`.
    #[allow(clippy::too_many_lines)]
    pub fn update_decorations(&mut self, viewport_start: u16, viewport_end: u16) {
        let Some(decorator) = self.decoration_provider.as_ref() else {
            return;
        };

        let line_count = self.contents.len();
        let start = viewport_start as usize;
        let end = (viewport_end as usize).min(line_count);

        // Check if any lines need computation (cache miss)
        let has_cache_miss = (start..end).any(|line_idx| {
            let hash = self.line_hash(line_idx);
            !self.decoration_cache.has(line_idx, hash)
        });

        if !has_cache_miss {
            return; // All cached, nothing to do
        }

        // Build content string for decoration analysis
        let content: String = self
            .contents
            .iter()
            .map(|line| line.inner.as_str())
            .collect::<Vec<_>>()
            .join("\n");

        // Compute decorations (SLOW - ~46ms)
        let all_decorations = decorator.decoration_range(
            &content,
            u32::from(viewport_start),
            u32::from(viewport_end),
        );

        // Group by line
        let mut line_decorations: Vec<Vec<Decoration>> = vec![Vec::new(); line_count];
        for deco in all_decorations {
            match deco {
                crate::decoration::Decoration::Conceal {
                    span,
                    replacement,
                    style,
                } => {
                    let line_idx = span.start_line as usize;
                    if line_idx < line_decorations.len() && span.start_line == span.end_line {
                        line_decorations[line_idx].push(Decoration {
                            start_col: span.start_col as usize,
                            end_col: span.end_col as usize,
                            kind: DecorationKind::Conceal {
                                replacement: Some(replacement),
                                col_mapping: None,
                            },
                        });
                        // If there's a style, also add it as a background decoration
                        if let Some(style) = style {
                            line_decorations[line_idx].push(Decoration {
                                start_col: span.start_col as usize,
                                end_col: span.end_col as usize,
                                kind: DecorationKind::Background { style },
                            });
                        }
                    }
                }
                crate::decoration::Decoration::Hide { span } => {
                    let line_idx = span.start_line as usize;
                    if line_idx < line_decorations.len() && span.start_line == span.end_line {
                        line_decorations[line_idx].push(Decoration {
                            start_col: span.start_col as usize,
                            end_col: span.end_col as usize,
                            kind: DecorationKind::Conceal {
                                replacement: None,
                                col_mapping: None,
                            },
                        });
                    }
                }
                crate::decoration::Decoration::LineBackground {
                    start_line,
                    end_line,
                    style,
                } => {
                    for line in start_line..=end_line {
                        let line_idx = line as usize;
                        if line_idx < line_decorations.len() {
                            let line_len = self.contents.get(line_idx).map_or(0, |l| l.inner.len());
                            line_decorations[line_idx].push(Decoration {
                                start_col: 0,
                                end_col: line_len,
                                kind: DecorationKind::Background {
                                    style: style.clone(),
                                },
                            });
                        }
                    }
                }
                crate::decoration::Decoration::InlineStyle { span, style } => {
                    let line_idx = span.start_line as usize;
                    if line_idx < line_decorations.len() && span.start_line == span.end_line {
                        line_decorations[line_idx].push(Decoration {
                            start_col: span.start_col as usize,
                            end_col: span.end_col as usize,
                            kind: DecorationKind::Background { style },
                        });
                    }
                }
            }
        }

        // Sort decorations by start column for each line
        for line_deco in &mut line_decorations {
            line_deco.sort_by_key(|d| d.start_col);
        }

        // Clone current cache entries to preserve cached lines outside viewport
        let mut new_entries = self.decoration_cache.clone_entries();

        // Insert new decorations for viewport lines
        for line_idx in start..end {
            let hash = self.line_hash(line_idx);
            new_entries.insert(
                line_idx,
                (hash, line_decorations.get(line_idx).cloned().unwrap_or_default()),
            );
        }

        // Atomic store (lock-free swap)
        self.decoration_cache.store(new_entries);
    }
}
