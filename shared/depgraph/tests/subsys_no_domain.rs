//! Enforces that subsys crates have zero domain dependencies.
//! Subsys contracts are domain-neutral by design — they define traits
//! and types that any domain (text, 3D, audio) can implement.

use cargo_metadata::MetadataCommand;

#[test]
fn subsys_crates_have_no_domain_deps() {
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
            let dep_name = metadata
                .packages
                .iter()
                .find(|p| p.id == dep.pkg)
                .map_or("<unknown>", |p| p.name.as_str());

            if dep_name.starts_with("reovim-domain-") || dep_name.starts_with("reovim-provider-") {
                violations.push(format!("{} -> {dep_name}", pkg.name));
            }
        }
    }

    assert!(
        violations.is_empty(),
        "Subsys crates must have zero domain/provider dependencies. \
         Violations:\n  {}",
        violations.join("\n  ")
    );
}
