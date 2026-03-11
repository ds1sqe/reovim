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
    let canonical = path.canonicalize().unwrap_or_else(|_| path.to_path_buf());
    let path_str = canonical.to_string_lossy();

    let existing = runtime.kernel().buffers.list().into_iter().find(|&id| {
        runtime
            .buffer_file_path(id)
            .is_some_and(|p| Path::new(&p) == canonical)
    });

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
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;

    fn services() -> ServiceRegistry {
        ServiceRegistry::new()
    }

    #[test]
    fn name_and_title() {
        let picker = LspLocationPicker::new();
        assert_eq!(picker.name(), "lsp-locations");
        assert_eq!(picker.title(), "LSP Locations");
    }

    #[test]
    fn prompt_value() {
        let picker = LspLocationPicker::new();
        assert_eq!(picker.prompt(), "> ");
    }

    #[test]
    fn is_static_true() {
        let picker = LspLocationPicker::new();
        assert!(picker.is_static());
    }

    #[test]
    #[allow(clippy::default_constructed_unit_structs)]
    fn default_impl() {
        let picker = LspLocationPicker::default();
        assert_eq!(picker.name(), "lsp-locations");
    }

    #[test]
    fn items_returns_empty() {
        let picker = LspLocationPicker::new();
        let ctx = PickerContext {
            cwd: PathBuf::from("."),
            query: String::new(),
            buffers: vec![],
            commands: vec![],
            options: vec![],
        };
        assert!(picker.items(&ctx, &services()).is_empty());
    }

    #[test]
    fn on_select_goto_location() {
        let picker = LspLocationPicker::new();
        let item = PickerItem {
            display: "test.rs:10:5".to_owned(),
            detail: None,
            data: PickerData::GotoLocation {
                path: PathBuf::from("test.rs"),
                line: 10,
                col: 5,
            },
            icon: None,
        };
        let action = picker.on_select(&item);
        assert!(
            matches!(action, PickerAction::GotoLocation { ref path, line: 10, col: 5 } if *path == Path::new("test.rs"))
        );
    }

    #[test]
    fn on_select_wrong_data_closes() {
        let picker = LspLocationPicker::new();
        let item = PickerItem {
            display: "x".to_owned(),
            detail: None,
            data: PickerData::Text("wrong".to_owned()),
            icon: None,
        };
        assert!(matches!(picker.on_select(&item), PickerAction::Close));
    }

    #[test]
    fn preview_goto_location() {
        let dir = tempfile::tempdir().expect("Failed to create temp dir");
        let path = dir.path().join("test.rs");
        let content = (1..=20)
            .map(|i| format!("line {i}"))
            .collect::<Vec<_>>()
            .join("\n");
        std::fs::write(&path, &content).expect("Failed to write file");

        let picker = LspLocationPicker::new();
        let item = PickerItem {
            display: "test.rs:10:line 10".to_owned(),
            detail: None,
            data: PickerData::GotoLocation {
                path: path.clone(),
                line: 10,
                col: 1,
            },
            icon: None,
        };
        let preview = picker.preview(&item, &services());
        assert!(preview.is_some());
        let preview = preview.unwrap();
        assert!(preview.highlight_line.is_some());
        assert_eq!(preview.file_path, Some(path));
    }

    #[test]
    fn preview_wrong_data() {
        let picker = LspLocationPicker::new();
        let item = PickerItem {
            display: "x".to_owned(),
            detail: None,
            data: PickerData::Text("wrong".to_owned()),
            icon: None,
        };
        assert!(picker.preview(&item, &services()).is_none());
    }

    #[test]
    fn preview_nonexistent_file() {
        let picker = LspLocationPicker::new();
        let item = PickerItem {
            display: "nope:1:x".to_owned(),
            detail: None,
            data: PickerData::GotoLocation {
                path: PathBuf::from("/nonexistent_lsp_nav_12345.rs"),
                line: 1,
                col: 1,
            },
            icon: None,
        };
        assert!(picker.preview(&item, &services()).is_none());
    }

    #[test]
    fn preview_line_beyond_file_end() {
        let dir = tempfile::tempdir().expect("Failed to create temp dir");
        let path = dir.path().join("short.rs");
        std::fs::write(&path, "line 1\nline 2\n").expect("Failed to write");

        let picker = LspLocationPicker::new();
        let item = PickerItem {
            display: "short.rs:9999:x".to_owned(),
            detail: None,
            data: PickerData::GotoLocation {
                path,
                line: 9999,
                col: 1,
            },
            icon: None,
        };
        assert!(picker.preview(&item, &services()).is_none());
    }
}
