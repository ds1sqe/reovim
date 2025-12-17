//! Core Runtime struct and initialization

use std::collections::{BTreeMap, HashMap};
use std::sync::Arc;

use crate::buffer::{Buffer, TextOps};
use crate::command::CommandRegistry;
use crate::command_line::CommandLine;
use crate::completion::{CompletionContext, CompletionEngine, CompletionItem, CompletionState};
use crate::constants::EVENT_CHANNEL_CAPACITY;
use crate::event::{CompletionEvent, InnerEvent};
use crate::explorer::ExplorerState;
use crate::folding::FoldManager;
use crate::highlight::{ColorMode, HighlightStore, Theme};
use crate::indent::IndentAnalyzer;
use crate::jumplist::JumpList;
use crate::leap::LeapState;
use crate::modd::ModeState;
use crate::register::Registers;
use crate::screen::{Screen, WhichKeyPanel};
use crate::telescope::picker::{
    BuffersPicker, CommandsPicker, FilesPicker, GrepPicker, HelpPicker, KeymapsPicker, Picker,
    RecentPicker,
};
use crate::telescope::{TelescopeMatcher, TelescopeState};
use crate::treesitter::TreesitterManager;
use tracing::debug;

use tokio::sync::{mpsc, watch};

/// The main runtime that owns all editor state
pub struct Runtime {
    pub buffers: BTreeMap<usize, Buffer>,
    pub screen: Screen,
    pub highlight_store: HighlightStore,
    pub mode_state: ModeState,
    pub color_mode: ColorMode,
    pub theme: Theme,
    pub registers: Registers,
    pub command_line: CommandLine,
    pub pending_keys: String,
    pub last_command: String,
    pub tx: mpsc::Sender<InnerEvent>,
    pub rx: mpsc::Receiver<InnerEvent>,
    pub initial_file: Option<String>,
    pub(crate) showing_landing_page: bool,
    /// Watch channel sender for broadcasting mode changes
    pub(crate) mode_tx: watch::Sender<ModeState>,
    /// Watch channel receiver (kept to allow subscribing)
    mode_rx: watch::Receiver<ModeState>,
    /// Command registry for trait-based command system
    pub command_registry: Arc<CommandRegistry>,
    /// Currently active buffer ID
    pub active_buffer_id: usize,
    /// Next buffer ID to assign
    next_buffer_id: usize,
    /// File explorer state
    pub explorer_state: Option<ExplorerState>,
    /// Jump list for Ctrl-O/Ctrl-I navigation
    pub jump_list: JumpList,
    /// Which-key popup panel state
    pub which_key_panel: WhichKeyPanel,
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
    /// Telescope fuzzy finder state
    pub telescope_state: TelescopeState,
    /// Telescope fuzzy matcher
    pub telescope_matcher: TelescopeMatcher,
    /// Telescope pickers registry
    pub telescope_pickers: HashMap<String, Arc<dyn Picker>>,
    /// Leap motion state
    pub leap_state: LeapState,
    /// Treesitter manager for syntax highlighting
    pub treesitter: TreesitterManager,
    /// Code folding manager
    pub fold_manager: FoldManager,
    /// Indent guide analyzer
    pub indent_analyzer: IndentAnalyzer,
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
        let (mode_tx, mode_rx) = watch::channel(ModeState::new());
        let (completion_active_tx, completion_active_rx) = watch::channel(false);
        Self {
            buffers: BTreeMap::new(),
            screen,
            highlight_store: HighlightStore::new(),
            mode_state: ModeState::new(),
            color_mode: ColorMode::detect(),
            theme: Theme::default(),
            registers: Registers::new(),
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
            jump_list: JumpList::new(),
            which_key_panel: WhichKeyPanel::new(),
            completion_engine: Arc::new(CompletionEngine::default()),
            completion_state: CompletionState::new(),
            completion_items_cache: Vec::new(),
            completion_active_tx,
            completion_active_rx,
            telescope_state: TelescopeState::new(),
            telescope_matcher: TelescopeMatcher::new(),
            telescope_pickers: Self::create_telescope_pickers(),
            leap_state: LeapState::new(),
            treesitter: TreesitterManager::new(),
            fold_manager: FoldManager::new(),
            indent_analyzer: IndentAnalyzer::default(),
        }
    }

    /// Create the telescope picker registry
    fn create_telescope_pickers() -> HashMap<String, Arc<dyn Picker>> {
        let mut pickers: HashMap<String, Arc<dyn Picker>> = HashMap::new();
        pickers.insert("files".to_string(), Arc::new(FilesPicker::new()));
        pickers.insert("buffers".to_string(), Arc::new(BuffersPicker::new()));
        pickers.insert("commands".to_string(), Arc::new(CommandsPicker::new()));
        pickers.insert("keymaps".to_string(), Arc::new(KeymapsPicker::new()));
        pickers.insert("grep".to_string(), Arc::new(GrepPicker::new()));
        pickers.insert("recent".to_string(), Arc::new(RecentPicker::new()));
        pickers.insert("help".to_string(), Arc::new(HelpPicker::new()));
        pickers
    }

    /// Subscribe to mode changes
    #[must_use]
    pub fn subscribe_mode(&self) -> watch::Receiver<ModeState> {
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
    pub(crate) fn set_mode(&mut self, mode_state: ModeState) {
        self.mode_state = mode_state.clone();
        let _ = self.mode_tx.send(mode_state);
    }

    /// Get current mode state
    #[must_use]
    pub const fn current_mode(&self) -> &ModeState {
        &self.mode_state
    }

    /// Set the initial file to open
    #[must_use]
    pub fn with_file(mut self, file_path: Option<String>) -> Self {
        self.initial_file = file_path;
        self
    }

    /// Render the screen with current state
    pub(crate) fn render(&mut self) {
        self.screen
            .render(
                &self.buffers,
                &self.highlight_store,
                &self.mode_state,
                &self.command_line,
                &self.pending_keys,
                &self.last_command,
                self.color_mode,
                &self.theme,
                self.explorer_state.as_ref(),
                &self.which_key_panel,
                &self.completion_state,
                &self.telescope_state,
                &self.leap_state,
                &self.fold_manager,
                &self.indent_analyzer,
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
        debug!(path, "create_buffer_from_file: attempting to read");
        match std::fs::read_to_string(path) {
            Ok(content) => {
                let id = self.next_buffer_id;
                self.next_buffer_id += 1;
                let mut buffer = Buffer::empty(id);
                buffer.set_content(&content);
                buffer.file_path = Some(path.to_string());
                self.buffers.insert(id, buffer.clone());

                // Initialize treesitter for this buffer
                let language_id = self.treesitter.init_buffer(id, Some(path));
                debug!(id, path, ?language_id, "create_buffer_from_file: treesitter initialized");

                // Generate initial syntax highlights and fold ranges
                #[allow(clippy::cast_possible_truncation)]
                if self.treesitter.has_parser(id) {
                    let line_count = buffer.contents.len() as u32;
                    let highlights = self.treesitter.parse_and_highlight(
                        id,
                        &content,
                        0,
                        line_count.saturating_sub(1),
                    );
                    if !highlights.is_empty() {
                        self.highlight_store.add(id, highlights);
                        debug!(id, "create_buffer_from_file: syntax highlights added");
                    }

                    // Compute fold ranges
                    let fold_ranges = self.treesitter.compute_fold_ranges(id, &content);
                    if !fold_ranges.is_empty() {
                        self.fold_manager.set_ranges(id, fold_ranges);
                        debug!(id, "create_buffer_from_file: fold ranges computed");
                    }
                }

                debug!(id, path, "create_buffer_from_file: success");
                Some(id)
            }
            Err(e) => {
                debug!(path, error = %e, "create_buffer_from_file: failed to read file");
                None
            }
        }
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
            // Clean up treesitter state
            self.treesitter.remove_buffer(buffer_id);
            // Clean up highlights
            self.highlight_store.clear_all(buffer_id);
            // Clean up fold state
            self.fold_manager.remove_buffer(buffer_id);

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
        debug!(path, "open_file: called");

        // Check if this file is already open in a buffer
        for (id, buf) in &self.buffers {
            if buf.file_path.as_deref() == Some(path) {
                debug!(id, path, "open_file: file already open, switching buffer");
                self.active_buffer_id = *id;
                return;
            }
        }

        // Create new buffer from file
        if let Some(id) = self.create_buffer_from_file(path) {
            debug!(id, path, "open_file: created new buffer");
            self.active_buffer_id = id;
            self.showing_landing_page = false;
        } else {
            debug!(path, "open_file: failed to create buffer");
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
