//! Recent files picker implementation

use std::future::Future;
use std::path::PathBuf;
use std::pin::Pin;

use crate::telescope::item::{TelescopeData, TelescopeItem};
use crate::telescope::state::PreviewContent;

use super::{Picker, PickerContext, TelescopeAction};

/// Picker for recently opened files
pub struct RecentPicker {
    /// Recent files list (set by runtime)
    recent_files: Vec<PathBuf>,
}

impl RecentPicker {
    /// Create a new recent files picker
    #[must_use]
    pub const fn new() -> Self {
        Self {
            recent_files: Vec::new(),
        }
    }

    /// Set the recent files list
    pub fn set_recent_files(&mut self, files: Vec<PathBuf>) {
        self.recent_files = files;
    }

    /// Add a file to the recent list
    pub fn add_recent(&mut self, path: PathBuf) {
        // Remove if already exists
        self.recent_files.retain(|p| p != &path);
        // Add to front
        self.recent_files.insert(0, path);
        // Limit to 100 files
        self.recent_files.truncate(100);
    }
}

impl Default for RecentPicker {
    fn default() -> Self {
        Self::new()
    }
}

impl Picker for RecentPicker {
    fn name(&self) -> &'static str {
        "recent"
    }

    fn title(&self) -> &'static str {
        "Recent Files"
    }

    fn prompt(&self) -> &'static str {
        "Recent> "
    }

    fn fetch(
        &self,
        ctx: &PickerContext,
    ) -> Pin<Box<dyn Future<Output = Vec<TelescopeItem>> + Send + '_>> {
        let cwd = ctx.cwd.clone();

        Box::pin(async move {
            self.recent_files
                .iter()
                .filter(|p| p.exists())
                .map(|path| {
                    let display = path
                        .strip_prefix(&cwd)
                        .unwrap_or(path)
                        .to_string_lossy()
                        .to_string();

                    TelescopeItem::new(
                        &display,
                        &display,
                        TelescopeData::FilePath(path.clone()),
                        "recent",
                    )
                })
                .collect()
        })
    }

    fn on_select(&self, item: &TelescopeItem) -> TelescopeAction {
        match &item.data {
            TelescopeData::FilePath(path) => TelescopeAction::OpenFile(path.clone()),
            _ => TelescopeAction::Nothing,
        }
    }

    fn preview(
        &self,
        item: &TelescopeItem,
    ) -> Pin<Box<dyn Future<Output = Option<PreviewContent>> + Send + '_>> {
        let path = match &item.data {
            TelescopeData::FilePath(p) => p.clone(),
            _ => return Box::pin(async { None }),
        };

        Box::pin(async move {
            tokio::fs::read_to_string(&path).await.ok().map(|content| {
                let lines: Vec<String> = content.lines().map(String::from).collect();
                let syntax = path
                    .extension()
                    .and_then(|e| e.to_str())
                    .map(String::from);
                PreviewContent {
                    lines,
                    highlight_line: None,
                    syntax,
                }
            })
        })
    }
}
