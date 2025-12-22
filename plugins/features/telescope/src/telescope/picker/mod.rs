//! Picker trait and implementations
//!
//! Pickers are responsible for:
//! - Fetching items (files, buffers, commands, etc.)
//! - Determining actions when an item is selected
//! - Optionally providing preview content

pub mod buffers;
pub mod commands;
pub mod files;
pub mod grep;
pub mod help;
pub mod keymaps;
pub mod profiles;
pub mod recent;
pub mod themes;

pub use {
    buffers::BuffersPicker, commands::CommandsPicker, files::FilesPicker, grep::GrepPicker,
    help::HelpPicker, keymaps::KeymapsPicker, profiles::ProfilesPicker, recent::RecentPicker,
    themes::ThemesPicker,
};

use std::{future::Future, path::PathBuf, pin::Pin};

use reovim_core::{command::CommandId, highlight::ThemeName};

use super::{item::TelescopeItem, state::PreviewContent};

pub use buffers::BufferInfo;

/// Context for picker operations
#[derive(Debug, Clone)]
pub struct PickerContext {
    /// Current search query
    pub query: String,
    /// Working directory
    pub cwd: PathBuf,
    /// Maximum items to fetch
    pub max_items: usize,
    /// Available buffers (for buffers picker)
    pub buffers: Vec<BufferInfo>,
}

impl Default for PickerContext {
    fn default() -> Self {
        Self {
            query: String::new(),
            cwd: std::env::current_dir().unwrap_or_default(),
            max_items: 1000,
            buffers: Vec::new(),
        }
    }
}

/// Action to perform when an item is selected
#[derive(Debug, Clone)]
pub enum TelescopeAction {
    /// Open a file
    OpenFile(PathBuf),
    /// Switch to a buffer
    SwitchBuffer(usize),
    /// Execute a command
    ExecuteCommand(CommandId),
    /// Go to a specific location
    GotoLocation {
        path: PathBuf,
        line: usize,
        col: usize,
    },
    /// Show help for a tag
    ShowHelp(String),
    /// Apply a theme/colorscheme
    ApplyTheme(ThemeName),
    /// Switch to a configuration profile
    SwitchProfile(String),
    /// Close telescope without action
    Close,
    /// Do nothing
    Nothing,
}

/// Trait for implementing telescope pickers
pub trait Picker: Send + Sync {
    /// Unique identifier for this picker
    fn name(&self) -> &'static str;

    /// Human-readable title for the picker window
    fn title(&self) -> &'static str;

    /// Prompt string (shown before input)
    fn prompt(&self) -> &'static str {
        "> "
    }

    /// Fetch items asynchronously
    fn fetch(
        &self,
        ctx: &PickerContext,
    ) -> Pin<Box<dyn Future<Output = Vec<TelescopeItem>> + Send + '_>>;

    /// Handle selection of an item
    fn on_select(&self, item: &TelescopeItem) -> TelescopeAction;

    /// Optional: preview content for the selected item
    fn preview(
        &self,
        _item: &TelescopeItem,
    ) -> Pin<Box<dyn Future<Output = Option<PreviewContent>> + Send + '_>> {
        Box::pin(async { None })
    }

    /// Whether this picker supports live filtering vs full re-fetch
    /// If true, the picker only needs to fetch once and nucleo handles filtering
    /// If false, the picker needs to re-fetch on each query change (e.g., live grep)
    fn supports_live_filter(&self) -> bool {
        true
    }
}
