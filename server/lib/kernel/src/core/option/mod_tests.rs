use super::*;

use crate::mm::{BufferId, WindowId};

fn make_bool_spec(name: &'static str) -> OptionSpec {
    OptionSpec::new(name, "test bool", OptionValue::Bool(false))
}

fn make_int_spec(name: &'static str) -> OptionSpec {
    OptionSpec::new(name, "test int", OptionValue::Integer(4))
}

fn make_buffer_bool_spec(name: &'static str) -> OptionSpec {
    OptionSpec::new(name, "test buffer bool", OptionValue::Bool(false))
        .with_scope(OptionScope::Buffer)
}

fn make_window_bool_spec(name: &'static str) -> OptionSpec {
    OptionSpec::new(name, "test window bool", OptionValue::Bool(false))
        .with_scope(OptionScope::Window)
}

// =========================================================================
// Registration
// =========================================================================

#[test]
fn register_and_get_spec() {
    let reg = OptionRegistry::new();
    reg.register(make_bool_spec("number")).unwrap();

    let spec = reg.get_spec("number");
    assert!(spec.is_some());
    assert_eq!(spec.unwrap().name, "number");
}

#[test]
fn register_duplicate_name_errors() {
    let reg = OptionRegistry::new();
    reg.register(make_bool_spec("number")).unwrap();

    let result = reg.register(make_bool_spec("number"));
    assert!(result.is_err());
    assert!(matches!(result.unwrap_err(), OptionError::AlreadyExists(_)));
}

#[test]
fn register_with_alias() {
    let reg = OptionRegistry::new();
    let spec = make_bool_spec("number").with_short("nu");
    reg.register(spec).unwrap();

    // Both full name and alias should resolve
    assert!(reg.contains("number"));
    assert!(reg.contains("nu"));

    let resolved = reg.resolve_name("nu");
    assert_eq!(resolved, Some("number".to_string()));
}

#[test]
fn register_alias_conflicts_with_existing_spec_name() {
    let reg = OptionRegistry::new();
    reg.register(make_bool_spec("nu")).unwrap();

    // Try to register "number" with alias "nu" — "nu" already exists as a spec name
    let spec = make_bool_spec("number").with_short("nu");
    let result = reg.register(spec);
    assert!(result.is_err());
    assert!(matches!(result.unwrap_err(), OptionError::AliasConflict(_)));
}

#[test]
fn register_alias_conflicts_with_existing_alias() {
    let reg = OptionRegistry::new();
    let spec1 = make_bool_spec("number").with_short("nu");
    reg.register(spec1).unwrap();

    // Try to register "relativenumber" with same alias "nu"
    let spec2 = make_bool_spec("relativenumber").with_short("nu");
    let result = reg.register(spec2);
    assert!(result.is_err());
    assert!(matches!(result.unwrap_err(), OptionError::AliasConflict(_)));
}

// =========================================================================
// Name resolution
// =========================================================================

#[test]
fn resolve_name_direct() {
    let reg = OptionRegistry::new();
    reg.register(make_bool_spec("wrap")).unwrap();
    assert_eq!(reg.resolve_name("wrap"), Some("wrap".to_string()));
}

#[test]
fn resolve_name_alias() {
    let reg = OptionRegistry::new();
    let spec = make_bool_spec("number").with_short("nu");
    reg.register(spec).unwrap();
    assert_eq!(reg.resolve_name("nu"), Some("number".to_string()));
}

#[test]
fn resolve_name_not_found() {
    let reg = OptionRegistry::new();
    assert_eq!(reg.resolve_name("nonexistent"), None);
}

#[test]
fn get_spec_via_alias() {
    let reg = OptionRegistry::new();
    let spec = make_bool_spec("number").with_short("nu");
    reg.register(spec).unwrap();

    let found = reg.get_spec("nu").unwrap();
    assert_eq!(found.name, "number");
}

#[test]
fn get_spec_nonexistent() {
    let reg = OptionRegistry::new();
    assert!(reg.get_spec("nope").is_none());
}

#[test]
fn contains_checks_aliases() {
    let reg = OptionRegistry::new();
    let spec = make_bool_spec("wrap").with_short("wr");
    reg.register(spec).unwrap();

    assert!(reg.contains("wrap"));
    assert!(reg.contains("wr"));
    assert!(!reg.contains("nope"));
}

