//! Guards that `reovim-subsys-net` has at least one in-tree driver
//! implementation.
//!
//! Plan 15 Phase N revived the dormant `reovim-subsys-net` crate by
//! defining a real `GrpcServerDriver` trait contract and landing
//! `ext/server/drivers/net-grpc/` as the canonical implementer. Before
//! that, the subsys had zero production consumers and was dead code
//! for multiple releases.
//!
//! This guard prevents a regression: if every `reovim-driver-net-*`
//! crate gets deleted or renamed away from the `reovim-driver-net-*`
//! prefix, the subsys becomes dead again. Catch the regression at
//! depgraph-test time rather than discovering it at release time.
//!
//! ## Approach (no AST parse, matches Plan 14 D.3 style)
//!
//! 1. `cargo_metadata` enumerates workspace packages.
//! 2. Filter packages whose name matches `reovim-driver-net-*`.
//! 3. Assert at least one of those declares a production dep
//!    (`DependencyKind::Normal`) on `reovim-subsys-net`.
//!
//! The crate-name regex is fragile to future renames. If the net
//! driver is ever renamed away from `reovim-driver-net-*`, this
//! guard must be updated alongside the rename — no silent drift.

use cargo_metadata::{DependencyKind, MetadataCommand};

const SUBSYS_NET_PACKAGE: &str = "reovim-subsys-net";
const DRIVER_NAME_PREFIX: &str = "reovim-driver-net-";

#[test]
fn subsys_net_has_at_least_one_driver_impl() {
    let metadata = MetadataCommand::new().exec().expect("cargo metadata");

    let driver_candidates: Vec<&cargo_metadata::Package> = metadata
        .workspace_packages()
        .into_iter()
        .filter(|p| p.name.as_str().starts_with(DRIVER_NAME_PREFIX))
        .collect();

    assert!(
        !driver_candidates.is_empty(),
        "no workspace crate matches `{DRIVER_NAME_PREFIX}*`. At least one \
         `reovim-driver-net-<transport>` crate must exist to implement \
         `GrpcServerDriver` from `reovim-subsys-net`. The crate-name regex \
         is load-bearing — if the net driver was renamed, update this \
         test alongside the rename."
    );

    let has_implementer = driver_candidates.iter().any(|p| {
        p.dependencies
            .iter()
            .any(|d| d.kind == DependencyKind::Normal && d.name == SUBSYS_NET_PACKAGE)
    });

    assert!(
        has_implementer,
        "`{SUBSYS_NET_PACKAGE}` has no production implementer in the \
         workspace. Found candidate crates matching `{DRIVER_NAME_PREFIX}*`: \
         {:?}. None of them declare `{SUBSYS_NET_PACKAGE}` as a `[dependencies]` \
         entry. The subsys revival invariant established by Plan 15 Phase N \
         requires at least one driver crate to take a production dep on the \
         subsys contract; otherwise the subsys is dead code.",
        driver_candidates
            .iter()
            .map(|p| p.name.as_str())
            .collect::<Vec<_>>()
    );
}
