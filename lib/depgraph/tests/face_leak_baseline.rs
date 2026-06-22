//! Final face-leak enforcement.
//!
//! This file started as a measured baseline. The allowlists have now shrunk to
//! zero, so each probe is a fail-closed gate over production source and
//! manifests.

mod common;

use std::{
    fs,
    path::{Path, PathBuf},
};

use reovim_depgraph::strip_line_comment;

#[test]
fn upper_direct_kabi_is_forbidden() {
    let root = common::workspace_root();
    let actual = upper_direct_kabi(&root);
    assert_no_leaks("upper direct kabi", actual);
}

#[test]
fn upper_lib_ds_world_services_are_forbidden() {
    let root = common::workspace_root();
    let actual = upper_lib_ds_world_services(&root);
    assert_no_leaks("upper lib/ds World-service use", actual);
}

#[test]
fn lower_direct_uapi_posix_is_forbidden() {
    let root = common::workspace_root();
    let actual = lower_direct_uapi_posix(&root);
    assert_no_leaks("lower direct uapi/posix", actual);
}

#[test]
fn upper_raw_arch_sys_is_forbidden() {
    let root = common::workspace_root();
    let actual = upper_raw_arch_sys(&root);
    assert_no_leaks("upper raw arch::sys", actual);
}

#[test]
fn upper_direct_kabi_fixture_trips() {
    let td = common::TempDir::new();
    let root = td.path();
    common::write_file(
        root,
        "editor/lib/kernel/Cargo.toml",
        "[dependencies]\nreovim-kabi-platform = { path = \"../../../kabi/platform\" }\n",
    );
    common::write_file(
        root,
        "client/platforms/tui/src/lib.rs",
        "use reovim_kabi_platform::handle;\n",
    );

    let actual = upper_direct_kabi(root);

    assert!(
        actual
            .iter()
            .any(|line| line.contains("reovim-kabi-platform"))
            && actual
                .iter()
                .any(|line| line.contains("reovim_kabi_platform::handle")),
        "upper direct kabi fixture should trip on manifest and source leaks: {actual:?}"
    );
}

#[test]
fn upper_lib_ds_world_fixture_trips() {
    let td = common::TempDir::new();
    let root = td.path();
    common::write_file(
        root,
        "editor/lib/server/src/lib.rs",
        "use reovim_lib_ds::{\n    time,\n};\n",
    );
    common::write_file(
        root,
        "apps/reovim/src/main.rs",
        "fn main() { let _ = reovim_lib_ds::thread::current_id(); }\n",
    );

    let actual = upper_lib_ds_world_services(root);

    assert!(
        actual.iter().any(|line| line.contains("time"))
            && actual
                .iter()
                .any(|line| line.contains("thread::current_id")),
        "upper lib/ds World-service fixture should trip on grouped and direct uses: {actual:?}"
    );
}

#[test]
fn lower_direct_uapi_posix_fixture_trips() {
    let td = common::TempDir::new();
    let root = td.path();
    common::write_file(
        root,
        "platform/linux-native/Cargo.toml",
        "[dependencies]\nreovim-uapi-posix = { path = \"../../uapi/posix\" }\n",
    );
    common::write_file(root, "arch/sys-linux-x86-64/src/lib.rs", "use reovim_uapi_posix::Errno;\n");

    let actual = lower_direct_uapi_posix(root);

    assert!(
        actual.iter().any(|line| line.contains("reovim-uapi-posix"))
            && actual
                .iter()
                .any(|line| line.contains("reovim_uapi_posix::Errno")),
        "lower direct uapi/posix fixture should trip on manifest and source leaks: {actual:?}"
    );
}

#[test]
fn upper_raw_arch_sys_fixture_trips() {
    let td = common::TempDir::new();
    let root = td.path();
    common::write_file(
        root,
        "apps/reovim/src/main.rs",
        "fn main() { let _ = reovim_arch::sys::write(2, b\"x\"); }\n",
    );
    common::write_file(
        root,
        "editor/lib/kernel/src/lib.rs",
        "fn tick() { let _ = arch::sys::gettid(); }\n",
    );

    let actual = upper_raw_arch_sys(root);

    assert!(
        actual
            .iter()
            .any(|line| line.contains("reovim_arch::sys::write"))
            && actual.iter().any(|line| line.contains("arch::sys::gettid")),
        "upper raw arch::sys fixture should trip on app and product leaks: {actual:?}"
    );
}

fn upper_direct_kabi(root: &Path) -> Vec<String> {
    let mut leaks = Vec::new();
    for dir in ["editor", "client", "apps"] {
        collect_manifest_lines(root, dir, &["reovim-kabi-"], "manifest", &mut leaks);
        collect_source_lines(root, dir, &["reovim_kabi_"], "source", &mut leaks);
    }
    leaks.sort();
    leaks
}

