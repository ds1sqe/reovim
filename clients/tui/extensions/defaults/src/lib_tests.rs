use super::*;

#[test]
fn test_create_native_modules_count() {
    let modules = create_native_modules();
    // statusline, hover, signature-help, landing, completion, notification, whichkey,
    // cmdline, microscope, explorer, polyblocks, line-numbers, fold, jump, pair,
    // diagnostics, markdown = 17
    assert_eq!(modules.len(), 17);
}

#[test]
fn test_native_module_kinds() {
    let modules = create_native_modules();
    let kinds: Vec<&str> = modules.iter().map(|m| m.kind()).collect();
    assert!(kinds.contains(&"statusline"));
    assert!(kinds.contains(&"hover"));
    assert!(kinds.contains(&"signature-help"));
    assert!(kinds.contains(&"landing"));
    assert!(kinds.contains(&"completion"));
    assert!(kinds.contains(&"notification"));
    assert!(kinds.contains(&"whichkey"));
    assert!(kinds.contains(&"cmdline"));
    assert!(kinds.contains(&"microscope"));
    assert!(kinds.contains(&"explorer"));
    assert!(kinds.contains(&"polyblocks"));
    assert!(kinds.contains(&"line-numbers"));
    assert!(kinds.contains(&"range-finder-fold"));
    assert!(kinds.contains(&"range-finder-jump"));
    assert!(kinds.contains(&"pair"));
    assert!(kinds.contains(&"diagnostics"));
    assert!(kinds.contains(&"markdown"));
}

#[test]
fn test_native_modules_unique_kinds() {
    let modules = create_native_modules();
    let kinds: Vec<&str> = modules.iter().map(|m| m.kind()).collect();
    let unique: std::collections::HashSet<&str> = kinds.iter().copied().collect();
    assert_eq!(kinds.len(), unique.len(), "All module kinds should be unique");
}
