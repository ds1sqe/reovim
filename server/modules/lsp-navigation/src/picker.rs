//! LSP location picker for multi-result navigation.

use std::{
    fs,
    io::{BufRead, BufReader},
    path::Path,
};

use {
    reovim_driver_picker::{
        Picker, PickerAction, PickerContext, PickerData, PickerItem, PreviewContent, SessionRuntime,
    },
    reovim_driver_session::{BufferApi, ChangeTracker, WindowApi},
    reovim_driver_vfs::VfsInstance,
    reovim_kernel::api::v1::ServiceRegistry,
};

/// Number of context lines to show around a location in preview.
const PREVIEW_CONTEXT_LINES: usize = 5;

/// Picker for displaying LSP navigation results (definitions, references).
///
/// Items are injected externally by the command handler via the engine
/// injector, not fetched via `items()`.
pub struct LspLocationPicker;

impl LspLocationPicker {
    /// Create a new instance.
    #[must_use]
    pub const fn new() -> Self {
        Self
    }
}

impl Default for LspLocationPicker {
    fn default() -> Self {
        Self::new()
    }
}

impl Picker for LspLocationPicker {
    fn name(&self) -> &'static str {
        "lsp-locations"
    }

    fn title(&self) -> &'static str {
        "LSP Locations"
    }

    fn prompt(&self) -> &'static str {
        "> "
    }

    fn items(&self, _ctx: &PickerContext, _services: &ServiceRegistry) -> Vec<PickerItem> {
        // Items are injected directly by the command handler, not fetched here.
        Vec::new()
    }

    fn on_select(&self, item: &PickerItem) -> PickerAction {
        match &item.data {
            PickerData::GotoLocation { path, line, col } => PickerAction::GotoLocation {
                path: path.clone(),
                line: *line,
                col: *col,
            },
            _ => PickerAction::Close,
        }
    }

    fn preview(&self, item: &PickerItem, _services: &ServiceRegistry) -> Option<PreviewContent> {
        let PickerData::GotoLocation { path, line, .. } = &item.data else {
            return None;
        };

        let file = fs::File::open(path).ok()?;
        let reader = BufReader::new(file);

        let start = line.saturating_sub(PREVIEW_CONTEXT_LINES);
        let end = line + PREVIEW_CONTEXT_LINES;

        let lines: Vec<String> = reader
            .lines()
            .skip(start)
            .take(end - start + 1)
            .filter_map(Result::ok)
            .collect();

        if lines.is_empty() {
            return None;
        }

        let highlight = line.saturating_sub(start);

        Some(PreviewContent {
            lines,
            highlight_line: Some(highlight),
            file_path: Some(path.clone()),
        })
    }

    fn is_static(&self) -> bool {
        true
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    fn execute(&self, action: PickerAction, runtime: &mut SessionRuntime<'_>) {
        if let PickerAction::GotoLocation { path, line, col } = action {
            open_file(runtime, &path);
            // Set cursor position (1-based from LSP -> 0-based).
            if let Some(window) = runtime.windows_mut().active_mut() {
                window.cursor.line = line.saturating_sub(1);
                window.cursor.column = col.saturating_sub(1);
            }
            if let Some(buf_id) = runtime.active_buffer() {
                runtime.record_cursor_move(buf_id);
            }
        }
    }
}

/// Open a file by path, reusing existing buffers when possible.
#[cfg_attr(coverage_nightly, coverage(off))]
fn open_file(runtime: &mut SessionRuntime<'_>, path: &Path) {
    use reovim_kernel::api::v1::events::kernel::FileOpened;

    let canonical = path.canonicalize().unwrap_or_else(|_| path.to_path_buf());
    let path_str = canonical.to_string_lossy();

    let existing = runtime.kernel().buffers.list().into_iter().find(|&id| {
        runtime
            .buffer_file_path(id)
            .is_some_and(|p| Path::new(&p) == canonical)
    });

    let is_new = existing.is_none();
    let buf_id = existing.unwrap_or_else(|| {
        let content = runtime
            .kernel()
            .services
            .get::<VfsInstance>()
            .and_then(|vfs| vfs.driver().read_to_string(&canonical).ok())
            .unwrap_or_default();
        let id = runtime.create_buffer(Some(&path_str), &content);
        runtime.set_buffer_modified(id, false);
        id
    });

    if let Some(win) = runtime.active_window() {
        let _ = runtime.set_window_buffer(win, buf_id);
    }
    runtime.set_active_buffer(Some(buf_id));

    if is_new {
        runtime.record_buffer_modified(buf_id);
        #[allow(clippy::cast_possible_truncation)]
        let buf_id_raw = buf_id.as_usize() as u64;
        runtime.kernel().event_bus.emit(FileOpened {
            buffer_id: buf_id_raw,
            path: path_str.to_string(),
        });
    }
}

#[cfg(test)]
#[path = "picker_tests.rs"]
mod tests;
