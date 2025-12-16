//! Core Runtime struct and initialization

use std::collections::BTreeMap;
use std::sync::Arc;

use crate::buffer::{Buffer, TextOps};
use crate::command::CommandRegistry;
use crate::command_line::CommandLine;
use crate::completion::{CompletionContext, CompletionEngine, CompletionItem, CompletionState};
use crate::constants::EVENT_CHANNEL_CAPACITY;
use crate::event::{CompletionEvent, InnerEvent};
use crate::explorer::ExplorerState;
use crate::highlight::{ColorMode, HighlightStore, Theme};
use crate::modd::Mod;
use crate::screen::Screen;

use tokio::sync::{mpsc, watch};

/// The main runtime that owns all editor state
pub struct Runtime {
    pub buffers: BTreeMap<usize, Buffer>,
    pub screen: Screen,
    pub highlight_store: HighlightStore,
    pub current_mode: Mod,
    pub color_mode: ColorMode,
    pub theme: Theme,
    pub clipboard: String,
    pub command_line: CommandLine,
    pub pending_keys: String,
    pub last_command: String,
    pub tx: mpsc::Sender<InnerEvent>,
    pub rx: mpsc::Receiver<InnerEvent>,
    pub initial_file: Option<String>,
    pub(crate) showing_landing_page: bool,
    /// Watch channel sender for broadcasting mode changes
    pub(crate) mode_tx: watch::Sender<Mod>,
    /// Watch channel receiver (kept to allow subscribing)
    mode_rx: watch::Receiver<Mod>,
    /// Command registry for trait-based command system
    pub command_registry: Arc<CommandRegistry>,
    /// Currently active buffer ID
    pub active_buffer_id: usize,
    /// Next buffer ID to assign
    next_buffer_id: usize,
    /// File explorer state
    pub explorer_state: Option<ExplorerState>,
    /// Completion engine
    pub completion_engine: Arc<CompletionEngine>,
    /// Current completion state
    pub completion_state: CompletionState,
    /// Cache of unfiltered completion items for re-filtering
    pub(crate) completion_items_cache: Vec<CompletionItem>,
    /// Watch channel sender for broadcasting completion active state
    pub(crate) completion_active_tx: watch::Sender<bool>,
    /// Watch channel receiver for completion active state
    completion_active_rx: watch::Receiver<bool>,
}

impl Default for Runtime {
    fn default() -> Self {
        Self::new(Screen::default())
    }
}

impl Runtime {
    /// Create a new Runtime with the given screen
    #[must_use]
    pub fn new(screen: Screen) -> Self {
        let (tx, rx) = mpsc::channel(EVENT_CHANNEL_CAPACITY);
        let (mode_tx, mode_rx) = watch::channel(Mod::Normal);
        let (completion_active_tx, completion_active_rx) = watch::channel(false);
        Self {
            buffers: BTreeMap::new(),
            screen,
            highlight_store: HighlightStore::new(),
            current_mode: Mod::Normal,
            color_mode: ColorMode::detect(),
            theme: Theme::default(),
            clipboard: String::new(),
            command_line: CommandLine::default(),
            pending_keys: String::new(),
            last_command: String::new(),
            tx,
            rx,
            initial_file: None,
            showing_landing_page: false,
            mode_tx,
            mode_rx,
            command_registry: Arc::new(CommandRegistry::with_defaults()),
            active_buffer_id: 0,
            next_buffer_id: 1, // Start at 1 since 0 is reserved for initial buffer
            explorer_state: None,
            completion_engine: Arc::new(CompletionEngine::default()),
            completion_state: CompletionState::new(),
            completion_items_cache: Vec::new(),
            completion_active_tx,
            completion_active_rx,
        }
    }

    /// Subscribe to mode changes
    #[must_use]
    pub fn subscribe_mode(&self) -> watch::Receiver<Mod> {
        self.mode_rx.clone()
    }

    /// Subscribe to completion active state changes
    #[must_use]
    pub fn subscribe_completion_active(&self) -> watch::Receiver<bool> {
        self.completion_active_rx.clone()
    }

    /// Broadcast completion active state change
    pub(crate) fn set_completion_active(&self, active: bool) {
        let _ = self.completion_active_tx.send(active);
    }

    /// Broadcast a mode change
    pub(crate) fn set_mode(&mut self, mode: Mod) {
        self.current_mode = mode.clone();
        let _ = self.mode_tx.send(mode);
    }

    /// Set the initial file to open
    #[must_use]
    pub fn with_file(mut self, file_path: Option<String>) -> Self {
        self.initial_file = file_path;
        self
    }

    /// Render the screen with current state
    pub(crate) fn render(&mut self) {
        let buffers: Vec<Buffer> = self.buffers.values().cloned().collect();
        self.screen
            .render(
                &buffers,
                &self.highlight_store,
                &self.current_mode,
                &self.command_line,
                &self.pending_keys,
                &self.last_command,
                self.color_mode,
                &self.theme,
                self.explorer_state.as_ref(),
                &self.completion_state,
            )
            .expect("failed to render");
        self.screen.flush().expect("failed to flush");
    }

