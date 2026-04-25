//! Determinism test: consecutive serializations of the same
//! [`Lockfile`] produce byte-identical output.
//!
//! The writer sorts packages by `(name, version)` before emission
//! so the in-memory `Vec` order cannot influence the serialized
//! string.

use {
    reovim_pkg_lockfile::{Lockfile, PackageLock, Source},
    std::path::PathBuf,
};

const TYPICAL: &str = include_str!("fixtures/typical.lock");

#[test]
fn double_serialize_is_byte_identical() {
    let parsed = Lockfile::from_toml_str(TYPICAL).expect("parse");
    let first = parsed.to_toml_string().expect("first serialize");
    let second = parsed.to_toml_string().expect("second serialize");
    assert_eq!(first, second);
}

#[test]
fn shuffled_input_vec_serializes_identically() {
    // Two lockfiles with the same contents but different in-memory
    // package order must serialize to the same bytes (deterministic
    // output invariant).
    let pkg_a = PackageLock {
        name: "alpha".to_string(),
        version: "0.1.0".to_string(),
        source: Source::LocalPath(PathBuf::from("/a")),
        target: None,
        kind: None,
        sha256: None,
        trigger: None,
        dependencies: vec![],
    };
    let pkg_b = PackageLock {
        name: "beta".to_string(),
        version: "0.2.0".to_string(),
        source: Source::LocalPath(PathBuf::from("/b")),
        target: None,
        kind: None,
        sha256: None,
        trigger: None,
        dependencies: vec![],
    };

    let ascending = Lockfile {
        version: 1,
        packages: vec![pkg_a.clone(), pkg_b.clone()],
    };
    let descending = Lockfile {
        version: 1,
        packages: vec![pkg_b, pkg_a],
    };

    let a_out = ascending.to_toml_string().expect("ascending serialize");
    let d_out = descending.to_toml_string().expect("descending serialize");
    assert_eq!(a_out, d_out);
}

#[test]
fn versions_tie_break_by_version_string() {
    let pkg_v1 = PackageLock {
        name: "twin".to_string(),
        version: "1.0.0".to_string(),
        source: Source::LocalPath(PathBuf::from("/v1")),
        target: None,
        kind: None,
        sha256: None,
        trigger: None,
        dependencies: vec![],
    };
    let pkg_v2 = PackageLock {
        name: "twin".to_string(),
        version: "2.0.0".to_string(),
        source: Source::LocalPath(PathBuf::from("/v2")),
        target: None,
        kind: None,
        sha256: None,
        trigger: None,
        dependencies: vec![],
    };

    let lock = Lockfile {
        version: 1,
        packages: vec![pkg_v2, pkg_v1],
    };
    let out = lock.to_toml_string().expect("serialize");
    let v1_pos = out.find("\"1.0.0\"").expect("v1 in output");
    let v2_pos = out.find("\"2.0.0\"").expect("v2 in output");
    assert!(v1_pos < v2_pos, "v1 must precede v2:\n{out}");
}
