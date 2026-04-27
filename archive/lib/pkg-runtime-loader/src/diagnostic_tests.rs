//! Tests for [`super::enrich_validation_error`].

use std::path::{Path, PathBuf};

use reovim_dylib_loader::cdylib_filename;

use super::enrich_validation_error;

/// Stand-in `Src` type that exposes a `from`-able shape so the
/// `E: From<Src>` bound the helper requires is satisfiable in tests
/// without dragging in the client / server error types.
#[derive(Debug, PartialEq, Eq)]
struct DummyErr(&'static str);

#[derive(Debug, PartialEq, Eq)]
enum DummyEnriched {
    Bare(DummyErr),
    AtPackage { package: String, source: DummyErr },
}

impl From<DummyErr> for DummyEnriched {
    fn from(value: DummyErr) -> Self {
        Self::Bare(value)
    }
}

fn convention_path(pkg: &str) -> PathBuf {
    PathBuf::from("/lib/reovim/driver").join(cdylib_filename(pkg))
}

#[test]
fn convention_filename_invokes_wrap_with_recovered_name() {
    let path = convention_path("vim-snippet");
    let err = DummyErr("abi mismatch");
    let enriched = enrich_validation_error(&path, err, |pkg, source| DummyEnriched::AtPackage {
        package: pkg,
        source,
    });
    assert_eq!(
        enriched,
        DummyEnriched::AtPackage {
            package: "vim-snippet".to_owned(),
            source: DummyErr("abi mismatch"),
        }
    );
}

#[test]
fn non_convention_filename_passes_through_via_from() {
    let path = Path::new("/usr/lib/libfoo.so");
    let err = DummyErr("abi mismatch");
    let enriched = enrich_validation_error::<_, DummyEnriched>(path, err, |_pkg, _source| {
        panic!("wrap closure must NOT be invoked when filename is non-convention")
    });
    assert_eq!(enriched, DummyEnriched::Bare(DummyErr("abi mismatch")));
}

#[test]
fn empty_recovered_package_name_passes_through() {
    // `libreovim_pkg_.so` matches the convention regex but the
    // recovered name is the empty string — treat as non-convention.
    let path = Path::new("/lib/reovim/driver/libreovim_pkg_.so");
    let err = DummyErr("abi mismatch");
    let enriched = enrich_validation_error::<_, DummyEnriched>(path, err, |_pkg, _source| {
        panic!("wrap closure must NOT be invoked when recovered name is empty")
    });
    assert_eq!(enriched, DummyEnriched::Bare(DummyErr("abi mismatch")));
}

#[test]
fn path_without_filename_passes_through() {
    let path = Path::new("/");
    let err = DummyErr("abi mismatch");
    let enriched = enrich_validation_error::<_, DummyEnriched>(path, err, |_pkg, _source| {
        panic!("wrap closure must NOT be invoked for path without filename")
    });
    assert_eq!(enriched, DummyEnriched::Bare(DummyErr("abi mismatch")));
}

#[test]
fn helper_is_generic_over_validation_error() {
    use reovim_pkg_runtime_loader_test_aux::{ClientEnriched, ClientErr};

    let path = convention_path("demo");
    let err = ClientErr::AbiVersionMismatch {
        found: 2,
        expected: 3,
    };
    let enriched = enrich_validation_error(&path, err, |pkg, source| ClientEnriched::AtPackage {
        package: pkg,
        source,
    });
    match enriched {
        ClientEnriched::AtPackage { package, source } => {
            assert_eq!(package, "demo");
            assert!(matches!(source, ClientErr::AbiVersionMismatch { .. }));
        }
        ClientEnriched::Bare(_) => panic!("expected AtPackage"),
    }
}

#[test]
fn helper_is_generic_over_module_error() {
    use reovim_pkg_runtime_loader_test_aux::{ServerEnriched, ServerErr};

    let path = convention_path("vim-mod");
    let err = ServerErr::IncompatibleVersion {
        module: (2, 0),
        kernel: (3, 0),
    };
    let enriched = enrich_validation_error(&path, err, |pkg, source| ServerEnriched::AtPackage {
        package: pkg,
        source,
    });
    match enriched {
        ServerEnriched::AtPackage { package, source } => {
            assert_eq!(package, "vim-mod");
            assert!(matches!(source, ServerErr::IncompatibleVersion { .. }));
        }
        ServerEnriched::Bare(_) => panic!("expected AtPackage"),
    }
}

/// Test-aux module: shapes that mimic the real client / server error
/// pairs so the helper's genericity is exercised here without dragging
/// in their actual crate-level imports.
mod reovim_pkg_runtime_loader_test_aux {
    #[derive(Debug, PartialEq, Eq)]
    pub enum ClientErr {
        AbiVersionMismatch { found: u32, expected: u32 },
    }

    #[derive(Debug, PartialEq, Eq)]
    pub enum ClientEnriched {
        Bare(ClientErr),
        AtPackage { package: String, source: ClientErr },
    }

    impl From<ClientErr> for ClientEnriched {
        fn from(value: ClientErr) -> Self {
            Self::Bare(value)
        }
    }

    #[derive(Debug, PartialEq, Eq)]
    pub enum ServerErr {
        IncompatibleVersion {
            module: (u32, u32),
            kernel: (u32, u32),
        },
    }

    #[derive(Debug, PartialEq, Eq)]
    pub enum ServerEnriched {
        Bare(ServerErr),
        AtPackage { package: String, source: ServerErr },
    }

    impl From<ServerErr> for ServerEnriched {
        fn from(value: ServerErr) -> Self {
            Self::Bare(value)
        }
    }
}
