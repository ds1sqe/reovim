//! Round-trip tests: parse → serialize → parse produces an equal
//! [`Lockfile`].

use reovim_pkg_lockfile::Lockfile;

const MINIMAL: &str = include_str!("fixtures/minimal.lock");
const TYPICAL: &str = include_str!("fixtures/typical.lock");
const WITH_SHA: &str = include_str!("fixtures/with-sha.lock");

fn round_trip(src: &str) {
    let parsed = Lockfile::from_toml_str(src).expect("first parse");
    let reserialized = parsed.to_toml_string().expect("serialize");
    let reparsed = Lockfile::from_toml_str(&reserialized).expect("second parse");
    assert_eq!(parsed, reparsed, "round-trip mismatch:\n{reserialized}");
}

#[test]
fn minimal_round_trips() {
    round_trip(MINIMAL);
}

#[test]
fn typical_round_trips() {
    round_trip(TYPICAL);
}

#[test]
fn with_sha_round_trips() {
    round_trip(WITH_SHA);
}

#[test]
fn with_sha_exercises_target_and_sha_fields() {
    let parsed = Lockfile::from_toml_str(WITH_SHA).expect("parse");
    assert_eq!(parsed.version, 1);
    assert_eq!(parsed.packages.len(), 2);

    let alpha = parsed
        .packages
        .iter()
        .find(|p| p.name == "alpha")
        .expect("alpha");
    assert_eq!(alpha.target.as_deref(), Some("x86_64-unknown-linux-gnu"));
    assert!(alpha.sha256.is_some(), "alpha has sha256");
    assert!(alpha.dependencies.is_empty());

    let beta = parsed
        .packages
        .iter()
        .find(|p| p.name == "beta")
        .expect("beta");
    assert_eq!(beta.dependencies, vec!["alpha".to_string()]);
}

#[test]
fn writer_sorts_packages_by_name() {
    use {
        reovim_pkg_lockfile::{Lockfile, PackageLock, Source},
        std::path::PathBuf,
    };

    // Construct in reverse sort order; verify the writer emits in
    // sorted order.
    let lock = Lockfile {
        version: 1,
        packages: vec![
            PackageLock {
                name: "zeta".to_string(),
                version: "0.1.0".to_string(),
                source: Source::LocalPath(PathBuf::from("/z")),
                target: None,
                kind: None,
                sha256: None,
                dependencies: vec![],
            },
            PackageLock {
                name: "alpha".to_string(),
                version: "0.1.0".to_string(),
                source: Source::LocalPath(PathBuf::from("/a")),
                target: None,
                kind: None,
                sha256: None,
                dependencies: vec![],
            },
        ],
    };
    let out = lock.to_toml_string().expect("serialize");
    let alpha_pos = out.find("\"alpha\"").expect("alpha present");
    let zeta_pos = out.find("\"zeta\"").expect("zeta present");
    assert!(alpha_pos < zeta_pos, "alpha must precede zeta in sorted output:\n{out}");
}

#[test]
fn registry_source_round_trips() {
    use reovim_pkg_lockfile::{Lockfile, PackageLock, Source};

    let lock = Lockfile {
        version: 1,
        packages: vec![PackageLock {
            name: "from-registry".to_string(),
            version: "2.0.0".to_string(),
            source: Source::Registry {
                url: "https://registry.example.com/crates/from-registry/2.0.0".to_string(),
            },
            target: None,
            kind: None,
            sha256: None,
            dependencies: vec![],
        }],
    };
    let out = lock.to_toml_string().expect("serialize");
    let parsed = Lockfile::from_toml_str(&out).expect("parse registry source");
    assert_eq!(parsed, lock, "registry-source round-trip mismatch:\n{out}");
    assert!(out.contains("kind = \"registry\""), "kind tag present:\n{out}");
    assert!(out.contains("url ="), "url field present:\n{out}");
}
