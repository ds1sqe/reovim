//! Core Runtime struct and initialization

use std::collections::BTreeMap;
use std::sync::Arc;

use crate::buffer::{Buffer, TextOps};
use crate::command::CommandRegistry;
use crate::command_line::CommandLine;
use crate::constants::EVENT_CHANNEL_CAPACITY;
use crate::event::InnerEvent;
use crate::explorer::ExplorerState;
use crate::highlight::{ColorMode, HighlightStore, Theme};
use crate::jumplist::JumpList;
use crate::modd::Mod;
use crate::screen::{Screen, WhichKeyPanel};
use tracing::debug;

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
    /// Jump list for Ctrl-O/Ctrl-I navigation
    pub jump_list: JumpList,
    /// Which-key popup panel state
    pub which_key_panel: WhichKeyPanel,
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
            jump_list: JumpList::new(),
            which_key_panel: WhichKeyPanel::new(),
        }
    }

    /// Subscribe to mode changes
    #[must_use]
    pub fn subscribe_mode(&self) -> watch::Receiver<Mod> {
        self.mode_rx.clone()
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
                &self.which_key_panel,
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
                self.buffers.insert(id, buffer);
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
}
