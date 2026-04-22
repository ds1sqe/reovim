//! Locks the `reovim-ext-client-tui-cap-cell-view` crate's shape in
//! the workspace.
//!
//! Plan 25 / 17-γ.1 adds a new ExtClient{tui} crate that carries the
//! `ViewHint` enum, the `ViewRasterizer` trait, a narrow terminal-
//! backend-level `RasterOutput` trait, and a `FullBlockRasterizer`
//! baseline impl. This probe pins:
//!
//! 1. The crate exists as a workspace member.
//! 2. It depends on `reovim-ext-client-tui-cap-cell` (sibling Ext) and
//!    is allowed to depend on `reovim-client-driver` (`RepoCore`).
//! 3. It has NO dep on `reovim-kernel` or any `reovim-subsys-*` crate
//!    (this is a TUI-ext crate, not a subsys consumer).
//! 4. The reverse edge is absent: `cap-cell` must not depend on
//!    `cap-cell-view` (that would create a cycle).

use cargo_metadata::{DependencyKind, MetadataCommand};

const CELL_VIEW: &str = "reovim-ext-client-tui-cap-cell-view";
const CELL: &str = "reovim-ext-client-tui-cap-cell";

fn normal_deps(pkg: &cargo_metadata::Package) -> Vec<&str> {
    pkg.dependencies
        .iter()
        .filter(|d| d.kind == DependencyKind::Normal)
        .map(|d| d.name.as_str())
        .collect()
}

#[test]
fn cell_view_crate_is_a_workspace_member() {
    let metadata = MetadataCommand::new().exec().expect("cargo metadata");
    let found = metadata
        .workspace_packages()
        .into_iter()
        .any(|p| p.name.as_str() == CELL_VIEW);
    assert!(
        found,
        "{CELL_VIEW} must be a workspace member (Plan 25 introduces it)"
    );
}

#[test]
fn cell_view_depends_on_cap_cell() {
    let metadata = MetadataCommand::new().exec().expect("cargo metadata");
    let pkg = metadata
        .workspace_packages()
        .into_iter()
        .find(|p| p.name.as_str() == CELL_VIEW)
        .expect("cell-view crate present");
    let deps = normal_deps(pkg);
    assert!(
        deps.contains(&CELL),
        "{CELL_VIEW} must declare a normal dep on {CELL} (needs \
         CellCapability, Cell, CellStyle). Found: {deps:?}"
    );
}

#[test]
fn cell_view_has_no_kernel_or_subsys_deps() {
    let metadata = MetadataCommand::new().exec().expect("cargo metadata");
    let pkg = metadata
        .workspace_packages()
        .into_iter()
        .find(|p| p.name.as_str() == CELL_VIEW)
        .expect("cell-view crate present");
    for dep in normal_deps(pkg) {
        assert!(
            !dep.starts_with("reovim-subsys-"),
            "{CELL_VIEW} must not depend on subsys crate '{dep}' — \
             this is a TUI ext crate, not a subsys consumer."
        );
        assert_ne!(
            dep, "reovim-kernel",
            "{CELL_VIEW} must not depend on reovim-kernel."
        );
    }
}

#[test]
fn cap_cell_does_not_depend_on_cell_view() {
    let metadata = MetadataCommand::new().exec().expect("cargo metadata");
    let pkg = metadata
        .workspace_packages()
        .into_iter()
        .find(|p| p.name.as_str() == CELL)
        .expect("cap-cell crate present");
    let deps = normal_deps(pkg);
    assert!(
        !deps.contains(&CELL_VIEW),
        "Cycle guard: {CELL} must NOT depend on {CELL_VIEW}. The \
         direction is cell-view → cap-cell (cell-view reads a \
         CellCapability grid); the reverse would create a cycle."
    );
}
