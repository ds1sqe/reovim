//! Two runs of [`resolve`] on the same input produce byte-identical
//! `pkg.lock` output.

use std::path::Path;

use {reovim_pkg_resolver::resolve, semver::Version};

#[test]
fn diamond_lockfile_is_byte_stable_across_runs() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/03-diamond");
    let runtime = Version::new(0, 15, 0);

    let a = resolve(&root, &runtime)
        .expect("first resolve")
        .into_lockfile()
        .to_toml_string()
        .expect("first serialize");
    let b = resolve(&root, &runtime)
        .expect("second resolve")
        .into_lockfile()
        .to_toml_string()
        .expect("second serialize");

    assert_eq!(a, b, "lockfile output is not byte-stable across runs");
}
