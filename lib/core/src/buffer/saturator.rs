//! Per-buffer saturator for background cache computation
//!
//! The saturator:
//! - Owns syntax and decoration providers (moved from Buffer)
//! - Holds Arc references to caches (shared with Buffer/Render)
//! - Runs as a background tokio task
//! - Computes highlights/decorations without blocking render
//! - Stores results via lock-free `ArcSwap`
//! - Sends `RenderSignal` when cache is updated

use std::sync::Arc;

use tokio::sync::mpsc;

use crate::{
    decoration::DecorationProvider,
    event::InnerEvent,
    render::{Decoration, DecorationCache, DecorationKind, HighlightCache, LineHighlight},
    syntax::SyntaxProvider,
};

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
}

/// Handle to communicate with a saturator task
#[derive(Debug)]
pub struct SaturatorHandle {
    /// Send requests to the saturator
    pub tx: mpsc::Sender<SaturatorRequest>,
}

impl SaturatorHandle {
    /// Request a cache update for the given viewport
    ///
    /// Non-blocking: uses `try_send` which skips if saturator is busy.
    pub fn request_update(&self, request: SaturatorRequest) {
        // Use try_send to avoid blocking - if saturator is busy, skip
        let _ = self.tx.try_send(request);
    }
}

/// Spawn a saturator task for a buffer
///
/// Returns a handle to send requests to the saturator.
///
/// The saturator owns the providers and updates the caches.
pub fn spawn_saturator(
    syntax: Option<Box<dyn SyntaxProvider>>,
    decoration: Option<Box<dyn DecorationProvider>>,
    highlight_cache: Arc<HighlightCache>,
    decoration_cache: Arc<DecorationCache>,
    event_tx: mpsc::Sender<InnerEvent>,
) -> SaturatorHandle {
    // Channel with buffer of 1 - only latest request matters
    let (tx, mut rx) = mpsc::channel::<SaturatorRequest>(1);

    tokio::spawn(async move {
        let mut syntax = syntax;
        let decoration = decoration;

        while let Some(request) = rx.recv().await {
            let mut cache_updated = false;

            // Update highlights if syntax provider exists
            if let Some(ref mut syn) = syntax {
                let updated = update_highlights(
                    syn.as_mut(),
                    &request,
                    &highlight_cache,
                );
                cache_updated |= updated;
            }

            // Update decorations if decoration provider exists
            if let Some(ref decorator) = decoration {
                let updated = update_decorations(
                    decorator.as_ref(),
                    &request,
                    &decoration_cache,
                );
                cache_updated |= updated;
            }

            // Signal re-render if cache was updated
            if cache_updated {
                let _ = event_tx.send(InnerEvent::RenderSignal).await;
            }
        }
    });

    SaturatorHandle { tx }
}

/// Update highlight cache for viewport range
///
/// Returns true if cache was updated.
#[allow(clippy::too_many_lines)]
fn update_highlights(
    syntax: &dyn SyntaxProvider,
    request: &SaturatorRequest,
    cache: &Arc<HighlightCache>,
) -> bool {
    let start = request.viewport_start as usize;
    let end = (request.viewport_end as usize).min(request.line_count);

    // Check if any lines need computation (cache miss)
    let has_cache_miss = (start..end).any(|line_idx| {
        let hash = request.line_hashes.get(line_idx).copied().unwrap_or(0);
        !cache.has(line_idx, hash)
    });

    if !has_cache_miss {
        return false; // All cached, nothing to do
    }

    // Compute highlights (SLOW - ~46ms)
    let all_highlights = syntax.highlight_range(
        &request.content,
        u32::from(request.viewport_start),
        u32::from(request.viewport_end),
    );

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
        new_entries.insert(
            line_idx,
            (hash, line_highlights.get(line_idx).cloned().unwrap_or_default()),
        );
    }

    // Atomic store (lock-free swap)
    cache.store(new_entries);

    true
}

/// Update decoration cache for viewport range
///
/// Returns true if cache was updated.
#[allow(clippy::too_many_lines)]
fn update_decorations(
    decorator: &dyn DecorationProvider,
    request: &SaturatorRequest,
    cache: &Arc<DecorationCache>,
) -> bool {
    let start = request.viewport_start as usize;
    let end = (request.viewport_end as usize).min(request.line_count);

    // Check if any lines need computation (cache miss)
    let has_cache_miss = (start..end).any(|line_idx| {
        let hash = request.line_hashes.get(line_idx).copied().unwrap_or(0);
        !cache.has(line_idx, hash)
    });

    if !has_cache_miss {
        return false; // All cached, nothing to do
    }

    // Compute decorations (SLOW - ~46ms)
    let all_decorations = decorator.decoration_range(
        &request.content,
        u32::from(request.viewport_start),
        u32::from(request.viewport_end),
    );

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
                        kind: DecorationKind::Conceal { replacement: None },
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

    // Sort decorations by start column for each line
    for line_deco in &mut line_decorations {
        line_deco.sort_by_key(|d| d.start_col);
    }

    // Clone current cache entries to preserve cached lines outside viewport
    let mut new_entries = cache.clone_entries();

    // Insert new decorations for viewport lines
    for line_idx in start..end {
        let hash = request.line_hashes.get(line_idx).copied().unwrap_or(0);
        new_entries.insert(
            line_idx,
            (hash, line_decorations.get(line_idx).cloned().unwrap_or_default()),
        );
    }

    // Atomic store (lock-free swap)
    cache.store(new_entries);

    true
}