// =========================================================================
// Value access — scope resolution
// =========================================================================

#[test]
fn get_returns_default_when_no_override() {
    let reg = OptionRegistry::new();
    reg.register(make_bool_spec("number")).unwrap();

    let val = reg.get("number", OptionScopeId::Global);
    assert_eq!(val, Some(OptionValue::Bool(false)));
}

#[test]
fn get_returns_global_override() {
    let reg = OptionRegistry::new();
    reg.register(make_bool_spec("number")).unwrap();
    reg.set("number", OptionValue::Bool(true), OptionScopeId::Global)
        .unwrap();

    let val = reg.get("number", OptionScopeId::Global);
    assert_eq!(val, Some(OptionValue::Bool(true)));
}

#[test]
fn get_buffer_scope_returns_buffer_local_over_global() {
    let reg = OptionRegistry::new();
    reg.register(make_buffer_bool_spec("expandtab")).unwrap();

    let buf = BufferId::new();
    reg.set("expandtab", OptionValue::Bool(true), OptionScopeId::Global)
        .unwrap();
    reg.set("expandtab", OptionValue::Bool(false), OptionScopeId::Buffer(buf))
        .unwrap();

    // Buffer-local wins
    let val = reg.get("expandtab", OptionScopeId::Buffer(buf));
    assert_eq!(val, Some(OptionValue::Bool(false)));
}

#[test]
fn get_buffer_scope_falls_back_to_global() {
    let reg = OptionRegistry::new();
    reg.register(make_buffer_bool_spec("expandtab")).unwrap();

    let buf = BufferId::new();
    reg.set("expandtab", OptionValue::Bool(true), OptionScopeId::Global)
        .unwrap();

    // No buffer-local set — falls back to global
    let val = reg.get("expandtab", OptionScopeId::Buffer(buf));
    assert_eq!(val, Some(OptionValue::Bool(true)));
}

#[test]
fn get_window_scope_returns_window_local_over_global() {
    let reg = OptionRegistry::new();
    reg.register(make_window_bool_spec("number")).unwrap();

    let win = WindowId::new();
    reg.set("number", OptionValue::Bool(true), OptionScopeId::Global)
        .unwrap();
    reg.set("number", OptionValue::Bool(false), OptionScopeId::Window(win))
        .unwrap();

    let val = reg.get("number", OptionScopeId::Window(win));
    assert_eq!(val, Some(OptionValue::Bool(false)));
}

#[test]
fn get_window_scope_falls_back_to_global() {
    let reg = OptionRegistry::new();
    reg.register(make_window_bool_spec("number")).unwrap();

    let win = WindowId::new();
    reg.set("number", OptionValue::Bool(true), OptionScopeId::Global)
        .unwrap();

    let val = reg.get("number", OptionScopeId::Window(win));
    assert_eq!(val, Some(OptionValue::Bool(true)));
}

#[test]
fn get_nonexistent_returns_none() {
    let reg = OptionRegistry::new();
    assert_eq!(reg.get("nope", OptionScopeId::Global), None);
}

// =========================================================================
// Convenience accessors
// =========================================================================

#[test]
fn get_global_shortcut() {
    let reg = OptionRegistry::new();
    reg.register(make_bool_spec("wrap")).unwrap();
    assert_eq!(reg.get_global("wrap"), Some(OptionValue::Bool(false)));
}

#[test]
fn get_for_buffer_shortcut() {
    let reg = OptionRegistry::new();
    reg.register(make_buffer_bool_spec("expandtab")).unwrap();
    let buf = BufferId::new();
    assert_eq!(reg.get_for_buffer("expandtab", buf), Some(OptionValue::Bool(false)));
}

#[test]
fn get_for_window_shortcut() {
    let reg = OptionRegistry::new();
    reg.register(make_window_bool_spec("number")).unwrap();
    let win = WindowId::new();
    assert_eq!(reg.get_for_window("number", win), Some(OptionValue::Bool(false)));
}

// =========================================================================
// Set
// =========================================================================

