//! Per-buffer saturator for background cache computation
//!
//! The saturator:
//! - Owns syntax and decoration providers (moved from Buffer)
//! - Holds Arc references to caches (shared with Buffer/Render)
//! - Runs as a background tokio task
//! - Computes highlights/decorations without blocking render
//! - Stores results via lock-free `ArcSwap`
//! - Sends `RenderSignal` when cache is updated
//!
//! ## Two-Path Architecture
//!
//! The saturator supports two computation paths:
//! - **Path 1 (`Viewport`)**: Immediate computation for visible lines (high priority)
//! - **Path 2 (`FullFile`)**: Background computation for entire file (low priority)
//!
//! This ensures instant viewport display while pre-computing the full file
//! for smooth scrolling.

use std::sync::Arc;

use tokio::sync::mpsc;

use crate::{
    decoration::DecorationProvider,
    event::RuntimeEvent,
    render::{Decoration, DecorationCache, DecorationKind, HighlightCache, LineHighlight},
    syntax::SyntaxProvider,
};

/// Request kind for saturator (two-path architecture)
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum SaturatorRequestKind {
    /// Compute for viewport only (immediate, high priority)
    #[default]
    Viewport,
    /// Compute for entire file (background, low priority)
    FullFile,
}

/// Request sent to saturator
#[derive(Debug)]
pub struct SaturatorRequest {
    /// Buffer content snapshot
    pub content: String,
    /// Number of lines in buffer
    pub line_count: usize,
    /// Per-line content hashes for cache validation
    pub line_hashes: Vec<u64>,
    /// Viewport start line
    pub viewport_start: u16,
    /// Viewport end line
    pub viewport_end: u16,
    /// Request kind (viewport or full-file)
    pub kind: SaturatorRequestKind,
}

/// Handle to communicate with a saturator task
#[derive(Debug)]
pub struct SaturatorHandle {
    /// Send viewport requests (high priority)
    viewport_tx: mpsc::Sender<SaturatorRequest>,
    /// Send full-file requests (low priority)
    fullfile_tx: mpsc::Sender<SaturatorRequest>,
}

impl SaturatorHandle {
    /// Request a cache update for the given viewport (PATH 1: immediate)
    ///
    /// Non-blocking: uses `try_send` which skips if saturator is busy.
    /// Automatically queues full-file computation after viewport completes.
    pub fn request_update(&self, request: SaturatorRequest) {
        // Use try_send to avoid blocking - if saturator is busy, skip
        let _ = self.viewport_tx.try_send(request);
    }

    /// Request full-file cache update (PATH 2: background)
    ///
    /// Use this for explicit full-file pre-computation.
    pub fn request_full_update(&self, request: SaturatorRequest) {
        let _ = self.fullfile_tx.try_send(SaturatorRequest {
            kind: SaturatorRequestKind::FullFile,
            ..request
        });
    }
}

