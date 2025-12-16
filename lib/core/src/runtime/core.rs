//! Core Runtime struct and initialization

use std::collections::BTreeMap;
use std::sync::Arc;

use crate::buffer::Buffer;
use crate::command::CommandRegistry;
use crate::command_line::CommandLine;
use crate::constants::EVENT_CHANNEL_CAPACITY;
use crate::event::InnerEvent;
use crate::highlight::{ColorMode, HighlightStore, Theme};
use crate::jumplist::JumpList;
use crate::modd::Mod;
use crate::screen::{Screen, WhichKeyPanel};

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
}
