//! File picker implementation

use std::{future::Future, pin::Pin};

use {
    ignore::WalkBuilder,
    reovim_plugin_microscope::{
        MicroscopeAction, MicroscopeData, MicroscopeItem, Picker, PickerContext, PreviewContent,
    },
};

/// Picker for finding files in the project
pub struct FilesPicker {
    /// Additional ignore patterns
    ignore_patterns: Vec<String>,
}

impl FilesPicker {
    /// Create a new files picker
    #[must_use]
    pub fn new() -> Self {
        Self {
            ignore_patterns: vec![
                ".git".to_string(),
                "node_modules".to_string(),
                "target".to_string(),
                ".cache".to_string(),
            ],
        }
    }

    /// Add an ignore pattern
    pub fn add_ignore(&mut self, pattern: impl Into<String>) {
        self.ignore_patterns.push(pattern.into());
    }
}

impl Default for FilesPicker {
    fn default() -> Self {
        Self::new()
    }
}

impl Picker for FilesPicker {
    fn name(&self) -> &'static str {
        "files"
    }

    fn title(&self) -> &'static str {
        "Find Files"
    }

    fn prompt(&self) -> &'static str {
        "Files> "
    }

    fn fetch(
        &self,
        ctx: &PickerContext,
    ) -> Pin<Box<dyn Future<Output = Vec<MicroscopeItem>> + Send + '_>> {
        let cwd = ctx.cwd.clone();
        let max_items = ctx.max_items;
        let ignore_patterns = self.ignore_patterns.clone();

        Box::pin(async move {
            let mut items = Vec::new();

            let walker = WalkBuilder::new(&cwd)
                .hidden(true)
                .git_ignore(true)
                .git_global(true)
                .git_exclude(true)
                .build();

            for entry in walker.flatten() {
                if items.len() >= max_items {
                    break;
                }

                let path = entry.path();

                // Skip directories
                if path.is_dir() {
                    continue;
                }

                // Skip ignored patterns
                let path_str = path.to_string_lossy();
                if ignore_patterns.iter().any(|p| path_str.contains(p)) {
                    continue;
                }

                // Get relative path
                let display = path
                    .strip_prefix(&cwd)
                    .unwrap_or(path)
                    .to_string_lossy()
                    .to_string();

                // Determine icon based on extension
                let icon = match path.extension().and_then(|e| e.to_str()) {
                    Some("rs") => '\u{e7a8}', // Rust icon or fallback
                    Some("js" | "ts") => '\u{e781}',
                    Some("py") => '\u{e73c}',
                    Some("md") => '\u{e73e}',
                    Some("json" | "toml" | "yaml" | "yml") => '\u{e60b}',
                    _ => '\u{f15b}', // Generic file
                };

                items.push(
                    MicroscopeItem::new(
                        &display,
                        &display,
                        MicroscopeData::FilePath(path.to_path_buf()),
                        "files",
                    )
                    .with_icon(icon),
                );
            }

            items
        })
    }

    fn on_select(&self, item: &MicroscopeItem) -> MicroscopeAction {
        match &item.data {
            MicroscopeData::FilePath(path) => MicroscopeAction::OpenFile(path.clone()),
            _ => MicroscopeAction::Nothing,
        }
    }

    fn preview(
        &self,
        item: &MicroscopeItem,
    ) -> Pin<Box<dyn Future<Output = Option<PreviewContent>> + Send + '_>> {
        let path = match &item.data {
            MicroscopeData::FilePath(p) => p.clone(),
            _ => return Box::pin(async { None }),
        };

        let file_name = path.file_name().and_then(|n| n.to_str()).map(String::from);
        Box::pin(async move {
            // Read file content for preview
            tokio::fs::read_to_string(&path).await.ok().map(|content| {
                let lines: Vec<String> = content.lines().map(String::from).collect();
                let syntax = path.extension().and_then(|e| e.to_str()).map(String::from);
                PreviewContent {
                    lines,
                    highlight_line: None,
                    syntax,
                    title: file_name,
                }
            })
        })
    }
}
