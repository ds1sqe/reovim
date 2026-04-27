//! Enforces that subsys crates have zero **production** driver dependencies.
//! Subsys contracts sit below the driver layer — drivers implement
//! subsys traits, never the other way around.
//!
//! Dev-dependencies are allowed: a host-side loader subsys (e.g.
//! `reovim-subsys-driver-loader`) needs to dlopen a real cdylib in its
//! integration tests, which requires the driver crate as a dev-dep.
//! Only normal (= production) deps trigger this probe.

use cargo_metadata::{DependencyKind, MetadataCommand};

#[test]
fn subsys_crates_have_no_driver_deps() {
    let metadata = MetadataCommand::new().exec().expect("cargo metadata");
    let resolve = metadata.resolve.as_ref().expect("resolved dep graph");

    let subsys_packages: Vec<_> = metadata
        .packages
        .iter()
        .filter(|p| p.name.starts_with("reovim-subsys-"))
        .collect();

    assert!(!subsys_packages.is_empty(), "no reovim-subsys-* packages found");

    let mut violations = Vec::new();

    for pkg in &subsys_packages {
        let node = resolve
            .nodes
            .iter()
            .find(|n| n.id == pkg.id)
            .expect("package in resolve graph");

        for dep in &node.deps {
            let is_normal = dep
                .dep_kinds
                .iter()
                .any(|k| k.kind == DependencyKind::Normal);
            if !is_normal {
                continue;
            }

            let dep_name = metadata
                .packages
                .iter()
                .find(|p| p.id == dep.pkg)
                .map_or("<unknown>", |p| p.name.as_str());

            if dep_name.starts_with("reovim-driver-") {
                violations.push(format!("{} -> {dep_name}", pkg.name));
            }
        }
    }

    assert!(
        violations.is_empty(),
        "Subsys crates must have zero driver dependencies. \
         Violations:\n  {}",
        violations.join("\n  ")
    );
}
