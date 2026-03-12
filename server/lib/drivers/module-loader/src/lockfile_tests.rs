use super::lockfile::*;

// ============================================================================
// sha256_file
// ============================================================================

#[test]
fn sha256_file_known_content() {
    let dir = std::env::temp_dir().join(format!(
        "reovim-sha256-test-{}",
        std::process::id()
    ));
    std::fs::create_dir_all(&dir).unwrap();

    let file = dir.join("test.bin");
    std::fs::write(&file, b"hello world").unwrap();

    let hash = sha256_file(&file).unwrap();
    // SHA-256 of "hello world" is well-known
    assert_eq!(
        hash,
        "b94d27b9934d3e08a52e52d7da7dabfac484efe37a5380ee9088f7ace2efcde9"
    );

    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn sha256_file_empty() {
    let dir = std::env::temp_dir().join(format!(
        "reovim-sha256-empty-{}",
        std::process::id()
    ));
    std::fs::create_dir_all(&dir).unwrap();

    let file = dir.join("empty.bin");
    std::fs::write(&file, b"").unwrap();

    let hash = sha256_file(&file).unwrap();
    // SHA-256 of empty input
    assert_eq!(
        hash,
        "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
    );

    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn sha256_file_not_found() {
    let result = sha256_file(std::path::Path::new("/nonexistent/file.so"));
    assert!(result.is_err());
}

// ============================================================================
// ModuleSource serde
// ============================================================================

#[test]
fn module_source_serde_roundtrip() {
    // Test via a wrapper struct since TOML doesn't support bare values
    #[derive(serde::Serialize, serde::Deserialize)]
    struct Wrapper {
        source: ModuleSource,
    }

    let builtin = Wrapper {
        source: ModuleSource::Builtin,
    };
    let toml_str = toml::to_string(&builtin).unwrap();
    assert!(toml_str.contains("builtin"));
    let back: Wrapper = toml::from_str(&toml_str).unwrap();
    assert_eq!(back.source, ModuleSource::Builtin);

    let external = Wrapper {
        source: ModuleSource::External,
    };
    let toml_str = toml::to_string(&external).unwrap();
    assert!(toml_str.contains("external"));
    let back: Wrapper = toml::from_str(&toml_str).unwrap();
    assert_eq!(back.source, ModuleSource::External);
}

// ============================================================================
// LockEntry / LockMeta
// ============================================================================

#[test]
fn lock_entry_builtin_skips_optional_fields() {
    let entry = LockEntry {
        id: "vim".into(),
        version: "0.10.0".into(),
        source: ModuleSource::Builtin,
        path: None,
        sha256: None,
        api_version: None,
        dependencies: Vec::new(),
        optional_dependencies: Vec::new(),
    };

    let toml_str = toml::to_string(&entry).unwrap();
    assert!(!toml_str.contains("path"));
    assert!(!toml_str.contains("sha256"));
    assert!(!toml_str.contains("api_version"));
    assert!(!toml_str.contains("dependencies"));
}

#[test]
fn lock_entry_external_includes_fields() {
    let entry = LockEntry {
        id: "treesitter-go".into(),
        version: "1.0.0".into(),
        source: ModuleSource::External,
        path: Some("/usr/lib/reovim/modules/libreovim_module_treesitter_go.so".into()),
        sha256: Some("abcdef".into()),
        api_version: Some("0.2.0".into()),
        dependencies: vec!["vim".into()],
        optional_dependencies: Vec::new(),
    };

    let toml_str = toml::to_string(&entry).unwrap();
    assert!(toml_str.contains("path"));
    assert!(toml_str.contains("sha256"));
    assert!(toml_str.contains("treesitter-go"));
}

// ============================================================================
// ModulesLock save/load roundtrip
// ============================================================================

#[test]
fn lock_roundtrip() {
    let lock = ModulesLock {
        meta: LockMeta {
            reovim_version: "0.10.0".into(),
            api_version: "0.2.0".into(),
            generated: "2026-03-12T10:00:00Z".into(),
        },
        modules: vec![
            LockEntry {
                id: "vim".into(),
                version: "0.10.0".into(),
                source: ModuleSource::Builtin,
                path: None,
                sha256: None,
                api_version: None,
                dependencies: Vec::new(),
                optional_dependencies: Vec::new(),
            },
            LockEntry {
                id: "treesitter-go".into(),
                version: "1.0.0".into(),
                source: ModuleSource::External,
                path: Some("/usr/lib/libreovim_module_treesitter_go.so".into()),
                sha256: Some("abc123".into()),
                api_version: Some("0.2.0".into()),
                dependencies: vec!["vim".into()],
                optional_dependencies: vec!["lsp".into()],
            },
        ],
    };

    let dir = std::env::temp_dir().join(format!(
        "reovim-lock-roundtrip-{}",
        std::process::id()
    ));
    std::fs::create_dir_all(&dir).unwrap();

    let lock_path = dir.join("modules.lock");
    lock.save(&lock_path).unwrap();

    let loaded = ModulesLock::load(&lock_path).unwrap();
    assert_eq!(loaded.meta.reovim_version, "0.10.0");
    assert_eq!(loaded.meta.api_version, "0.2.0");
    assert_eq!(loaded.modules.len(), 2);
    assert_eq!(loaded.modules[0].id, "vim");
    assert_eq!(loaded.modules[0].source, ModuleSource::Builtin);
    assert_eq!(loaded.modules[1].id, "treesitter-go");
    assert_eq!(loaded.modules[1].source, ModuleSource::External);
    assert_eq!(loaded.modules[1].dependencies, vec!["vim"]);
    assert_eq!(loaded.modules[1].optional_dependencies, vec!["lsp"]);

    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn lock_load_not_found() {
    let result = ModulesLock::load(std::path::Path::new("/nonexistent/modules.lock"));
    assert!(result.is_err());
}

#[test]
fn lock_load_invalid_toml() {
    let dir = std::env::temp_dir().join(format!(
        "reovim-lock-invalid-{}",
        std::process::id()
    ));
    std::fs::create_dir_all(&dir).unwrap();

    let lock_path = dir.join("modules.lock");
    std::fs::write(&lock_path, "this is not valid toml {{{").unwrap();

    let result = ModulesLock::load(&lock_path);
    assert!(result.is_err());

    std::fs::remove_dir_all(&dir).unwrap();
}

// ============================================================================
// verify_checksums
// ============================================================================

#[test]
fn verify_checksums_pass() {
    let dir = std::env::temp_dir().join(format!(
        "reovim-verify-pass-{}",
        std::process::id()
    ));
    std::fs::create_dir_all(&dir).unwrap();

    let file = dir.join("module.so");
    std::fs::write(&file, b"module content").unwrap();
    let hash = sha256_file(&file).unwrap();

    let lock = ModulesLock {
        meta: LockMeta {
            reovim_version: "0.10.0".into(),
            api_version: "0.2.0".into(),
            generated: "2026-03-12T10:00:00Z".into(),
        },
        modules: vec![LockEntry {
            id: "test".into(),
            version: "1.0.0".into(),
            source: ModuleSource::External,
            path: Some(file.display().to_string()),
            sha256: Some(hash),
            api_version: None,
            dependencies: Vec::new(),
            optional_dependencies: Vec::new(),
        }],
    };

    assert!(lock.verify_checksums().is_empty());

    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn verify_checksums_fail_tampered() {
    let dir = std::env::temp_dir().join(format!(
        "reovim-verify-fail-{}",
        std::process::id()
    ));
    std::fs::create_dir_all(&dir).unwrap();

    let file = dir.join("module.so");
    std::fs::write(&file, b"original content").unwrap();
    let original_hash = sha256_file(&file).unwrap();

    // Tamper with file
    std::fs::write(&file, b"tampered content").unwrap();

    let lock = ModulesLock {
        meta: LockMeta {
            reovim_version: "0.10.0".into(),
            api_version: "0.2.0".into(),
            generated: "2026-03-12T10:00:00Z".into(),
        },
        modules: vec![LockEntry {
            id: "test".into(),
            version: "1.0.0".into(),
            source: ModuleSource::External,
            path: Some(file.display().to_string()),
            sha256: Some(original_hash),
            api_version: None,
            dependencies: Vec::new(),
            optional_dependencies: Vec::new(),
        }],
    };

    let mismatches = lock.verify_checksums();
    assert_eq!(mismatches.len(), 1);
    assert_eq!(mismatches[0].id, "test");

    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn verify_checksums_file_not_found() {
    let lock = ModulesLock {
        meta: LockMeta {
            reovim_version: "0.10.0".into(),
            api_version: "0.2.0".into(),
            generated: "2026-03-12T10:00:00Z".into(),
        },
        modules: vec![LockEntry {
            id: "missing".into(),
            version: "1.0.0".into(),
            source: ModuleSource::External,
            path: Some("/nonexistent/module.so".into()),
            sha256: Some("abc".into()),
            api_version: None,
            dependencies: Vec::new(),
            optional_dependencies: Vec::new(),
        }],
    };

    let mismatches = lock.verify_checksums();
    assert_eq!(mismatches.len(), 1);
    assert_eq!(mismatches[0].actual, "<file not found>");
}

#[test]
fn verify_checksums_skips_builtin() {
    let lock = ModulesLock {
        meta: LockMeta {
            reovim_version: "0.10.0".into(),
            api_version: "0.2.0".into(),
            generated: "2026-03-12T10:00:00Z".into(),
        },
        modules: vec![LockEntry {
            id: "vim".into(),
            version: "0.10.0".into(),
            source: ModuleSource::Builtin,
            path: None,
            sha256: None,
            api_version: None,
            dependencies: Vec::new(),
            optional_dependencies: Vec::new(),
        }],
    };

    assert!(lock.verify_checksums().is_empty());
}

// ============================================================================
// is_stale
// ============================================================================

#[test]
fn is_stale_same_version() {
    let lock = ModulesLock {
        meta: LockMeta {
            reovim_version: "0.10.0".into(),
            api_version: "0.2.0".into(),
            generated: "2026-03-12T10:00:00Z".into(),
        },
        modules: Vec::new(),
    };

    assert!(!lock.is_stale("0.10.0"));
}

#[test]
fn is_stale_different_version() {
    let lock = ModulesLock {
        meta: LockMeta {
            reovim_version: "0.9.0".into(),
            api_version: "0.2.0".into(),
            generated: "2026-03-12T10:00:00Z".into(),
        },
        modules: Vec::new(),
    };

    assert!(lock.is_stale("0.10.0"));
}

// ============================================================================
// find
// ============================================================================

#[test]
fn find_existing_entry() {
    let lock = ModulesLock {
        meta: LockMeta {
            reovim_version: "0.10.0".into(),
            api_version: "0.2.0".into(),
            generated: "2026-03-12T10:00:00Z".into(),
        },
        modules: vec![LockEntry {
            id: "vim".into(),
            version: "0.10.0".into(),
            source: ModuleSource::Builtin,
            path: None,
            sha256: None,
            api_version: None,
            dependencies: Vec::new(),
            optional_dependencies: Vec::new(),
        }],
    };

    let id = reovim_kernel::api::v1::ModuleId::new("vim");
    assert!(lock.find(&id).is_some());
    assert_eq!(lock.find(&id).unwrap().version, "0.10.0");
}

#[test]
fn find_nonexistent_entry() {
    let lock = ModulesLock {
        meta: LockMeta {
            reovim_version: "0.10.0".into(),
            api_version: "0.2.0".into(),
            generated: "2026-03-12T10:00:00Z".into(),
        },
        modules: Vec::new(),
    };

    let id = reovim_kernel::api::v1::ModuleId::new("nope");
    assert!(lock.find(&id).is_none());
}

// ============================================================================
// LockFileError display
// ============================================================================

#[test]
fn lock_file_error_display() {
    let io_err = LockFileError::Io(std::io::Error::new(
        std::io::ErrorKind::NotFound,
        "not found",
    ));
    assert!(format!("{io_err}").contains("I/O"));

    let parse_err = LockFileError::Parse("bad toml".into());
    assert!(format!("{parse_err}").contains("parse"));

    let ser_err = LockFileError::Serialize("cannot serialize".into());
    assert!(format!("{ser_err}").contains("serialize"));
}

// ============================================================================
// generate
// ============================================================================

#[test]
fn generate_builtin_only() {
    use reovim_kernel::api::v1::{
        Module, ModuleContext, ModuleError, ModuleId, ProbeResult, Version,
    };

    struct TestModule;
    impl Module for TestModule {
        fn id(&self) -> ModuleId {
            ModuleId::new("test")
        }
        fn name(&self) -> &'static str {
            "Test"
        }
        fn version(&self) -> Version {
            Version::new(1, 0, 0)
        }
        fn init(&mut self, _: &ModuleContext) -> ProbeResult {
            ProbeResult::Success
        }
        fn exit(&mut self) -> Result<(), ModuleError> {
            Ok(())
        }
    }

    let registry = super::registry::ModuleRegistry::new();
    registry.register(TestModule).unwrap();

    let lock = ModulesLock::generate(&registry, "0.10.0-dev");
    assert_eq!(lock.meta.reovim_version, "0.10.0-dev");
    assert_eq!(lock.modules.len(), 1);
    assert_eq!(lock.modules[0].id, "test");
    assert_eq!(lock.modules[0].source, ModuleSource::Builtin);
    assert!(lock.modules[0].sha256.is_none());
}

// ============================================================================
// ChecksumMismatch debug
// ============================================================================

#[test]
fn checksum_mismatch_debug() {
    let m = ChecksumMismatch {
        id: "test".into(),
        expected: "aaa".into(),
        actual: "bbb".into(),
        path: "/test".into(),
    };
    let debug = format!("{m:?}");
    assert!(debug.contains("test"));
    assert!(debug.contains("aaa"));
    assert!(debug.contains("bbb"));
}
