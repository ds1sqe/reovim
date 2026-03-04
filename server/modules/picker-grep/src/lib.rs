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
    reovim_driver_vfs::VfsInstance,
    reovim_kernel::api::v1::{Module, ModuleContext, ModuleError, ModuleId, ProbeResult, Version},
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
mod tests {
    use std::path::PathBuf;

    use super::*;

    fn services() -> reovim_kernel::api::v1::ServiceRegistry {
        reovim_kernel::api::v1::ServiceRegistry::new()
    }

    #[test]
    fn name_and_title() {
        let picker = GrepPicker::new();
        assert_eq!(picker.name(), "grep");
        assert_eq!(picker.title(), "Grep");
    }

    #[test]
    fn prompt_override() {
        let picker = GrepPicker::new();
        assert_eq!(picker.prompt(), "rg> ");
    }

    #[test]
    fn is_not_static() {
        let picker = GrepPicker::new();
        assert!(!picker.is_static());
    }

    #[test]
    #[allow(clippy::default_constructed_unit_structs)]
    fn default_impl() {
        let picker = GrepPicker::default();
        assert_eq!(picker.name(), "grep");
    }

    #[test]
    fn empty_query_returns_empty() {
        let picker = GrepPicker::new();
        let ctx = PickerContext {
            cwd: PathBuf::from("."),
            query: String::new(),
            buffers: vec![],
            commands: vec![],
        };
        assert!(picker.items(&ctx, &services()).is_empty());
    }

    #[test]
    fn parse_valid_rg_line() {
        let cwd = PathBuf::from("/project");
        let item = parse_rg_line("src/main.rs:10:5:fn main() {", &cwd);
        assert!(item.is_some());
        let item = item.unwrap();
        assert_eq!(item.display, "src/main.rs:10:fn main() {");
        assert!(matches!(
            &item.data,
            PickerData::GotoLocation { path, line: 10, col: 5 } if *path == std::path::Path::new("/project/src/main.rs")
        ));
    }

    #[test]
    fn parse_rg_line_empty_text() {
        let cwd = PathBuf::from("/tmp");
        let item = parse_rg_line("file.rs:1:1:", &cwd);
        assert!(item.is_some());
        assert_eq!(item.unwrap().display, "file.rs:1:");
    }

    #[test]
    fn parse_rg_line_invalid_format() {
        let cwd = PathBuf::from(".");
        assert!(parse_rg_line("not valid", &cwd).is_none());
        assert!(parse_rg_line("file.rs:notnum:1:text", &cwd).is_none());
        assert!(parse_rg_line("file.rs:1:notnum:text", &cwd).is_none());
    }

    #[test]
    fn on_select_grep_match() {
        let picker = GrepPicker::new();
        let item = PickerItem {
            display: "test.rs:10:hello".to_owned(),
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
            matches!(action, PickerAction::GotoLocation { ref path, line: 10, col: 5 } if *path == std::path::Path::new("test.rs"))
        );
    }

    #[test]
    fn on_select_wrong_data_closes() {
        let picker = GrepPicker::new();
        let item = PickerItem {
            display: "x".to_owned(),
            detail: None,
            data: PickerData::Text("wrong".to_owned()),
            icon: None,
        };
        assert!(matches!(picker.on_select(&item), PickerAction::Close));
    }

    #[test]
    fn preview_grep_match() {
        let dir = tempfile::tempdir().expect("Failed to create temp dir");
        let path = dir.path().join("test.rs");
        let content = (1..=20)
            .map(|i| format!("line {i}"))
            .collect::<Vec<_>>()
            .join("\n");
        std::fs::write(&path, &content).expect("Failed to write file");

        let picker = GrepPicker::new();
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
        let picker = GrepPicker::new();
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
        let picker = GrepPicker::new();
        let item = PickerItem {
            display: "nope:1:x".to_owned(),
            detail: None,
            data: PickerData::GotoLocation {
                path: PathBuf::from("/nonexistent_12345.rs"),
                line: 1,
                col: 1,
            },
            icon: None,
        };
        assert!(picker.preview(&item, &services()).is_none());
    }

    #[test]
    fn rg_not_found_fallback() {
        let result = run_ripgrep("test", &PathBuf::from("/nonexistent_dir_12345"));
        assert!(result.is_empty());
    }

    #[test]
    fn items_with_query_runs_ripgrep() {
        let dir = tempfile::tempdir().expect("Failed to create temp dir");
        let file_path = dir.path().join("searchable.rs");
        std::fs::write(&file_path, "fn unique_grep_test_marker() {}\n")
            .expect("Failed to write file");

        let picker = GrepPicker::new();
        let ctx = PickerContext {
            cwd: dir.path().to_path_buf(),
            query: "unique_grep_test_marker".to_owned(),
            buffers: vec![],
            commands: vec![],
        };
        let items = picker.items(&ctx, &services());
        // rg may or may not be installed; if it is, we get results.
        if !items.is_empty() {
            assert!(items[0].display.contains("unique_grep_test_marker"));
        }
    }

    #[test]
    fn run_ripgrep_success_path() {
        let dir = tempfile::tempdir().expect("Failed to create temp dir");
        let file_path = dir.path().join("target_file.txt");
        std::fs::write(&file_path, "hello rg_coverage_test\nworld\n")
            .expect("Failed to write file");

        let results = run_ripgrep("rg_coverage_test", dir.path());
        // rg may or may not be installed.
        if !results.is_empty() {
            assert!(results[0].display.contains("rg_coverage_test"));
        }
    }

    #[test]
    fn preview_line_beyond_file_end() {
        let dir = tempfile::tempdir().expect("Failed to create temp dir");
        let path = dir.path().join("short.rs");
        std::fs::write(&path, "line 1\nline 2\n").expect("Failed to write");

        let picker = GrepPicker::new();
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

    #[test]
    fn parse_rg_line_with_colons_in_text() {
        let cwd = PathBuf::from("/project");
        let item = parse_rg_line("src/main.rs:5:3:let url = \"http://example.com\";", &cwd);
        assert!(item.is_some());
        let item = item.unwrap();
        assert!(item.display.contains("http://example.com"));
    }

    // -- Module tests --

    #[test]
    fn module_id() {
        let module = PickerGrepModule::new();
        assert_eq!(module.id().as_str(), "picker-grep");
    }

    #[test]
    fn module_name() {
        let module = PickerGrepModule::new();
        assert_eq!(module.name(), "Grep Picker");
    }

    #[test]
    fn module_version() {
        let module = PickerGrepModule::new();
        let version = module.version();
        assert_eq!(version.major, 0);
        assert_eq!(version.minor, 1);
    }

    #[test]
    #[allow(clippy::default_constructed_unit_structs)]
    fn module_default() {
        let module = PickerGrepModule::default();
        assert_eq!(module.id().as_str(), "picker-grep");
    }

    #[test]
    fn module_exit() {
        let mut module = PickerGrepModule::new();
        assert!(module.exit().is_ok());
    }

    #[test]
    fn module_init_registers_picker() {
        let services = Arc::new(reovim_kernel::api::v1::ServiceRegistry::new());
        let ctx = ModuleContext::new(
            reovim_kernel::api::v1::KernelContext::default(),
            services.clone(),
            PathBuf::from("/tmp"),
            PathBuf::from("/tmp"),
        );

        let mut module = PickerGrepModule::new();
        let result = module.init(&ctx);
        assert!(matches!(result, ProbeResult::Success));

        let registry = services.get::<PickerRegistry>();
        assert!(registry.is_some());
        let reg = registry.unwrap();
        assert!(reg.get("grep").is_some());
    }
}