/// Spawn a saturator task for a buffer
///
/// Returns a handle to send requests to the saturator.
///
/// The saturator owns the providers and updates the caches.
/// Uses two-path architecture:
/// - Path 1 (Viewport): High priority, immediate computation
/// - Path 2 (FullFile): Low priority, background computation
pub fn spawn_saturator(
    syntax: Option<Box<dyn SyntaxProvider>>,
    decoration: Option<Box<dyn DecorationProvider>>,
    highlight_cache: Arc<HighlightCache>,
    decoration_cache: Arc<DecorationCache>,
    event_tx: mpsc::Sender<RuntimeEvent>,
) -> SaturatorHandle {
    // High priority channel: viewport requests (buffer of 1)
    let (viewport_tx, mut viewport_rx) = mpsc::channel::<SaturatorRequest>(1);
    // Low priority channel: full-file requests (buffer of 1)
    let (fullfile_tx, mut fullfile_rx) = mpsc::channel::<SaturatorRequest>(1);

    // Clone fullfile_tx for auto-queueing after viewport
    let fullfile_tx_clone = fullfile_tx.clone();

    tokio::spawn(async move {
        let mut syntax = syntax;
        let mut decoration = decoration;

        loop {
            tokio::select! {
                biased; // Check viewport first (high priority)

                // PATH 1: Viewport requests (high priority)
                Some(request) = viewport_rx.recv() => {
                    let cache_updated = process_request(
                        &mut syntax,
                        &mut decoration,
                        &request,
                        &highlight_cache,
                        &decoration_cache,
                    );

                    // Signal re-render if cache was updated
                    if cache_updated {
                        let _ = event_tx.send(RuntimeEvent::render_signal()).await;
                    }

                    // Auto-queue full-file computation (PATH 2)
                    let full_request = SaturatorRequest {
                        kind: SaturatorRequestKind::FullFile,
                        content: request.content,
                        line_count: request.line_count,
                        line_hashes: request.line_hashes,
                        viewport_start: request.viewport_start,
                        viewport_end: request.viewport_end,
                    };
                    let _ = fullfile_tx_clone.try_send(full_request);
                }

                // PATH 2: Full-file requests (low priority)
                Some(request) = fullfile_rx.recv() => {
                    let cache_updated = process_request(
                        &mut syntax,
                        &mut decoration,
                        &request,
                        &highlight_cache,
                        &decoration_cache,
                    );

                    // Signal re-render if cache was updated
                    if cache_updated {
                        let _ = event_tx.send(RuntimeEvent::render_signal()).await;
                    }
                }

                else => break, // Both channels closed
            }
        }
    });

    SaturatorHandle {
        viewport_tx,
        fullfile_tx,
    }
}

/// Process a saturator request (shared between viewport and full-file paths)
fn process_request(
    syntax: &mut Option<Box<dyn SyntaxProvider>>,
    decoration: &mut Option<Box<dyn DecorationProvider>>,
    request: &SaturatorRequest,
    highlight_cache: &Arc<HighlightCache>,
    decoration_cache: &Arc<DecorationCache>,
) -> bool {
    let mut cache_updated = false;

    // Determine range based on request kind
    let (start, end) = match request.kind {
        SaturatorRequestKind::Viewport => (
            request.viewport_start as usize,
            (request.viewport_end as usize).min(request.line_count),
        ),
        SaturatorRequestKind::FullFile => (0, request.line_count),
    };

    // Update highlights if syntax provider exists
    if let Some(syn) = syntax.as_mut() {
        let updated = update_highlights(syn.as_mut(), request, start, end, highlight_cache);
        cache_updated |= updated;
    }

    // Update decorations if decoration provider exists
    if let Some(decorator) = decoration.as_mut() {
        let updated = update_decorations(decorator.as_mut(), request, start, end, decoration_cache);
        cache_updated |= updated;
    }

    cache_updated
}

