//! Macro-expansion smoke tests for `declare_module!` and `declare_*_driver!`
//! migrated to the no_std selftest runner (#786 Phase 5).
//!
//! These tests verify that the macro-emitted vtable static carries a correctly
//! populated `VtableHeader` (kind, size_of_self, no catch_unwind).  They
//! exercise the macro from the call site rather than the lib's internal
//! sibling test files.
//!
//! AB12 assertion: no `catch_unwind` anywhere in either macro crate (asserted
//! by absence — verified by grep in CI; not a runtime test).

use reovim_arch::arch_test;
use reovim_uapi_abi::{
    ids::{AbiVersion, Version},
    vtable::{ManifestKind, VtableHeader},
};

// ---------------------------------------------------------------------------
// declare_module! — server-side module vtable smoke
// ---------------------------------------------------------------------------

/// Minimal caller-defined vtable: just a VtableHeader (AB3: header at offset 0).
#[repr(C)]
struct TestServerModuleVtable {
    pub header: VtableHeader,
}

const TEST_SERVER_MODULE_VTABLE: TestServerModuleVtable = TestServerModuleVtable {
    header: VtableHeader {
        abi: AbiVersion { major: 1, minor: 0, patch: 0, pad: 0 },
        api: Version { major: 0, minor: 1, patch: 0, pad: 0 },
        size_of_self: core::mem::size_of::<TestServerModuleVtable>(),
        kind: ManifestKind::ModuleServer,
        flags: 0,
    },
};

// Invoke the macro to emit the `REOVIM_MODULE_SERVER_VTABLE` static.
reovim_uapi_module_macros::declare_module!(TestServerModuleVtable, TEST_SERVER_MODULE_VTABLE);

arch_test!(macro_declare_module_vtable_header_kind, {
    // SAFETY: REOVIM_MODULE_SERVER_VTABLE is a `'static` that the macro emitted
    // above; the first field is a VtableHeader at offset 0 (AB3).
    let kind = REOVIM_MODULE_SERVER_VTABLE.header.kind;
    reovim_arch::testrt::check_eq(kind, ManifestKind::ModuleServer);
});

arch_test!(macro_declare_module_vtable_size_of_self, {
    let size = REOVIM_MODULE_SERVER_VTABLE.header.size_of_self;
    reovim_arch::testrt::check_eq(
        size,
        core::mem::size_of::<TestServerModuleVtable>(),
    );
});

arch_test!(macro_declare_module_vtable_abi_major, {
    reovim_arch::testrt::check_eq(REOVIM_MODULE_SERVER_VTABLE.header.abi.major, 1);
});

// ---------------------------------------------------------------------------
// declare_module_client! — client-side module vtable smoke
// ---------------------------------------------------------------------------

#[repr(C)]
struct TestClientModuleVtable {
    pub header: VtableHeader,
}

const TEST_CLIENT_MODULE_VTABLE: TestClientModuleVtable = TestClientModuleVtable {
    header: VtableHeader {
        abi: AbiVersion { major: 1, minor: 0, patch: 0, pad: 0 },
        api: Version { major: 0, minor: 2, patch: 0, pad: 0 },
        size_of_self: core::mem::size_of::<TestClientModuleVtable>(),
        kind: ManifestKind::ModuleClient,
        flags: 0,
    },
};

reovim_uapi_module_macros::declare_module_client!(
    TestClientModuleVtable,
    TEST_CLIENT_MODULE_VTABLE
);

arch_test!(macro_declare_module_client_vtable_kind, {
    reovim_arch::testrt::check_eq(REOVIM_MODULE_CLIENT_VTABLE.header.kind, ManifestKind::ModuleClient);
});

arch_test!(macro_declare_module_client_vtable_size_of_self, {
    reovim_arch::testrt::check_eq(
        REOVIM_MODULE_CLIENT_VTABLE.header.size_of_self,
        core::mem::size_of::<TestClientModuleVtable>(),
    );
});

// ---------------------------------------------------------------------------
// declare_driver! — server-side driver vtable smoke
// ---------------------------------------------------------------------------

#[repr(C)]
struct TestServerDriverVtable {
    pub header: VtableHeader,
}

const TEST_SERVER_DRIVER_VTABLE: TestServerDriverVtable = TestServerDriverVtable {
    header: VtableHeader {
        abi: AbiVersion { major: 1, minor: 0, patch: 0, pad: 0 },
        api: Version { major: 0, minor: 1, patch: 0, pad: 0 },
        size_of_self: core::mem::size_of::<TestServerDriverVtable>(),
        kind: ManifestKind::DriverServer,
        flags: 0,
    },
};

reovim_uapi_driver_macros::declare_driver_server!(TestServerDriverVtable, TEST_SERVER_DRIVER_VTABLE);

arch_test!(macro_declare_driver_vtable_kind, {
    reovim_arch::testrt::check_eq(REOVIM_DRIVER_SERVER_VTABLE.header.kind, ManifestKind::DriverServer);
});

arch_test!(macro_declare_driver_vtable_size_of_self, {
    reovim_arch::testrt::check_eq(
        REOVIM_DRIVER_SERVER_VTABLE.header.size_of_self,
        core::mem::size_of::<TestServerDriverVtable>(),
    );
});

// ---------------------------------------------------------------------------
// declare_driver_client! — client-side driver vtable smoke
// ---------------------------------------------------------------------------

#[repr(C)]
struct TestClientDriverVtable {
    pub header: VtableHeader,
}

const TEST_CLIENT_DRIVER_VTABLE: TestClientDriverVtable = TestClientDriverVtable {
    header: VtableHeader {
        abi: AbiVersion { major: 1, minor: 0, patch: 0, pad: 0 },
        api: Version { major: 0, minor: 1, patch: 0, pad: 0 },
        size_of_self: core::mem::size_of::<TestClientDriverVtable>(),
        kind: ManifestKind::DriverClient,
        flags: 0,
    },
};

reovim_uapi_driver_macros::declare_driver_client!(
    TestClientDriverVtable,
    TEST_CLIENT_DRIVER_VTABLE
);

arch_test!(macro_declare_driver_client_vtable_kind, {
    reovim_arch::testrt::check_eq(
        REOVIM_DRIVER_CLIENT_VTABLE.header.kind,
        ManifestKind::DriverClient,
    );
});
