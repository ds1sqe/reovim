use reovim_kernel_interface::{
    Error,
    module::{
        Declaration, Dependency, DependencyScope, Import, ModuleId, control,
    },
};

fn import(name: &str, path: &[u8]) -> Result<Import, Error> {
    Import::new(name, path, control::declaration().unwrap(), 1, 0, false)
}
fn dependency(id: u8, imports: Vec<Import>) -> Dependency {
    Dependency {
        module: ModuleId([id; 16]),
        revision: 1,
        scope: DependencyScope::Same,
        optional: false,
        imports: imports.into_boxed_slice(),
    }
}
fn declaration(dependencies: Vec<Dependency>) -> Result<Declaration, Error> {
    Declaration::new(
        ModuleId([1; 16]),
        "consumer",
        1,
        1,
        dependencies.into_boxed_slice(),
    )
}
#[test]
fn imports_preserve_exact_contracts_and_separate_requested_rights() {
    let api = control::declaration().unwrap();
    let required =
        Import::new("device", b"controls/status", api.clone(), 1, 0, false)
            .unwrap();
    let module = declaration(vec![dependency(2, vec![required])]).unwrap();
    let required = &module.dependencies()[0].imports[0];
    assert_eq!(required.declaration(), &api);
    assert_eq!(required.name(), "device");
    assert_eq!(required.export(), b"controls/status");
    assert_eq!(required.access(), 1);
    assert_eq!(required.create_access(), 0);
    assert!(!required.retain_loans());
}
#[test]
fn local_names_and_export_paths_cannot_alias_or_escape() {
    for name in ["", ".", "..", "a/b", "a\0b", "读取"] {
        assert_eq!(import(name, b"control"), Err(Error::INVALID));
    }
    for path in
        [b"/control".as_slice(), b"a//b", b"a/../b", b"a/./b", b"a/", b"a\0b"]
    {
        assert_eq!(import("device", path), Err(Error::INVALID));
    }
    assert!(import("device", b"").is_ok());
    assert_eq!(import(&"x".repeat(256), b""), Err(Error::PRESSURE));
    assert_eq!(import("device", &vec![b'x'; 4097]), Err(Error::PRESSURE));
    // The export namespace owns non-ASCII policy, not passive metadata.
    assert!(import("device", "设备".as_bytes()).is_ok());
}
#[test]
fn duplicate_local_names_conflict_across_dependency_packages() {
    let first = dependency(2, vec![import("counter", b"first").unwrap()]);
    let second = dependency(3, vec![import("counter", b"second").unwrap()]);
    assert_eq!(declaration(vec![first, second]), Err(Error::CONFLICT));
}
#[test]
fn requested_masks_and_total_import_budget_are_validated() {
    for (access, create) in [(8, 0), (0, 8)] {
        assert_eq!(
            Import::new(
                "device",
                b"",
                control::declaration().unwrap(),
                access,
                create,
                false
            ),
            Err(Error::INVALID)
        );
    }
    let mut imports = Vec::new();
    for index in 0..257 {
        imports.push(import(&format!("device{index}"), b"").unwrap());
    }
    assert_eq!(declaration(vec![dependency(2, imports)]), Err(Error::PRESSURE));
}