/// Update highlight cache for a line range
///
/// Returns true if cache was updated.
#[allow(clippy::too_many_lines)]
fn update_highlights(
    syntax: &mut dyn SyntaxProvider,
    request: &SaturatorRequest,
    start: usize,
    end: usize,
    cache: &Arc<HighlightCache>,
) -> bool {
    // Check if any lines need computation (cache miss)
    let has_cache_miss = (start..end).any(|line_idx| {
        let hash = request.line_hashes.get(line_idx).copied().unwrap_or(0);
        !cache.has(line_idx, hash)
    });

    if !has_cache_miss {
        return false; // All cached, nothing to do
    }

    // Parse content to update tree and detect injection regions
    // This is critical for language injections (e.g., code blocks in markdown)
    syntax.parse(&request.content);

    // Eagerly saturate injection regions for embedded language highlighting
    // This ensures code blocks in markdown (and doc comments in Rust, etc.)
    // get proper syntax highlighting by pre-computing all injection layers
    syntax.saturate_injections(&request.content);

    // Compute highlights (SLOW - ~46ms for complex grammars)
    // Now includes injection highlights from embedded languages
    #[allow(clippy::cast_possible_truncation)]
    let all_highlights = syntax.highlight_range(&request.content, start as u32, end as u32);

    // Group by line
    let mut line_highlights: Vec<Vec<LineHighlight>> = vec![Vec::new(); request.line_count];

    // Parse content to get line lengths
    let lines: Vec<&str> = request.content.lines().collect();

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
                let line_len = lines.get(line_idx).map_or(0, |l| l.len());
                line_highlights[line_idx].push(LineHighlight {
                    start_col: hl.span.start_col as usize,
                    end_col: line_len,
                    style: hl.style.clone(),
                });

                // Middle lines
                for mid_line in (hl.span.start_line + 1)..hl.span.end_line {
                    let mid_idx = mid_line as usize;
                    if mid_idx < line_highlights.len() {
                        let mid_len = lines.get(mid_idx).map_or(0, |l| l.len());
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
    let mut new_entries = cache.clone_entries();

    // Insert new highlights for viewport lines
    for line_idx in start..end {
        let hash = request.line_hashes.get(line_idx).copied().unwrap_or(0);
        new_entries
            .insert(line_idx, (hash, line_highlights.get(line_idx).cloned().unwrap_or_default()));
    }

    // Atomic store (lock-free swap)
    cache.store(new_entries);

    true
}

/// Update decoration cache for a line range
///
/// Returns true if cache was updated.
#[allow(clippy::too_many_lines)]
fn update_decorations(
    decorator: &mut dyn DecorationProvider,
    request: &SaturatorRequest,
    start: usize,
    end: usize,
    cache: &Arc<DecorationCache>,
) -> bool {
    // Check if any lines need computation (cache miss)
    let has_cache_miss = (start..end).any(|line_idx| {
        let hash = request.line_hashes.get(line_idx).copied().unwrap_or(0);
        !cache.has(line_idx, hash)
    });

    if !has_cache_miss {
        return false; // All cached, nothing to do
    }

    // Re-parse content to update cached tree
    // This matches the pattern from update_highlights() line 122
    decorator.refresh(&request.content);

    // Compute decorations (SLOW - ~46ms)
    #[allow(clippy::cast_possible_truncation)]
    let all_decorations = decorator.decoration_range(&request.content, start as u32, end as u32);

    // Group by line
    let mut line_decorations: Vec<Vec<Decoration>> = vec![Vec::new(); request.line_count];

    // Parse content to get line lengths
    let lines: Vec<&str> = request.content.lines().collect();

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
                        let line_len = lines.get(line_idx).map_or(0, |l| l.len());
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

    // Sort decorations by start column, with Conceal before Background at same position
    // This ensures concealment takes priority over styling
    for line_deco in &mut line_decorations {
        line_deco.sort_by(|a, b| {
            // Primary sort: by start column
            a.start_col.cmp(&b.start_col).then_with(|| {
                // Secondary sort: Conceal before Background (lower = higher priority)
                let kind_priority = |k: &DecorationKind| -> u8 {
                    match k {
                        DecorationKind::Conceal { .. } => 0,
                        DecorationKind::VirtualText { .. } => 1,
                        DecorationKind::Background { .. } => 2,
                    }
                };
                kind_priority(&a.kind).cmp(&kind_priority(&b.kind))
            })
        });
    }

    // Clone current cache entries to preserve cached lines outside viewport
    let mut new_entries = cache.clone_entries();

    // Insert new decorations for viewport lines
    for line_idx in start..end {
        let hash = request.line_hashes.get(line_idx).copied().unwrap_or(0);
        new_entries
            .insert(line_idx, (hash, line_decorations.get(line_idx).cloned().unwrap_or_default()));
    }

    // Atomic store (lock-free swap)
    cache.store(new_entries);

    true
}
