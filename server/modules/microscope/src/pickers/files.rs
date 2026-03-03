//! Files picker - fuzzy file finder with .gitignore support.
//!
//! Uses the `ignore` crate for .gitignore-aware file walking.

use std::{
    fs,
    io::{BufRead, BufReader},
    path::PathBuf,
};

use reovim_driver_picker::{
    Picker, PickerAction, PickerContext, PickerData, PickerItem, PreviewContent,
};

/// Maximum number of preview lines to read from a file.
const PREVIEW_MAX_LINES: usize = 50;

/// Picker that lists files in the working directory.
///
/// Walks files respecting `.gitignore` rules via the `ignore` crate.
/// Returns relative paths from `ctx.cwd`.
pub struct FilesPicker;

impl FilesPicker {
    /// Create a new instance.
    #[must_use]
    pub const fn new() -> Self {
        Self
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
        "Files"
    }

    fn items(&self, ctx: &PickerContext) -> Vec<PickerItem> {
        let walker = ignore::WalkBuilder::new(&ctx.cwd)
            .hidden(true)
            .git_ignore(true)
            .git_global(true)
            .git_exclude(true)
            .build();

        walker
            .filter_map(Result::ok)
            .filter(|entry| entry.file_type().is_some_and(|ft| ft.is_file()))
            .filter_map(|entry| {
                let path = entry.path();
                let relative = path.strip_prefix(&ctx.cwd).ok()?;
                let display = relative.to_string_lossy().into_owned();
                Some(PickerItem {
                    display,
                    detail: None,
                    data: PickerData::FilePath(path.to_path_buf()),
                    icon: None,
                })
            })
            .collect()
    }

    fn on_select(&self, item: &PickerItem) -> PickerAction {
        match &item.data {
            PickerData::FilePath(path) => PickerAction::OpenFile(path.clone()),
            _ => PickerAction::Close,
        }
    }

    fn preview(&self, item: &PickerItem) -> Option<PreviewContent> {
        let PickerData::FilePath(path) = &item.data else {
            return None;
        };

        // Skip binary files by checking the first few bytes.
        if is_likely_binary(path) {
            return None;
        }

        let file = fs::File::open(path).ok()?;
        let reader = BufReader::new(file);
        let lines: Vec<String> = reader
            .lines()
            .take(PREVIEW_MAX_LINES)
            .filter_map(Result::ok)
            .collect();

        if lines.is_empty() {
            return None;
        }

        Some(PreviewContent {
            lines,
            highlight_line: None,
            file_path: Some(path.clone()),
        })
    }
}

/// Check if a file is likely binary by reading the first 512 bytes.
fn is_likely_binary(path: &PathBuf) -> bool {
    let Ok(bytes) = fs::read(path) else {
        return true;
    };
    let check_len = bytes.len().min(512);
    bytes[..check_len].contains(&0)
}

#[cfg(test)]
mod tests {
    use std::fs;

    use super::*;

