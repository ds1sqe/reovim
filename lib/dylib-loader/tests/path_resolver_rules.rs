//! Six-rule table-driven tests for `PathResolverBuilder`.
//!
//! Each of R1..R6 is exercised with one **present** row (rule
//! contributes its expected entry) and one **absent** row (rule
//! contributes nothing). A separate cluster of tests pins the merge
//! semantics, the replace-guard, empty-env handling, and the
//! colon-parsing edge case requested in the telemetry countdown notes.

use {
    reovim_dylib_loader::{EnvProvider, Kind, PathResolverBuilder, split_paths},
    std::{
        collections::HashMap,
        path::{Path, PathBuf},
        sync::Mutex,
    },
};

/// Test `EnvProvider` backed by an in-memory map — no ambient env.
struct MockEnv {
    map: Mutex<HashMap<String, String>>,
}

impl MockEnv {
    fn new() -> Self {
        Self {
            map: Mutex::new(HashMap::new()),
        }
    }

    fn set(self, key: &str, value: &str) -> Self {
        self.map.lock().unwrap().insert(key.to_owned(), value.to_owned());
        self
    }
}

impl EnvProvider for MockEnv {
    fn get(&self, key: &str) -> Option<String> {
        self.map.lock().unwrap().get(key).cloned()
    }
}

fn contains_path(paths: &[PathBuf], needle: &Path) -> bool {
    paths.iter().any(|p| p == needle)
}

// ============================================================
// R1 — CLI --driver (driver kind)
// ============================================================

#[test]
fn r1_cli_driver_present_appears_in_search_list() {
    let resolver = PathResolverBuilder::for_kind(Kind::Driver)
        .with_env(MockEnv::new())
        .push_cli_path("/tmp/cli-driver")
        .build();
    assert!(contains_path(resolver.paths(), Path::new("/tmp/cli-driver")));
    assert_eq!(resolver.paths().first(), Some(&PathBuf::from("/tmp/cli-driver")));
}

#[test]
fn r1_cli_driver_absent_contributes_nothing() {
    let resolver = PathResolverBuilder::for_kind(Kind::Driver)
        .with_env(MockEnv::new())
        .without_system_fallback()
        .build();
    assert!(!contains_path(resolver.paths(), Path::new("/tmp/cli-driver")));
}

// ============================================================
// R2 — CLI --module (module kind)
// ============================================================

#[test]
fn r2_cli_module_present_appears_in_search_list() {
    let resolver = PathResolverBuilder::for_kind(Kind::Module)
        .with_env(MockEnv::new())
        .push_cli_path("/tmp/cli-module")
        .build();
    assert_eq!(resolver.paths().first(), Some(&PathBuf::from("/tmp/cli-module")));
}

#[test]
fn r2_cli_module_absent_contributes_nothing() {
    let resolver = PathResolverBuilder::for_kind(Kind::Module)
        .with_env(MockEnv::new())
        .without_system_fallback()
        .build();
    assert!(!contains_path(resolver.paths(), Path::new("/tmp/cli-module")));
}

// ============================================================
// R3 — REOVIM_DRIVER_PATH (driver kind)
// ============================================================

#[test]
fn r3_driver_env_present_contributes_entries() {
    let env = MockEnv::new().set("REOVIM_DRIVER_PATH", "/env/a:/env/b");
    let resolver = PathResolverBuilder::for_kind(Kind::Driver)
        .with_env(env)
        .without_system_fallback()
        .build();
    assert!(contains_path(resolver.paths(), Path::new("/env/a")));
    assert!(contains_path(resolver.paths(), Path::new("/env/b")));
}

#[test]
fn r3_driver_env_absent_contributes_nothing() {
    let resolver = PathResolverBuilder::for_kind(Kind::Driver)
        .with_env(MockEnv::new())
        .without_system_fallback()
        .build();
    assert!(resolver.paths().is_empty());
}

// ============================================================
// R4 — REOVIM_MODULE_PATH (module kind)
// ============================================================

