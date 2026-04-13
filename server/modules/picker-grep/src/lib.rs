#![cfg_attr(coverage_nightly, allow(unused_features))]
#![cfg_attr(coverage_nightly, feature(coverage_attribute))]
//! Grep picker module for reovim.
//!
//! Provides content search using ripgrep subprocess.
//! Registers `GrepPicker` in the `PickerRegistry` during module init.

use std::{
    fs,
    io::{BufRead, BufReader},
    path::Path,
    process::Command,
    sync::Arc,
};

use {
    reovim_driver_picker::{
        Picker, PickerAction, PickerContext, PickerData, PickerItem, PickerRegistry,
        PreviewContent, SessionRuntime,
    },
    reovim_driver_session::{BufferApi, ChangeTracker, WindowApi},
    reovim_kernel::api::v1::{Module, ModuleContext, ModuleError, ModuleId, ProbeResult, Version},
    reovim_subsys_vfs::VfsInstance,
};

/// Number of context lines to show around a grep match in preview.
const PREVIEW_CONTEXT_LINES: usize = 5;

/// Maximum number of grep results to return.
const MAX_RESULTS: usize = 1000;

// ============================================================================
// GrepPicker
// ============================================================================

/// Picker that searches file contents using ripgrep.
///
/// This is a dynamic picker (`is_static() = false`) that re-fetches
/// results whenever the query changes.
pub struct GrepPicker;

impl GrepPicker {
    /// Create a new instance.
    #[must_use]
    pub const fn new() -> Self {
        Self
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
        "Grep"
    }

    fn prompt(&self) -> &'static str {
        "rg> "
    }

    fn items(
        &self,
        ctx: &PickerContext,
        _services: &reovim_kernel::api::v1::ServiceRegistry,
    ) -> Vec<PickerItem> {
        if ctx.query.is_empty() {
            return Vec::new();
        }

        run_ripgrep(&ctx.query, &ctx.cwd)
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

    fn preview(
        &self,
        item: &PickerItem,
        _services: &reovim_kernel::api::v1::ServiceRegistry,
    ) -> Option<PreviewContent> {
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

        // Highlight line is relative to the start of preview.
        let highlight = line.saturating_sub(start);

        Some(PreviewContent {
            lines,
            highlight_line: Some(highlight),
            file_path: Some(path.clone()),
            ..Default::default()
        })
    }

    fn is_static(&self) -> bool {
        false
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    fn execute(&self, action: PickerAction, runtime: &mut SessionRuntime<'_>) {
        if let PickerAction::GotoLocation { path, line, col } = action {
            open_file(runtime, &path);
            // Set cursor position (1-based from rg -> 0-based).
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

/// Run ripgrep and parse results.
///
/// Uses `--` to separate the pattern from arguments (prevents shell injection).
/// Returns empty vec if rg is not found.
fn run_ripgrep(query: &str, cwd: &std::path::Path) -> Vec<PickerItem> {
    let output = Command::new("rg")
        .args(["--line-number", "--column", "--no-heading", "--", query])
        .current_dir(cwd)
        .output();

    let Ok(output) = output else {
        // rg not found or failed to execute.
        return Vec::new();
    };

    let stdout = String::from_utf8_lossy(&output.stdout);
    stdout
        .lines()
        .take(MAX_RESULTS)
        .filter_map(|line| parse_rg_line(line, cwd))
        .collect()
}

/// Parse a single ripgrep output line.
///
/// Format: `path:line:col:text`
fn parse_rg_line(line: &str, cwd: &std::path::Path) -> Option<PickerItem> {
    // Split on first three colons: path:line:col:text
    let mut parts = line.splitn(4, ':');
    let path_str = parts.next()?;
    let line_str = parts.next()?;
    let col_str = parts.next()?;
    let text = parts.next().unwrap_or("");

    let line_num: usize = line_str.parse().ok()?;
    let col_num: usize = col_str.parse().ok()?;
    let path = cwd.join(path_str);

    Some(PickerItem {
        display: format!("{path_str}:{line_num}:{text}"),
        detail: None,
        data: PickerData::GotoLocation {
            path,
            line: line_num,
            col: col_num,
        },
        icon: None,
    })
}

// ============================================================================
// open_file utility (duplicated from picker-files for independence)
// ============================================================================

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

// ============================================================================
// Module implementation
// ============================================================================

/// Grep picker module.
///
/// Registers `GrepPicker` in `PickerRegistry` during init.
pub struct PickerGrepModule;

impl PickerGrepModule {
    /// Create a new instance.
    #[must_use]
    pub const fn new() -> Self {
        Self
    }
}

impl Default for PickerGrepModule {
    fn default() -> Self {
        Self::new()
    }
}

impl Module for PickerGrepModule {
    fn id(&self) -> ModuleId {
        ModuleId::new("picker-grep")
    }

    fn name(&self) -> &'static str {
        "Grep Picker"
    }

    fn version(&self) -> Version {
        Version::new(0, 1, 0)
    }

    fn init(&mut self, ctx: &ModuleContext) -> ProbeResult {
        let registry = ctx.services.get_or_create::<PickerRegistry>();
        registry.register(Arc::new(GrepPicker));
        ProbeResult::Success
    }

    fn exit(&mut self) -> Result<(), ModuleError> {
        Ok(())
    }
}

#[cfg(feature = "dynamic")]
reovim_module_macros::declare_module!(PickerGrepModule);

#[cfg(test)]
#[path = "lib_tests.rs"]
mod tests;
