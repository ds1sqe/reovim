//! Picker trait and infrastructure
//!
//! This module provides the core picker infrastructure:
//! - `Picker` trait for implementing custom pickers
//! - `PickerContext` for picker operations
//! - `MicroscopeAction` for actions on selection
//! - `PickerRegistry` for dynamic picker registration
//!
//! Actual picker implementations are in the separate `reovim-plugin-pickers` crate.

pub mod registry;

pub use registry::PickerRegistry;

use std::{future::Future, path::PathBuf, pin::Pin};

use reovim_core::{command::CommandId, highlight::ThemeName};

use super::{item::MicroscopeItem, state::PreviewContent};

/// Buffer info passed from runtime for buffer picker
#[derive(Debug, Clone)]
pub struct BufferInfo {
    /// Buffer ID
    pub id: usize,
    /// Buffer name/path
    pub name: String,
    /// Whether the buffer is modified
    pub modified: bool,
    /// Preview lines
    pub preview_lines: Vec<String>,
}

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
pub enum MicroscopeAction {
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
    /// Close microscope without action
    Close,
    /// Do nothing
    Nothing,
}

/// Trait for implementing microscope pickers
///
/// Pickers are responsible for:
/// - Fetching items (files, buffers, commands, etc.)
/// - Determining actions when an item is selected
/// - Optionally providing preview content
///
/// # Example
///
/// ```ignore
/// use reovim_plugin_microscope::{Picker, PickerContext, MicroscopeAction, MicroscopeItem};
///
/// pub struct MyPicker;
///
/// impl Picker for MyPicker {
///     fn name(&self) -> &'static str { "my_picker" }
///     fn title(&self) -> &'static str { "My Picker" }
///
///     fn fetch(&self, ctx: &PickerContext) -> Pin<Box<dyn Future<Output = Vec<MicroscopeItem>> + Send + '_>> {
///         Box::pin(async { vec![] })
///     }
///
///     fn on_select(&self, item: &MicroscopeItem) -> MicroscopeAction {
///         MicroscopeAction::Nothing
///     }
/// }
/// ```
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
    ) -> Pin<Box<dyn Future<Output = Vec<MicroscopeItem>> + Send + '_>>;

    /// Handle selection of an item
    fn on_select(&self, item: &MicroscopeItem) -> MicroscopeAction;

    /// Optional: preview content for the selected item
    fn preview(
        &self,
        _item: &MicroscopeItem,
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
