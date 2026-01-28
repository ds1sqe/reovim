//! Buffer module for text storage and manipulation

mod access;
mod cache;
mod cursor;
mod delimiter;
mod edit;
mod history;
mod motion_ops;
mod position;
pub mod saturator;
mod selection;
mod text;
mod undo;
mod word;

#[cfg(test)]
mod tests;

pub use {
    access::{BufferRead, BufferSnapshot},
    cursor::{CursorOps, calculate_motion, calculate_motion_with_desired_col},
    history::{Change, UndoHistory},
    selection::{Selection, SelectionMode, SelectionOps},
    text::TextOps,
};

use std::sync::Arc;

use crate::{
    decoration::DecorationProvider,
    motion::Motion,
    render::{DecorationCache, HighlightCache},
    syntax::SyntaxProvider,
};

use crate::screen::Position;

#[derive(Clone, Debug)]
pub struct Line {
    pub inner: String,
}

impl From<&str> for Line {
    fn from(value: &str) -> Self {
        Self {
            inner: value.to_string(),
        }
    }
}

/// Buffer for text storage and manipulation
///
/// Each buffer owns its syntax state (if a language was detected).
pub struct Buffer {
    pub id: usize,
    pub cur: Position,
    /// Track preferred column for vertical movement (j/k)
    /// Used to preserve horizontal position when moving through lines of different lengths
    pub desired_col: Option<u16>,
    pub contents: Vec<Line>,
    pub selection: Selection,
    pub file_path: Option<String>,
    /// Whether the buffer has unsaved modifications
    pub modified: bool,
    /// Undo/redo history
    history: UndoHistory,
    /// Changes accumulated during insert mode to be batched as single undo unit
    pending_batch: Vec<Change>,
    /// Whether batching is active (during insert mode)
    batching: bool,
    /// Syntax provider for highlighting (None if no language detected)
    /// Note: Moved to saturator when `start_saturator()` is called
    syntax: Option<Box<dyn SyntaxProvider>>,
    /// Decoration provider for visual decorations (None if no language-specific decorations)
    /// Note: Moved to saturator when `start_saturator()` is called
    decoration_provider: Option<Box<dyn DecorationProvider>>,
    /// Double-buffered highlight cache (saturator writes, render reads)
    /// Wrapped in Arc for sharing with background saturator task
    pub highlight_cache: Arc<HighlightCache>,
    /// Double-buffered decoration cache (saturator writes, render reads)
    /// Wrapped in Arc for sharing with background saturator task
    pub decoration_cache: Arc<DecorationCache>,
    /// Handle to background saturator task (if started)
    saturator: Option<saturator::SaturatorHandle>,
}

impl Clone for Buffer {
    fn clone(&self) -> Self {
        Self {
            id: self.id,
            cur: self.cur,
            desired_col: self.desired_col,
            contents: self.contents.clone(),
            selection: self.selection,
            file_path: self.file_path.clone(),
            modified: self.modified,
            history: self.history.clone(),
            pending_batch: self.pending_batch.clone(),
            batching: self.batching,
            // Syntax state is not cloned - it can be reattached if needed
            syntax: None,
            // Decoration provider is not cloned - it can be reattached if needed
            decoration_provider: None,
            // Highlight cache is cloned (cheap, just HashMaps)
            highlight_cache: self.highlight_cache.clone(),
            // Decoration cache is cloned (cheap, just HashMaps)
            decoration_cache: self.decoration_cache.clone(),
            // Saturator is not cloned - it can be restarted if needed
            saturator: None,
        }
    }
}

impl std::fmt::Debug for Buffer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Buffer")
            .field("id", &self.id)
            .field("cur", &self.cur)
            .field("desired_col", &self.desired_col)
            .field("contents", &self.contents)
            .field("selection", &self.selection)
            .field("file_path", &self.file_path)
            .field("modified", &self.modified)
            .field("history", &self.history)
            .field("pending_batch", &self.pending_batch)
            .field("batching", &self.batching)
            .field("syntax", &self.syntax.as_ref().map(|s| s.language_id()))
            .field(
                "decoration_provider",
                &self.decoration_provider.as_ref().map(|d| d.language_id()),
            )
            .field("highlight_cache", &self.highlight_cache)
            .field("decoration_cache", &self.decoration_cache)
            .field("has_saturator", &self.saturator.is_some())
            .finish()
    }
}

