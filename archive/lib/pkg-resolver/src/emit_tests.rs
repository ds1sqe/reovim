//! Unit tests for the [`emit`] module.

use std::path::PathBuf;

use {reovim_pkg_lockfile::Source, reovim_pkg_manifest::LazyTrigger, semver::Version};

use crate::graph::{Resolved, ResolvedPackage};

fn sample() -> Resolved {
    Resolved {
        packages: vec![
            ResolvedPackage {
                name: "a".into(),
                version: Version::new(1, 0, 0),
                source: Source::LocalPath(PathBuf::from("/pkgs/a")),
                dependencies: vec!["leaf".into()],
            },
            ResolvedPackage {
                name: "leaf".into(),
                version: Version::new(0, 1, 0),
                source: Source::LocalPath(PathBuf::from("/pkgs/leaf")),
                dependencies: vec![],
            },
        ],
        lazy: Vec::new(),
    }
}

#[test]
fn into_lockfile_preserves_every_package_field() {
    let lock = sample().into_lockfile();
    assert_eq!(lock.version, 1);
    assert_eq!(lock.packages.len(), 2);
    assert!(lock.packages.iter().all(|p| p.target.is_none()));
    assert!(lock.packages.iter().all(|p| p.sha256.is_none()));
    assert!(lock.packages.iter().all(|p| p.trigger.is_none()));
    assert_eq!(lock.packages[0].name, "a");
    assert_eq!(lock.packages[0].version, "1.0.0");
    assert_eq!(lock.packages[0].dependencies, vec!["leaf"]);
    assert_eq!(lock.packages[1].name, "leaf");
    assert_eq!(lock.packages[1].version, "0.1.0");
}

#[test]
fn into_lockfile_propagates_lazy_triggers() {
    let mut resolved = sample();
    resolved.lazy = vec![
        ("a".into(), LazyTrigger::OnDomain("text".into())),
        ("leaf".into(), LazyTrigger::Eager),
    ];
    let lock = resolved.into_lockfile();
    assert_eq!(lock.packages[0].trigger.as_deref(), Some("on-domain:text"));
    assert_eq!(lock.packages[1].trigger.as_deref(), Some("eager"));
}

#[test]
fn into_lockfile_ignores_lazy_entries_for_unknown_packages() {
    let mut resolved = sample();
    resolved.lazy = vec![("ghost".into(), LazyTrigger::Eager)];
    let lock = resolved.into_lockfile();
    assert!(lock.packages.iter().all(|p| p.trigger.is_none()));
}