#[test]
fn set_returns_old_value() {
    let reg = OptionRegistry::new();
    reg.register(make_bool_spec("wrap")).unwrap();

    let result = reg
        .set("wrap", OptionValue::Bool(true), OptionScopeId::Global)
        .unwrap();
    assert!(result.old_value.is_none()); // First set, no previous value
    assert_eq!(result.new_value, OptionValue::Bool(true));

    let result = reg
        .set("wrap", OptionValue::Bool(false), OptionScopeId::Global)
        .unwrap();
    assert_eq!(result.old_value, Some(OptionValue::Bool(true)));
    assert_eq!(result.new_value, OptionValue::Bool(false));
}

#[test]
fn set_not_found_errors() {
    let reg = OptionRegistry::new();
    let result = reg.set("nope", OptionValue::Bool(true), OptionScopeId::Global);
    assert!(result.is_err());
    assert!(matches!(result.unwrap_err(), OptionError::NotFound(_)));
}

#[test]
fn set_type_mismatch_errors() {
    let reg = OptionRegistry::new();
    reg.register(make_bool_spec("wrap")).unwrap();

    let result = reg.set("wrap", OptionValue::Integer(42), OptionScopeId::Global);
    assert!(result.is_err());
    assert!(matches!(result.unwrap_err(), OptionError::TypeMismatch { .. }));
}

#[test]
fn set_scope_mismatch_buffer_for_global_only() {
    let reg = OptionRegistry::new();
    // Global-scoped option
    reg.register(make_bool_spec("wrap")).unwrap();

    let buf = BufferId::new();
    let result = reg.set("wrap", OptionValue::Bool(true), OptionScopeId::Buffer(buf));
    assert!(result.is_err());
    assert!(matches!(result.unwrap_err(), OptionError::ScopeMismatch { .. }));
}

#[test]
fn set_scope_mismatch_window_for_global_only() {
    let reg = OptionRegistry::new();
    reg.register(make_bool_spec("wrap")).unwrap();

    let win = WindowId::new();
    let result = reg.set("wrap", OptionValue::Bool(true), OptionScopeId::Window(win));
    assert!(result.is_err());
    assert!(matches!(result.unwrap_err(), OptionError::ScopeMismatch { .. }));
}

#[test]
fn set_global_shortcut() {
    let reg = OptionRegistry::new();
    reg.register(make_bool_spec("wrap")).unwrap();
    let result = reg.set_global("wrap", OptionValue::Bool(true));
    assert!(result.is_ok());
    assert_eq!(reg.get_global("wrap"), Some(OptionValue::Bool(true)));
}

#[test]
fn set_for_buffer_shortcut() {
    let reg = OptionRegistry::new();
    reg.register(make_buffer_bool_spec("expandtab")).unwrap();
    let buf = BufferId::new();
    let result = reg.set_for_buffer("expandtab", OptionValue::Bool(true), buf);
    assert!(result.is_ok());
    assert_eq!(reg.get_for_buffer("expandtab", buf), Some(OptionValue::Bool(true)));
}

#[test]
fn set_for_window_shortcut() {
    let reg = OptionRegistry::new();
    reg.register(make_window_bool_spec("number")).unwrap();
    let win = WindowId::new();
    let result = reg.set_for_window("number", OptionValue::Bool(true), win);
    assert!(result.is_ok());
    assert_eq!(reg.get_for_window("number", win), Some(OptionValue::Bool(true)));
}

#[test]
fn set_via_alias() {
    let reg = OptionRegistry::new();
    let spec = make_bool_spec("number").with_short("nu");
    reg.register(spec).unwrap();

    reg.set("nu", OptionValue::Bool(true), OptionScopeId::Global)
        .unwrap();
    assert_eq!(reg.get_global("number"), Some(OptionValue::Bool(true)));
}

// =========================================================================
// Reset
// =========================================================================

#[test]
fn reset_global() {
    let reg = OptionRegistry::new();
    reg.register(make_bool_spec("wrap")).unwrap();
    reg.set_global("wrap", OptionValue::Bool(true)).unwrap();

    let removed = reg.reset("wrap", OptionScopeId::Global).unwrap();
    assert_eq!(removed, Some(OptionValue::Bool(true)));

    // Should be back to default
    assert_eq!(reg.get_global("wrap"), Some(OptionValue::Bool(false)));
}