impl Buffer {
    #[must_use]
    pub fn empty(id: usize) -> Self {
        Self {
            id,
            cur: Position { x: 0, y: 0 },
            desired_col: None,
            contents: Vec::new(),
            selection: Selection::default(),
            file_path: None,
            modified: false,
            history: UndoHistory::new(),
            pending_batch: Vec::new(),
            batching: false,
            syntax: None,
            decoration_provider: None,
            highlight_cache: Arc::new(HighlightCache::new()),
            decoration_cache: Arc::new(DecorationCache::new()),
            saturator: None,
        }
    }

    /// Attach a syntax provider for this buffer
    ///
    /// The syntax provider is used for highlighting and other syntax-aware features.
    pub fn attach_syntax(&mut self, syntax: Box<dyn SyntaxProvider>) {
        self.syntax = Some(syntax);
    }

    /// Detach and return the syntax provider
    ///
    /// Returns None if no syntax was attached.
    pub fn detach_syntax(&mut self) -> Option<Box<dyn SyntaxProvider>> {
        self.syntax.take()
    }

    /// Get a reference to the syntax provider if available
    #[must_use]
    pub fn syntax(&self) -> Option<&dyn SyntaxProvider> {
        self.syntax.as_deref()
    }

    /// Get a mutable reference to the syntax provider
    pub fn syntax_mut(&mut self) -> Option<&mut (dyn SyntaxProvider + 'static)> {
        self.syntax.as_deref_mut()
    }

    /// Check if buffer has syntax highlighting enabled
    #[must_use]
    pub fn has_syntax(&self) -> bool {
        self.syntax.is_some()
    }

    /// Attach a decoration provider for this buffer
    ///
    /// The decoration provider is used for language-specific visual decorations
    /// like conceals, backgrounds, and inline styles.
    pub fn attach_decoration_provider(&mut self, provider: Box<dyn DecorationProvider>) {
        self.decoration_provider = Some(provider);
    }

    /// Detach and return the decoration provider
    ///
    /// Returns None if no decoration provider was attached.
    pub fn detach_decoration_provider(&mut self) -> Option<Box<dyn DecorationProvider>> {
        self.decoration_provider.take()
    }

    /// Get a reference to the decoration provider if available
    #[must_use]
    pub fn decoration_provider(&self) -> Option<&dyn DecorationProvider> {
        self.decoration_provider.as_deref()
    }

    /// Get a mutable reference to the decoration provider
    pub fn decoration_provider_mut(&mut self) -> Option<&mut (dyn DecorationProvider + 'static)> {
        self.decoration_provider.as_deref_mut()
    }

    /// Check if buffer has decoration provider enabled
    #[must_use]
    pub fn has_decoration_provider(&self) -> bool {
        self.decoration_provider.is_some()
    }

    /// Start the background saturator task
    ///
    /// This moves the syntax and decoration providers to a background task.
    /// The saturator computes highlights/decorations without blocking render.
    /// Call this after attaching syntax/decoration providers.
    ///
    /// # Arguments
    /// * `event_tx` - Channel to send `RenderSignal` when cache is updated
    pub fn start_saturator(
        &mut self,
        event_tx: tokio::sync::mpsc::Sender<crate::event::RuntimeEvent>,
    ) {
        // Only start if we have something to compute
        if self.syntax.is_none() && self.decoration_provider.is_none() {
            return;
        }

        // Move providers to saturator (they're now owned by the task)
        let syntax = self.syntax.take();
        let decoration = self.decoration_provider.take();

        // Spawn the saturator task
        let handle = saturator::spawn_saturator(
            syntax,
            decoration,
            Arc::clone(&self.highlight_cache),
            Arc::clone(&self.decoration_cache),
            event_tx,
        );

        self.saturator = Some(handle);
    }

    /// Check if saturator is running
    #[must_use]
    pub const fn has_saturator(&self) -> bool {
        self.saturator.is_some()
    }

    /// Request the saturator to update cache for viewport
    ///
    /// This is non-blocking - the request is sent to the background task.
    /// If the saturator is busy, the request may be dropped (only latest matters).
    pub fn request_saturator_update(&self, viewport_start: u16, viewport_end: u16) {
        let Some(handle) = &self.saturator else {
            return;
        };

        // Build content snapshot
        let content: String = self
            .contents
            .iter()
            .map(|line| line.inner.as_str())
            .collect::<Vec<_>>()
            .join("\n");

        // Build line hashes
        let line_hashes: Vec<u64> = (0..self.contents.len())
            .map(|i| self.line_hash(i))
            .collect();

        let request = saturator::SaturatorRequest {
            content,
            line_count: self.contents.len(),
            line_hashes,
            viewport_start,
            viewport_end,
            kind: saturator::SaturatorRequestKind::Viewport,
        };

        // Non-blocking send - uses request_update internally
        // We don't await here since this is called from sync context
        handle.request_update(request);
    }

