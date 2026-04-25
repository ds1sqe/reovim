//! Tests for [`super::LazyRegistry`].

use std::path::PathBuf;

use {
    reovim_pkg_lockfile::{Lockfile, PackageLock, Source},
    reovim_pkg_manifest::{LazyTrigger, trigger_str},
};

use {super::LazyRegistry, crate::error::LazyError};

fn pkg(name: &str, trigger: Option<&str>) -> PackageLock {
    PackageLock {
        name: name.into(),
        version: "1.0.0".into(),
        source: Source::LocalPath(PathBuf::from(format!("/pkgs/{name}"))),
        target: None,
        kind: None,
        sha256: None,
        trigger: trigger.map(str::to_owned),
        dependencies: Vec::new(),
    }
}

fn lockfile(packages: Vec<PackageLock>) -> Lockfile {
    Lockfile {
        version: 1,
        packages,
    }
}

#[test]
fn registry_empty_treats_every_name_as_eager() {
    let reg = LazyRegistry::empty();
    assert_eq!(reg.entries().count(), 0);
    assert_eq!(reg.trigger_for("anything"), LazyTrigger::Eager);
    assert!(!reg.is_lazy("anything"));
}

#[test]
fn registry_from_lockfile_happy_path() {
    let lock = lockfile(vec![
        pkg("eager-one", Some("eager")),
        pkg("on-domain-one", Some("on-domain:text")),
        pkg("on-event-one", Some("on-event:save")),
        pkg("on-capability-one", Some("on-capability:render")),
    ]);
    let reg = LazyRegistry::from_lockfile(&lock).expect("build registry");
    assert_eq!(reg.trigger_for("eager-one"), LazyTrigger::Eager);
    assert_eq!(reg.trigger_for("on-domain-one"), LazyTrigger::OnDomain("text".into()),);
    assert_eq!(reg.trigger_for("on-event-one"), LazyTrigger::OnEvent("save".into()),);
    assert_eq!(reg.trigger_for("on-capability-one"), LazyTrigger::OnCapability("render".into()),);
    assert!(reg.is_lazy("on-domain-one"));
    assert!(!reg.is_lazy("eager-one"));
}

#[test]
fn registry_defaults_unknown_packages_to_eager() {
    let lock = lockfile(vec![pkg("known", Some("on-domain:text"))]);
    let reg = LazyRegistry::from_lockfile(&lock).expect("build registry");
    assert_eq!(reg.trigger_for("unknown"), LazyTrigger::Eager);
    assert!(!reg.is_lazy("unknown"));
}

#[test]
fn registry_defaults_missing_trigger_to_eager() {
    let lock = lockfile(vec![pkg("legacy", None)]);
    let reg = LazyRegistry::from_lockfile(&lock).expect("build registry");
    assert_eq!(reg.trigger_for("legacy"), LazyTrigger::Eager);
    assert!(!reg.is_lazy("legacy"));
}

#[test]
fn registry_rejects_malformed_trigger() {
    let lock = lockfile(vec![pkg("broken", Some("garbage"))]);
    let err = LazyRegistry::from_lockfile(&lock).expect_err("should reject");
    match err {
        LazyError::MalformedTrigger { pkg, value, .. } => {
            assert_eq!(pkg, "broken");
            assert_eq!(value, "garbage");
        }
    }
}

#[test]
fn registry_round_trip_through_trigger_str() {
    let triggers = [
        LazyTrigger::OnDomain("text".into()),
        LazyTrigger::OnEvent("save".into()),
        LazyTrigger::OnCapability("render".into()),
        LazyTrigger::Eager,
    ];
    let packages = triggers
        .iter()
        .enumerate()
        .map(|(idx, trigger)| pkg(&format!("p{idx}"), Some(&trigger_str(trigger))))
        .collect();
    let lock = lockfile(packages);
    let reg = LazyRegistry::from_lockfile(&lock).expect("build registry");
    for (idx, trigger) in triggers.iter().enumerate() {
        assert_eq!(reg.trigger_for(&format!("p{idx}")), trigger.clone());
    }
}