#[test]
fn r4_module_env_present_contributes_entries() {
    let env = MockEnv::new().set("REOVIM_MODULE_PATH", "/mod/a:/mod/b");
    let resolver = PathResolverBuilder::for_kind(Kind::Module)
        .with_env(env)
        .without_system_fallback()
        .build();
    assert!(contains_path(resolver.paths(), Path::new("/mod/a")));
    assert!(contains_path(resolver.paths(), Path::new("/mod/b")));
}

#[test]
fn r4_module_env_absent_contributes_nothing() {
    let resolver = PathResolverBuilder::for_kind(Kind::Module)
        .with_env(MockEnv::new())
        .without_system_fallback()
        .build();
    assert!(resolver.paths().is_empty());
}

// ============================================================
// R5 — REOVIM_LIBRARY_ROOT/<kind>/ (both kinds)
// ============================================================

#[test]
fn r5_library_root_appends_kind_subdir_for_driver() {
    let env = MockEnv::new().set("REOVIM_LIBRARY_ROOT", "/root");
    let resolver = PathResolverBuilder::for_kind(Kind::Driver)
        .with_env(env)
        .without_system_fallback()
        .build();
    assert!(contains_path(resolver.paths(), Path::new("/root/driver")));
}

#[test]
fn r5_library_root_appends_kind_subdir_for_module() {
    let env = MockEnv::new().set("REOVIM_LIBRARY_ROOT", "/root");
    let resolver = PathResolverBuilder::for_kind(Kind::Module)
        .with_env(env)
        .without_system_fallback()
        .build();
    assert!(contains_path(resolver.paths(), Path::new("/root/modules")));
}

#[test]
fn r5_library_root_absent_contributes_nothing() {
    let resolver = PathResolverBuilder::for_kind(Kind::Driver)
        .with_env(MockEnv::new())
        .without_system_fallback()
        .build();
    assert!(!contains_path(resolver.paths(), Path::new("/root/driver")));
}

// ============================================================
// R6 — XDG + system fallback
// ============================================================

#[test]
fn r6_xdg_fallback_honors_xdg_data_home_for_driver() {
    let env = MockEnv::new().set("XDG_DATA_HOME", "/custom-xdg");
    let resolver = PathResolverBuilder::for_kind(Kind::Driver).with_env(env).build();
    assert!(contains_path(resolver.paths(), Path::new("/custom-xdg/reovim/driver")));
}

#[test]
fn r6_xdg_fallback_honors_home_when_xdg_absent() {
    let env = MockEnv::new().set("HOME", "/home/user");
    let resolver = PathResolverBuilder::for_kind(Kind::Module).with_env(env).build();
    assert!(contains_path(
        resolver.paths(),
        Path::new("/home/user/.local/share/reovim/modules"),
    ));
}

#[test]
fn r6_system_fallback_always_present_for_both_kinds() {
    let driver = PathResolverBuilder::for_kind(Kind::Driver)
        .with_env(MockEnv::new())
        .build();
    assert!(contains_path(driver.paths(), Path::new("/usr/local/lib/reovim/driver")));
    assert!(contains_path(driver.paths(), Path::new("/usr/lib/reovim/driver")));

    let module = PathResolverBuilder::for_kind(Kind::Module)
        .with_env(MockEnv::new())
        .build();
    assert!(contains_path(module.paths(), Path::new("/usr/local/lib/reovim/modules")));
    assert!(contains_path(module.paths(), Path::new("/usr/lib/reovim/modules")));
}

#[test]
fn r6_system_fallback_suppressed_when_disabled() {
    let resolver = PathResolverBuilder::for_kind(Kind::Driver)
        .with_env(MockEnv::new())
        .without_system_fallback()
        .build();
    assert!(!contains_path(resolver.paths(), Path::new("/usr/lib/reovim/driver")));
}

// ============================================================
// Merge semantics — all rules simultaneously populated
// ============================================================