    /// Check if undo is available
    #[must_use]
    pub const fn can_undo(&self) -> bool {
        self.history.can_undo()
    }

    /// Check if redo is available
    #[must_use]
    pub const fn can_redo(&self) -> bool {
        self.history.can_redo()
    }
}

// === Selection Operations ===
impl SelectionOps for Buffer {
    fn start_selection(&mut self) {
        self.selection.anchor = self.cur;
        self.selection.active = true;
        self.selection.mode = SelectionMode::Character;
    }

    fn start_block_selection(&mut self) {
        self.selection.anchor = self.cur;
        self.selection.active = true;
        self.selection.mode = SelectionMode::Block;
    }

    fn start_line_selection(&mut self) {
        self.selection.anchor = self.cur;
        self.selection.active = true;
        self.selection.mode = SelectionMode::Line;
    }

    fn clear_selection(&mut self) {
        self.selection.active = false;
    }

    #[allow(clippy::missing_const_for_fn)]
    fn selection_bounds(&self) -> (Position, Position) {
        let anchor = self.selection.anchor;
        let cursor = self.cur;

        if anchor.y < cursor.y || (anchor.y == cursor.y && anchor.x <= cursor.x) {
            (anchor, cursor)
        } else {
            (cursor, anchor)
        }
    }

    #[allow(clippy::missing_const_for_fn)]
    fn block_bounds(&self) -> (Position, Position) {
        let anchor = self.selection.anchor;
        let cursor = self.cur;
        let top_left = Position {
            x: anchor.x.min(cursor.x),
            y: anchor.y.min(cursor.y),
        };
        let bottom_right = Position {
            x: anchor.x.max(cursor.x),
            y: anchor.y.max(cursor.y),
        };
        (top_left, bottom_right)
    }

    fn selection_mode(&self) -> SelectionMode {
        self.selection.mode
    }

    fn get_selected_text(&self) -> String {
        if !self.selection.active {
            return String::new();
        }

        match self.selection.mode {
            SelectionMode::Block => {
                let (top_left, bottom_right) = self.block_bounds();
                self.extract_block_text(top_left, bottom_right)
            }
            SelectionMode::Character | SelectionMode::Line => {
                let (start, end) = self.selection_bounds();
                self.extract_text(start, end, true) // Visual selection is inclusive
            }
        }
    }

    #[allow(clippy::cast_possible_truncation)]
    fn delete_selection(&mut self) -> String {
        if !self.selection.active {
            return String::new();
        }

        // Handle block mode deletion separately
        if self.selection.mode == SelectionMode::Block {
            let (top_left, bottom_right) = self.block_bounds();
            return self.delete_block(top_left, bottom_right);
        }

        // Character mode deletion
        let text = self.get_selected_text();
        let (start, end) = self.selection_bounds();

        if start.y == end.y {
            // Single line deletion
            if let Some(line) = self.contents.get_mut(start.y as usize) {
                let start_x = start.x as usize;
                let end_x = (end.x as usize + 1).min(line.inner.len());
                if start_x < line.inner.len() {
                    line.inner.drain(start_x..end_x);
                }
            }
        } else {
            // Multi-line deletion
            if let Some(first_line) = self.contents.get(start.y as usize) {
                let prefix = first_line.inner[..start.x as usize].to_string();
                if let Some(last_line) = self.contents.get(end.y as usize) {
                    let end_x = (end.x as usize + 1).min(last_line.inner.len());
                    let suffix = last_line.inner[end_x..].to_string();

                    // Remove lines from end to start+1
                    for _ in (start.y + 1..=end.y).rev() {
                        if (start.y as usize + 1) < self.contents.len() {
                            self.contents.remove(start.y as usize + 1);
                        }
                    }

                    // Merge prefix and suffix into start line
                    if let Some(line) = self.contents.get_mut(start.y as usize) {
                        line.inner = prefix + &suffix;
                    }
                }
            }
        }

        self.cur = start;
        self.clear_selection();
        text
    }
}

// === Text Operations ===
impl TextOps for Buffer {
    fn set_content(&mut self, content: &str) {
        self.contents.clear();
        for line in content.lines() {
            let new_line = Line::from(line);
            self.contents.push(new_line);
        }
        // Invalidate ALL caches (full content replacement)
        self.highlight_cache.clear();
        self.decoration_cache.clear();
    }

    fn content_to_string(&self) -> String {
        self.contents
            .iter()
            .map(|line| line.inner.as_str())
            .collect::<Vec<_>>()
            .join("\n")
    }

