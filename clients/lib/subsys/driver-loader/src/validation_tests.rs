use {
    super::*, reovim_client_subsys_render::abi::ClientRenderVTable,
    reovim_client_subsys_render::client_render::ClientRenderDriverProbe,
    std::ffi::{c_char, c_int, c_void}, std::ptr,
};

unsafe extern "C" fn stub_probe() -> ClientRenderDriverProbe {
    ClientRenderDriverProbe::new("stub", "stub")
}
unsafe extern "C" fn stub_construct(
    _: *mut c_void,
    _: *mut *mut c_void,
    _: *mut *mut c_char,
) -> c_int {
    -1
}
unsafe extern "C" fn stub_target(
    _: *mut c_void,
    _: *mut *const reovim_client_subsys_render::abi::RenderTargetVTable,
    _: *mut *mut c_void,
    _: *mut *mut c_char,
) -> c_int {
    -1
}
unsafe extern "C" fn stub_shutdown(_: *mut c_void, _: *mut *mut c_char) -> c_int {
    0
}
unsafe extern "C" fn stub_destroy(_: *mut c_void) {}
unsafe extern "C" fn stub_destroy_err(_: *mut c_char) {}

fn vtable(abi: u32, api: Version, size: usize) -> ClientRenderVTable {
    ClientRenderVTable {
        abi_version: abi,
        api_version: api,
        size_of_self: size,
        probe: stub_probe,
        construct: stub_construct,
        target: stub_target,
        shutdown: stub_shutdown,
        destroy: stub_destroy,
        destroy_error_string: stub_destroy_err,
    }
}

fn host() -> ClientRenderExpectations {
    ClientRenderExpectations::from_host()
}

#[test]
fn rejects_null_pointer() {
    let err = unsafe { check_client_render(ptr::null(), host()) }.unwrap_err();
    assert!(matches!(err, ValidationError::VtablePointerNull));
}

#[test]
fn accepts_matching_vtable() {
    let h = host();
    let vt = vtable(h.abi_version, h.api_version, h.size_of_self);
    unsafe { check_client_render(&raw const vt, h) }.unwrap();
}

#[test]
fn rejects_abi_mismatch() {
    let h = host();
    let vt = vtable(h.abi_version + 1, h.api_version, h.size_of_self);
    let err = unsafe { check_client_render(&raw const vt, h) }.unwrap_err();
    assert!(matches!(err, ValidationError::AbiVersionMismatch { .. }));
}

#[test]
fn rejects_api_major_mismatch() {
    let h = host();
    let vt = vtable(
        h.abi_version,
        Version::new(h.api_version.major + 1, 0, 0),
        h.size_of_self,
    );
    let err = unsafe { check_client_render(&raw const vt, h) }.unwrap_err();
    assert!(matches!(err, ValidationError::ApiVersionIncompatible { .. }));
}

#[test]
fn rejects_api_minor_below_required() {
    let h = ClientRenderExpectations {
        abi_version: 1,
        api_version: Version::new(1, 2, 0),
        size_of_self: std::mem::size_of::<ClientRenderVTable>(),
    };
    let vt = vtable(1, Version::new(1, 1, 0), h.size_of_self);
    let err = unsafe { check_client_render(&raw const vt, h) }.unwrap_err();
    assert!(matches!(err, ValidationError::ApiVersionIncompatible { .. }));
}

#[test]
fn accepts_api_minor_above_required() {
    let h = ClientRenderExpectations {
        abi_version: 1,
        api_version: Version::new(1, 0, 0),
        size_of_self: std::mem::size_of::<ClientRenderVTable>(),
    };
    let vt = vtable(1, Version::new(1, 9, 3), h.size_of_self);
    unsafe { check_client_render(&raw const vt, h) }.unwrap();
}

#[test]
fn rejects_size_mismatch() {
    let h = host();
    let vt = vtable(h.abi_version, h.api_version, h.size_of_self + 8);
    let err = unsafe { check_client_render(&raw const vt, h) }.unwrap_err();
    assert!(matches!(err, ValidationError::SizeOfSelfMismatch { .. }));
}
