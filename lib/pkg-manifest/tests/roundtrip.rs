//! Round-trip tests: parse → serialize → parse produces an equal
//! [`Manifest`].
//!
//! Three fixtures cover minimal, typical, and full-lazy cases. Every
//! [`Dependency`] and [`LazyTrigger`] enum variant is exercised.

use reovim_pkg_manifest::{LazyTrigger, Manifest, parse_trigger, trigger_str};

const MINIMAL: &str = include_str!("fixtures/minimal.toml");
const TYPICAL: &str = include_str!("fixtures/typical.toml");
const FULL_LAZY: &str = include_str!("fixtures/full-lazy.toml");

fn round_trip(src: &str) {
    let parsed = Manifest::from_toml_str(src).expect("first parse");
    let reserialized = parsed.to_toml_string().expect("serialize");
    let reparsed = Manifest::from_toml_str(&reserialized).expect("second parse");
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
fn full_lazy_round_trips() {
    round_trip(FULL_LAZY);
}

#[test]
fn typical_exercises_every_dependency_variant() {
    use reovim_pkg_manifest::Dependency;

    let parsed = Manifest::from_toml_str(TYPICAL).expect("parse");
    let vim_core = parsed.dependencies.get("vim-core").expect("vim-core");
    let vim_text = parsed.dependencies.get("vim-text").expect("vim-text");
    let my_theme = parsed.dependencies.get("my-theme").expect("my-theme");

    assert!(matches!(vim_core, Dependency::Version(v) if v == "1.0"));
    assert!(matches!(vim_text, Dependency::Detailed(d) if d.features == ["ripgrep"]));
    assert!(matches!(my_theme, Dependency::Detailed(d) if d.path.is_some()));
}

#[test]
fn kind_round_trips_for_driver_and_module() {
    use reovim_pkg_manifest::PackageKind;

    let typical = Manifest::from_toml_str(TYPICAL).expect("typical parse");
    assert_eq!(typical.package.kind, Some(PackageKind::Module));

    let full = Manifest::from_toml_str(FULL_LAZY).expect("full-lazy parse");
    assert_eq!(full.package.kind, Some(PackageKind::Driver));

    let minimal = Manifest::from_toml_str(MINIMAL).expect("minimal parse");
    assert_eq!(minimal.package.kind, None, "minimal fixture omits kind");
}

#[test]
fn full_lazy_exercises_every_trigger_variant() {
    use reovim_pkg_manifest::LazyTrigger;

    let parsed = Manifest::from_toml_str(FULL_LAZY).expect("parse");
    assert_eq!(parsed.lazy.get("core-kit"), Some(&LazyTrigger::Eager), "eager trigger");
    assert_eq!(
        parsed.lazy.get("git-tools"),
        Some(&LazyTrigger::OnEvent("buffer-opened".to_string())),
        "on-event trigger",
    );
    assert_eq!(
        parsed.lazy.get("theme-solar"),
        Some(&LazyTrigger::OnCapability("theme-provider".to_string())),
        "on-capability trigger",
    );

    let parsed_typical = Manifest::from_toml_str(TYPICAL).expect("typical parse");
    assert_eq!(
        parsed_typical.lazy.get("vim-text"),
        Some(&LazyTrigger::OnDomain("text".to_string())),
        "on-domain trigger",
    );
}

#[test]
fn trigger_str_round_trips_every_variant() {
    let cases = [
        LazyTrigger::OnDomain("text".to_string()),
        LazyTrigger::OnEvent("buffer-opened".to_string()),
        LazyTrigger::OnCapability("theme-provider".to_string()),
        LazyTrigger::Eager,
    ];
    for original in cases {
        let encoded = trigger_str(&original);
        let decoded = parse_trigger(&encoded).expect("parse");
        assert_eq!(original, decoded, "round-trip failed for `{encoded}`");
    }
}
