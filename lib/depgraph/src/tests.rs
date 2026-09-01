use std::{
    fs,
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
};

use super::*;

static TEMP_ID: AtomicU64 = AtomicU64::new(0);

struct TempRoot(PathBuf);

impl TempRoot {
    fn new() -> Self {
        let sequence = TEMP_ID.fetch_add(1, Ordering::Relaxed);
        let path =
            std::env::temp_dir().join(format!("reovim-depgraph-{}-{sequence}", std::process::id()));
        fs::create_dir(&path).expect("create fixture root");
        Self(path)
    }

    fn path(&self) -> &Path {
        &self.0
    }

    fn write(&self, relative: &str, contents: &str) {
        let path = self.0.join(relative);
        fs::create_dir_all(path.parent().expect("fixture file has parent"))
            .expect("create fixture directory");
        fs::write(path, contents).expect("write fixture file");
    }
}

impl Drop for TempRoot {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn production(owner: ArchitecturalRole) -> SourceRole {
    SourceRole::production(owner)
}

fn map(rules: impl IntoIterator<Item = (&'static str, ArchitecturalRole)>) -> SourceRoleMap {
    SourceRoleMap::new(
        1,
        rules
            .into_iter()
            .map(|(prefix, owner)| SourceRoleRule::new(prefix, 10, SourceRole::production(owner))),
    )
    .expect("valid role map")
}

#[test]
fn role_vocabulary_is_complete_and_stable() {
    let exact_names = [
        "Foundation",
        "Kernel Interface",
        "Kernel Core",
        "Kernel Runtime",
        "Public SDKs",
        "Concrete policy",
        "Resource Adapter",
        "Host Adapter",
        "External Protocol Binding",
        "Protocol Adapter",
        "Extension Adapter",
        "Extension Engine Adapter",
        "Client Family",
        "Client Runtime",
        "Presentation Adapter",
        "Assembly",
    ];
    assert_eq!(ArchitecturalRole::ALL.map(ArchitecturalRole::as_str), exact_names);
}

#[test]
fn source_role_map_is_fail_closed_and_uses_explicit_precedence() {
    use ArchitecturalRole::{Foundation, KernelInterface};

    let source_map = SourceRoleMap::new(
        3,
        [
            SourceRoleRule::new("tree", 1, production(Foundation)),
            SourceRoleRule::new("tree/inner", 2, production(KernelInterface)),
        ],
    )
    .unwrap();

    assert_eq!(source_map.schema_version(), 3);
    assert_eq!(source_map.classify("tree/inner/item.rs").unwrap(), production(KernelInterface));
    assert!(matches!(
        source_map.classify("elsewhere/item.rs"),
        Err(RoleMapError::Unmatched { .. })
    ));
    assert!(matches!(source_map.classify("../escape"), Err(RoleMapError::EscapesRoot { .. })));

    let ambiguous = SourceRoleMap::new(
        1,
        [
            SourceRoleRule::new("tree", 4, production(Foundation)),
            SourceRoleRule::new("tree/inner", 4, production(KernelInterface)),
        ],
    )
    .unwrap();
    assert!(matches!(
        ambiguous.classify("tree/inner/item.rs"),
        Err(RoleMapError::Ambiguous { .. })
    ));
}

#[test]
fn loads_and_preflights_supported_declared_path_graph() {
    let root = TempRoot::new();
    root.write("Cargo.toml", "[workspace]\nmembers = []\n");
    root.write("nodes/one/Cargo.toml", "[package]\nname = \"alpha\"\nversion = \"0.0.0\"\n");
    root.write(
        "nodes/two/Cargo.toml",
        r#"[package]
name = "beta"
version = "0.0.0"

[dependencies.renamed]
package = "alpha"
path = "../one"
optional = true
"#,
    );
    root.write(
        "nodes/three/Cargo.toml",
        r#"[package]
name = "gamma"
version = "0.0.0"

[target.'cfg(unix)'.dev-dependencies]
beta = { path = "../two" }

[build-dependencies]
beta = { path = "../two" }

[dependencies]
outside = "1"
"#,
    );
    root.write("target/ignored/Cargo.toml", "[package]\nname = \"ignored\"\nversion = \"0.0.0\"\n");

    let graph = load_declared_manifest_graph(root.path(), &DiscoveryOptions::default()).unwrap();
    assert_eq!(graph.packages().len(), 3);
    assert_eq!(graph.local_edges().len(), 3);
    assert!(graph.local_edges().iter().any(|edge| {
        edge.consumer == "beta"
            && edge.provider == "alpha"
            && edge.kind == DependencyKind::Normal
            && edge.optional
    }));
    assert!(graph.local_edges().iter().any(|edge| {
        edge.consumer == "gamma"
            && edge.provider == "beta"
            && edge.kind == DependencyKind::Development
            && edge.target.as_deref() == Some("cfg(unix)")
    }));
    assert!(graph.packages()["gamma"].dependencies.iter().any(|dependency| {
        dependency.alias == "outside" && dependency.source == DependencySource::External
    }));

    let source_map = map([
        ("nodes/one", ArchitecturalRole::Foundation),
        ("nodes/two", ArchitecturalRole::KernelInterface),
        ("nodes/three", ArchitecturalRole::KernelCore),
    ]);
    let policy = PreflightRolePolicy::new()
        .allow(ArchitecturalRole::KernelInterface, ArchitecturalRole::Foundation)
        .allow(ArchitecturalRole::KernelCore, ArchitecturalRole::KernelInterface);
    let report = preflight_declared_graph(&graph, &source_map, &policy);
    assert!(report.is_clean(), "{:?}", report.issues);
}

#[test]
fn same_role_dependencies_are_automatically_admitted() {
    let root = TempRoot::new();
    root.write(
        "nodes/one/Cargo.toml",
        r#"[package]
name = "alpha"
version = "0.0.0"
[dependencies]
beta = { path = "../two" }
"#,
    );
    root.write("nodes/two/Cargo.toml", "[package]\nname = \"beta\"\nversion = \"0.0.0\"\n");

    let graph = load_declared_manifest_graph(root.path(), &DiscoveryOptions::default()).unwrap();
    let source_map = map([
        ("nodes/one", ArchitecturalRole::Foundation),
        ("nodes/two", ArchitecturalRole::Foundation),
    ]);
    let policy = PreflightRolePolicy::new();

    assert!(policy.allows(ArchitecturalRole::Foundation, ArchitecturalRole::Foundation));
    assert!(!policy.allows(ArchitecturalRole::Foundation, ArchitecturalRole::KernelInterface));
    assert!(preflight_declared_graph(&graph, &source_map, &policy).is_clean());
}

#[test]
fn preflight_classifies_package_directories_only() {
    let root = TempRoot::new();
    root.write("node/Cargo.toml", "[package]\nname = \"alpha\"\nversion = \"0.0.0\"\n");
    root.write("node/src/private.rs", "pub struct Private;\n");
    let source_map = SourceRoleMap::new(
        1,
        [
            SourceRoleRule::new("node", 1, production(ArchitecturalRole::Foundation)),
            SourceRoleRule::new("node/src", 2, production(ArchitecturalRole::KernelCore)),
            SourceRoleRule::new("node/src", 2, production(ArchitecturalRole::KernelRuntime)),
        ],
    )
    .unwrap();

    assert!(matches!(
        source_map.classify("node/src/private.rs"),
        Err(RoleMapError::Ambiguous { .. })
    ));
    let graph = load_declared_manifest_graph(root.path(), &DiscoveryOptions::default()).unwrap();
    let report = preflight_declared_graph(&graph, &source_map, &PreflightRolePolicy::new());
    assert!(report.is_clean());
    assert_eq!(report.package_roles["alpha"], production(ArchitecturalRole::Foundation));
}

#[test]
fn forbidden_reverse_edge_is_reported() {
    let root = TempRoot::new();
    root.write(
        "nodes/one/Cargo.toml",
        r#"[package]
name = "alpha"
version = "0.0.0"

[dependencies]
beta = { path = "../two" }
"#,
    );
    root.write("nodes/two/Cargo.toml", "[package]\nname = \"beta\"\nversion = \"0.0.0\"\n");

    let graph = load_declared_manifest_graph(root.path(), &DiscoveryOptions::default()).unwrap();
    let source_map = map([
        ("nodes/one", ArchitecturalRole::Foundation),
        ("nodes/two", ArchitecturalRole::KernelInterface),
    ]);
    let report = preflight_declared_graph(&graph, &source_map, &PreflightRolePolicy::new());
    assert!(report.issues.iter().any(|issue| matches!(
        issue,
        PreflightIssue::ForbiddenRoleDependency {
            consumer_role: ArchitecturalRole::Foundation,
            provider_role: ArchitecturalRole::KernelInterface,
            ..
        }
    )));
}

#[test]
fn concrete_and_projected_cycles_are_reported() {
    let root = TempRoot::new();
    root.write(
        "nodes/one/Cargo.toml",
        r#"[package]
name = "alpha"
version = "0.0.0"
[dependencies]
beta = { path = "../two" }
"#,
    );
    root.write(
        "nodes/two/Cargo.toml",
        r#"[package]
name = "beta"
version = "0.0.0"
[dependencies]
alpha = { path = "../one" }
"#,
    );

    let graph = load_declared_manifest_graph(root.path(), &DiscoveryOptions::default()).unwrap();
    let source_map = map([
        ("nodes/one", ArchitecturalRole::Foundation),
        ("nodes/two", ArchitecturalRole::KernelInterface),
    ]);
    let permissive_cycle = PreflightRolePolicy::new()
        .allow(ArchitecturalRole::Foundation, ArchitecturalRole::KernelInterface)
        .allow(ArchitecturalRole::KernelInterface, ArchitecturalRole::Foundation);
    let report = preflight_declared_graph(&graph, &source_map, &permissive_cycle);

    assert!(report.issues.iter().any(|issue| matches!(issue, PreflightIssue::PackageCycle { .. })));
    assert!(report.issues.iter().any(|issue| matches!(issue, PreflightIssue::RoleCycle { .. })));
}

#[test]
fn unresolved_and_mismatched_path_dependencies_fail_loading() {
    let unresolved = TempRoot::new();
    unresolved.write(
        "node/Cargo.toml",
        r#"[package]
name = "alpha"
version = "0.0.0"
[dependencies]
missing = { path = "../missing" }
"#,
    );
    assert!(matches!(
        load_declared_manifest_graph(unresolved.path(), &DiscoveryOptions::default()),
        Err(GraphError::UnresolvedPathDependency { .. })
    ));

    let mismatch = TempRoot::new();
    mismatch.write(
        "nodes/one/Cargo.toml",
        r#"[package]
name = "alpha"
version = "0.0.0"
[dependencies]
wrong = { path = "../two" }
"#,
    );
    mismatch.write("nodes/two/Cargo.toml", "[package]\nname = \"beta\"\nversion = \"0.0.0\"\n");
    assert!(matches!(
        load_declared_manifest_graph(mismatch.path(), &DiscoveryOptions::default()),
        Err(GraphError::PathPackageMismatch { .. })
    ));
}

#[test]
fn workspace_inheritance_fails_closed_until_resolved() {
    let root = TempRoot::new();
    root.write(
        "node/Cargo.toml",
        r#"[package]
name = "alpha"
version = "0.0.0"
[dependencies]
shared = { workspace = true }
"#,
    );

    assert!(matches!(
        load_declared_manifest_graph(root.path(), &DiscoveryOptions::default()),
        Err(GraphError::WorkspaceDependencyUnsupported { .. })
    ));
}

#[test]
fn cargo_resolution_indirection_sections_fail_closed() {
    for (sequence, section, expected) in [
        ("one", "[patch.crates-io]\nalpha = { path = \"node\" }\n", "patch.crates-io"),
        ("two", "[replace]\n\"alpha:0.0.0\" = { path = \"node\" }\n", "replace"),
        (
            "three",
            "[workspace.dependencies]\nalpha = { path = \"node\" }\n",
            "workspace.dependencies",
        ),
        (
            "four",
            "[workspace.dependencies.alpha]\npath = \"node\"\n",
            "workspace.dependencies.alpha",
        ),
        ("five", "patch = { crates-io = { alpha = { path = \"node\" } } }\n", "patch"),
        (
            "six",
            "[workspace]\ndependencies = { alpha = { path = \"node\" } }\n",
            "workspace.dependencies",
        ),
        (
            "seven",
            "\"patch\".\"crates-io\".\"alpha\" = { path = \"node\" }\n",
            "patch.crates-io.alpha",
        ),
        (
            "eight",
            "workspace = { dependencies = { alpha = { path = \"node\" } } }\n",
            "workspace.dependencies",
        ),
        (
            "nine",
            "[workspace]\n\"dependencies\".\"alpha\" = { path = \"node\" }\n",
            "workspace.dependencies.alpha",
        ),
    ] {
        let root = TempRoot::new();
        root.write("Cargo.toml", section);
        root.write("node/Cargo.toml", "[package]\nname = \"alpha\"\nversion = \"0.0.0\"\n");

        let error = load_declared_manifest_graph(root.path(), &DiscoveryOptions::default())
            .expect_err(sequence);
        assert!(matches!(
            error,
            GraphError::UnsupportedResolutionSection { section, .. } if section == expected
        ));
    }
}

#[test]
fn every_source_treatment_remains_distinct() {
    let treatments = [
        SourceTreatment::Production,
        SourceTreatment::Generated,
        SourceTreatment::Vendored,
        SourceTreatment::Test,
        SourceTreatment::Example,
        SourceTreatment::Tool,
        SourceTreatment::BuildOutput,
    ];
    assert_eq!(treatments.into_iter().collect::<std::collections::BTreeSet<_>>().len(), 7);
}
