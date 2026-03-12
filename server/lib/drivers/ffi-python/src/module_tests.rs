use super::*;

#[test]
fn test_py_module_context_from_kernel() {
    let kernel_ctx = ModuleContext::default();
    let py_ctx = PyModuleContext::from_kernel(&kernel_ctx);

    assert!(py_ctx.data_dir.contains("reovim-test"));
    assert!(py_ctx.cache_dir.contains("reovim-test"));
    assert!(py_ctx.optional_deps().is_empty());
}

#[test]
fn test_py_module_context_has_optional_dep() {
    let py_ctx = PyModuleContext {
        data_dir: "/tmp/data".to_string(),
        cache_dir: "/tmp/cache".to_string(),
        optional_deps: vec!["lsp".to_string(), "treesitter".to_string()],
    };

    assert!(py_ctx.has_optional_dep("lsp"));
    assert!(py_ctx.has_optional_dep("treesitter"));
    assert!(!py_ctx.has_optional_dep("unknown"));
}

#[test]
fn test_py_module_context_repr() {
    let py_ctx = PyModuleContext {
        data_dir: "/data".to_string(),
        cache_dir: "/cache".to_string(),
        optional_deps: vec![],
    };

    let repr = py_ctx.__repr__();
    assert!(repr.contains("/data"));
    assert!(repr.contains("/cache"));
}

#[test]
fn test_py_module_context_clone() {
    let py_ctx = PyModuleContext {
        data_dir: "/data".to_string(),
        cache_dir: "/cache".to_string(),
        optional_deps: vec!["dep1".to_string()],
    };
    #[allow(clippy::redundant_clone)]
    let cloned = py_ctx.clone();
    assert_eq!(cloned.data_dir, "/data");
    assert_eq!(cloned.cache_dir, "/cache");
    assert_eq!(cloned.optional_deps(), vec!["dep1".to_string()]);
}

#[test]
fn test_py_module_context_optional_deps_empty() {
    let py_ctx = PyModuleContext {
        data_dir: String::new(),
        cache_dir: String::new(),
        optional_deps: vec![],
    };
    assert!(py_ctx.optional_deps().is_empty());
    assert!(!py_ctx.has_optional_dep("anything"));
}

#[test]
fn test_py_module_context_multiple_optional_deps() {
    let py_ctx = PyModuleContext {
        data_dir: String::new(),
        cache_dir: String::new(),
        optional_deps: vec![
            "lsp".to_string(),
            "treesitter".to_string(),
            "git".to_string(),
        ],
    };
    assert_eq!(py_ctx.optional_deps().len(), 3);
    assert!(py_ctx.has_optional_dep("lsp"));
    assert!(py_ctx.has_optional_dep("treesitter"));
    assert!(py_ctx.has_optional_dep("git"));
    assert!(!py_ctx.has_optional_dep("debug"));
}

// Integration tests with actual Python require the GIL and are tested
// via integration tests in server/lib/server/tests/