#[test]
fn all_six_rules_populated_produce_expected_precedence_order() {
    let env = MockEnv::new()
        .set("REOVIM_DRIVER_PATH", "/env3/a:/env3/b")
        .set("REOVIM_LIBRARY_ROOT", "/lib-root")
        .set("XDG_DATA_HOME", "/xdg");
    let resolver = PathResolverBuilder::for_kind(Kind::Driver)
        .with_env(env)
        .push_cli_path("/cli/x")
        .push_cli_path("/cli/y")
        .build();

    let paths = resolver.paths();
    // Expected order: R1 (cli) → R3 (env) → R5 (library root) → R6 (xdg then system).
    let expected = [
        PathBuf::from("/cli/x"),
        PathBuf::from("/cli/y"),
        PathBuf::from("/env3/a"),
        PathBuf::from("/env3/b"),
        PathBuf::from("/lib-root/driver"),
        PathBuf::from("/xdg/reovim/driver"),
        PathBuf::from("/usr/local/lib/reovim/driver"),
        PathBuf::from("/usr/lib/reovim/driver"),
    ];
    assert_eq!(paths, &expected);
}

// ============================================================
// Replace-guard — library root does NOT evict system defaults
// ============================================================

#[test]
fn r5_set_does_not_evict_r6_system_defaults() {
    let env = MockEnv::new().set("REOVIM_LIBRARY_ROOT", "/root");
    let resolver = PathResolverBuilder::for_kind(Kind::Driver).with_env(env).build();
    assert!(contains_path(resolver.paths(), Path::new("/root/driver")));
    assert!(contains_path(resolver.paths(), Path::new("/usr/local/lib/reovim/driver")));
    assert!(contains_path(resolver.paths(), Path::new("/usr/lib/reovim/driver")));
}

// ============================================================
// Empty-env handling — "" vs unset; colon-parsing edge cases
// ============================================================

#[test]
fn empty_env_var_yields_zero_entries_not_one_empty_pathbuf() {
    let env = MockEnv::new().set("REOVIM_DRIVER_PATH", "");
    let resolver = PathResolverBuilder::for_kind(Kind::Driver)
        .with_env(env)
        .without_system_fallback()
        .build();
    assert!(resolver.paths().is_empty());
}

#[test]
fn colon_parsing_filters_empty_segments_in_a_colon_colon_b() {
    // Telemetry countdown note: confirm `a::b` yields `[a, b]` not
    // `[a, "", b]`.
    let parsed = split_paths("a::b");
    assert_eq!(parsed, vec![PathBuf::from("a"), PathBuf::from("b")]);
}

#[test]
fn colon_parsing_filters_leading_and_trailing_empties() {
    let parsed = split_paths(":a:b:");
    assert_eq!(parsed, vec![PathBuf::from("a"), PathBuf::from("b")]);
}

// ============================================================
// Dedup — same path in multiple rules appears once
// ============================================================

#[test]
fn duplicate_path_across_rules_appears_once_at_highest_precedence() {
    let env = MockEnv::new().set("REOVIM_DRIVER_PATH", "/shared");
    let resolver = PathResolverBuilder::for_kind(Kind::Driver)
        .with_env(env)
        .push_cli_path("/shared")
        .without_system_fallback()
        .build();
    let occurrences = resolver.paths().iter().filter(|p| p.as_path() == Path::new("/shared")).count();
    assert_eq!(occurrences, 1);
    assert_eq!(resolver.paths().first(), Some(&PathBuf::from("/shared")));
}

// ============================================================
// Replace escape hatch (private API) — CLI paths become the only set
// ============================================================

#[test]
fn replace_mode_discards_lower_precedence_rules() {
    let env = MockEnv::new().set("REOVIM_DRIVER_PATH", "/should-be-ignored");
    let resolver = PathResolverBuilder::for_kind(Kind::Driver)
        .with_env(env)
        .push_cli_path("/only")
        .replace(true)
        .build();
    assert_eq!(resolver.paths(), &[PathBuf::from("/only")]);
}