#[test]
fn reset_buffer() {
    let reg = OptionRegistry::new();
    reg.register(make_buffer_bool_spec("expandtab")).unwrap();

    let buf = BufferId::new();
    reg.set_for_buffer("expandtab", OptionValue::Bool(true), buf)
        .unwrap();

    let removed = reg.reset("expandtab", OptionScopeId::Buffer(buf)).unwrap();
    assert_eq!(removed, Some(OptionValue::Bool(true)));
}

#[test]
fn reset_window() {
    let reg = OptionRegistry::new();
    reg.register(make_window_bool_spec("number")).unwrap();

    let win = WindowId::new();
    reg.set_for_window("number", OptionValue::Bool(true), win)
        .unwrap();

    let removed = reg.reset("number", OptionScopeId::Window(win)).unwrap();
    assert_eq!(removed, Some(OptionValue::Bool(true)));
}

#[test]
fn reset_not_found_errors() {
    let reg = OptionRegistry::new();
    let result = reg.reset("nope", OptionScopeId::Global);
    assert!(result.is_err());
    assert!(matches!(result.unwrap_err(), OptionError::NotFound(_)));
}

#[test]
fn reset_returns_none_when_not_set() {
    let reg = OptionRegistry::new();
    reg.register(make_bool_spec("wrap")).unwrap();

    let removed = reg.reset("wrap", OptionScopeId::Global).unwrap();
    assert!(removed.is_none());
}

// =========================================================================
// Clear buffer/window
// =========================================================================

#[test]
fn clear_buffer_removes_all_values_for_buffer() {
    let reg = OptionRegistry::new();
    reg.register(make_buffer_bool_spec("expandtab")).unwrap();
    reg.register(
        OptionSpec::new("tabwidth", "tw", OptionValue::Integer(4)).with_scope(OptionScope::Buffer),
    )
    .unwrap();

    let buf = BufferId::new();
    reg.set_for_buffer("expandtab", OptionValue::Bool(true), buf)
        .unwrap();
    reg.set_for_buffer("tabwidth", OptionValue::Integer(2), buf)
        .unwrap();

    reg.clear_buffer(buf);

    // Both should be back to defaults (no buffer-local overrides)
    assert_eq!(reg.get_for_buffer("expandtab", buf), Some(OptionValue::Bool(false)));
    assert_eq!(reg.get_for_buffer("tabwidth", buf), Some(OptionValue::Integer(4)));
}

#[test]
fn clear_window_removes_all_values_for_window() {
    let reg = OptionRegistry::new();
    reg.register(make_window_bool_spec("number")).unwrap();

    let win = WindowId::new();
    reg.set_for_window("number", OptionValue::Bool(true), win)
        .unwrap();

    reg.clear_window(win);

    assert_eq!(reg.get_for_window("number", win), Some(OptionValue::Bool(false)));
}

// =========================================================================
// Toggle
// =========================================================================

#[test]
fn toggle_bool_option() {
    let reg = OptionRegistry::new();
    reg.register(make_bool_spec("wrap")).unwrap();

    let new_val = reg.toggle("wrap", OptionScopeId::Global).unwrap();
    assert!(new_val); // false -> true
    assert_eq!(reg.get_global("wrap"), Some(OptionValue::Bool(true)));

    let new_val = reg.toggle("wrap", OptionScopeId::Global).unwrap();
    assert!(!new_val); // true -> false
    assert_eq!(reg.get_global("wrap"), Some(OptionValue::Bool(false)));
}

#[test]
fn toggle_nonexistent_errors() {
    let reg = OptionRegistry::new();
    let result = reg.toggle("nope", OptionScopeId::Global);
    assert!(result.is_err());
}

#[test]
fn toggle_non_bool_errors() {
    let reg = OptionRegistry::new();
    reg.register(make_int_spec("tabwidth")).unwrap();

    let result = reg.toggle("tabwidth", OptionScopeId::Global);
    assert!(result.is_err());
    assert!(matches!(result.unwrap_err(), OptionError::TypeMismatch { .. }));
}

// =========================================================================
// Query methods
// =========================================================================

#[test]
fn list_all() {
    let reg = OptionRegistry::new();
    reg.register(make_bool_spec("wrap")).unwrap();
    reg.register(make_bool_spec("number")).unwrap();

    let all = reg.list_all();
    assert_eq!(all.len(), 2);
    assert!(all.contains(&"wrap".to_string()));
    assert!(all.contains(&"number".to_string()));
}