fn upper_lib_ds_world_services(root: &Path) -> Vec<String> {
    let mut leaks = Vec::new();
    for dir in ["editor", "client", "apps"] {
        collect_lib_ds_world_lines(root, dir, &mut leaks);
    }
    leaks.sort();
    leaks
}

fn lower_direct_uapi_posix(root: &Path) -> Vec<String> {
    let mut leaks = Vec::new();
    for dir in ["arch", "platform", "kabi"] {
        collect_manifest_lines(root, dir, &["reovim-uapi-posix"], "manifest", &mut leaks);
        collect_source_lines(root, dir, &["reovim_uapi_posix"], "source", &mut leaks);
    }
    leaks.sort();
    leaks
}

fn upper_raw_arch_sys(root: &Path) -> Vec<String> {
    let mut leaks = Vec::new();
    for dir in ["editor", "client", "apps"] {
        collect_source_lines(
            root,
            dir,
            &["reovim_arch::sys::", "arch::sys::"],
            "source",
            &mut leaks,
        );
    }
    leaks.sort();
    leaks
}

fn collect_manifest_lines(
    root: &Path,
    rel_dir: &str,
    needles: &[&str],
    class: &str,
    out: &mut Vec<String>,
) {
    for path in files_with_name(&root.join(rel_dir), "Cargo.toml") {
        let Ok(text) = fs::read_to_string(&path) else {
            continue;
        };
        for raw in text.lines() {
            let line = raw.trim();
            if line_matches(line, needles) {
                out.push(format_record(root, class, &path, line));
            }
        }
    }
}

fn collect_source_lines(
    root: &Path,
    rel_dir: &str,
    needles: &[&str],
    class: &str,
    out: &mut Vec<String>,
) {
    for path in rust_sources(&root.join(rel_dir)) {
        let Ok(text) = fs::read_to_string(&path) else {
            continue;
        };
        for raw in text.lines() {
            let line = strip_line_comment(raw).trim();
            if line_matches(line, needles) {
                out.push(format_record(root, class, &path, line));
            }
        }
    }
}

fn collect_lib_ds_world_lines(root: &Path, rel_dir: &str, out: &mut Vec<String>) {
    for path in rust_sources(&root.join(rel_dir)) {
        let Ok(text) = fs::read_to_string(&path) else {
            continue;
        };

        let mut in_reovim_lib_ds_use = false;
        for raw in text.lines() {
            let line = strip_line_comment(raw).trim();
            if line.contains("reovim_lib_ds::{") || line.contains("use reovim_lib_ds::{") {
                in_reovim_lib_ds_use = true;
            }

            let direct_path =
                line.contains("reovim_lib_ds::") && contains_lib_ds_world_service_token(line);
            let grouped_use = in_reovim_lib_ds_use && contains_lib_ds_world_service_token(line);

            if direct_path || grouped_use {
                out.push(format_record(root, "source", &path, line));
            }

            if in_reovim_lib_ds_use && line.contains('}') {
                in_reovim_lib_ds_use = false;
            }
        }
    }
}

fn line_matches(line: &str, needles: &[&str]) -> bool {
    !line.is_empty() && needles.iter().any(|needle| line.contains(needle))
}

fn contains_lib_ds_world_service_token(line: &str) -> bool {
    line.split(|c: char| !c.is_ascii_alphanumeric() && c != '_')
        .any(|token| matches!(token, "fs" | "net" | "thread" | "time"))
}

fn rust_sources(root: &Path) -> Vec<PathBuf> {
    let mut files = Vec::new();
    collect_files(root, &mut files, |path| {
        path.extension().is_some_and(|ext| ext == "rs")
            && path
                .file_name()
                .and_then(|name| name.to_str())
                .is_none_or(|name| !name.ends_with("_tests.rs") && name != "tests.rs")
    });
    files
}

fn files_with_name(root: &Path, name: &str) -> Vec<PathBuf> {
    let mut files = Vec::new();
    collect_files(root, &mut files, |path| {
        path.file_name()
            .and_then(|file_name| file_name.to_str())
            .is_some_and(|file_name| file_name == name)
    });
    files
}

fn collect_files(root: &Path, out: &mut Vec<PathBuf>, keep: impl Fn(&Path) -> bool + Copy) {
    let Ok(entries) = fs::read_dir(root) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if path.is_dir() {
            if matches!(name.as_ref(), "target" | "tmp" | "tests" | "fixtures") {
                continue;
            }
            collect_files(&path, out, keep);
        } else if keep(&path) {
            out.push(path);
        }
    }
    out.sort();
}

fn format_record(root: &Path, class: &str, path: &Path, line: &str) -> String {
    let rel = path.strip_prefix(root).unwrap_or(path).display();
    format!("{class}|{rel}|{line}")
}

fn assert_no_leaks(label: &str, actual: Vec<String>) {
    assert!(
        actual.is_empty(),
        "{label} enforcement found forbidden references:\n{}",
        actual.join("\n")
    );
}
