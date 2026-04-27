//! Green-path integration tests: 5 fixtures should [`resolve`]
//! cleanly and produce the documented package list.

use std::path::{Path, PathBuf};

use {
    reovim_pkg_resolver::{Resolved, resolve},
    semver::Version,
};

fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(name)
}

const fn runtime() -> Version {
    Version::new(0, 15, 0)
}

fn names_versions(resolved: &Resolved) -> Vec<(&str, String)> {
    resolved
        .packages
        .iter()
        .map(|p| (p.name.as_str(), p.version.to_string()))
        .collect()
}

fn deps_of<'a>(resolved: &'a Resolved, name: &str) -> &'a [String] {
    resolved
        .packages
        .iter()
        .find(|p| p.name == name)
        .unwrap_or_else(|| panic!("missing package {name}"))
        .dependencies
        .as_slice()
}

#[test]
fn fixture_01_single_dep_resolves() {
    let resolved = resolve(&fixture("01-single-dep"), &runtime()).expect("ok");
    assert_eq!(names_versions(&resolved), vec![("foo", "1.0.0".into())]);
    assert!(deps_of(&resolved, "foo").is_empty());
}

#[test]
fn fixture_02_transitive_chain_resolves() {
    let resolved = resolve(&fixture("02-transitive-chain"), &runtime()).expect("ok");
    assert_eq!(
        names_versions(&resolved),
        vec![
            ("a", "1.0.0".into()),
            ("b", "2.0.0".into()),
            ("c", "3.0.0".into()),
        ],
    );
    assert_eq!(deps_of(&resolved, "a"), &["b".to_string()]);
    assert_eq!(deps_of(&resolved, "b"), &["c".to_string()]);
    assert!(deps_of(&resolved, "c").is_empty());
}

#[test]
fn fixture_03_diamond_resolves() {
    let resolved = resolve(&fixture("03-diamond"), &runtime()).expect("ok");
    assert_eq!(
        names_versions(&resolved),
        vec![
            ("a", "1.0.0".into()),
            ("b", "1.0.0".into()),
            ("leaf", "1.2.3".into()),
        ],
    );
    assert_eq!(deps_of(&resolved, "a"), &["leaf".to_string()]);
    assert_eq!(deps_of(&resolved, "b"), &["leaf".to_string()]);
    assert!(deps_of(&resolved, "leaf").is_empty());
}

#[test]
fn fixture_04_multi_feature_resolves() {
    let resolved = resolve(&fixture("04-multi-feature"), &runtime()).expect("ok");
    assert_eq!(names_versions(&resolved), vec![("tool", "1.4.2".into())]);
}

#[test]
fn fixture_05_eager_lazy_mix_resolves() {
    let resolved = resolve(&fixture("05-eager-lazy-mix"), &runtime()).expect("ok");
    assert_eq!(
        names_versions(&resolved),
        vec![("eager-dep", "0.1.0".into()), ("lazy-dep", "0.1.0".into()),],
    );
}

#[test]
fn resolve_propagates_lazy_triggers() {
    use reovim_pkg_manifest::LazyTrigger;
    let resolved = resolve(&fixture("05-eager-lazy-mix"), &runtime()).expect("ok");
    let lazy: std::collections::BTreeMap<_, _> = resolved
        .lazy
        .iter()
        .map(|(n, t)| (n.as_str(), t.clone()))
        .collect();
    assert_eq!(lazy.get("eager-dep"), Some(&LazyTrigger::Eager));
    assert_eq!(lazy.get("lazy-dep"), Some(&LazyTrigger::OnDomain("text".into())),);
}

#[test]
fn into_lockfile_writes_triggers_for_resolved_fixture() {
    let lock = resolve(&fixture("05-eager-lazy-mix"), &runtime())
        .expect("ok")
        .into_lockfile();
    let by_name: std::collections::BTreeMap<_, _> =
        lock.packages.iter().map(|p| (p.name.as_str(), p)).collect();
    assert_eq!(by_name["eager-dep"].trigger.as_deref(), Some("eager"));
    assert_eq!(by_name["lazy-dep"].trigger.as_deref(), Some("on-domain:text"),);
}