    #[allow(clippy::cast_possible_truncation)]
    fn insert_char(&mut self, c: char) {
        let pos = self.cur;
        if self.contents.is_empty() {
            self.contents.push(Line::from(""));
        }
        if let Some(line) = self.contents.get_mut(self.cur.y as usize) {
            let x = self.cur.x as usize;
            if x <= line.inner.len() {
                line.inner.insert(x, c);
                self.cur.x += 1;
                // Record change for undo
                self.record_change(Change::Insert {
                    pos,
                    text: c.to_string(),
                });
                // Invalidate single line caches
                self.highlight_cache.invalidate_line(self.cur.y as usize);
                self.decoration_cache.invalidate_line(self.cur.y as usize);
            }
        }
    }

    #[allow(clippy::cast_possible_truncation)]
    fn insert_newline(&mut self) {
        let pos = self.cur;
        if self.contents.is_empty() {
            self.contents.push(Line::from(""));
        }
        let y = self.cur.y as usize;
        let x = self.cur.x as usize;

        if let Some(line) = self.contents.get_mut(y) {
            // Split the current line at cursor position
            let rest = if x < line.inner.len() {
                line.inner.split_off(x)
            } else {
                String::new()
            };
            // Insert the rest as a new line below
            self.contents.insert(y + 1, Line { inner: rest });
        }
        // Move cursor to start of the new line
        self.cur.y += 1;
        self.cur.x = 0;
        // Record change for undo
        self.record_change(Change::Insert {
            pos,
            text: "\n".to_string(),
        });
        // Invalidate from original line to end (lines shifted)
        self.highlight_cache.invalidate_from(y);
        self.decoration_cache.invalidate_from(y);
    }

    #[allow(clippy::cast_possible_truncation)]
    fn delete_char_backward(&mut self) {
        if self.cur.x > 0
            && let Some(line) = self.contents.get_mut(self.cur.y as usize)
        {
            let x = (self.cur.x - 1) as usize;
            if x < line.inner.len() {
                let deleted_char = line.inner.remove(x);
                self.cur.x -= 1;
                // Record change for undo
                self.record_change(Change::Delete {
                    pos: self.cur,
                    text: deleted_char.to_string(),
                });
                // Invalidate single line caches
                self.highlight_cache.invalidate_line(self.cur.y as usize);
                self.decoration_cache.invalidate_line(self.cur.y as usize);
            }
        }
    }

    #[allow(clippy::cast_possible_truncation)]
    fn delete_char_forward(&mut self) {
        let pos = self.cur;
        if let Some(line) = self.contents.get_mut(self.cur.y as usize) {
            let x = self.cur.x as usize;
            if x < line.inner.len() {
                let deleted_char = line.inner.remove(x);
                // Record change for undo
                self.record_change(Change::Delete {
                    pos,
                    text: deleted_char.to_string(),
                });
                // Invalidate single line caches
                self.highlight_cache.invalidate_line(self.cur.y as usize);
                self.decoration_cache.invalidate_line(self.cur.y as usize);
            }
        }
    }

    #[allow(clippy::cast_possible_truncation)]
    fn delete_line(&mut self) -> String {
        let y = self.cur.y as usize;
        if y < self.contents.len() {
            let deleted_line = self.contents.remove(y);
            // Record change for undo (include newline if not last line)
            let pos = Position {
                x: 0,
                y: self.cur.y,
            };
            let text = if y < self.contents.len() {
                deleted_line.inner + "\n"
            } else {
                deleted_line.inner
            };
            self.record_change(Change::Delete {
                pos,
                text: text.clone(),
            });

            if self.cur.y as usize >= self.contents.len() && !self.contents.is_empty() {
                self.cur.y = (self.contents.len() - 1) as u16;
            }
            // Invalidate from deleted line to end (lines shifted)
            self.highlight_cache.invalidate_from(y);
            self.decoration_cache.invalidate_from(y);
            return text;
        }
        String::new()
    }
}

// === Cursor Operations ===
impl CursorOps for Buffer {
    fn word_forward(&mut self) {
        self.cur = calculate_motion(&self.contents, self.cur, Motion::WordForward, 1);
    }

    fn word_backward(&mut self) {
        self.cur = calculate_motion(&self.contents, self.cur, Motion::WordBackward, 1);
    }

    fn word_end(&mut self) {
        self.cur = calculate_motion(&self.contents, self.cur, Motion::WordEnd, 1);
    }

    fn apply_motion(&mut self, motion: Motion, count: usize) {
        self.cur = calculate_motion(&self.contents, self.cur, motion, count);
    }
}