    /// Set color mode (for :set colormode command)
    #[allow(clippy::missing_const_for_fn)]
    pub fn set_color_mode(&mut self, mode: ColorMode) {
        self.color_mode = mode;
    }

    /// Create a new empty buffer and return its ID
    pub fn create_buffer(&mut self) -> usize {
        let id = self.next_buffer_id;
        self.next_buffer_id += 1;
        let buffer = Buffer::empty(id);
        self.buffers.insert(id, buffer);
        id
    }

    /// Create a new buffer from a file and return its ID
    /// Returns None if the file cannot be read
    pub fn create_buffer_from_file(&mut self, path: &str) -> Option<usize> {
        let content = std::fs::read_to_string(path).ok()?;
        let id = self.next_buffer_id;
        self.next_buffer_id += 1;
        let mut buffer = Buffer::empty(id);
        buffer.set_content(&content);
        buffer.file_path = Some(path.to_string());
        self.buffers.insert(id, buffer);
        Some(id)
    }

    /// Switch to a different buffer by ID
    pub fn switch_buffer(&mut self, buffer_id: usize) {
        if self.buffers.contains_key(&buffer_id) {
            self.active_buffer_id = buffer_id;
        }
    }

    /// Close a buffer by ID
    /// Returns true if buffer was closed, false if it was the last buffer
    pub fn close_buffer(&mut self, buffer_id: usize) -> bool {
        // Don't close the last buffer
        if self.buffers.len() <= 1 {
            return false;
        }

        if self.buffers.remove(&buffer_id).is_some() {
            // If we closed the active buffer, switch to another one
            if self.active_buffer_id == buffer_id
                && let Some(&new_id) = self.buffers.keys().next()
            {
                self.active_buffer_id = new_id;
            }
            true
        } else {
            false
        }
    }

    /// Get the currently active buffer
    #[must_use]
    pub fn active_buffer(&self) -> Option<&Buffer> {
        self.buffers.get(&self.active_buffer_id)
    }

    /// Get the currently active buffer mutably
    pub fn active_buffer_mut(&mut self) -> Option<&mut Buffer> {
        self.buffers.get_mut(&self.active_buffer_id)
    }

    /// Open a file, creating a new buffer or switching to existing one
    pub fn open_file(&mut self, path: &str) {
        // Check if this file is already open in a buffer
        for (id, buf) in &self.buffers {
            if buf.file_path.as_deref() == Some(path) {
                self.active_buffer_id = *id;
                return;
            }
        }

        // Create new buffer from file
        if let Some(id) = self.create_buffer_from_file(path) {
            self.active_buffer_id = id;
            self.showing_landing_page = false;
        }
    }

    /// Trigger completion at current cursor position
    pub(crate) fn trigger_completion(&self, buffer_id: usize) {
        let Some(buffer) = self.buffers.get(&buffer_id) else {
            return;
        };

        // Build completion context
        let ctx = Self::build_completion_context(buffer);

        // Don't trigger if prefix is empty
        if ctx.prefix.is_empty() {
            return;
        }

        let content = buffer.contents.clone();
        let engine = self.completion_engine.clone();
        let tx = self.tx.clone();

        // Spawn async completion fetch
        tokio::spawn(async move {
            let items = engine.complete(&ctx, &content).await;
            if !items.is_empty() {
                let _ = tx
                    .send(InnerEvent::CompletionEvent(CompletionEvent::Update {
                        items,
                        prefix: ctx.prefix,
                        start_col: ctx.word_start_col,
                        start_row: ctx.position.y,
                    }))
                    .await;
            }
        });
    }

    /// Build completion context from buffer state
    #[allow(clippy::cast_possible_truncation)]
    fn build_completion_context(buffer: &Buffer) -> CompletionContext {
        let position = buffer.cur;
        let line = buffer
            .contents
            .get(position.y as usize)
            .map(|l| l.inner.clone())
            .unwrap_or_default();

        // Find word start by walking backward
        let chars: Vec<char> = line.chars().collect();
        let mut word_start = position.x as usize;
        while word_start > 0 {
            let ch = chars.get(word_start - 1).copied().unwrap_or(' ');
            if !ch.is_alphanumeric() && ch != '_' {
                break;
            }
            word_start -= 1;
        }

        let prefix = if word_start < position.x as usize {
            chars[word_start..position.x as usize].iter().collect()
        } else {
            String::new()
        };

        CompletionContext::new(
            buffer.id,
            position,
            line,
            word_start as u16,
            prefix,
        )
    }

    /// Insert a completion item at current cursor position
    pub(crate) fn insert_completion(&mut self, item: &CompletionItem) {
        let Some(buffer) = self.buffers.get_mut(&0) else {
            return;
        };

        // Delete the prefix (characters from start_col to cursor)
        let prefix_len = self.completion_state.prefix.len();
        for _ in 0..prefix_len {
            buffer.delete_char_backward();
        }

        // Insert the completion text
        for ch in item.insert_text.chars() {
            buffer.insert_char(ch);
        }
    }
}
