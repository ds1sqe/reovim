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
        options: vec![],
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
        options: vec![],
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
