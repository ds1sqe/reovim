use super::{BUILTIN_MODULE_COUNT, builtin_client_modules};

#[test]
fn factory_map_count() {
    let map = builtin_client_modules();
    assert_eq!(
        map.len(),
        BUILTIN_MODULE_COUNT,
        "factory map size must match BUILTIN_MODULE_COUNT"
    );
}

#[test]
fn factory_map_all_constructible() {
    let map = builtin_client_modules();
    for (kind, factory) in &map {
        let module = factory();
        assert_eq!(
            module.kind(),
            *kind,
            "factory for '{kind}' produced module with mismatched kind"
        );
    }
}

#[test]
fn factory_map_keys_match_kind() {
    let map = builtin_client_modules();
    let expected = [
        "statusline",
        "hover",
        "signature-help",
        "landing",
        "completion",
        "notification",
        "whichkey",
        "cmdline",
        "microscope",
        "explorer",
        "polyblocks",
        "line-numbers",
        "range-finder-fold",
        "range-finder-jump",
        "pair",
        "yank-flash",
        "diagnostics",
        "markdown",
    ];
    for kind in expected {
        assert!(map.contains_key(kind), "missing factory for '{kind}'");
    }
}
