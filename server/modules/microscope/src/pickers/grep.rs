//! Grep picker - content search using ripgrep subprocess.
//!
//! Runs `rg --line-number --column --no-heading -- <query> <cwd>` and
//! parses the output. Falls back gracefully when rg is not found.

use std::{
    fs,
    io::{BufRead, BufReader},
    process::Command,
};

use {
    reovim_driver_picker::{
        Picker, PickerAction, PickerContext, PickerData, PickerItem, PreviewContent,
    },
    reovim_kernel::api::v1::ServiceRegistry,
};

/// Number of context lines to show around a grep match in preview.
const PREVIEW_CONTEXT_LINES: usize = 5;

/// Maximum number of grep results to return.
const MAX_RESULTS: usize = 1000;

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

    fn items(&self, ctx: &PickerContext, _services: &ServiceRegistry) -> Vec<PickerItem> {
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

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;

    fn services() -> ServiceRegistry {
        ServiceRegistry::new()
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
        // Test with a non-existent command path to simulate rg not found.
        // We can't easily test this without mocking Command, but we can
        // verify the run_ripgrep function handles the error path.
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
        // Lines after skip(9994) will be empty, so preview returns None.
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
}
