//! Tests for [`super::RuntimeLoaderConfig`].

use std::{path::PathBuf, sync::Arc};

use {reovim_dylib_loader::Kind, reovim_pkg_lazyload::LazyRegistry};

use super::RuntimeLoaderConfig;

#[test]
fn from_registry_bundles_inputs_unchanged() {
    let registry = Arc::new(LazyRegistry::empty());
    let cfg =
        RuntimeLoaderConfig::from_registry("/var/lib/reovim", Kind::Module, Arc::clone(&registry));
    assert_eq!(cfg.library_root, PathBuf::from("/var/lib/reovim"));
    assert_eq!(cfg.kind, Kind::Module);
    assert!(Arc::ptr_eq(&cfg.registry, &registry));
}

#[test]
fn scan_dir_uses_kind_subdir() {
    let registry = Arc::new(LazyRegistry::empty());
    let driver_cfg =
        RuntimeLoaderConfig::from_registry("/root", Kind::Driver, Arc::clone(&registry));
    assert_eq!(driver_cfg.scan_dir(), PathBuf::from("/root/driver"));

    let module_cfg = RuntimeLoaderConfig::from_registry("/root", Kind::Module, registry);
    assert_eq!(module_cfg.scan_dir(), PathBuf::from("/root/modules"));
}

#[test]
fn load_returns_empty_registry_when_lockfile_missing() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let cfg = RuntimeLoaderConfig::load(tmp.path(), Kind::Module).expect("load");
    assert_eq!(cfg.library_root, tmp.path());
    assert_eq!(cfg.registry.entries().count(), 0);
    assert_eq!(cfg.kind, Kind::Module);
}

#[allow(unsafe_code, clippy::significant_drop_tightening)]
mod resolve_library_root_tests {
    //! `resolve_library_root` reads process-global env vars and the
    //! filesystem; tests use `parking_lot::Mutex` to serialize
    //! mutations and `tempfile::tempdir()` for the directory probes.
    //!
    //! Clippy's `significant_drop_tightening` lint mis-reads the
    //! `let g = scrub(...); g.set(...)` pattern as a candidate for
    //! collapsing to one expression — but `g` is the env-lock guard
    //! that must remain live across the subsequent
    //! `resolve_library_root()` call. Suppressed at module scope.

    use std::sync::OnceLock;

    use parking_lot::Mutex;

    use super::super::resolve_library_root;

    fn env_lock() -> &'static Mutex<()> {
        static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        LOCK.get_or_init(|| Mutex::new(()))
    }

    struct EnvGuard {
        _guard: parking_lot::MutexGuard<'static, ()>,
        keys: Vec<&'static str>,
        prior: Vec<(&'static str, Option<std::ffi::OsString>)>,
    }

    impl EnvGuard {
        fn scrub(keys: &[&'static str]) -> Self {
            let guard = env_lock().lock();
            let prior: Vec<_> = keys.iter().map(|k| (*k, std::env::var_os(k))).collect();
            for k in keys {
                // SAFETY: env mutations are serialized by `env_lock()`
                // for this test module. The wider workspace tests do
                // not touch these keys.
                unsafe { std::env::remove_var(k) };
            }
            Self {
                _guard: guard,
                keys: keys.to_vec(),
                prior,
            }
        }

        fn set(&self, key: &'static str, value: &std::path::Path) {
            assert!(self.keys.contains(&key), "{key} must be in scrub list");
            // SAFETY: see EnvGuard::scrub.
            unsafe { std::env::set_var(key, value) };
        }
    }

    impl Drop for EnvGuard {
        fn drop(&mut self) {
            for (k, v) in &self.prior {
                // SAFETY: see EnvGuard::scrub.
                unsafe {
                    if let Some(prev) = v {
                        std::env::set_var(k, prev);
                    } else {
                        std::env::remove_var(k);
                    }
                }
            }
        }
    }

    #[test]
    fn returns_none_when_no_env_set() {
        let _g = EnvGuard::scrub(&["REOVIM_LIBRARY_ROOT", "XDG_DATA_HOME", "HOME"]);
        assert_eq!(resolve_library_root(), None);
    }

    #[test]
    fn prefers_reovim_library_root_when_dir_exists() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let g = EnvGuard::scrub(&["REOVIM_LIBRARY_ROOT", "XDG_DATA_HOME", "HOME"]);
        g.set("REOVIM_LIBRARY_ROOT", tmp.path());
        assert_eq!(resolve_library_root().as_deref(), Some(tmp.path()));
    }

    #[test]
    fn ignores_reovim_library_root_when_dir_missing() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let bogus = tmp.path().join("does-not-exist");
        let g = EnvGuard::scrub(&["REOVIM_LIBRARY_ROOT", "XDG_DATA_HOME", "HOME"]);
        g.set("REOVIM_LIBRARY_ROOT", &bogus);
        assert_eq!(resolve_library_root(), None);
    }

    #[test]
    fn falls_back_to_xdg_data_home_reovim() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let xdg = tmp.path();
        let reovim_dir = xdg.join("reovim");
        std::fs::create_dir(&reovim_dir).expect("mkdir reovim");
        let g = EnvGuard::scrub(&["REOVIM_LIBRARY_ROOT", "XDG_DATA_HOME", "HOME"]);
        g.set("XDG_DATA_HOME", xdg);
        assert_eq!(resolve_library_root().as_deref(), Some(reovim_dir.as_path()));
    }

    #[test]
    fn skips_xdg_when_reovim_subdir_absent() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let g = EnvGuard::scrub(&["REOVIM_LIBRARY_ROOT", "XDG_DATA_HOME", "HOME"]);
        g.set("XDG_DATA_HOME", tmp.path()); // tempdir is empty, no `reovim` subdir
        assert_eq!(resolve_library_root(), None);
    }

    #[test]
    fn falls_back_to_home_local_share_reovim() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let home = tmp.path();
        let reovim_dir = home.join(".local").join("share").join("reovim");
        std::fs::create_dir_all(&reovim_dir).expect("mkdir reovim");
        let g = EnvGuard::scrub(&["REOVIM_LIBRARY_ROOT", "XDG_DATA_HOME", "HOME"]);
        g.set("HOME", home);
        assert_eq!(resolve_library_root().as_deref(), Some(reovim_dir.as_path()));
    }

    #[test]
    fn skips_home_when_reovim_subdir_absent() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let g = EnvGuard::scrub(&["REOVIM_LIBRARY_ROOT", "XDG_DATA_HOME", "HOME"]);
        g.set("HOME", tmp.path()); // no `.local/share/reovim` subtree
        assert_eq!(resolve_library_root(), None);
    }

    #[test]
    fn empty_env_value_is_ignored() {
        let _g = EnvGuard::scrub(&["REOVIM_LIBRARY_ROOT", "XDG_DATA_HOME", "HOME"]);
        // SAFETY: set_var serialized via env_lock() held by `_g`.
        unsafe { std::env::set_var("REOVIM_LIBRARY_ROOT", "") };
        let result = resolve_library_root();
        assert_eq!(result, None);
    }
}
