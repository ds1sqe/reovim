//! Locks the shape of `reovim-client-ext-platform-web` (Flight 76).
//!
//! 1. Asserts the crate is a workspace member.
//! 2. Asserts the three direct normal deps (tokio / clap / thiserror).
//! 3. Reasserts the forbidden-edge rule from
//!    `.claude/rules/architecture.md`: a
//!    `ext/client/platforms/*` crate MUST NOT directly depend on
//!    `ext/client/capabilities/*` or `ext/client/driver/*`. The
//!    general rule is enforced by
//!    `ext_client_category_isolation.rs`; this probe is complementary
//!    and sits next to the crate-specific sibling
//!    `cell_view_half_block_exists.rs` so a web-side dep surprise
//!    surfaces in the expected place.
//! 4. Smoke-compiles the four public symbols Flight 76 exports
//!    (`run`, `WebArgs`, `canonical_frame`, `render_page`).

use cargo_metadata::{DependencyKind, MetadataCommand};

const CRATE: &str = "reovim-client-ext-platform-web";

fn normal_deps(pkg: &cargo_metadata::Package) -> Vec<&str> {
    pkg.dependencies
        .iter()
        .filter(|d| d.kind == DependencyKind::Normal)
        .map(|d| d.name.as_str())
        .collect()
}

fn platform_web_pkg(metadata: &cargo_metadata::Metadata) -> &cargo_metadata::Package {
    metadata
        .workspace_packages()
        .into_iter()
        .find(|p| p.name.as_str() == CRATE)
        .expect("reovim-client-ext-platform-web must be a workspace member")
}

#[test]
fn platforms_web_crate_is_a_workspace_member() {
    let metadata = MetadataCommand::new().exec().expect("cargo metadata");
    let found = metadata
        .workspace_packages()
        .into_iter()
        .any(|p| p.name.as_str() == CRATE);
    assert!(found, "{CRATE} must be a workspace member (Plan 03 introduces it)");
}

#[test]
fn platforms_web_depends_on_tokio_and_clap_and_thiserror() {
    let metadata = MetadataCommand::new().exec().expect("cargo metadata");
    let pkg = platform_web_pkg(&metadata);
    let deps = normal_deps(pkg);
    for required in ["tokio", "clap", "thiserror"] {
        assert!(
            deps.contains(&required),
            "{CRATE} must declare a direct normal dep on `{required}`. Found: {deps:?}"
        );
    }
}

#[test]
fn platforms_web_has_no_capability_or_driver_deps() {
    let metadata = MetadataCommand::new().exec().expect("cargo metadata");
    let pkg = platform_web_pkg(&metadata);
    for dep in normal_deps(pkg) {
        assert!(
            !dep.starts_with("reovim-ext-client-") || !dep.contains("-cap-"),
            "CLM v7 forbids platforms → capabilities direct edges; \
             {CRATE} depends on capability crate `{dep}`."
        );
        assert!(
            !dep.starts_with("reovim-driver-"),
            "CLM v7 forbids platforms → drivers direct edges; \
             {CRATE} depends on driver crate `{dep}`."
        );
    }
}

#[test]
fn platforms_web_public_symbols_compile() {
    // Flight 76 public surface — any rename or removal fails at build.
    #[allow(unused_imports)]
    use reovim_client_ext_platform_web::{
        RunError, WebArgs, WebCell, WebColor, WebFrame, canonical_frame, render_frame_svg,
        render_page, run, serve,
    };

    let frame = canonical_frame();
    assert_eq!(frame.width(), 20);
    assert_eq!(frame.height(), 3);
    let svg = render_frame_svg(&frame);
    assert!(svg.contains("<svg"));
    let page = render_page(&frame);
    assert!(page.contains(&svg));

    // Stateless handle smoke check. `serve` is already in the
    // `use` import above — any rename breaks compilation there.
    let _: fn(WebArgs) -> Result<(), RunError> = run;
    let _ = WebColor::Default;
    let _ = WebCell::default();
    let _: Option<&WebCell> = frame.cell(0, 0);
}