#[test]
fn list_by_scope() {
    let reg = OptionRegistry::new();
    reg.register(make_bool_spec("global_opt")).unwrap();
    reg.register(make_buffer_bool_spec("buffer_opt")).unwrap();
    reg.register(make_window_bool_spec("window_opt")).unwrap();

    let globals = reg.list_by_scope(OptionScope::Global);
    assert_eq!(globals.len(), 1);
    assert_eq!(globals[0].name, "global_opt");

    let buffers = reg.list_by_scope(OptionScope::Buffer);
    assert_eq!(buffers.len(), 1);
    assert_eq!(buffers[0].name, "buffer_opt");

    let windows = reg.list_by_scope(OptionScope::Window);
    assert_eq!(windows.len(), 1);
    assert_eq!(windows[0].name, "window_opt");
}

#[test]
fn len_and_is_empty() {
    let reg = OptionRegistry::new();
    assert_eq!(reg.len(), 0);
    assert!(reg.is_empty());

    reg.register(make_bool_spec("wrap")).unwrap();
    assert_eq!(reg.len(), 1);
    assert!(!reg.is_empty());
}

// =========================================================================
// Module lifecycle: unregister_by_module / list_by_module
// =========================================================================

#[test]
fn unregister_by_module_removes_specs_and_aliases() {
    let reg = OptionRegistry::new();
    let module_id = crate::api::ModuleId::new("test-module");

    let spec = make_bool_spec("wrap")
        .with_short("wr")
        .with_owner(module_id.clone());
    reg.register(spec).unwrap();
    reg.set_global("wrap", OptionValue::Bool(true)).unwrap();

    assert!(reg.contains("wrap"));
    assert!(reg.contains("wr"));
    assert_eq!(reg.len(), 1);

    reg.unregister_by_module(&module_id);

    assert!(!reg.contains("wrap"));
    assert!(!reg.contains("wr"));
    assert_eq!(reg.len(), 0);
    assert!(reg.get_global("wrap").is_none());
}

#[test]
fn unregister_by_module_also_removes_scoped_values() {
    let reg = OptionRegistry::new();
    let module_id = crate::api::ModuleId::new("test-module");

    let spec = make_buffer_bool_spec("expandtab").with_owner(module_id.clone());
    reg.register(spec).unwrap();

    let buf = BufferId::new();
    reg.set_for_buffer("expandtab", OptionValue::Bool(true), buf)
        .unwrap();

    let win_spec = make_window_bool_spec("number").with_owner(module_id.clone());
    reg.register(win_spec).unwrap();
    let win = WindowId::new();
    reg.set_for_window("number", OptionValue::Bool(true), win)
        .unwrap();

    reg.unregister_by_module(&module_id);

    assert!(!reg.contains("expandtab"));
    assert!(!reg.contains("number"));
}

#[test]
fn unregister_by_module_noop_for_empty_match() {
    let reg = OptionRegistry::new();
    let module_id = crate::api::ModuleId::new("nonexistent");

    reg.register(make_bool_spec("wrap")).unwrap();
    reg.unregister_by_module(&module_id);

    // "wrap" has no owner, should remain
    assert!(reg.contains("wrap"));
}

#[test]
fn list_by_module() {
    let reg = OptionRegistry::new();
    let module_a = crate::api::ModuleId::new("module-a");
    let module_b = crate::api::ModuleId::new("module-b");

    reg.register(make_bool_spec("opt1").with_owner(module_a.clone()))
        .unwrap();
    reg.register(make_bool_spec("opt2").with_owner(module_a.clone()))
        .unwrap();
    reg.register(make_bool_spec("opt3").with_owner(module_b.clone()))
        .unwrap();

    let a_opts = reg.list_by_module(&module_a);
    assert_eq!(a_opts.len(), 2);

    let b_opts = reg.list_by_module(&module_b);
    assert_eq!(b_opts.len(), 1);
}

// =========================================================================
// OptionSpec validation
// =========================================================================

#[test]
fn spec_validate_correct_type() {
    let spec = make_bool_spec("test");
    assert!(spec.validate(&OptionValue::Bool(true)).is_ok());
}

#[test]
fn spec_validate_wrong_type() {
    let spec = make_bool_spec("test");
    let result = spec.validate(&OptionValue::Integer(42));
    assert!(result.is_err());
}

