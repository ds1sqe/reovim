//! Live grep picker implementation

use std::{future::Future, pin::Pin, process::Stdio};

use {
    reovim_plugin_microscope::{
        MicroscopeAction, MicroscopeData, MicroscopeItem, Picker, PickerContext, PreviewContent,
    },
    tokio::{
        io::{AsyncBufReadExt, BufReader},
        process::Command,
    },
};

/// Picker for live grep search using ripgrep
pub struct GrepPicker {
    /// Minimum query length before searching
    min_query_len: usize,
}

impl GrepPicker {
    /// Create a new grep picker
    #[must_use]
    pub const fn new() -> Self {
        Self { min_query_len: 2 }
    }

    /// Set minimum query length
    pub const fn set_min_query_len(&mut self, len: usize) {
        self.min_query_len = len;
    }
}

impl Default for GrepPicker {
    fn default() -> Self {
        Self::new()
    }
}

impl Picker for GrepPicker {
    fn name(&self) -> &'static str {
        "grep"
    }

    fn title(&self) -> &'static str {
        "Live Grep"
    }

    fn prompt(&self) -> &'static str {
        "Grep> "
    }

    fn supports_live_filter(&self) -> bool {
        false // Need to re-fetch on each query change
    }

    fn fetch(
        &self,
        ctx: &PickerContext,
    ) -> Pin<Box<dyn Future<Output = Vec<MicroscopeItem>> + Send + '_>> {
        let query = ctx.query.clone();
        let cwd = ctx.cwd.clone();
        let max_items = ctx.max_items;
        let min_len = self.min_query_len;

        Box::pin(async move {
            if query.len() < min_len {
                return Vec::new();
            }

            let mut items = Vec::new();

            // Try ripgrep first, fall back to grep
            let output = Command::new("rg")
                .args([
                    "--line-number",
                    "--column",
                    "--no-heading",
                    "--color=never",
                    "--max-count=1000",
                    &query,
                ])
                .current_dir(&cwd)
                .stdout(Stdio::piped())
                .stderr(Stdio::null())
                .spawn();

            let mut child = match output {
                Ok(c) => c,
                Err(_) => {
                    // Fall back to grep if rg is not available
                    match Command::new("grep")
                        .args(["-rn", "--color=never", &query, "."])
                        .current_dir(&cwd)
                        .stdout(Stdio::piped())
                        .stderr(Stdio::null())
                        .spawn()
                    {
                        Ok(c) => c,
                        Err(_) => return Vec::new(),
                    }
                }
            };

            if let Some(stdout) = child.stdout.take() {
                let reader = BufReader::new(stdout);
                let mut lines = reader.lines();

                while let Ok(Some(line)) = lines.next_line().await {
                    if items.len() >= max_items {
                        break;
                    }

                    // Parse ripgrep output: file:line:col:content
                    let parts: Vec<&str> = line.splitn(4, ':').collect();
                    if parts.len() >= 3 {
                        let file = parts[0];
                        let line_num: usize = parts[1].parse().unwrap_or(1);
                        let col: usize = parts.get(2).and_then(|c| c.parse().ok()).unwrap_or(1);
                        let content = parts.get(3).unwrap_or(&"").trim();

                        let display = format!("{file}:{line_num}: {content}");
                        let path = cwd.join(file);

                        items.push(
                            MicroscopeItem::new(
                                format!("{file}:{line_num}:{col}"),
                                display,
                                MicroscopeData::GrepMatch {
                                    path,
                                    line: line_num,
                                    col,
                                },
                                "grep",
                            )
                            .with_detail(file.to_string()),
                        );
                    }
                }
            }

            let _ = child.wait().await;
            items
        })
    }

    fn on_select(&self, item: &MicroscopeItem) -> MicroscopeAction {
        match &item.data {
            MicroscopeData::GrepMatch { path, line, col } => MicroscopeAction::GotoLocation {
                path: path.clone(),
                line: *line,
                col: *col,
            },
            _ => MicroscopeAction::Nothing,
        }
    }

    fn preview(
        &self,
        item: &MicroscopeItem,
        ctx: &PickerContext,
    ) -> Pin<Box<dyn Future<Output = Option<PreviewContent>> + Send + '_>> {
        let (path, highlight_line) = match &item.data {
            MicroscopeData::GrepMatch { path, line, .. } => (path.clone(), *line),
            _ => return Box::pin(async { None }),
        };

        let syntax_factory = ctx.syntax_factory.clone();

        Box::pin(async move {
            tokio::fs::read_to_string(&path).await.ok().map(|content| {
                let lines: Vec<String> = content.lines().map(String::from).collect();
                let syntax_ext = path.extension().and_then(|e| e.to_str()).map(String::from);

                // Compute highlights if factory available
                let styled_lines = if let Some(factory) = syntax_factory {
                    let file_path = path.to_string_lossy().to_string();
                    crate::syntax_helper::compute_styled_lines(
                        factory.as_ref(),
                        &file_path,
                        &content,
                        &lines,
                    )
                } else {
                    None
                };

                PreviewContent::new(lines)
                    .with_highlight_line(highlight_line.saturating_sub(1))
                    .with_syntax(syntax_ext.unwrap_or_default())
                    .with_styled_lines(styled_lines.unwrap_or_default())
            })
        })
    }
}