    fn temp_dir_with_files(files: &[(&str, &str)]) -> tempfile::TempDir {
        let dir = tempfile::tempdir().expect("Failed to create temp dir");
        for (name, content) in files {
            let path = dir.path().join(name);
            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent).expect("Failed to create parent dir");
            }
            fs::write(&path, content).expect("Failed to write file");
        }
        dir
    }

    fn ctx_for(dir: &std::path::Path) -> PickerContext {
        PickerContext {
            cwd: dir.to_path_buf(),
            query: String::new(),
            buffers: vec![],
            commands: vec![],
        }
    }

    #[test]
    fn name_and_title() {
        let picker = FilesPicker::new();
        assert_eq!(picker.name(), "files");
        assert_eq!(picker.title(), "Files");
    }

    #[test]
    fn is_static() {
        let picker = FilesPicker::new();
        assert!(picker.is_static());
    }

    #[test]
    #[allow(clippy::default_constructed_unit_structs)]
    fn default_impl() {
        let picker = FilesPicker::default();
        assert_eq!(picker.name(), "files");
    }

    #[test]
    fn items_from_temp_dir() {
        let dir = temp_dir_with_files(&[
            ("main.rs", "fn main() {}"),
            ("lib.rs", "pub mod foo;"),
            ("src/utils.rs", "pub fn helper() {}"),
        ]);
        let picker = FilesPicker::new();
        let ctx = ctx_for(dir.path());
        let items = picker.items(&ctx);

        assert_eq!(items.len(), 3);
        let displays: Vec<&str> = items.iter().map(|i| i.display.as_str()).collect();
        assert!(displays.contains(&"main.rs"));
        assert!(displays.contains(&"lib.rs"));
        assert!(displays.contains(&"src/utils.rs"));
    }

    #[test]
    fn items_returns_relative_paths() {
        let dir = temp_dir_with_files(&[("a/b/c.txt", "hello")]);
        let picker = FilesPicker::new();
        let ctx = ctx_for(dir.path());
        let items = picker.items(&ctx);

        assert_eq!(items.len(), 1);
        assert_eq!(items[0].display, "a/b/c.txt");
    }

    #[test]
    fn gitignore_respected() {
        let dir = temp_dir_with_files(&[
            (".gitignore", "*.log\ntarget/\n"),
            ("main.rs", "fn main() {}"),
            ("debug.log", "log data"),
            ("target/output.bin", "binary"),
        ]);
        // Initialize git repo so .gitignore is respected.
        std::process::Command::new("git")
            .args(["init"])
            .current_dir(dir.path())
            .output()
            .expect("Failed to run git init");

        let picker = FilesPicker::new();
        let ctx = ctx_for(dir.path());
        let items = picker.items(&ctx);

        let displays: Vec<&str> = items.iter().map(|i| i.display.as_str()).collect();
        assert!(displays.contains(&"main.rs"));
        assert!(!displays.contains(&"debug.log"), "*.log should be gitignored");
        assert!(!displays.iter().any(|d| d.contains("target")), "target/ should be gitignored");
    }

    #[test]
    fn on_select_file_path() {
        let picker = FilesPicker::new();
        let item = PickerItem {
            display: "main.rs".to_owned(),
            detail: None,
            data: PickerData::FilePath(PathBuf::from("/tmp/main.rs")),
            icon: None,
        };
        let action = picker.on_select(&item);
        assert!(
            matches!(action, PickerAction::OpenFile(ref p) if p == &PathBuf::from("/tmp/main.rs"))
        );
    }

    #[test]
    fn on_select_wrong_data_closes() {
        let picker = FilesPicker::new();
        let item = PickerItem {
            display: "test".to_owned(),
            detail: None,
            data: PickerData::Text("wrong".to_owned()),
            icon: None,
        };
        let action = picker.on_select(&item);
        assert!(matches!(action, PickerAction::Close));
    }

    #[test]
    fn preview_text_file() {
        let dir = temp_dir_with_files(&[("hello.rs", "fn main() {\n    println!(\"hello\");\n}")]);
        let picker = FilesPicker::new();
        let path = dir.path().join("hello.rs");
        let item = PickerItem {
            display: "hello.rs".to_owned(),
            detail: None,
            data: PickerData::FilePath(path.clone()),
            icon: None,
        };
        let preview = picker.preview(&item);
        assert!(preview.is_some());
        let preview = preview.unwrap();
        assert_eq!(preview.lines.len(), 3);
        assert_eq!(preview.lines[0], "fn main() {");
        assert!(preview.highlight_line.is_none());
        assert_eq!(preview.file_path, Some(path));
    }

    #[test]
    fn preview_nonexistent_file() {
        let picker = FilesPicker::new();
        let item = PickerItem {
            display: "nope.rs".to_owned(),
            detail: None,
            data: PickerData::FilePath(PathBuf::from("/tmp/nonexistent_file_12345.rs")),
            icon: None,
        };
        assert!(picker.preview(&item).is_none());
    }

    #[test]
    fn preview_binary_file() {
        let dir = tempfile::tempdir().expect("Failed to create temp dir");
        let path = dir.path().join("binary.bin");
        fs::write(&path, [0u8, 1, 2, 255, 0, 3]).expect("Failed to write binary");

        let picker = FilesPicker::new();
        let item = PickerItem {
            display: "binary.bin".to_owned(),
            detail: None,
            data: PickerData::FilePath(path),
            icon: None,
        };
        assert!(picker.preview(&item).is_none());
    }

    #[test]
    fn preview_wrong_data_type() {
        let picker = FilesPicker::new();
        let item = PickerItem {
            display: "x".to_owned(),
            detail: None,
            data: PickerData::Text("y".to_owned()),
            icon: None,
        };
        assert!(picker.preview(&item).is_none());
    }

    #[test]
    fn preview_empty_file() {
        let dir = tempfile::tempdir().expect("Failed to create temp dir");
        let path = dir.path().join("empty.txt");
        fs::write(&path, "").expect("Failed to write empty file");

        let picker = FilesPicker::new();
        let item = PickerItem {
            display: "empty.txt".to_owned(),
            detail: None,
            data: PickerData::FilePath(path),
            icon: None,
        };
        assert!(picker.preview(&item).is_none());
    }
}