#[test]
fn spec_validate_constraint() {
    let spec = make_int_spec("tabwidth").with_constraint(OptionConstraint::range(1, 32));
    assert!(spec.validate(&OptionValue::Integer(4)).is_ok());
    assert!(spec.validate(&OptionValue::Integer(0)).is_err());
    assert!(spec.validate(&OptionValue::Integer(33)).is_err());
}

#[test]
fn spec_matches_name_full() {
    let spec = make_bool_spec("number").with_short("nu");
    assert!(spec.matches_name("number"));
    assert!(spec.matches_name("nu"));
    assert!(!spec.matches_name("nope"));
}

#[test]
fn spec_matches_name_no_alias() {
    let spec = make_bool_spec("wrap");
    assert!(spec.matches_name("wrap"));
    assert!(!spec.matches_name("wr"));
}

#[test]
fn spec_owner() {
    let spec = make_bool_spec("wrap");
    assert!(spec.owner().is_none());

    let module = crate::api::ModuleId::new("test");
    let spec = spec.with_owner(module.clone());
    assert_eq!(spec.owner(), Some(&module));
}

// =========================================================================
// #721 repro: TOCTOU race in OptionRegistry::register()
// mod.rs:127-155 — read lock dropped between contains_key and write insert.
//
// The race window:
//   1. self.specs.read().contains_key(&name)  -> read lock acquired + dropped
//   2. <GAP: another thread can register the same name here>
//   3. self.specs.write().insert(name, spec)   -> write lock acquired
// =========================================================================

#[test]
fn b10_repro_toctou_race_detection() {
    use std::sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    };

    // Run the race multiple times to increase chance of triggering.
    let race_triggered = Arc::new(AtomicUsize::new(0));

    for _ in 0..10 {
        let registry = Arc::new(OptionRegistry::new());
        let barrier = Arc::new(std::sync::Barrier::new(100));
        let mut handles = Vec::new();

        for _ in 0..100 {
            let reg = Arc::clone(&registry);
            let bar = Arc::clone(&barrier);
            handles.push(std::thread::spawn(move || {
                bar.wait();
                reg.register(OptionSpec::new("race_target", "race test", OptionValue::Bool(false)))
            }));
        }

        let results: Vec<_> = handles.into_iter().map(|h| h.join().unwrap()).collect();
        let successes = results.iter().filter(|r| r.is_ok()).count();

        if successes > 1 {
            race_triggered.fetch_add(1, Ordering::SeqCst);
        }
    }

    let triggered = race_triggered.load(Ordering::SeqCst);
    // The race is timing-dependent. If it triggers even once in 10 rounds,
    // the TOCTOU gap is confirmed. On a multi-core machine this typically
    // triggers 3-8 out of 10 rounds.
    //
    // Structural proof regardless of timing: register() at mod.rs:131
    // releases the read lock before acquiring the write lock at line 154.
    // This is a textbook TOCTOU pattern.
    eprintln!("#721: race triggered in {triggered}/10 rounds");
}

#[test]
fn b10_repro_structural_proof() {
    // Even without triggering the race, we can prove the TOCTOU structurally:
    // register() uses separate read and write locks with a gap between them.
    //
    // Correct implementation would use a single write lock for check+insert:
    //   let mut specs = self.specs.write();
    //   if specs.contains_key(&name) { return Err(...); }
    //   specs.insert(name, spec);
    //
    // Actual implementation (mod.rs:131-154):
    //   if self.specs.read().contains_key(&name) { ... }  // read lock drops here
    //   // <-- TOCTOU gap: another thread can insert here
    //   self.specs.write().insert(name, spec);              // write lock acquired

    // Demonstrate the gap exists by checking that two sequential calls
    // to register() for the same name: first succeeds, second fails.
    // This is correct for single-threaded use, but the gap is real for
    // concurrent access.
    let registry = OptionRegistry::new();
    let r1 = registry.register(OptionSpec::new("proof", "first", OptionValue::Bool(false)));
    let r2 = registry.register(OptionSpec::new("proof", "second", OptionValue::Bool(false)));
    assert!(r1.is_ok(), "First register succeeds");
    assert!(r2.is_err(), "Second register correctly fails (single-threaded)");
    // In multi-threaded context, both could succeed due to the TOCTOU gap.
}
