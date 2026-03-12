use std::fs;

use super::*;

fn services() -> reovim_kernel::api::v1::ServiceRegistry {
    reovim_kernel::api::v1::ServiceRegistry::new()
}

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
        options: vec![],
    }
}

// -- Picker metadata tests --

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

// -- Items tests --

#[test]
fn items_from_temp_dir() {
    let dir = temp_dir_with_files(&[
        ("main.rs", "fn main() {}"),
        ("lib.rs", "pub mod foo;"),
        ("src/utils.rs", "pub fn helper() {}"),
    ]);
    let picker = FilesPicker::new();
    let ctx = ctx_for(dir.path());
    let items = picker.items(&ctx, &services());

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
    let items = picker.items(&ctx, &services());

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
    let items = picker.items(&ctx, &services());

    let displays: Vec<&str> = items.iter().map(|i| i.display.as_str()).collect();
    assert!(displays.contains(&"main.rs"));
    assert!(!displays.contains(&"debug.log"), "*.log should be gitignored");
    assert!(!displays.iter().any(|d| d.contains("target")), "target/ should be gitignored");
}

// -- on_select tests --

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

// -- Preview tests --

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
    let preview = picker.preview(&item, &services());
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
    assert!(picker.preview(&item, &services()).is_none());
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
    assert!(picker.preview(&item, &services()).is_none());
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
    assert!(picker.preview(&item, &services()).is_none());
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
    assert!(picker.preview(&item, &services()).is_none());
}

// -- Module tests --

#[test]
fn module_id() {
    let module = PickerFilesModule::new();
    assert_eq!(module.id().as_str(), "picker-files");
}

#[test]
fn module_name() {
    let module = PickerFilesModule::new();
    assert_eq!(module.name(), "File Picker");
}

#[test]
fn module_version() {
    let module = PickerFilesModule::new();
    let version = module.version();
    assert_eq!(version.major, 0);
    assert_eq!(version.minor, 1);
}

#[test]
#[allow(clippy::default_constructed_unit_structs)]
fn module_default() {
    let module = PickerFilesModule::default();
    assert_eq!(module.id().as_str(), "picker-files");
}

#[test]
fn module_exit() {
    let mut module = PickerFilesModule::new();
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

    let mut module = PickerFilesModule::new();
    let result = module.init(&ctx);
    assert!(matches!(result, ProbeResult::Success));

    let registry = services.get::<PickerRegistry>();
    assert!(registry.is_some());
    let reg = registry.unwrap();
    assert!(reg.get("files").is_some());
}
