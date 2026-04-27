//! The `[package].reovim-version` constraint on the root manifest is
//! validated against the runtime version passed to [`resolve`].

use std::path::Path;

use {
    reovim_pkg_resolver::{ResolveError, resolve},
    semver::Version,
};

#[test]
fn runtime_outside_root_constraint_errors() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/01-single-dep");
    let too_new = Version::new(1, 0, 0);

    let err = resolve(&root, &too_new).expect_err("incompat must fail");
    match err {
        ResolveError::IncompatibleReovimVersion { required, actual } => {
            assert_eq!(required, "^0.15");
            assert_eq!(actual, "1.0.0");
        }
        other => panic!("expected IncompatibleReovimVersion, got {other:?}"),
    }
}
